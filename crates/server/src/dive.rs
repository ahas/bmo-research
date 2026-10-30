//! The deep dive and its notices (`docs/api.md`, 「The deep dive」, 「The Korean rendering」 and 「The
//! notices」): the next run picked among the running categories whose road no bench holds, asked of the
//! runner, read until it closes, its document read and its items filed, the document rendered in Korean, and every
//! recipient told; a pending rendering asked again on the retry wait and its follow-up told; a try the
//! bench cut put back uncounted.

use std::sync::Arc;
use std::time::Duration;

use bmo_research_client::{
  Category, CategoryShape, Item, RecipientKind, ResearchRunAsk, ResearchRunState, Running, Shape,
};
use tokio::task::JoinHandle;

use crate::App;
use crate::bench;
use crate::reading::{self, Exposure};
use crate::runner::{self, Refused};
use crate::store::{Filed, now_ms};
use crate::translation::{self, English, Rendering};
use crate::words::{self, fill};

/// What a try reads of the document its run filed.
struct Document {
  id: i64,
  title: String,
  description: Option<String>,
}

/// How an item's Korean rendering stands as its report is told: filed, pending a later try, or no longer
/// asked.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Korean {
  Filed(Rendering),
  Pending,
  Unavailable,
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

/// One pass of the loop: the bench watch read, a scan, every pending Korean rendering whose retry is due
/// asked again, then as many tries started as `concurrency` leaves room for. Answers the tasks started,
/// each carried to its end on a task of its own.
pub async fn tick(app: &Arc<App>) -> Vec<JoinHandle<()>> {
  bench::look(app).await;
  if let Err(why) = crate::scan::scan(app).await {
    eprintln!("bmo-research: the scan: {why}");
  }
  let mut started = Vec::new();
  for item in app.store.translations_due(now_ms()).unwrap_or_default() {
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
      match runner::launch(app.clone(), ask, false).await {
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
  let document = Document {
    id: document_id,
    title: closed.title.clone().unwrap_or_default(),
    description: closed.description.clone(),
  };
  let (content, link) = match (app.peers.content(document.id).await, app.peers.share(document.id).await) {
    (Ok(c), Ok(l)) => (c, l),
    (Err(why), _) | (_, Err(why)) => {
      return fail(&app, run, &lead, "failed", outcome.as_deref(), &why).await;
    }
  };
  let _ = app.store.end_run(run, "closed", outcome.as_deref(), None);
  let mut filed = Filed {
    document_id: document.id,
    title: document.title.clone(),
    description: document.description.clone(),
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
          Some(document.id),
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
    let korean = translate(&app, &lead, &filed, Some(&content)).await;
    tell(&app, &category, &lead, &filed, &korean, &exposure).await;
  }
}

/// An item's Korean rendering asked, its try counted and its next set at the retry wait before it is asked,
/// the English text read where it is not given. A rendering that cannot be had stays pending, or after its
/// last retry is no longer asked.
async fn translate(app: &App, lead: &Item, filed: &Filed, content: Option<&str>) -> Korean {
  let next = now_ms() + app.settings.retry_minutes * 60_000;
  let tries = match app.store.claim_translation(lead.id, next) {
    Ok(tries) => tries,
    Err(why) => {
      eprintln!("bmo-research: item {}'s Korean rendering could not be counted: {why}", lead.id);
      return Korean::Unavailable;
    }
  };
  let body = match content {
    Some(content) => Ok(content.to_string()),
    None => app.peers.content(filed.document_id).await,
  };
  let rendered = match body {
    Ok(body) => {
      let english = English {
        item_id: lead.id,
        name: &lead.name,
        document_id: filed.document_id,
        title: &filed.title,
        description: filed.description.as_deref(),
        body: &body,
      };
      translation::render(app, &english).await
    }
    Err(why) => Err(why),
  };
  match rendered {
    Ok(rendering) => {
      let _ = app.store.keep_rendering(filed.document_id, rendering.document_id, &rendering.share_url);
      Korean::Filed(rendering)
    }
    Err(why) if tries > app.settings.retries => {
      eprintln!(
        "bmo-research: item {}'s Korean rendering failed after {tries} tries and is no longer asked: {why}",
        lead.id
      );
      let _ = app.store.drop_translation(lead.id);
      Korean::Unavailable
    }
    Err(why) => {
      eprintln!("bmo-research: item {}'s Korean rendering is pending: {why}", lead.id);
      Korean::Pending
    }
  }
}

/// What a filed item holds of its document, for a rendering asked again after its report was told.
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

/// A pending Korean rendering asked again; once it is filed, every recipient of the category is told the
/// follow-up.
async fn render_again(app: Arc<App>, item: Item) {
  let Some(document_id) = item.document_id else {
    let _ = app.store.drop_translation(item.id);
    return;
  };
  let filed = filed_of(&item, document_id);
  if let Korean::Filed(rendering) = translate(&app, &item, &filed, None).await
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

/// The notice a filed item's recipients hear: the line of the Korean rendering and the English document's
/// link after it, or the English line and the pending line while the rendering is pending; then for a `fit`
/// item what its recipient needs to act.
pub fn notice(lead: &Item, filed: &Filed, korean: &Korean) -> String {
  let words = words::report();
  let line = |title: &str, description: Option<&str>, link: &str| {
    let summary = reading::first_sentence(description.unwrap_or_default());
    fill(&words.line, &[("title", title), ("summary", &summary), ("link", link)])
  };
  let mut lines = match korean {
    Korean::Filed(rendering) => vec![
      line(&rendering.title, rendering.description.as_deref(), &rendering.share_url),
      fill(&words.english, &[("link", &filed.share_url)]),
    ],
    Korean::Pending => vec![
      line(&filed.title, filed.description.as_deref(), &filed.share_url),
      words.pending.clone(),
    ],
    Korean::Unavailable => vec![line(&filed.title, filed.description.as_deref(), &filed.share_url)],
  };
  lines.extend(acting(lead, filed));
  lines.join("\n")
}

/// The follow-up told once a pending rendering is filed: the Korean link, the English one after it, and for
/// a `fit` item what its recipient needs to act, since it replaces the report where that still waits.
pub fn follow_up(lead: &Item, filed: &Filed, rendering: &Rendering) -> String {
  let words = words::report();
  let mut lines = vec![
    fill(&words.translated, &[("title", &rendering.title), ("link", &rendering.share_url)]),
    fill(&words.english, &[("link", &filed.share_url)]),
  ];
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
