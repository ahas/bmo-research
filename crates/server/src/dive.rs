//! The deep dive and its notices (`docs/api.md`, 「The deep dive」, 「The page」 and 「The notices」): the next
//! run picked among the running categories whose road no bench holds, asked of the runner, read until it
//! closes, its result read and its items filed, its page finished with its Korean side and its narration, and
//! every recipient told; what a page still lacks asked again on the retry wait and its follow-up told; a try
//! the bench cut put back uncounted.

use std::sync::Arc;
use std::time::Duration;

use bmo_research_client::{
  Category, CategoryShape, Item, RecipientKind, ResearchRunAsk, ResearchRunState, Running, Shape,
};
use serde_json::{Value, json};
use tokio::task::JoinHandle;

use crate::App;
use crate::bench;
use crate::document::{self, Drawn};
use crate::page::{self, Side};
use crate::reading::{self, Exposure};
use crate::runner::{self, Filing, PageAsk, Refused};
use crate::store::{Filed, now_ms};
use crate::translation::{self, English, Rendering};
use crate::words::{self, fill};

/// How often a missing narration is asked: once, and once more on the retry wait.
const NARRATION_TRIES: i64 = 2;

/// The languages a page may hold, in the order it shows them.
const LANGUAGES: [&str; 2] = ["ko", "en"];

/// How an item's page's Korean side stands as its report is told: had, pending a later try, or no longer
/// asked.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Korean {
  Filed(Rendering),
  Pending,
  Unavailable,
}

/// What a deep dive's page is drawn from: the run's result as the runner drew it, the page's English title and
/// the labels its document carries.
struct Source {
  drawn: Drawn,
  title: String,
  labels: Value,
}

/// The page's source, read from the research run's ledger row.
fn source(app: &App, run_id: i64) -> Result<Source, String> {
  let row = app.store.research_run(run_id).map_err(|why| why.to_string())?;
  let written = row
    .written
    .clone()
    .filter(|w| !w.trim().is_empty())
    .ok_or("the run's result is not kept")?;
  let drawn = document::draw(&written, words::search_document(document::language_of(&row.query)));
  let title = row.title.clone().unwrap_or_else(|| document::page_title(&drawn, &row.query));
  let labels = document::labels(&drawn, run_id, &row.key, &row.query);
  Ok(Source {
    drawn,
    title,
    labels,
  })
}

/// The daemon's loop: the tries it left running taken up, the owner's room given to every category holding
/// no recipient, then a scan and a start of what may run every `tick_seconds`.
pub async fn daemon(app: Arc<App>) {
  take_up(&app);
  give_owner_room(&app).await;
  loop {
    let _ = tick(&app).await;
    tokio::time::sleep(Duration::from_secs(app.settings.tick_seconds.max(1))).await;
  }
}

/// The tries a stopped daemon left running: one asked is read again, one never asked waits again.
pub fn take_up(app: &Arc<App>) {
  let runs = match app.store.open_runs() {
    Ok(r) => r,
    Err(why) => {
      eprintln!("bmo-research: the store: {why}");
      return;
    }
  };
  for run in runs {
    match run.run_id {
      None => {
        let _ = app.store.requeue(run.id, "failed", None, "the daemon stopped before the run was asked");
      }
      Some(run_id) => {
        let app = app.clone();
        tokio::spawn(async move { carry(app, run.id, Some(run_id)).await });
      }
    }
  }
}

/// The owner's private room added to every category holding no recipient.
pub async fn give_owner_room(app: &App) {
  let bare = match app.store.categories_without_recipients() {
    Ok(b) if !b.is_empty() => b,
    _ => return,
  };
  match app.peers.owner_room().await {
    Ok(room) => {
      for category in bare {
        let _ = app.store.add_recipient(&category, RecipientKind::Room, Some(room), None, "owner");
      }
    }
    Err(why) => eprintln!("bmo-research: the owner's room could not be read: {why}"),
  }
}

/// One pass of the loop: the bench watch read, a scan, every page whose pending part's retry is due asked
/// again, then as many tries started as `concurrency` leaves room for. Answers the tasks started,
/// each carried to its end on a task of its own.
pub async fn tick(app: &Arc<App>) -> Vec<JoinHandle<()>> {
  bench::look(app).await;
  if let Err(why) = crate::scan::scan(app).await {
    eprintln!("bmo-research: the scan: {why}");
  }
  let mut started = Vec::new();
  for item in app.store.pages_due(now_ms()).unwrap_or_default() {
    let app = app.clone();
    started.push(tokio::spawn(async move { render_again(app, item).await }));
  }
  loop {
    let running = app.store.running_count().unwrap_or(i64::MAX);
    if running >= app.settings.concurrency.max(1) {
      break;
    }
    let Some((category, items)) = next(app) else {
      break;
    };
    let ids: Vec<i64> = items.iter().map(|i| i.id).collect();
    let Ok(Some((run, _))) = app.store.start_try(&category.name, &ids) else {
      break;
    };
    let app = app.clone();
    started.push(tokio::spawn(async move { carry(app, run, None).await }));
  }
  started
}

/// The next try to ask: in each running category whose road no bench holds, its next waiting item, or a
/// brief's whole gathering once it is due; among them, the highest priority, then the category whose last
/// try was asked longest ago.
fn next(app: &App) -> Option<(Category, Vec<Item>)> {
  let now = now_ms();
  let mut best: Option<(i64, Option<i64>, Category, Vec<Item>)> = None;
  for category in app.store.categories().ok()? {
    if category.state == Running::Stopped {
      continue;
    }
    let (provider, model) = road(app, &category);
    if bench::held(app, &provider, &model) {
      continue;
    }
    let last = app.store.last_try_at(&category.name).ok()?;
    let items = if category.shape == CategoryShape::Brief {
      let gather = category.gather_minutes.unwrap_or(1440).max(1) * 60_000;
      let due =
        last.is_none_or(|at| now - at >= gather) || app.store.retry_due(&category.name, now).ok()?;
      if !due {
        continue;
      }
      app.store.waiting(&category.name, now, category.cap.max(1)).ok()?
    } else {
      app.store.waiting(&category.name, now, 1).ok()?
    };
    let Some(first) = items.first() else {
      continue;
    };
    let better = match &best {
      None => true,
      Some((priority, at, _, _)) => {
        first.priority > *priority
          || (first.priority == *priority && last.unwrap_or(i64::MIN) < at.unwrap_or(i64::MIN))
      }
    };
    if better {
      best = Some((first.priority, last, category, items));
    }
  }
  best.map(|(_, _, category, items)| (category, items))
}

/// The provider and the model a category's tries go out on: its own, else the settings' road.
pub fn road(app: &App, category: &Category) -> (String, String) {
  (
    category.provider.clone().unwrap_or_else(|| app.settings.road.provider.clone()),
    category.model.clone().unwrap_or_else(|| app.settings.road.model.clone()),
  )
}

/// The question a try is asked: one item's shape with its name and address, an advisory's with the stack,
/// a brief's with every item it gathers.
fn question(app: &App, items: &[Item]) -> String {
  let lead = &items[0];
  let address = lead.address.clone().unwrap_or_default();
  match lead.shape {
    Shape::Brief => {
      let lines: Vec<String> = items
        .iter()
        .map(|i| format!("- {} — {}", i.name, i.address.clone().unwrap_or_default()))
        .collect();
      words::question(Shape::Brief, &[("items", &lines.join("\n"))])
    }
    Shape::Advisory => {
      let stack: Vec<String> = app
        .store
        .stack()
        .unwrap_or_default()
        .into_iter()
        .map(|row| {
          let at: Vec<String> = [row.place, row.repository].into_iter().flatten().collect();
          format!("- {} {} ({})", row.component, row.version.unwrap_or_default(), at.join(", "))
        })
        .collect();
      words::question(
        Shape::Advisory,
        &[("name", &lead.name), ("address", &address), ("stack", &stack.join("\n"))],
      )
    }
    shape => words::question(shape, &[("name", &lead.name), ("address", &address)]),
  }
}

/// One try carried to its end: asked where it was not, read until its run closes, filed and told.
pub async fn carry(app: Arc<App>, run: i64, asked: Option<i64>) {
  let Ok(row) = app.store.run(run) else {
    return;
  };
  let items: Vec<Item> = row.item_ids.iter().filter_map(|id| app.store.item(*id).ok()).collect();
  let Some(lead) = items.first().cloned() else {
    let _ = app.store.end_run(run, "failed", None, Some("the try carries no item"));
    return;
  };
  let Ok(category) = app.store.category(&row.category_name) else {
    let _ = app.store.end_run(run, "failed", None, Some("the try's category no longer stands"));
    return;
  };
  let run_id = match asked {
    Some(id) => id,
    None => {
      let (provider, model) = road(&app, &category);
      let ask = ResearchRunAsk {
        key: app.settings.road.key.clone(),
        query: question(&app, &items),
        provider: Some(provider),
        model: Some(model),
        ..ResearchRunAsk::default()
      };
      let filing = Filing::Page(PageAsk {
        sections: words::document().sections.of(lead.shape).to_vec(),
        title: words::page().title("en", lead.shape, &lead.name),
      });
      match runner::launch(app.clone(), ask, filing).await {
        Ok(launched) => {
          let _ = app.store.keep_run_id(run, launched.run_id);
          launched.run_id
        }
        Err(Refused::Ask(why) | Refused::Unavailable(why) | Refused::Failed(why)) => {
          return fail(&app, run, &lead, "refused", None, &why).await;
        }
      }
    }
  };
  let closed = loop {
    match app.store.research_run(run_id) {
      Ok(row) if row.state == ResearchRunState::Closed => break row,
      Ok(_) => {}
      Err(why) => eprintln!("bmo-research: research run {run_id} could not be read: {why}"),
    }
    tokio::time::sleep(Duration::from_millis((app.settings.poll_seconds * 1000).max(50))).await;
  };
  let outcome = closed.outcome.clone();
  if outcome.as_deref() == Some("cut") {
    return fail(&app, run, &lead, "cut", outcome.as_deref(), "the run was cut").await;
  }
  let Some(document_id) = closed.document_id else {
    let why = closed.error.clone().unwrap_or_else(|| "the run closed with no document filed".into());
    return fail(&app, run, &lead, "failed", outcome.as_deref(), &why).await;
  };
  let source = match source(&app, run_id) {
    Ok(source) => source,
    Err(why) => return fail(&app, run, &lead, "failed", outcome.as_deref(), &why).await,
  };
  let link = match app.peers.share(document_id).await {
    Ok(link) => link,
    Err(why) => return fail(&app, run, &lead, "failed", outcome.as_deref(), &why).await,
  };
  let content = source.drawn.body.clone();
  let _ = app.store.end_run(run, "closed", outcome.as_deref(), None);
  let mut filed = Filed {
    document_id,
    title: source.title.clone(),
    description: closed.description.clone(),
    share_url: link.clone(),
    ..Filed::default()
  };
  let mut exposure = Exposure::default();
  match lead.shape {
    Shape::Manual => {
      let read = reading::manual_fit(&content, &words::document().manual, &app.settings.model_nodes);
      filed.fit = read.fit;
      filed.fit_nodes = read.nodes;
      filed.plan = read.plan;
    }
    Shape::Advisory => {
      exposure = reading::exposure(&content, &words::document().advisory);
      filed.fit = !exposure.touches.is_empty();
      filed.touches = exposure.summary();
      filed.plan = exposure.plan.clone();
    }
    Shape::Brief => {
      for figure in reading::figures(&content, &words::document().brief) {
        let _ = app.store.keep_fact(
          &figure.name,
          &figure.value,
          figure.unit.as_deref(),
          figure.as_of.as_deref(),
          figure.source.as_deref(),
          Some(run_id),
          Some(document_id),
        );
      }
    }
    Shape::Investigation => {}
  }
  let mut told = false;
  for item in &items {
    if app.store.file(item.id, &filed).unwrap_or(false) && item.id == lead.id {
      told = true;
    }
  }
  if told {
    let (korean, _) = finish(&app, &lead, &filed, &source).await;
    tell(&app, &category, &lead, &filed, &korean, &exposure).await;
  }
}

/// The file a narration's sound is kept under beside its page.
pub fn sound_name(code: &str) -> String {
  format!("narration-{code}.ogg")
}

/// One narration's sound attached beside its page, which the archive's reader plays: the one the document
/// already keeps, else the script spoken — in Korean translated to be spoken first — and attached, the Korean
/// as the sound heard at the document's head.
async fn narration(
  app: &App,
  lead: &Item,
  document_id: i64,
  code: &str,
  script: &str,
) -> Result<(), String> {
  let name = sound_name(code);
  if app.peers.attached(document_id, &name).await? {
    return Ok(());
  }
  let text = match code {
    "en" => script.to_string(),
    _ => translation::narration(app, &lead.name, script).await?,
  };
  let (bytes, media_type) = app.peers.speak(&text).await?;
  let role = (code == LANGUAGES[0]).then_some("head");
  app.peers.attach(document_id, &name, &media_type, role, &bytes).await
}

/// The page drawn again with the sides it holds, Korean first, and set in the archive.
async fn redraw(
  app: &App,
  document_id: i64,
  source: &Source,
  rendering: Option<&Rendering>,
) -> Result<(), String> {
  let english = document::page_body(&source.drawn, &source.title);
  let mut sides = Vec::new();
  if let Some(korean) = rendering {
    sides.push(Side {
      code: LANGUAGES[0],
      title: &korean.title,
      description: korean.description.as_deref(),
      body: &korean.body,
    });
  }
  sides.push(Side {
    code: LANGUAGES[1],
    title: &source.title,
    description: source.drawn.description.as_deref(),
    body: &english,
  });
  let html = page::compose(&sides);
  let mut labels = source.labels.clone();
  labels["languages"] = sides.iter().map(|side| side.code).collect::<Vec<_>>().join(",").into();
  app
    .peers
    .artifact_send(
      reqwest::Method::PATCH,
      &format!("/documents/{document_id}"),
      &json!({ "title": sides[0].title, "html": html, "labels": labels }),
    )
    .await
    .map(|_| ())
}

/// An item's page finished as far as it can be now: its Korean side asked where it lacks one and its tries
/// allow, each narration it lacks asked and attached where its tries allow, the page drawn again where its
/// Korean side was had, and what is still lacking set at the retry wait. Answers how the Korean side stands,
/// and whether it was had on this call.
async fn finish(app: &App, lead: &Item, filed: &Filed, source: &Source) -> (Korean, bool) {
  let document_id = filed.document_id;
  let state = match app.store.page_state(lead.id) {
    Ok(state) => state,
    Err(why) => {
      eprintln!("bmo-research: item {}'s page could not be read: {why}", lead.id);
      return (Korean::Unavailable, false);
    }
  };
  let korean_had = state.languages.iter().any(|code| code == LANGUAGES[0]);
  let ask_korean = !korean_had && state.translation_tries <= app.settings.retries;
  let script = source.drawn.narration.clone().filter(|script| !script.trim().is_empty());
  let lacking: Vec<&str> = LANGUAGES
    .into_iter()
    .filter(|code| !state.narrations.iter().any(|had| had == code))
    .collect();
  let ask_narration = script.is_some() && !lacking.is_empty() && state.narration_tries < NARRATION_TRIES;
  let next = now_ms() + app.settings.retry_minutes * 60_000;
  let claimed = match app.store.claim_page(lead.id, next, ask_korean, ask_narration) {
    Ok(claimed) => claimed,
    Err(why) => {
      eprintln!("bmo-research: item {}'s page could not be counted: {why}", lead.id);
      return (Korean::Unavailable, false);
    }
  };
  let mut rendering = if korean_had { app.store.rendering(document_id).ok().flatten() } else { None };
  let mut fresh = false;
  if ask_korean {
    let body = document::page_body(&source.drawn, &source.title);
    let english = English {
      name: &lead.name,
      title: &source.title,
      description: source.drawn.description.as_deref(),
      body: &body,
    };
    let fallback = words::page().title(LANGUAGES[0], lead.shape, &lead.name);
    match translation::render(app, &english, &fallback).await {
      Ok(korean) => {
        rendering = Some(korean);
        fresh = true;
      }
      Err(why) => eprintln!("bmo-research: item {}'s Korean side is pending: {why}", lead.id),
    }
  }
  let mut sounds: Vec<String> = state.narrations.clone();
  let mut heard = false;
  for code in LANGUAGES {
    let asked = ask_narration && lacking.contains(&code);
    let Some(script) = script.as_deref().filter(|_| asked) else {
      continue;
    };
    match narration(app, lead, document_id, code, script).await {
      Ok(()) => {
        heard = true;
        sounds.push(code.to_owned());
      }
      Err(why) => eprintln!("bmo-research: item {}'s {code} narration was not made: {why}", lead.id),
    }
  }
  if heard {
    let _ = app.store.keep_narrations(document_id, &sounds);
  }
  if fresh {
    match redraw(app, document_id, source, rendering.as_ref()).await {
      Ok(()) => {
        if let Some(korean) = &rendering {
          let _ = app.store.keep_rendering(document_id, korean);
        }
      }
      Err(why) => {
        eprintln!("bmo-research: item {}'s page was not drawn again: {why}", lead.id);
        rendering = None;
        fresh = false;
      }
    }
  }
  let narrated = app.store.page_state(lead.id).map(|s| s.narrations.len()).unwrap_or_default();
  let korean_pending = rendering.is_none() && claimed.translation_tries <= app.settings.retries;
  let narration_pending =
    script.is_some() && narrated < LANGUAGES.len() && claimed.narration_tries < NARRATION_TRIES;
  let _ = app.store.pend_page(lead.id, (korean_pending || narration_pending).then_some(next));
  let korean = match rendering {
    Some(korean) => Korean::Filed(korean),
    None if korean_pending => Korean::Pending,
    None => {
      if ask_korean {
        eprintln!(
          "bmo-research: item {}'s Korean side failed after {} tries and is no longer asked",
          lead.id, claimed.translation_tries
        );
      }
      Korean::Unavailable
    }
  };
  (korean, fresh)
}

/// What a filed item holds of its document, for a page finished after its report was told.
fn filed_of(item: &Item, document_id: i64) -> Filed {
  Filed {
    document_id,
    title: item.title.clone().unwrap_or_default(),
    description: item.description.clone(),
    share_url: item.share_url.clone().unwrap_or_default(),
    fit: item.fit,
    fit_nodes: item.fit_nodes.clone(),
    touches: item.touches.clone(),
    plan: item.plan.clone(),
  }
}

/// What a page still lacks asked again; once its Korean side is had, every recipient of the category is told
/// the follow-up.
async fn render_again(app: Arc<App>, item: Item) {
  let (Some(document_id), Some(run_id)) = (item.document_id, item.run_id) else {
    let _ = app.store.pend_page(item.id, None);
    return;
  };
  let source = match source(&app, run_id) {
    Ok(source) => source,
    Err(why) => {
      eprintln!("bmo-research: item {}'s page cannot be drawn again: {why}", item.id);
      let _ = app.store.pend_page(item.id, None);
      return;
    }
  };
  let filed = filed_of(&item, document_id);
  if let (Korean::Filed(rendering), true) = finish(&app, &item, &filed, &source).await
    && let Ok(category) = app.store.category(&item.category_name)
  {
    notify(&app, &category, &item, &follow_up(&item, &filed, &rendering), "translated").await;
  }
}

/// A failed try: the bench's is not counted and its items wait again at once; any other's wait again after
/// `retry_minutes`, or after the last retry are failed and every recipient is told once.
async fn fail(app: &App, run: i64, lead: &Item, state: &str, outcome: Option<&str>, why: &str) {
  if let Some(model) = benched(app, run, lead).await {
    let error = format!("{why}; {model} was benched or does not answer, so the try is not counted");
    eprintln!("bmo-research: item {}: {error}", lead.id);
    let _ = app.store.requeue(run, state, outcome, &error);
    return;
  }
  let _ = app.store.end_run(run, state, outcome, Some(why));
  let tries = app.store.item(lead.id).map(|i| i.tries).unwrap_or(lead.tries);
  let last = tries > app.settings.retries;
  let retry_at = (!last).then(|| now_ms() + app.settings.retry_minutes * 60_000);
  let moved = app.store.fail_try(run, why, retry_at).unwrap_or_default();
  if last && moved.contains(&lead.id) {
    eprintln!("bmo-research: item {} failed after {tries} tries: {why}", lead.id);
    if let Ok(category) = app.store.category(&lead.category_name) {
      let words = words::report();
      let notice = fill(&words.failed, &[("name", &lead.name), ("tries", &tries.to_string())]);
      notify(app, &category, lead, &notice, "failed").await;
    }
  }
}

/// The watched model a failed try's road stands on, where the bench cut the try; none otherwise.
async fn benched(app: &App, run: i64, lead: &Item) -> Option<String> {
  let category = app.store.category(&lead.category_name).ok()?;
  let started_at = app.store.run(run).ok()?.started_at;
  let (provider, model) = road(app, &category);
  bench::evicted(app, &provider, &model, started_at).await.then_some(model)
}

/// The notice a filed item's recipients hear: the line of the page's Korean side and its one link, or the
/// English line and the pending line while the Korean side is pending; then for a `fit` item what its
/// recipient needs to act.
pub fn notice(lead: &Item, filed: &Filed, korean: &Korean) -> String {
  let words = words::report();
  let line = |title: &str, description: Option<&str>| {
    let summary = reading::first_sentence(description.unwrap_or_default());
    fill(&words.line, &[("title", title), ("summary", &summary), ("link", &filed.share_url)])
  };
  let mut lines = match korean {
    Korean::Filed(rendering) => vec![line(&rendering.title, rendering.description.as_deref())],
    Korean::Pending => vec![line(&filed.title, filed.description.as_deref()), words.pending.clone()],
    Korean::Unavailable => vec![line(&filed.title, filed.description.as_deref())],
  };
  lines.extend(acting(lead, filed));
  lines.join("\n")
}

/// The follow-up told once a pending Korean side is had: the Korean title and the same link, and for a `fit`
/// item what its recipient needs to act, since it replaces the report where that still waits.
pub fn follow_up(lead: &Item, filed: &Filed, rendering: &Rendering) -> String {
  let words = words::report();
  let mut lines = vec![fill(&words.translated, &[("title", &rendering.title), ("link", &filed.share_url)])];
  lines.extend(acting(lead, filed));
  lines.join("\n")
}

/// What a `fit` item's recipient needs to act on it; nothing for another item.
fn acting(lead: &Item, filed: &Filed) -> Vec<String> {
  let words = words::report();
  let mut lines = Vec::new();
  if filed.fit {
    match lead.shape {
      Shape::Advisory => {
        lines.push(fill(&words.touches, &[("touches", filed.touches.as_deref().unwrap_or_default())]))
      }
      _ => lines.push(words.fit.clone()),
    }
    lines.push(fill(&words.name, &[("name", &lead.name)]));
    if let Some(plan) = &filed.plan {
      lines.push(fill(&words.plan, &[("plan", plan)]));
    }
    if !filed.fit_nodes.is_empty() {
      lines.push(fill(&words.bench, &[("nodes", &filed.fit_nodes.join(", "))]));
    }
  }
  lines
}

/// Every recipient told of a filed item, and for an advisory each touched repository's worker its part.
async fn tell(
  app: &App,
  category: &Category,
  lead: &Item,
  filed: &Filed,
  korean: &Korean,
  exposure: &Exposure,
) {
  notify(app, category, lead, &notice(lead, filed, korean), "report").await;
  if lead.shape != Shape::Advisory {
    return;
  }
  let stack = app.store.stack().unwrap_or_default();
  let words = words::report();
  for repository in exposure.repositories() {
    let mut workers: Vec<String> = stack
      .iter()
      .filter(|row| row.repository.as_deref() == Some(repository.as_str()))
      .filter_map(|row| row.worker.clone())
      .filter(|w| !w.is_empty())
      .collect();
    workers.sort();
    workers.dedup();
    if workers.is_empty() {
      continue;
    }
    let rows: Vec<String> = exposure
      .touches
      .iter()
      .filter(|t| t.repository.as_deref() == Some(repository.as_str()))
      .map(|t| {
        format!("- {} {} {}", t.component, t.version, t.host.clone().unwrap_or_default())
          .trim_end()
          .to_string()
      })
      .collect();
    let part = fill(
      &words.worker,
      &[
        ("name", &lead.name),
        ("repository", &repository),
        ("rows", &rows.join("\n")),
        ("link", &filed.share_url),
        ("plan", filed.plan.as_deref().unwrap_or_default()),
      ],
    );
    for worker in workers {
      let said = app.peers.turn(&worker, &part).await;
      if let Err(why) = &said {
        eprintln!("bmo-research: worker {worker} could not be told item {}: {why}", lead.id);
      }
      let _ =
        app
          .store
          .keep_report(lead.id, &format!("agent:{worker}"), "part", None, said.err().as_deref());
    }
  }
}

/// One notice to every recipient of the category, each kept as a report row.
async fn notify(app: &App, category: &Category, lead: &Item, notice: &str, kind: &str) {
  let recipients = app.store.recipients(&category.name).unwrap_or_default();
  let summary = format!("research {kind}: {}", lead.name);
  for recipient in recipients {
    let (who, said) = match (recipient.kind, recipient.room_id, recipient.agent_name.as_deref()) {
      (RecipientKind::Room, Some(room), _) => (
        format!("room:{room}"),
        app
          .peers
          .instruct(room, &format!("research-report:{}", lead.id), notice, &summary)
          .await,
      ),
      (RecipientKind::Agent, _, Some(agent)) => {
        (format!("agent:{agent}"), app.peers.turn(agent, notice).await.map(|()| None))
      }
      _ => continue,
    };
    if let Err(why) = &said {
      eprintln!("bmo-research: {who} refused item {}: {why}", lead.id);
    }
    let (instruction, error) = match said {
      Ok(id) => (id, None),
      Err(why) => (None, Some(why)),
    };
    let _ = app.store.keep_report(lead.id, &who, kind, instruction, error.as_deref());
  }
}
