//! The research runner (`docs/runner.md`): a search at a depth or a browse, composed here — its line's
//! instructions with the web namespace's listing and the rulebooks, its request, the record its sessions
//! carry — sent out on a seat of agency's co-worker pool in a thrown-away session of agency's, asked its
//! rounds in a second session, its result filed as a document and told to its room, every step kept in the
//! run's ledger and log. The research settings it is held to are kept here too.

use std::collections::BTreeMap;
use std::sync::Arc;
use std::time::Duration;

use bmo_research_client::{
  ResearchKeySettings, ResearchRun, ResearchRunAsk, ResearchRunState, ResearchSettings,
  ResearchSettingsChange, SearchEngineState,
};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

use crate::App;
use crate::config::KeyDefaults;
use crate::document;
use crate::peers::Ending;
use crate::store::{NewResearchRun, ResearchRunRow, RunDocument, now_ms};
use crate::tool_host::ToolHost;
use crate::words::{self, FINAL, PastCeiling, fill};

/// The research keys, in the order the settings answer them.
pub const KEYS: [&str; 4] = ["search:basic", "search:thorough", "search:exhaustive", "browse"];

/// How the web tools may read a page.
pub const STRATEGIES: [&str; 3] = ["direct-then-browser", "browser-first", "direct-only"];

/// The capability that shows a run the branch tool.
const BRANCHES: &str = "web.branches";

/// The capability that shows a run the rulebook reader.
const RULEBOOKS: &str = "web.rulebooks";

/// How often a seat still waiting is asked for its place.
pub const LOOK_EVERY: Duration = Duration::from_millis(500);

/// How often an end answered busy is asked again.
const END_EVERY: Duration = Duration::from_secs(2);

/// How long past a turn's bound its send is waited on.
const SEND_SLACK: Duration = Duration::from_secs(60);

/// The line a research key goes out on.
pub fn line_of(key: &str) -> &'static str {
  if key == "browse" { "browse" } else { "search" }
}

// The depth a search key names, as the request words it.
fn depth_of(key: &str) -> &str {
  key.split_once(':').map_or(key, |(_, depth)| depth)
}

// A value an ask says, none where it is left out or blank.
fn said(value: &Option<String>) -> Option<&str> {
  value.as_deref().map(str::trim).filter(|said| !said.is_empty())
}

// The research settings.

/// The settings every run is held to: the owner's where he changed them, the settings file's otherwise.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Held {
  pub branch_role: String,
  pub strategy: String,
  pub host_interval_ms: Option<i64>,
  pub engine_interval_ms: Option<i64>,
  pub keys: BTreeMap<String, KeyDefaults>,
}

impl Held {
  /// A key's branches, clock and budget; none and five minutes for a key the settings name nothing for.
  pub fn key(&self, key: &str) -> KeyDefaults {
    self.keys.get(key).copied().unwrap_or(KeyDefaults {
      branches: 0,
      minutes: 5,
      pages: None,
      branch_pages: None,
    })
  }
}

/// The settings file's defaults.
pub fn defaults(app: &App) -> Held {
  let declared = &app.settings.research_settings;
  Held {
    branch_role: declared.branch_role.clone(),
    strategy: declared.strategy.clone(),
    host_interval_ms: declared.host_interval_ms,
    engine_interval_ms: declared.engine_interval_ms,
    keys: KEYS
      .iter()
      .map(|key| (key.to_string(), declared.keys.get(*key).copied()))
      .filter_map(|(k, v)| v.map(|v| (k, v)))
      .collect(),
  }
}

/// The settings as they stand, and whether the owner set them: his where they are kept and read, a key they
/// leave out, or a budget a kept key names none of, taking the defaults'.
pub fn held(app: &App) -> Result<(Held, bool), String> {
  let fallback = defaults(app);
  let Some(kept) = app
    .store
    .kept_research_settings()?
    .and_then(|text| serde_json::from_str::<Held>(&text).ok())
  else {
    return Ok((fallback, false));
  };
  let mut keys = fallback.keys;
  for (key, own) in kept.keys {
    if let Some(declared) = keys.get(&key).copied() {
      keys.insert(
        key,
        KeyDefaults {
          pages: own.pages.or(declared.pages),
          branch_pages: own.branch_pages.or(declared.branch_pages),
          ..own
        },
      );
    }
  }
  Ok((Held { keys, ..kept }, true))
}

/// The settings with a change laid over them, or why it cannot stand: a strategy outside the three, a gap
/// below nothing, a key that is no research key, branches below nothing, a clock of no minutes, a budget of no
/// page, or a role agency's pool does not list.
pub fn changed(mut held: Held, change: &ResearchSettingsChange, roles: &[String]) -> Result<Held, String> {
  if let Some(strategy) = &change.strategy {
    if !STRATEGIES.contains(&strategy.as_str()) {
      return Err(format!("{strategy} is no fetch strategy: {}", STRATEGIES.join(", ")));
    }
    held.strategy.clone_from(strategy);
  }
  if let Some(role) = &change.branch_role {
    if !roles.contains(role) {
      return Err(format!("{role} is no co-worker role: {}", roles.join(", ")));
    }
    held.branch_role.clone_from(role);
  }
  for (gap, given) in [
    (&mut held.host_interval_ms, change.host_interval_ms),
    (&mut held.engine_interval_ms, change.engine_interval_ms),
  ] {
    if let Some(given) = given {
      if given.is_some_and(|ms| ms < 0) {
        return Err("a gap is a number of milliseconds, never below nothing".into());
      }
      *gap = given;
    }
  }
  for key in change.keys.iter().flatten() {
    let Some(kept) = held.keys.get_mut(&key.key) else {
      return Err(format!("{} is no research key: {}", key.key, KEYS.join(", ")));
    };
    if key.branches < 0 || key.minutes < 1 {
      return Err(format!(
        "{}: branches are never below nothing, and a clock runs a minute at least",
        key.key
      ));
    }
    if key.pages.is_some_and(|pages| pages < 1) || key.branch_pages.is_some_and(|pages| pages < 1) {
      return Err(format!("{}: a budget reads one page at least", key.key));
    }
    *kept = KeyDefaults {
      branches: key.branches,
      minutes: key.minutes,
      pages: key.pages.or(kept.pages),
      branch_pages: key.branch_pages.or(kept.branch_pages),
    };
  }
  Ok(held)
}

/// The search engines' state, as the web tools answer it through the tool host, or why it could not be read.
pub async fn engines(app: &App) -> Result<Vec<SearchEngineState>, String> {
  let read = ToolHost::new(&app.settings)
    .system_call("explorer", "search_engines", json!({}))
    .await?;
  let listed = read["engines"].as_array().ok_or("the web tools named no engines")?;
  let text = |engine: &Value, name: &str| engine[name].as_str().map(str::to_owned);
  Ok(
    listed
      .iter()
      .map(|engine| SearchEngineState {
        name: text(engine, "name").unwrap_or_default(),
        category: text(engine, "category"),
        state: text(engine, "state").unwrap_or_else(|| "unknown".into()),
        reason: text(engine, "reason"),
        observed_at: engine["observed_at"].as_i64(),
        suspended_until: engine["suspended_until"].as_i64(),
      })
      .collect(),
  )
}

/// The settings as the routes answer them, with the engines' state.
pub async fn view(app: &App, held: &Held, set: bool) -> ResearchSettings {
  let (engines, engines_unread) = match engines(app).await {
    Ok(engines) => (Some(engines), None),
    Err(why) => (None, Some(why)),
  };
  ResearchSettings {
    branch_role: held.branch_role.clone(),
    strategy: held.strategy.clone(),
    host_interval_ms: held.host_interval_ms,
    engine_interval_ms: held.engine_interval_ms,
    keys: KEYS
      .iter()
      .filter_map(|key| {
        held.keys.get(*key).map(|kept| ResearchKeySettings {
          key: key.to_string(),
          branches: kept.branches,
          minutes: kept.minutes,
          pages: kept.pages,
          branch_pages: kept.branch_pages,
        })
      })
      .collect(),
    source: if set { "set" } else { "default" }.into(),
    engines,
    engines_unread,
  }
}

// The line's instructions.

fn rulebook_whole(name: &str) -> Result<String, String> {
  Ok(format!("RULEBOOK prompts/rules/{name}.md:\n{}", words::rulebook(name)?))
}

/// The sections of a rulebook a run judges by, each from its `## ` heading to the next; a heading the
/// rulebook no longer carries is a broken prompt.
pub fn rulebook_sections(name: &str, headings: &[String]) -> Result<String, String> {
  let text = words::rulebook(name)?;
  let sections = headings
    .iter()
    .map(|heading| {
      let start = text
        .find(&format!("\n## {heading}\n"))
        .ok_or_else(|| format!("prompts/rules/{name}.md has no section \"{heading}\""))?;
      let end = text[start + 1..].find("\n## ").map_or(text.len(), |at| at + start + 1);
      Ok(text[start + 1..end].trim().to_owned())
    })
    .collect::<Result<Vec<_>, String>>()?;
  Ok(format!(
    "RULEBOOK prompts/rules/{name}.md (sections: {}):\n{}",
    headings.join(", "),
    sections.join("\n\n")
  ))
}

/// A search's instructions: the task with the web namespace's listing, the search rulebooks — the ones
/// binding only some searches by their deciding section — then plain writing and self-preservation.
pub fn search_instructions(tools: &str) -> Result<String, String> {
  let search = &words::runner().search;
  let mut parts = vec![fill(words::SEARCH_TASK, &[("tools", tools)]).trim().to_string()];
  for name in &search.rulebooks {
    parts.push(if search.on_request.contains(name) {
      rulebook_sections(name, std::slice::from_ref(&search.applies_section))?
    } else {
      rulebook_whole(name)?
    });
  }
  parts.push(rulebook_whole(&search.plain_writing)?);
  parts.push(rulebook_whole(&search.self_preservation)?);
  Ok(parts.join("\n\n"))
}

/// A branch's instructions, with the same listing the search is shown.
pub fn branch_instructions(tools: &str) -> Result<String, String> {
  Ok(
    [
      fill(words::SEARCH_BRANCH, &[("tools", tools)]).trim().to_string(),
      rulebook_whole(&words::runner().search.self_preservation)?,
    ]
    .join("\n\n"),
  )
}

/// A browse's instructions.
pub fn browse_instructions() -> Result<String, String> {
  Ok(
    [
      words::BROWSE_TASK.trim().to_string(),
      rulebook_whole(&words::runner().search.self_preservation)?,
    ]
    .join("\n\n"),
  )
}

/// The budget a run is told as its ceiling: the pages its first answer may fetch and each branch's, or
/// nothing where the key names none.
pub fn budget_line(kept: KeyDefaults) -> String {
  let Some(pages) = kept.pages else { return String::new() };
  let branches = match (kept.branches, kept.branch_pages) {
    (0, _) => String::new(),
    (count, Some(each)) => format!(", and each of the {count} branches it may open at most {each}"),
    (count, None) => format!(", beside the {count} branches it may open"),
  };
  format!(
    "Its budget, a hard ceiling: the first answer fetches at most {pages} pages{branches}. Once it is spent, \
     fetch nothing more and write with what you have; what stays open goes under UNCONFIRMED."
  )
}

/// The request a run is put to, its line's template filled with the ask.
pub fn request(ask: &ResearchRunAsk, kept: KeyDefaults) -> String {
  if line_of(&ask.key) == "browse" {
    let synthesis = (kept.minutes / 5).max(1);
    let research = (kept.minutes - synthesis).max(1);
    return fill(
      words::BROWSE_REQUEST,
      &[
        ("task", ask.query.trim()),
        ("budget", &format!("{} minutes", kept.minutes)),
        ("research", &format!("{research} minutes")),
        ("synthesis", &format!("{synthesis} minutes")),
      ],
    )
    .trim()
    .to_string();
  }
  let branches = match kept.branches {
    0 => "no pooled branches".to_string(),
    1 => "one pooled branch".to_string(),
    many => format!("up to {many} pooled branches"),
  };
  let asked_at = chrono::Utc::now().to_rfc3339();
  fill(
    words::SEARCH_REQUEST,
    &[
      ("query", ask.query.trim()),
      ("asker", said(&ask.asker).unwrap_or("the owner")),
      ("askedAt", &asked_at),
      ("options", said(&ask.options).unwrap_or("none")),
      ("depth", depth_of(&ask.key)),
      ("branches", &branches),
      ("budget", &budget_line(kept)),
    ],
  )
  .trim()
  .to_string()
}

// The records its sessions carry.

/// What a run's record is made of.
pub struct Recorded<'a> {
  pub run: i64,
  pub ask: &'a ResearchRunAsk,
  pub held: &'a Held,
  pub kept: KeyDefaults,
  pub closes_at: i64,
  pub provider: &'a str,
  pub model: &'a str,
  pub rulebooks: &'a [String],
}

/// The run's record, which its first session carries under `labels.research` (`docs/runner.md`).
pub fn record(app: &App, made: &Recorded<'_>) -> Value {
  let mut record = json!({
    "closesAt": made.closes_at,
    "run": made.run,
    "source": line_of(&made.ask.key),
    "role": made.held.branch_role,
    "branches": made.kept.branches,
    "chat": made.ask.room_id.unwrap_or(0),
    "strategy": made.held.strategy,
    "rulebooks": made.rulebooks,
    "provider": made.provider,
    "model": made.model,
  });
  let mut set = |key: &str, value: Option<Value>| {
    if let Some(value) = value {
      record[key] = value;
    }
  };
  set("owner", said(&made.ask.asker).map(|asker| json!(asker)));
  set("pages", made.kept.pages.map(|pages| json!(pages)));
  set("branchPages", made.kept.branch_pages.map(|pages| json!(pages)));
  set("hostIntervalMs", made.held.host_interval_ms.map(|ms| json!(ms)));
  set("engineIntervalMs", made.held.engine_interval_ms.map(|ms| json!(ms)));
  if made.kept.branches > 0 {
    set("branchesAt", Some(json!(format!("http://{}", app.settings.listen))));
    set("branchesToken", Some(json!(app.token)));
  }
  record
}

/// A round's record: the run's, opening no branch.
pub fn round_record(record: &Value) -> Value {
  let mut round = record.clone();
  if let Some(fields) = round.as_object_mut() {
    fields.insert("branches".into(), json!(0));
    fields.remove("branchesAt");
    fields.remove("branchesToken");
  }
  round
}

/// A branch's record: the run's, held to a branch's budget, opening no branch, naming the run it is one of.
pub fn branch_record(record: &Value, run: i64) -> Value {
  let mut branch = round_record(record);
  if let Some(fields) = branch.as_object_mut() {
    match fields.remove("branchPages") {
      Some(pages) => fields.insert("pages".into(), pages),
      None => fields.remove("pages"),
    };
    fields.insert("branchOf".into(), json!(run));
  }
  branch
}

/// The capabilities a session's record keeps: the settings', less the branch tool where it opens no branch
/// and the rulebook reader where it is handed no rulebook.
pub fn capabilities(app: &App, branches: bool, rulebooks: bool) -> Vec<String> {
  app
    .settings
    .runner
    .capabilities
    .iter()
    .filter(|c| (branches || c.as_str() != BRANCHES) && (rulebooks || c.as_str() != RULEBOOKS))
    .cloned()
    .collect()
}

// Agency's seats and sessions.

/// A seat request as agency answers it: its token and state, and the road its work key resolved to.
#[derive(Clone, Debug, Default)]
pub struct Seat {
  pub token: String,
  pub state: String,
  pub provider: String,
  pub model: String,
  pub effort: Option<String>,
  pub tier: Option<String>,
}

// The seat states a request is admitted in, and those it no longer waits in.
const ADMITTED: [&str; 3] = ["active", "admitted", "bound"];
const ENDED: [&str; 3] = ["released", "cancelled", "finished"];

impl Seat {
  pub fn of(said: &Value) -> Option<Seat> {
    let text = |field: &str| said[field].as_str().filter(|value| !value.is_empty());
    Some(Seat {
      token: text("token")?.to_owned(),
      state: text("state").unwrap_or_default().to_owned(),
      provider: text("provider").unwrap_or_default().to_owned(),
      model: text("model").unwrap_or_default().to_owned(),
      effort: text("effort").map(str::to_owned),
      tier: text("tier").map(str::to_owned),
    })
  }

  /// The seat an admission answered, where it let one in: the request under `request`, null while it waits,
  /// or the request itself.
  pub fn admission(said: &Value) -> Option<Seat> {
    match said.get("request") {
      Some(request) => Seat::of(request),
      None => Seat::of(said),
    }
  }

  pub fn admitted(&self) -> bool {
    ADMITTED.contains(&self.state.as_str())
  }

  pub fn ended(&self) -> bool {
    ENDED.contains(&self.state.as_str())
  }
}

/// A seat given back with what its work came to; cancelled where it never ran.
pub async fn give_back(app: &App, run: i64, token: &str, outcome: &str, cancel: bool) {
  let outcome: String = outcome.chars().take(300).collect();
  let mut change = json!({ "state": "finished", "outcome": outcome });
  if cancel {
    change["cancel"] = json!(true);
  }
  if let Err(why) = app.peers.move_seat(token, &change).await {
    eprintln!("bmo-research: research {run}: a seat was not given back: {why}");
  }
  let _ = app.store.let_go(run, "seat", token);
}

/// A session ended once its turn is over: an end answered busy asked again until `until` (epoch
/// milliseconds) has passed.
pub async fn end_session(app: &App, run: i64, session: &str, until: i64) {
  loop {
    match app.peers.end_session(session).await {
      Ok(()) => break,
      Err(Ending::Busy) if now_ms() < until => tokio::time::sleep(END_EVERY).await,
      Err(Ending::Busy) => {
        eprintln!("bmo-research: research {run}: session {session} was still busy at its bound");
        break;
      }
      Err(Ending::Failed(why)) => {
        eprintln!("bmo-research: research {run}: session {session} was not ended: {why}");
        break;
      }
    }
  }
  let _ = app.store.let_go(run, "session", session);
}

/// What a session is started with.
pub struct Start<'a> {
  pub line: &'a str,
  pub provider: &'a str,
  pub model: &'a str,
  pub system: &'a str,
  pub record: &'a Value,
  pub capabilities: &'a [String],
  pub bound_ms: i64,
}

/// A thrown-away session started on agency's line, its id kept as the run's hold.
pub async fn start_session(app: &App, run: i64, start: &Start<'_>) -> Result<String, String> {
  let body = json!({
    "agent": start.line,
    "scope": "apart",
    "provider": start.provider,
    "model": start.model,
    "cwd": "",
    "line": start.line,
    "systemPrompt": start.system,
    "labels": { "research": start.record },
    "toolsAllowed": start.capabilities,
    "boundMs": start.bound_ms,
  });
  let session = app.peers.start_session(&body).await?;
  let _ = app.store.hold(run, "session", &session);
  Ok(session)
}

/// What a turn is sent with beside its prompt.
pub struct Turn<'a> {
  pub provider: &'a str,
  pub model: &'a str,
  pub seat: &'a Seat,
  pub name: &'a str,
  pub title: &'a str,
  pub chat: i64,
  pub work_key: &'a str,
}

/// A turn's body: its prompt and its work parts, laid on the seat's mapping. The work key and whether it
/// hears words go under the camelCase names agency's contract gives them.
pub fn turn_body(prompt: &str, turn: &Turn<'_>) -> Value {
  json!({
    "prompt": prompt,
    "provider": turn.provider,
    "model": turn.model,
    "mapping": { "model": turn.model, "effort": turn.seat.effort, "tier": turn.seat.tier },
    "resume": { "kind": "fresh" },
    "cwd": "",
    "name": turn.name,
    "title": turn.title,
    "chat": turn.chat,
    "workKey": turn.work_key,
    "hearsWords": false,
    "variables": {},
  })
}

/// What a turn wrote: its answer, or its text blocks where it names no answer.
pub fn written(turn: &Value) -> String {
  match turn["answer"].as_str().filter(|answer| !answer.trim().is_empty()) {
    Some(answer) => answer.to_owned(),
    None => turn["text"]
      .as_array()
      .map(|blocks| blocks.iter().filter_map(Value::as_str).collect::<Vec<_>>().join("\n"))
      .unwrap_or_default(),
  }
}

// The run.

/// Why a research is not sent out: the ask does not read, nothing can carry it now, or the runner failed.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Refused {
  Ask(String),
  Unavailable(String),
  Failed(String),
}

/// Why an ask is refused before anything is looked up.
pub fn ask_fault(ask: &ResearchRunAsk) -> Option<String> {
  if !KEYS.contains(&ask.key.as_str()) {
    return Some(format!("{} is no research key: {}", ask.key, KEYS.join(", ")));
  }
  ask.query.trim().is_empty().then(|| "a research asks a question".to_string())
}

fn note(app: &App, run: i64, kind: &str, text: &str) {
  if let Err(why) = app.store.log_research(run, kind, text) {
    eprintln!("bmo-research: research {run}'s log: {why}");
  }
}

/// Who a run is asked for, which decides how its result is filed and told: a room, told its end with a link
/// to a document of the result; or the daemon's deep dive, whose result is held to its shape and filed as a
/// page the daemon finishes (`docs/runner.md`, 「The deep dive's page」).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Filing {
  Room,
  Page(PageAsk),
}

/// What a deep dive asks of its run's page: the section headings its shape holds, in every language a result
/// may be written in, and the title it falls back on.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PageAsk {
  pub sections: Vec<String>,
  pub title: String,
}

/// A run under way: what it was composed of and what it holds.
struct Going {
  run: i64,
  ask: ResearchRunAsk,
  filing: Filing,
  line: &'static str,
  seat: Seat,
  session: String,
  system: String,
  prompt: String,
  record: Value,
  capabilities: Vec<String>,
  provider: String,
  model: String,
  closes_at: i64,
  bound_ms: i64,
}

/// One research sent out (`docs/runner.md`, 「One run, step by step」): composed, seated, its session started,
/// and the rest carried on in the background; answered at once with what it went out on. A run asked for a
/// room queues its end for it; a deep dive's tells nothing.
pub async fn launch(app: Arc<App>, ask: ResearchRunAsk, filing: Filing) -> Result<ResearchRun, Refused> {
  let tells = filing == Filing::Room;
  if let Some(why) = ask_fault(&ask) {
    return Err(Refused::Ask(why));
  }
  let line = line_of(&ask.key);
  let (held, _) = held(&app).map_err(Refused::Failed)?;
  let kept = held.key(&ask.key);
  let (system, rulebooks) = if line == "search" {
    let tools = ToolHost::new(&app.settings)
      .listing("web")
      .await
      .map_err(|why| Refused::Unavailable(format!("the search's instructions are not composed: {why}")))?;
    (
      search_instructions(&tools).map_err(Refused::Failed)?,
      words::runner().search.on_request.clone(),
    )
  } else {
    (browse_instructions().map_err(Refused::Failed)?, Vec::new())
  };
  let prompt = request(&ask, kept);
  let closes_at = now_ms() + kept.minutes * 60_000;
  let bound_ms = (kept.minutes + app.settings.runner.writing_margin_minutes.max(0)) * 60_000;
  let run = app
    .store
    .open_research_run(&NewResearchRun {
      key: &ask.key,
      line,
      query: ask.query.trim(),
      asker: said(&ask.asker),
      options: said(&ask.options),
      room_id: ask.room_id.filter(|room| *room != 0),
      provider: said(&ask.provider).unwrap_or_default(),
      model: said(&ask.model).unwrap_or_default(),
      closes_at,
      branches_allowed: kept.branches,
      tells,
    })
    .map_err(Refused::Failed)?;
  note(&app, run, "asked", &format!("Research on {}: {}", ask.key, ask.query.trim()));
  let refuse = |why: String| {
    note(&app, run, "ended", &format!("The research was refused: {why}"));
    let _ = app.store.close_research_run(run, "failed", Some(&why), None);
    Refused::Unavailable(why)
  };
  let mut asked = json!({
    "requestKey": format!("research:{run}"),
    "owner": format!("research:{run}"),
    "role": app.settings.runner.role,
    "selection": "auto",
    "workKey": ask.key,
    "source": line,
  });
  if let Some(provider) = said(&ask.provider) {
    asked["provider"] = json!(provider);
  }
  let seat = match app.peers.ask_seat(&asked).await.map(|said| Seat::of(&said)) {
    Ok(Some(seat)) => seat,
    Ok(None) => return Err(refuse("agency answered the run's seat with no token".into())),
    Err(why) => return Err(refuse(format!("agency gave the run no seat: {why}"))),
  };
  let _ = app.store.hold(run, "seat", &seat.token);
  let provider = said(&ask.provider).unwrap_or(&seat.provider).to_string();
  let model = said(&ask.model).unwrap_or(&seat.model).to_string();
  if model.is_empty() {
    give_back(&app, run, &seat.token, "cancelled: no model", true).await;
    return Err(refuse("the run's seat was given no model".into()));
  }
  let record = record(
    &app,
    &Recorded {
      run,
      ask: &ask,
      held: &held,
      kept,
      closes_at,
      provider: &provider,
      model: &model,
      rulebooks: &rulebooks,
    },
  );
  let _ = app.store.keep_research_road(run, &provider, &model, &record);
  let capabilities = capabilities(&app, kept.branches > 0, !rulebooks.is_empty());
  let started = start_session(
    &app,
    run,
    &Start {
      line,
      provider: &provider,
      model: &model,
      system: &system,
      record: &record,
      capabilities: &capabilities,
      bound_ms,
    },
  )
  .await;
  let session = match started {
    Ok(session) => session,
    Err(why) => {
      give_back(&app, run, &seat.token, "cancelled: no session", true).await;
      return Err(refuse(format!("agency started no session for the run: {why}")));
    }
  };
  let state = if seat.admitted() { ResearchRunState::Running } else { ResearchRunState::Waiting };
  let launched = ResearchRun {
    run_id: run,
    key: ask.key.clone(),
    state,
    provider: provider.clone(),
    model: model.clone(),
    closes_at: Some(closes_at),
  };
  let going = Going {
    run,
    ask,
    filing,
    line,
    seat,
    session,
    system,
    prompt,
    record,
    capabilities,
    provider,
    model,
    closes_at,
    bound_ms,
  };
  tokio::spawn(go(app, going));
  Ok(launched)
}

/// The run to its end: admitted, its first answer, its rounds, its seat given back, its document filed, closed
/// in the ledger and told to its room.
async fn go(app: Arc<App>, going: Going) {
  let run = going.run;
  let until = now_ms() + going.bound_ms + SEND_SLACK.as_millis() as i64;
  let seated = admitted(&app, &going).await;
  // A seat that never ran is cancelled rather than finished.
  let never_ran = seated.is_err();
  let result = match seated {
    Ok(seat) => {
      let first = first_answer(&app, &going, &seat).await;
      let rounds_follow = words::runner().rounds.contains_key(&going.ask.key);
      let first = match first {
        Ok(answer) if !rounds_follow => Ok(narrated(&app, &going, &going.session, &seat, answer).await),
        other => other,
      };
      end_session(&app, run, &going.session, until).await;
      match first {
        Ok(answer) if rounds_follow => Ok(rounds(&app, &going, &seat, answer).await),
        other => other,
      }
    }
    Err(why) => {
      end_session(&app, run, &going.session, until).await;
      Err(why)
    }
  };
  let outcome = match &result {
    Ok(_) => "answered".to_string(),
    Err(why) => format!("failed: {why}"),
  };
  give_back(&app, run, &going.seat.token, &outcome, never_ran).await;
  ended(&app, &going, result).await;
}

/// The seat let in and bound to the run's session, asked for its place every half second until the run's
/// clock runs out; or why it was not.
async fn admitted(app: &App, going: &Going) -> Result<Seat, String> {
  let run = going.run;
  let mut seat = going.seat.clone();
  loop {
    if seat.admitted() {
      break;
    }
    if seat.ended() {
      return Err("the pool request was ended before it got a place".into());
    }
    if now_ms() >= going.closes_at {
      return Err("the common co-worker pool stayed full until research closed".into());
    }
    tokio::time::sleep(LOOK_EVERY).await;
    match app.peers.move_seat(&seat.token, &json!({ "state": "admitted" })).await {
      Ok(said) => {
        if let Some(now) = Seat::admission(&said) {
          seat = now;
        }
      }
      Err(why) => return Err(format!("the run's seat could not be let in: {why}")),
    }
  }
  let bound = app
    .peers
    .move_seat(&seat.token, &json!({ "state": "bound", "childId": going.session }))
    .await;
  if let Err(why) = bound {
    return Err(format!("the run's seat could not be bound to its session: {why}"));
  }
  let _ = app.store.research_running(run);
  note(app, run, "seated", &format!("Seated on {} · {}.", going.provider, going.model));
  Ok(seat)
}

/// One turn into a session, answered once it ends: what it wrote, or why it wrote nothing.
async fn turn(
  app: &App,
  going: &Going,
  session: &str,
  seat: &Seat,
  name: &str,
  prompt: &str,
) -> Result<String, String> {
  let title = format!("{} research {}", going.line, going.run);
  let body = turn_body(
    prompt,
    &Turn {
      provider: &going.provider,
      model: &going.model,
      seat,
      name,
      title: &title,
      chat: going.ask.room_id.unwrap_or(0),
      work_key: &going.ask.key,
    },
  );
  let within = Duration::from_millis(going.bound_ms.max(0) as u64) + SEND_SLACK;
  let answered = app.peers.send_turn(session, &body, within).await?;
  let text = written(&answered);
  let _ = app.store.keep_research_turn(
    going.run,
    name,
    Some(&text).filter(|t| !t.trim().is_empty()).map(String::as_str),
  );
  if text.trim().is_empty() {
    let outcome = answered["outcome"].as_str().unwrap_or("no outcome");
    return Err(format!("the turn {name} wrote nothing ({outcome})"));
  }
  Ok(text)
}

async fn first_answer(app: &App, going: &Going, seat: &Seat) -> Result<String, String> {
  let name = format!("research-{}", going.run);
  let answer = turn(app, going, &going.session, seat, &name, &going.prompt).await;
  match &answer {
    Ok(_) => note(app, going.run, "answer", "The first answer came back."),
    Err(why) => note(app, going.run, "answer", &format!("The first answer did not come: {why}")),
  }
  answer
}

/// The prompt one round is asked with: the question, the task as the run was given it, and the answer the
/// run wrote last.
pub fn round_prompt(question: &str, task: &str, last: &str) -> String {
  format!("{question}\n\n## The task\n\n{task}\n\n## What you wrote last\n\n{last}")
}

/// The key's rounds after the first answer, in a session of their own, past the ceiling the final one asked
/// at once or the last answer kept; a round that does not answer leaves the last answer standing. A deep
/// dive's narration is asked of the same session before it is ended. Answers the run's result.
async fn rounds(app: &App, going: &Going, seat: &Seat, first: String) -> String {
  let run = going.run;
  let runner = words::runner();
  let Some(process) = runner.rounds.get(&going.ask.key) else {
    return first;
  };
  let record = round_record(&going.record);
  let capabilities: Vec<String> = going
    .capabilities
    .iter()
    .filter(|c| c.as_str() != BRANCHES && c.as_str() != RULEBOOKS)
    .cloned()
    .collect();
  let start = Start {
    line: going.line,
    provider: &going.provider,
    model: &going.model,
    system: &going.system,
    record: &record,
    capabilities: &capabilities,
    bound_ms: going.bound_ms,
  };
  let session = match start_session(app, run, &start).await {
    Ok(session) => session,
    Err(why) => {
      note(
        app,
        run,
        "round",
        &format!("The rounds' session did not start, so the first answer stands: {why}"),
      );
      return first;
    }
  };
  let began = now_ms();
  let mut last = first;
  let mut asking = 0;
  while asking < process.rounds.len() {
    let name = process.rounds[asking].as_str();
    let past = (now_ms() - began) / 1000 >= process.ceiling_seconds;
    if past && name != FINAL {
      match (process.past_ceiling, process.rounds.iter().position(|one| one == FINAL)) {
        (PastCeiling::Final, Some(at)) if at >= asking => {
          asking = at;
          continue;
        }
        _ => break,
      }
    }
    let Some(question) = runner.questions.get(name) else {
      asking += 1;
      continue;
    };
    let prompt = round_prompt(question, &going.prompt, &last);
    match turn(app, going, &session, seat, &format!("research-{run}-{name}"), &prompt).await {
      Ok(answer) => {
        note(app, run, "round", &format!("The round {name} answered."));
        last = answer;
      }
      Err(why) => {
        note(app, run, "round", &format!("The round {name} did not answer: {why}"));
        break;
      }
    }
    if name == FINAL {
      break;
    }
    asking += 1;
  }
  let last = narrated(app, going, &session, seat, last).await;
  end_session(app, run, &session, now_ms() + going.bound_ms + SEND_SLACK.as_millis() as i64).await;
  last
}

/// The prompt a deep dive's missing narration is asked with: the narration question and the rulebook's
/// section on the narration, the task as the run was given it, and the answer the run wrote last.
pub fn narration_prompt(task: &str, last: &str) -> Result<String, String> {
  let ask = &words::runner().narration;
  let rule = rulebook_sections(&ask.rulebook, std::slice::from_ref(&ask.section))?;
  Ok(round_prompt(&format!("{}\n\n{rule}", ask.question), task, last))
}

/// A narration turn's text as the script: a `LISTEN:` line it wrote itself dropped, its edges trimmed.
pub fn narration_script(text: &str) -> String {
  let lines: Vec<&str> = text.lines().filter(|line| line.trim() != "LISTEN:").collect();
  lines.join("\n").trim().to_owned()
}

/// A deep dive's result with its narration (`docs/runner.md`, 「The narration」): where it carries no `LISTEN:`
/// script, `session` is asked one more turn for the narration alone, and its text set after the result as
/// the script and kept as what the run wrote. A run told to a room, a result that has its script, or a turn
/// that writes none leaves the result as it stands.
async fn narrated(app: &App, going: &Going, session: &str, seat: &Seat, result: String) -> String {
  let run = going.run;
  if going.filing == Filing::Room {
    return result;
  }
  let drawn = document::draw(&result, words::search_document(document::language_of(&going.ask.query)));
  if drawn.narration.is_some() {
    return result;
  }
  let prompt = match narration_prompt(&going.prompt, &result) {
    Ok(prompt) => prompt,
    Err(why) => {
      note(app, run, "narration", &format!("The narration was not asked: {why}"));
      return result;
    }
  };
  let name = format!("research-{run}-narration");
  let script = match turn(app, going, session, seat, &name, &prompt).await {
    Ok(text) => narration_script(&text),
    Err(why) => {
      note(app, run, "narration", &format!("The narration did not come: {why}"));
      return result;
    }
  };
  let whole = if script.is_empty() {
    note(app, run, "narration", "The narration turn wrote no script.");
    result
  } else {
    note(app, run, "narration", "The narration was asked apart and came back.");
    format!("{}\n\nLISTEN:\n{script}\n", result.trim_end())
  };
  if let Err(why) = app.store.keep_research_written(run, &whole) {
    eprintln!("bmo-research: research {run}'s result was not kept: {why}");
  }
  whole
}

/// A deep dive's result held to its shape and filed as its page: the outcome, the error and the document.
async fn file_page(
  app: &App,
  going: &Going,
  page: &PageAsk,
  text: &str,
) -> (&'static str, Option<String>, Option<RunDocument>) {
  let (run, ask) = (going.run, &going.ask);
  let drawn = document::draw(text, words::search_document(document::language_of(&ask.query)));
  if let Some(short) = document::short_of(&drawn, &page.sections) {
    let why = format!("the result was not filed: {short}");
    note(app, run, "filed", &format!("The result was not filed, since {short}."));
    return ("failed", Some(why), None);
  }
  let title = document::page_title(&drawn, &page.title);
  let filing = document::page_filing(&drawn, run, &ask.key, &ask.query, &title);
  match document::file_page(app, &drawn, &filing).await {
    Ok(filed) => {
      note(app, run, "filed", &format!("The result was filed as page {}: {}", filed.id, filed.link));
      ("answered", None, Some(filed))
    }
    Err(why) => {
      note(app, run, "filed", &format!("The result was not filed: {why}"));
      ("failed", Some(format!("the result was not filed: {why}")), None)
    }
  }
}

/// The run's end: its result filed, the run closed in the ledger, and its room told.
async fn ended(app: &App, going: &Going, result: Result<String, String>) {
  let run = going.run;
  let ask = &going.ask;
  if let (Filing::Page(page), Ok(text)) = (&going.filing, &result) {
    let (outcome, error, filed) = file_page(app, going, page, text).await;
    match app.store.close_research_run(run, outcome, error.as_deref(), filed.as_ref()) {
      Ok(true) => note(app, run, "ended", &format!("The research ended {outcome}.")),
      Ok(false) => {}
      Err(why) => eprintln!("bmo-research: research {run} was not closed: {why}"),
    }
    return;
  }
  let (told, outcome, error, filed) = match &result {
    Ok(text) => {
      let drawn = document::draw(text, words::search_document(document::language_of(&ask.query)));
      let filing = document::filing(&drawn, run, &ask.key, &ask.query, ask.room_id);
      match document::file(app, &drawn, &filing).await {
        Ok(filed) => {
          note(
            app,
            run,
            "filed",
            &format!("The result was filed as document {}: {}", filed.id, filed.link),
          );
          let brief = brief_of(text).unwrap_or_else(|| text.clone());
          (linked_instruction(run, ask, &brief, &filed.link), "answered", None, Some(filed))
        }
        Err(why) => {
          note(
            app,
            run,
            "filed",
            &format!("The result was not filed, so it goes to the room whole: {why}"),
          );
          (whole_instruction(run, ask, &going.model, &result), "answered", None, None)
        }
      }
    }
    Err(why) => (whole_instruction(run, ask, &going.model, &result), "failed", Some(why.clone()), None),
  };
  match app.store.close_research_run(run, outcome, error.as_deref(), filed.as_ref()) {
    Ok(true) => note(app, run, "ended", &format!("The research ended {outcome}.")),
    Ok(false) => return,
    Err(why) => eprintln!("bmo-research: research {run} was not closed: {why}"),
  }
  if app.store.research_run(run).map(|row| row.tells).unwrap_or(true) {
    tell(app, run, &told).await;
  }
}

async fn tell(app: &App, run: i64, told: &Value) {
  match app.peers.instruct_end(told).await {
    Ok(()) => note(app, run, "told", "The room was told."),
    Err(why) => note(app, run, "told", &format!("The room was not told: {why}")),
  }
}

// What the room is told.

/// The structure lines a result's blocks open with, which end the block before them.
const BLOCK_OPENERS: [&str; 6] =
  ["VERDICT:", "SUMMARY:", "SECTION:", "UNCONFIRMED:", "===ITEM===", "SOURCE "];

// The value of a head line, such as `TITLE: <it>`, where the result writes one.
fn head_line<'a>(result: &'a str, name: &str) -> Option<&'a str> {
  result
    .lines()
    .find_map(|line| line.trim().strip_prefix(name).map(str::trim))
    .filter(|value| !value.is_empty())
}

// The block a structure line opens, to the next one, where the result writes it.
fn block(result: &str, opener: &str) -> Option<String> {
  let mut lines = result.lines().skip_while(|line| line.trim() != opener);
  lines.next()?;
  let text: Vec<&str> = lines
    .take_while(|line| !BLOCK_OPENERS.iter().any(|next| line.trim_start().starts_with(next)))
    .collect();
  Some(text.join("\n").trim().to_owned()).filter(|text| !text.is_empty())
}

/// The answer in brief a result gives the room: its verdict, else its summary, else its title.
pub fn brief_of(result: &str) -> Option<String> {
  block(result, "VERDICT:")
    .or_else(|| block(result, "SUMMARY:"))
    .or_else(|| head_line(result, "TITLE:").map(str::to_owned))
}

/// An instruction for the room, its words said: as a notice of the run's end, for the room the run names, or
/// for none, which the receiver hands the owner's private room.
fn instruction(run: i64, room: Option<i64>, said: String, summary: String) -> Value {
  json!({
    "room": room.filter(|room| *room != 0),
    "said": { "order": said },
    "sort": "notice",
    "kind": format!("research-ended:{run}"),
    "summary": summary,
    "events": [format!("research-ended:{run}")],
  })
}

/// An answered run whose document was filed: the answer in brief and the one link, with the shape the chat
/// agent answers in.
pub fn linked_instruction(run: i64, ask: &ResearchRunAsk, brief: &str, link: &str) -> Value {
  let said = format!(
    "The research on {} answered \"{}\". The answer in brief:\n\n{brief}\n\nIts document, with the grounds and \
     the sources: {link}\n\nTell the owner the answer in one or two short lines, in his language and your own \
     voice, saying what was found and never how, then this one link on a line of its own. Put no grounds, \
     sources or document text in the chat: they stand in the document.",
    ask.key,
    ask.query.trim()
  );
  instruction(run, ask.room_id, said, format!("The research {run} ended"))
}

/// A run whose result could not be filed, told whole, or a run that ended with no answer.
pub fn whole_instruction(
  run: i64,
  ask: &ResearchRunAsk,
  model: &str,
  result: &Result<String, String>,
) -> Value {
  let said = match result {
    Ok(text) => format!(
      "The research on {} ({model}) answered: \"{}\".\n\nIts result, as the run wrote it:\n\n{text}\n\nTell the \
       owner this result in his language, whole, keeping every figure, source and address as it stands.",
      ask.key,
      ask.query.trim()
    ),
    Err(why) => format!(
      "The research on {} (\"{}\") ended with no answer: {why}\n\nTell the owner what it could not do, and what \
       was tried.",
      ask.key,
      ask.query.trim()
    ),
  };
  instruction(run, ask.room_id, said, format!("The research {run} ended"))
}

/// A cut run: what it had written, handed back as unfinished, or one line saying the work was cut off.
pub fn cut_instruction(row: &ResearchRunRow) -> Value {
  let asked = Some(row.query.trim())
    .filter(|q| !q.is_empty())
    .map(|q| format!(" (\"{q}\")"))
    .unwrap_or_default();
  let said = match row.written.as_deref().map(str::trim).filter(|w| !w.is_empty()) {
    Some(text) => format!(
      "The research on {}{asked} was cut off before it finished. What it had written:\n\n{text}\n\nHand the owner \
       what it had written, in his language, saying it is unfinished.",
      row.key
    ),
    None => format!(
      "The research on {}{asked} was cut off before it wrote anything.\n\nTell the owner in one line that the work \
       was cut off.",
      row.key
    ),
  };
  instruction(row.id, row.room_id, said, format!("The research {} was cut", row.id))
}

/// A daemon's start (`docs/runner.md`, 「A restart」): every run its store holds open closed as cut, what it
/// held of agency given back, and its room told. Answers how many were cut.
pub async fn sweep(app: &Arc<App>) -> usize {
  let open = match app.store.open_research_runs() {
    Ok(open) => open,
    Err(why) => {
      eprintln!("bmo-research: the open research runs could not be read: {why}");
      return 0;
    }
  };
  for row in &open {
    let run = row.id;
    // A turn under way runs on in agency until its session's bound, which is the run's clock and the writing
    // margin: its session is ended once it is over.
    let until = row.closes_at.unwrap_or_else(now_ms)
      + (app.settings.runner.writing_margin_minutes.max(0) + 1) * 60_000
      + SEND_SLACK.as_millis() as i64;
    for (kind, value) in app.store.holds(run).unwrap_or_default() {
      match kind.as_str() {
        "session" => {
          let app = app.clone();
          tokio::spawn(async move { end_session(&app, run, &value, until).await });
        }
        _ => give_back(app, run, &value, "stopped: the research unit restarted", false).await,
      }
    }
    let why = "the research unit restarted while the run was under way";
    if app.store.close_research_run(run, "cut", Some(why), None).unwrap_or(false) {
      note(app, run, "ended", "The research was cut: the research unit restarted.");
      if row.tells {
        tell(app, run, &cut_instruction(row)).await;
      }
    }
  }
  open.len()
}
