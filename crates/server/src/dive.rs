//! The deep dive and its notices (`docs/api.md`, 「The deep dive」, 「The page」 and 「The notices」): the next
//! run picked among the running categories, its engine's card waited for where its road is watched, asked of
//! the runner, read until it closes, its result read and its items filed, its page finished with its Korean
//! side and its narrations, and every recipient told once the page is whole; what a page still lacks asked
//! again on every retry wait until it is had; an interrupted try put back uncounted and asked again at once,
//! a failed one counted and waited, and an item stuck past its retries handed to the helper to repair.

use std::collections::HashSet;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use bmo_research_client::{
  Category, CategoryShape, Item, ItemState, OriginKind, RecipientKind, ResearchRunAsk, ResearchRunState,
  Running, Shape,
};
use serde_json::{Value, json};
use tokio::task::JoinHandle;
use unicode_segmentation::UnicodeSegmentation;

use crate::App;
use crate::document::{self, Drawn};
use crate::engine;
use crate::page::{self, Side, Sound};
use crate::peers::Synthesis;
use crate::reading::{self, Exposure};
use crate::runner::{self, Filing, PageAsk, Refused};
use crate::store::{Filed, now_ms};
use crate::translation::{self, English, Rendering};
use crate::words::{self, fill};

/// The most characters the voice unit speaks in one text; it refuses a longer one.
pub const VOICE_MOST_CHARACTERS: usize = 4_000;

/// The languages a page may hold, in the order it shows them.
const LANGUAGES: [&str; 2] = ["ko", "en"];

/// The name the queue gives a page's pending Korean side; a pending narration is named `narration-<code>`.
const KOREAN: &str = "korean";

/// How many of the latest research run's log lines a repair carries.
const REPAIR_LOG_LINES: usize = 20;

/// The last failure to file a repair, said in the log once while it stays the same.
#[derive(Debug, Default)]
pub struct Repairs(Mutex<Option<String>>);

impl Repairs {
  fn failed(&self, why: String) {
    let mut failure = self.0.lock().unwrap_or_else(|e| e.into_inner());
    if failure.as_deref() != Some(why.as_str()) {
      eprintln!("bmo-research: a repair was not filed: {why}");
      *failure = Some(why);
    }
  }

  fn filed(&self) {
    *self.0.lock().unwrap_or_else(|e| e.into_inner()) = None;
  }
}

/// The pages being finished now, by their item, so a page whose finishing outlasts its retry wait is not
/// asked a second time beside it.
#[derive(Debug, Default)]
pub struct Pages(Mutex<HashSet<i64>>);

impl Pages {
  /// The item's page held for finishing, or none where it is being finished already.
  fn hold(&self, item: i64) -> Option<PageHold<'_>> {
    // Built only where it is taken: a hold refused and dropped would give back the page held already.
    let taken = self.0.lock().unwrap_or_else(|e| e.into_inner()).insert(item);
    taken.then(|| PageHold { pages: self, item })
  }
}

/// A page held for finishing, given back as it is dropped.
struct PageHold<'a> {
  pages: &'a Pages,
  item: i64,
}

impl Drop for PageHold<'_> {
  fn drop(&mut self) {
    self.pages.0.lock().unwrap_or_else(|e| e.into_inner()).remove(&self.item);
  }
}

/// How an item's page stands once it is finished as far as it can be now: its Korean side where it is had,
/// what it still lacks, whole where that is nothing, and whether that changed on this call.
struct Finished {
  korean: Option<Rendering>,
  pending: Vec<String>,
  changed: bool,
}

/// What a page lacks, as the run's log says it.
fn lacking_said(pending: &[String]) -> String {
  let parts: Vec<String> = pending
    .iter()
    .map(|part| match part.strip_prefix("narration-") {
      Some(code) => format!("the {code} narration"),
      None => "the Korean side".to_string(),
    })
    .collect();
  match parts.split_last() {
    Some((last, [])) => last.clone(),
    Some((last, rest)) => format!("{} and {last}", rest.join(", ")),
    None => String::new(),
  }
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

/// The daemon's loop: the tries it left running taken up, every filed page drawn as this binary draws it, the
/// owner's room given to every category holding no recipient, then a scan and a start of what may run every
/// `tick_seconds`.
pub async fn daemon(app: Arc<App>) {
  take_up(&app);
  let drawing = app.clone();
  tokio::spawn(async move { redraw_filed(&drawing).await });
  give_owner_room(&app).await;
  loop {
    let _ = tick(&app).await;
    tokio::time::sleep(Duration::from_secs(app.settings.tick_seconds.max(1))).await;
  }
}

/// The tries a stopped daemon left running: one asked is read again, one never asked goes back as it stood.
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
        let why = "the daemon stopped before the run was asked";
        let _ = app.store.requeue(run.id, "failed", None, why, app.settings.retries);
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

/// One pass of the loop: the demands left behind given back, a scan, a repair filed for every stuck item
/// holding none, every page whose pending part's retry is due asked again, then as many tries started as
/// `concurrency` leaves room for, a try waiting for its card among them. Answers the tasks started, each
/// carried to its end on a task of its own.
pub async fn tick(app: &Arc<App>) -> Vec<JoinHandle<()>> {
  engine::give_back_left(app).await;
  if let Err(why) = crate::scan::scan(app).await {
    eprintln!("bmo-research: the scan: {why}");
  }
  repair(app).await;
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

/// The next try to ask: in each running category, its next waiting item, or a brief's whole gathering once it
/// is due; among them, the highest priority, then the category whose last try was asked longest ago.
fn next(app: &App) -> Option<(Category, Vec<Item>)> {
  let now = now_ms();
  let mut best: Option<(i64, Option<i64>, Category, Vec<Item>)> = None;
  for category in app.store.categories().ok()? {
    if category.state == Running::Stopped {
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

/// The Korean title of the document a follow-up follows from, where its page has a Korean side.
fn parent_korean_title(app: &App, item: &Item) -> Option<String> {
  let document = item.origin.document_id?;
  app.store.rendering(document).ok().flatten().map(|rendering| rendering.title)
}

/// The line under an item's page's title saying where its research came from, in one language, the parent's
/// Korean title named on the Korean side; none for a brief, which its category's sources gather.
fn origin_of(app: &App, item: &Item, code: &str) -> Option<String> {
  if item.shape == Shape::Brief {
    return None;
  }
  let title = if code == LANGUAGES[0] { parent_korean_title(app, item) } else { None };
  page::origin_line(item, code, title.as_deref())
}

/// The question a try is asked: one item's shape with its name, address and origin, an advisory's with the
/// stack, a brief's with every item it gathers.
fn question(app: &App, items: &[Item]) -> String {
  let lead = &items[0];
  let address = lead.address.clone().unwrap_or_default();
  let origin = origin_of(app, lead, LANGUAGES[1]).unwrap_or_default();
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
        &[
          ("name", &lead.name),
          ("address", &address),
          ("origin", &origin),
          ("stack", &stack.join("\n")),
        ],
      )
    }
    shape => words::question(shape, &[("name", &lead.name), ("address", &address), ("origin", &origin)]),
  }
}

/// One try carried to its end, and the demand it held given back where it ended before its run did.
pub async fn carry(app: Arc<App>, run: i64, asked: Option<i64>) {
  carried(app.clone(), run, asked).await;
  engine::give_back_try(&app, run).await;
}

/// One try carried to its end: its engine's card waited for and the try asked where it was not, read until
/// its run closes, filed and told. A try whose category is stopped before its card is granted, or that is
/// interrupted, goes back uncounted; one that fails on its content is counted.
async fn carried(app: Arc<App>, run: i64, asked: Option<i64>) {
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
      let demand = match engine::watched(&app, &provider, &model) {
        Some(watched) => match engine::card(&app, run, &category.name, &lead, &watched).await {
          Some(id) => Some(id),
          None => {
            let why =
              "the category was stopped before the run's card was granted, so the try is not counted";
            let _ = app.store.requeue(run, "failed", None, why, app.settings.retries);
            return;
          }
        },
        None => None,
      };
      let ask = ResearchRunAsk {
        key: app.settings.road.key.clone(),
        query: question(&app, &items),
        provider: Some(provider),
        model: Some(model),
        ..ResearchRunAsk::default()
      };
      let filing = Filing::Page(PageAsk {
        sections: words::document().sections.of(lead.shape).to_vec(),
        opening: words::document().opening.of(lead.shape).to_vec(),
        origin: origin_of(&app, &lead, LANGUAGES[1]),
      });
      match runner::launch(app.clone(), ask, filing, demand).await {
        Ok(launched) => {
          let _ = app.store.keep_run_id(run, launched.run_id);
          launched.run_id
        }
        Err(Refused::Unavailable(why)) => {
          return interrupt(&app, run, &lead, "refused", None, &why);
        }
        Err(Refused::Ask(why) | Refused::Upstream(why) | Refused::Failed(why)) => {
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
    let why = closed.error.clone().unwrap_or_else(|| "the run was cut".into());
    return interrupt(&app, run, &lead, "cut", outcome.as_deref(), &why);
  }
  let Some(document_id) = closed.document_id else {
    let why = closed.error.clone().unwrap_or_else(|| "the run closed with no document filed".into());
    return fail(&app, run, &lead, "failed", outcome.as_deref(), &why).await;
  };
  // The run has ended: its card is given back before its page is finished.
  engine::give_back_try(&app, run).await;
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
      let read = reading::manual_fit(&content, &words::document().manual);
      filed.fit = read.fit;
      filed.plan = read.plan;
    }
    Shape::Advisory => {
      exposure = exposure_of(&lead, &source);
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
  let retry_at = now_ms() + app.settings.retry_minutes * 60_000;
  let mut finishing = false;
  for item in &items {
    let retry = (item.id == lead.id).then_some(retry_at);
    if app.store.file(item.id, &filed, retry).unwrap_or(false) && item.id == lead.id {
      finishing = true;
    }
  }
  if !finishing {
    // The lead was dismissed while its run went on: nobody is told, and its gathering stands filed.
    let _ = app.store.finished(lead.id, document_id);
    return;
  }
  let Some(_held) = app.pages.hold(lead.id) else {
    return;
  };
  if let Some(page) = finish(&app, run_id, &lead, &filed, &source).await {
    report(&app, &category, run_id, &lead, &filed, &page, &exposure).await;
  }
}

/// What an advisory's document says it touches; nothing for another shape.
fn exposure_of(lead: &Item, source: &Source) -> Exposure {
  match lead.shape {
    Shape::Advisory => reading::exposure(&source.drawn.body, &words::document().advisory),
    _ => Exposure::default(),
  }
}

/// The file a narration's sound is kept under beside its page.
pub fn sound_name(code: &str) -> String {
  format!("narration-{code}.ogg")
}

/// A text as the voice unit can speak it: whole where it holds at most `most` characters, else up to the last
/// sentence that ends within them, the last word where no sentence does, or its first `most` characters.
pub fn speakable(text: &str, most: usize) -> &str {
  let text = text.trim();
  if text.chars().count() <= most {
    return text;
  }
  let within = |pieces: &mut dyn Iterator<Item = (usize, &str)>| {
    let mut end = 0;
    let mut count = 0;
    for (at, piece) in pieces {
      count += piece.chars().count();
      if count > most {
        break;
      }
      end = at + piece.len();
    }
    end
  };
  let end = match within(&mut text.split_sentence_bound_indices()) {
    0 => within(&mut text.split_word_bound_indices()),
    end => end,
  };
  let end = match end {
    0 => within(&mut text.char_indices().map(|(at, c)| (at, &text[at..at + c.len_utf8()]))),
    end => end,
  };
  text[..end].trim_end()
}

/// One narration's sound attached beside its page, which the archive's reader plays: the one the document
/// already keeps, else the sound of the synthesis kept for it, else the script spoken — in Korean translated
/// to be spoken first, and either cut to what the voice unit speaks, the run's log saying so — its synthesis
/// kept until its sound is attached, so the narration has one synthesis living at a time. A kept synthesis
/// that has failed or is gone is set aside and another asked; the one followed is followed however long it
/// waits, the run's log saying what on whenever that changes, and attached, the Korean as the sound heard at
/// the document's head.
async fn narration(
  app: &App,
  run: i64,
  lead: &Item,
  document_id: i64,
  code: &str,
  script: &str,
) -> Result<(), String> {
  let name = sound_name(code);
  if app.peers.attached(document_id, &name).await? {
    let _ = app.store.forget_synthesis(document_id, code);
    return Ok(());
  }
  let mut kept = app.store.synthesis(document_id, code)?;
  if let Some((id, _)) = kept
    && matches!(app.peers.read_synthesis(id).await, Ok(Synthesis::Failed | Synthesis::Gone))
  {
    app.store.forget_synthesis(document_id, code)?;
    kept = None;
  }
  let (id, size) = match kept {
    Some(kept) => kept,
    None => {
      let text = match code {
        "en" => script.to_string(),
        _ => translation::narration(app, &lead.name, script).await?,
      };
      let spoken = speakable(&text, VOICE_MOST_CHARACTERS);
      let (whole, kept) = (text.trim().chars().count(), spoken.chars().count());
      if kept < whole {
        let said = format!(
          "The {code} narration holds {whole} characters, more than the voice speaks, so the first {kept} \
           are spoken."
        );
        let _ = app.store.log_research(run, "narration", &said);
      }
      let id = app.peers.ask_synthesis(spoken, code).await?;
      app.store.keep_synthesis(document_id, code, id, kept as i64)?;
      (id, kept as i64)
    }
  };
  let ended = app
    .peers
    .follow_synthesis(id, |on| {
      let said = format!("The {code} narration waits for the voice: {on}");
      let _ = app.store.log_research(run, "narration", &said);
    })
    .await?;
  if ended != Synthesis::Done {
    app.store.forget_synthesis(document_id, code)?;
    let how = if ended == Synthesis::Gone { "is gone" } else { "failed" };
    return Err(format!("the voice unit's synthesis {id} of {size} characters {how}"));
  }
  let (bytes, media_type) = app.peers.synthesis_sound(id).await?;
  let role = (code == LANGUAGES[0]).then_some("head");
  app.peers.attach(document_id, &name, &media_type, role, &bytes).await?;
  app.store.forget_synthesis(document_id, code)
}

/// A page as it is drawn now: its HTML, the title it is set under and its document's labels.
struct Drawing {
  html: String,
  title: String,
  labels: Value,
}

/// The page drawn with the sides it holds, Korean first, and its listen band referring to each narration
/// attached.
fn drawing(
  app: &App,
  lead: &Item,
  source: &Source,
  rendering: Option<&Rendering>,
  narrations: &[String],
) -> Drawing {
  let english = document::page_body(&source.drawn, &source.title, words::document().opening.of(lead.shape));
  let origins = LANGUAGES.map(|code| origin_of(app, lead, code));
  let mut sides = Vec::new();
  if let Some(korean) = rendering {
    sides.push(Side {
      code: LANGUAGES[0],
      title: &korean.title,
      description: korean.description.as_deref(),
      origin: origins[0].as_deref(),
      body: &korean.body,
    });
  }
  sides.push(Side {
    code: LANGUAGES[1],
    title: &source.title,
    description: source.drawn.description.as_deref(),
    origin: origins[1].as_deref(),
    body: &english,
  });
  let names: Vec<(&str, String)> = LANGUAGES
    .into_iter()
    .filter(|code| narrations.iter().any(|had| had == code))
    .map(|code| (code, sound_name(code)))
    .collect();
  let sounds: Vec<Sound<'_>> = names.iter().map(|(code, name)| Sound { code, name }).collect();
  let mut labels = source.labels.clone();
  labels["languages"] = sides.iter().map(|side| side.code).collect::<Vec<_>>().join(",").into();
  Drawing {
    html: page::compose(&sides, &sounds),
    title: sides[0].title.to_owned(),
    labels,
  }
}

/// A page drawn set in the archive.
async fn set_page(app: &App, document_id: i64, drawn: &Drawing) -> Result<(), String> {
  app
    .peers
    .artifact_send(
      reqwest::Method::PATCH,
      &format!("/documents/{document_id}"),
      &json!({ "title": drawn.title, "html": drawn.html, "labels": drawn.labels }),
    )
    .await
    .map(|_| ())
}

/// The page drawn again with the sides it holds, Korean first, and its listen band referring to each narration
/// attached, and set in the archive.
async fn redraw(
  app: &App,
  lead: &Item,
  document_id: i64,
  source: &Source,
  rendering: Option<&Rendering>,
  narrations: &[String],
) -> Result<(), String> {
  set_page(app, document_id, &drawing(app, lead, source, rendering, narrations)).await
}

/// Every filed page drawn as this binary draws it (`docs/api.md`, 「The page」): the page of every item holding
/// a document, for the item its document's run was asked for, drawn again from what is had of it and set where
/// it differs from the page the archive keeps; a page being finished now is left to its finishing, which draws
/// it the same way. Answers how many pages were set.
pub async fn redraw_filed(app: &App) -> usize {
  let items = match app.store.filed_pages() {
    Ok(items) => items,
    Err(why) => {
      eprintln!("bmo-research: the filed pages could not be read: {why}");
      return 0;
    }
  };
  let mut set = 0;
  for item in items {
    let (Some(document_id), Some(run_id)) = (item.document_id, item.run_id) else {
      continue;
    };
    let Some(_held) = app.pages.hold(item.id) else {
      continue;
    };
    match drawn_again(app, &item, document_id, run_id).await {
      Ok(true) => set += 1,
      Ok(false) => {}
      Err(why) => {
        eprintln!("bmo-research: item {}'s page {document_id} was not drawn again: {why}", item.id)
      }
    }
  }
  eprintln!("bmo-research: {set} filed pages were drawn again");
  set
}

/// An item's filed page drawn again from what is had of it, and set where it differs from the page the archive
/// keeps. Answers whether it was set.
async fn drawn_again(app: &App, item: &Item, document_id: i64, run_id: i64) -> Result<bool, String> {
  let source = source(app, run_id)?;
  let rendering = match item.languages.iter().any(|code| code == LANGUAGES[0]) {
    true => app.store.rendering(document_id)?,
    false => None,
  };
  let drawn = drawing(app, item, &source, rendering.as_ref(), &item.narrations);
  if app.peers.kept_page(document_id).await? == drawn.html {
    return Ok(false);
  }
  set_page(app, document_id, &drawn).await?;
  Ok(true)
}

/// An item's page finished as far as it can be now: its Korean side asked where it lacks one, each narration
/// its script allows asked and attached where it lacks it, the page drawn again with what was had, and what it
/// still lacks kept, to be asked again `retry_minutes` from now. Answers how it stands, or none where the
/// store could not be read.
async fn finish(app: &App, run: i64, lead: &Item, filed: &Filed, source: &Source) -> Option<Finished> {
  let document_id = filed.document_id;
  let unread = |why: String| eprintln!("bmo-research: item {}'s page could not be read: {why}", lead.id);
  let state = app.store.page_state(lead.id).map_err(unread).ok()?;
  let had = state.languages.iter().any(|code| code == LANGUAGES[0]);
  let mut rendering = if had { app.store.rendering(document_id).ok().flatten() } else { None };
  let ask_korean = rendering.is_none();
  let script = source.drawn.narration.clone().filter(|script| !script.trim().is_empty());
  let lacking: Vec<&str> = LANGUAGES
    .into_iter()
    .filter(|code| script.is_some() && !state.narrations.iter().any(|had| had == code))
    .collect();
  let next = now_ms() + app.settings.retry_minutes * 60_000;
  app
    .store
    .claim_page(lead.id, next, ask_korean, !lacking.is_empty())
    .map_err(unread)
    .ok()?;
  let mut fresh = false;
  if ask_korean {
    let body = document::page_body(&source.drawn, &source.title, words::document().opening.of(lead.shape));
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
      Err(why) => {
        eprintln!("bmo-research: item {}'s Korean side is pending: {why}", lead.id);
        let _ = app.store.log_research(run, "filed", &format!("The Korean side is pending: {why}"));
      }
    }
  }
  let mut sounds: Vec<String> = state.narrations.clone();
  let mut heard = false;
  for code in lacking {
    let Some(script) = script.as_deref() else {
      continue;
    };
    match narration(app, run, lead, document_id, code, script).await {
      Ok(()) => {
        heard = true;
        sounds.push(code.to_owned());
        let _ = app.store.log_research(run, "narration", &format!("The {code} narration was attached."));
      }
      Err(why) => {
        eprintln!("bmo-research: item {}'s {code} narration was not made: {why}", lead.id);
        let said = format!("The {code} narration was not made: {why}");
        let _ = app.store.log_research(run, "narration", &said);
      }
    }
  }
  if fresh || heard {
    match redraw(app, lead, document_id, source, rendering.as_ref(), &sounds).await {
      Ok(()) => {
        if let Some(korean) = rendering.as_ref().filter(|_| fresh) {
          let _ = app.store.keep_rendering(document_id, korean);
        }
        if heard {
          let _ = app.store.keep_narrations(document_id, &sounds);
        }
      }
      Err(why) => {
        // What the page was not drawn with is lacking still: a sound attached is found again on the next
        // try, and the Korean side is asked again.
        eprintln!("bmo-research: item {}'s page was not drawn again: {why}", lead.id);
        let _ = app.store.log_research(run, "filed", &format!("The page was not drawn again: {why}"));
        if fresh {
          rendering = None;
        }
        sounds = state.narrations.clone();
      }
    }
  }
  let mut pending = Vec::new();
  if rendering.is_none() {
    pending.push(KOREAN.to_string());
  }
  if script.is_some() {
    for code in LANGUAGES.into_iter().filter(|code| !sounds.iter().any(|had| had == code)) {
      pending.push(format!("narration-{code}"));
    }
  }
  let retry = (!pending.is_empty()).then(|| now_ms() + app.settings.retry_minutes * 60_000);
  let _ = app.store.pend_page(lead.id, &pending, retry);
  let changed = pending != state.pending;
  Some(Finished {
    korean: rendering,
    pending,
    changed,
  })
}

/// What a filed item holds of its document, for a page finished on a later try.
fn filed_of(item: &Item, document_id: i64) -> Filed {
  Filed {
    document_id,
    title: item.title.clone().unwrap_or_default(),
    description: item.description.clone(),
    share_url: item.share_url.clone().unwrap_or_default(),
    fit: item.fit,
    touches: item.touches.clone(),
    plan: item.plan.clone(),
  }
}

/// What a page still lacks asked again, unless it is being finished already, and its report told once the
/// page is whole. A page whose source cannot be read now is asked again on the next retry wait.
async fn render_again(app: Arc<App>, item: Item) {
  let (Some(document_id), Some(run_id)) = (item.document_id, item.run_id) else {
    let _ = app.store.pend_page(item.id, &item.pending, None);
    return;
  };
  let Some(_held) = app.pages.hold(item.id) else {
    return;
  };
  let source = match source(&app, run_id) {
    Ok(source) => source,
    Err(why) => {
      eprintln!("bmo-research: item {}'s page cannot be drawn again now: {why}", item.id);
      let next = now_ms() + app.settings.retry_minutes * 60_000;
      let _ = app.store.pend_page(item.id, &item.pending, Some(next));
      return;
    }
  };
  let filed = filed_of(&item, document_id);
  let Some(page) = finish(&app, run_id, &item, &filed, &source).await else {
    return;
  };
  match app.store.category(&item.category_name) {
    Ok(category) => {
      let exposure = exposure_of(&item, &source);
      report(&app, &category, run_id, &item, &filed, &page, &exposure).await;
    }
    Err(why) => eprintln!("bmo-research: item {}'s category could not be read: {why:?}", item.id),
  }
}

/// A finished page's report: told to every recipient once the page is whole while its item stands
/// `finishing`, the run's log saying so under `told`; while it lacks a part, the run's log saying under `filed`
/// what it waits on each time that changes. A page whose report was told before is not told again.
async fn report(
  app: &App,
  category: &Category,
  run: i64,
  lead: &Item,
  filed: &Filed,
  page: &Finished,
  exposure: &Exposure,
) {
  let finishing = app.store.item(lead.id).map(|item| item.state == ItemState::Finishing).unwrap_or(false);
  let every = app.settings.retry_minutes;
  match page.korean.as_ref().filter(|_| page.pending.is_empty()) {
    None if page.changed => {
      let lacks = lacking_said(&page.pending);
      let said = if finishing {
        format!(
          "The report waits for its page, which lacks {lacks}; what it lacks is asked again every {every} \
           minutes until it is had, and the report is told once the page is whole."
        )
      } else {
        format!(
          "The page lacks {lacks}, asked again every {every} minutes until it is had; its report was told before."
        )
      };
      let _ = app.store.log_research(run, "filed", &said);
    }
    None => {}
    Some(korean) if finishing => {
      if app.store.finished(lead.id, filed.document_id).unwrap_or(false) {
        tell(app, category, lead, filed, korean, exposure).await;
        let _ = app.store.log_research(run, "told", "The page is whole, and its report was told.");
      }
    }
    Some(_) if page.changed => {
      let _ = app.store.log_research(run, "filed", "The page is whole; its report was told before.");
    }
    Some(_) => {}
  }
}

/// An interrupted try (`docs/api.md`, 「The deep dive」): something the run needs could not be reached or was
/// taken away, so the try is not counted and its items go back as they stood, asked again on the next pass.
fn interrupt(app: &App, run: i64, lead: &Item, state: &str, outcome: Option<&str>, why: &str) {
  let error = format!("{why}, so the try is not counted");
  eprintln!("bmo-research: item {}: {error}", lead.id);
  let _ = app.store.requeue(run, state, outcome, &error, app.settings.retries);
}

/// A failed try: the arbiter's is interrupted instead; any other is counted and its items wait again after
/// `retry_minutes`, `stuck` once the lead's tries pass `retries`, every recipient told once as it first
/// becomes so.
async fn fail(app: &App, run: i64, lead: &Item, state: &str, outcome: Option<&str>, why: &str) {
  if let Some(gone) = engine::evicted(app, run).await {
    return interrupt(app, run, lead, state, outcome, &format!("{why}; {gone}"));
  }
  let _ = app.store.end_run(run, state, outcome, Some(why));
  let tries = app.store.item(lead.id).map(|i| i.tries).unwrap_or(lead.tries);
  let stuck = tries > app.settings.retries;
  let retry_at = now_ms() + app.settings.retry_minutes * 60_000;
  let moved = app.store.fail_try(run, why, retry_at, stuck).unwrap_or_default();
  if tries == app.settings.retries + 1 && moved.contains(&lead.id) {
    eprintln!("bmo-research: item {} is stuck after {tries} tries: {why}", lead.id);
    if let Ok(category) = app.store.category(&lead.category_name) {
      let words = words::report();
      let notice = fill(&words.stuck, &[("name", &lead.name), ("tries", &tries.to_string())]);
      notify(app, &category, lead, &notice, "stuck").await;
    }
  }
}

/// A repair filed in the job service for the stuck items of each research run that hold none, and for each
/// stuck item with no run, the job item's number kept on each (`docs/api.md`, 「The repair」).
pub async fn repair(app: &App) {
  let mut groups: Vec<Vec<Item>> = Vec::new();
  for item in app.store.unrepaired().unwrap_or_default() {
    match groups.iter_mut().find(|group| item.run_id.is_some() && group[0].run_id == item.run_id) {
      Some(group) => group.push(item),
      None => groups.push(vec![item]),
    }
  }
  for group in groups {
    let filed = match repair_item(app, &group) {
      Ok(item) => app.peers.file_item(&item).await,
      Err(why) => Err(why),
    };
    match filed {
      Ok(job_item) => {
        app.repairs.filed();
        let ids: Vec<i64> = group.iter().map(|item| item.id).collect();
        eprintln!("bmo-research: item {} is stuck; repair {job_item} was filed for the helper", ids[0]);
        let _ = app.store.keep_repair(&ids, job_item);
      }
      Err(why) => app.repairs.failed(why),
    }
  }
}

/// The job item a stuck group's repair is filed as: the words of `prompts/repair.toml` with the evidence set
/// in, for the settings' queue, worker and labels.
fn repair_item(app: &App, group: &[Item]) -> Result<Value, String> {
  let words = words::repair();
  let lead = &group[0];
  let mut labels = serde_json::Map::new();
  for label in &app.settings.job.labels {
    match label.split_once('=') {
      Some((key, value)) if !key.trim().is_empty() && !value.trim().is_empty() => {
        labels.insert(key.trim().to_owned(), json!(value.trim()));
      }
      _ => return Err(format!("the job label {label} is no key=value")),
    }
  }
  labels.insert("research-item".into(), json!(lead.id.to_string()));
  let items: Vec<String> = group
    .iter()
    .map(|item| {
      fill(
        &words.item,
        &[
          ("id", &item.id.to_string()),
          ("name", &item.name),
          ("address", item.address.as_deref().unwrap_or("none")),
          ("category", &item.category_name),
          ("shape", item.shape.as_str()),
          ("tries", &item.tries.to_string()),
          ("error", item.error.as_deref().unwrap_or("none")),
        ],
      )
    })
    .collect();
  let tries = app.store.runs(Some(lead.id), None, None, 10).map(|page| page.items).unwrap_or_default();
  let runs: Vec<String> = tries
    .iter()
    .map(|attempt| {
      let run = attempt.run_id.map_or("none".to_string(), |id| id.to_string());
      let state = match attempt.outcome.as_deref() {
        Some(outcome) => format!("{} ({outcome})", attempt.state),
        None => attempt.state.clone(),
      };
      fill(
        &words.attempt,
        &[
          ("try", &attempt.attempt.to_string()),
          ("run", &run),
          ("state", &state),
          ("error", attempt.error.as_deref().unwrap_or("none")),
        ],
      )
    })
    .collect();
  let latest = lead.run_id.or_else(|| tries.iter().find_map(|attempt| attempt.run_id));
  let lines = latest.map(|run| app.store.research_log(run).unwrap_or_default()).unwrap_or_default();
  let log: Vec<String> = lines
    .iter()
    .skip(lines.len().saturating_sub(REPAIR_LOG_LINES))
    .map(|line| fill(&words.line, &[("kind", &line.kind), ("text", &line.text)]))
    .collect();
  let log = if log.is_empty() { words.no_log.clone() } else { log.join("\n") };
  let id = lead.id.to_string();
  let run = latest.map_or("none".to_string(), |run| run.to_string());
  let address = format!("http://{}", app.settings.listen);
  let brief = fill(
    &words.brief,
    &[
      ("items", &items.join("\n")),
      ("tries", &lead.tries.to_string()),
      ("runs", &runs.join("\n")),
      ("run", &run),
      ("log", &log),
      ("id", &id),
      ("address", &address),
    ],
  );
  Ok(json!({
    "title": fill(&words.title, &[("id", &id), ("name", &lead.name)]),
    "brief": brief,
    "queueName": app.settings.job.queue,
    "assigneeName": app.settings.job.assignee,
    "labels": labels,
  }))
}

/// The notice a whole page's recipients hear: the line of the page's Korean side and its one link, naming the
/// document a follow-up follows from by `parent`, its Korean title, else the title the item keeps, else its
/// number; then for a `fit` item what its recipient needs to act.
pub fn notice(lead: &Item, filed: &Filed, korean: &Rendering, parent: Option<&str>) -> String {
  let words = words::report();
  let summary = reading::first_sentence(korean.description.as_deref().unwrap_or_default());
  let origin = &lead.origin;
  let follows = match (origin.kind, parent.or(origin.title.as_deref()), origin.document_id) {
    (OriginKind::FollowUp, Some(title), _) if !title.trim().is_empty() => {
      fill(&words.follows, &[("parent", title.trim())])
    }
    (OriginKind::FollowUp, _, Some(document)) => {
      fill(&words.follows_document, &[("document", &document.to_string())])
    }
    _ => String::new(),
  };
  let mut lines = vec![fill(
    &words.line,
    &[
      ("title", &korean.title),
      ("follows", &follows),
      ("summary", &summary),
      ("link", &filed.share_url),
    ],
  )];
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
  }
  lines
}

/// Every recipient told of a filed item, and for an advisory each touched repository's worker its part.
async fn tell(
  app: &App,
  category: &Category,
  lead: &Item,
  filed: &Filed,
  korean: &Rendering,
  exposure: &Exposure,
) {
  let parent = parent_korean_title(app, lead);
  notify(app, category, lead, &notice(lead, filed, korean, parent.as_deref()), "report").await;
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
