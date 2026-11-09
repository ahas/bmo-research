//! The research runner (`docs/runner.md`): a search at a depth or a browse, composed here — its line's
//! instructions with the web namespace's listing and the rulebooks, its request, the record its sessions
//! carry — sent out on a slot of its line's role in a session of that role agency starts, asked its rounds
//! in a second session, its result filed as a document and told to its room, every step kept in the run's
//! ledger and log. The research settings it is held to are kept here too.

use std::collections::BTreeMap;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use bmo_research_client::{
  ResearchKeySettings, ResearchRun, ResearchRunAsk, ResearchRunState, ResearchSettings,
  ResearchSettingsChange, SearchEngineState, Shape,
};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

use crate::App;
use crate::config::KeyDefaults;
use crate::dive::{self, Speech};
use crate::document;
use crate::peers::{Ending, Fault, Peers};
use crate::references::{self, ReferenceAsk};
use crate::store::{NewResearchRun, ResearchRunRow, RunDocument, Store, now_ms};
use crate::tool_host::ToolHost;
use crate::words::{self, FINAL, fill};

/// The research keys, in the order the settings answer them.
pub const KEYS: [&str; 5] =
  ["search:basic", "search:thorough", "search:exhaustive", "browse", "references"];

/// How the web tools may read a page.
pub const STRATEGIES: [&str; 3] = ["direct-then-browser", "browser-first", "direct-only"];

/// The capability that shows a run the branch tool.
const BRANCHES: &str = "web.branches";

/// The capability that shows a run the rulebook reader.
const RULEBOOKS: &str = "web.rulebooks";

/// How often a slot still waiting is asked for its place.
pub const LOOK_EVERY: Duration = Duration::from_millis(500);

/// How often an end answered busy is asked again.
const END_EVERY: Duration = Duration::from_secs(2);

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

// An ask's options as the store and the prompts hold them: a string as it was written, another value as its
// JSON.
fn options_text(options: &Option<Value>) -> Option<String> {
  match options {
    Some(Value::String(text)) => Some(text.trim().to_owned()).filter(|text| !text.is_empty()),
    Some(value) => Some(value.to_string()),
    None => None,
  }
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
  /// A key's branches and budget; none for a key the settings name nothing for.
  pub fn key(&self, key: &str) -> KeyDefaults {
    self.keys.get(key).copied().unwrap_or(KeyDefaults {
      branches: 0,
      pages: None,
      branch_pages: None,
    })
  }
}

/// The settings file's defaults.
pub fn defaults(app: &App) -> Held {
  let declared = &app.settings.research_settings;
  // A settings file naming `keys` answers only the keys it names, so a key it leaves out keeps the
  // settings' own default rather than falling out of the settings the boundary answers.
  let standard = crate::config::ResearchDefaults::default().keys;
  Held {
    branch_role: declared.branch_role.clone(),
    strategy: declared.strategy.clone(),
    host_interval_ms: declared.host_interval_ms,
    engine_interval_ms: declared.engine_interval_ms,
    keys: KEYS
      .iter()
      .filter_map(|key| {
        declared
          .keys
          .get(*key)
          .or_else(|| standard.get(*key))
          .map(|kept| ((*key).to_string(), *kept))
      })
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
/// below nothing, a key that is no research key, branches below nothing, a budget of no page, or a role
/// agency's store does not hold.
pub fn changed(mut held: Held, change: &ResearchSettingsChange, roles: &[String]) -> Result<Held, String> {
  if let Some(strategy) = &change.strategy {
    if !STRATEGIES.contains(&strategy.as_str()) {
      return Err(format!("{strategy} is no fetch strategy: {}", STRATEGIES.join(", ")));
    }
    held.strategy.clone_from(strategy);
  }
  if let Some(role) = &change.branch_role {
    if !roles.contains(role) {
      return Err(format!("{role} is no role agency holds: {}", roles.join(", ")));
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
    if key.branches < 0 {
      return Err(format!("{}: branches are never below nothing", key.key));
    }
    if key.pages.is_some_and(|pages| pages < 1) || key.branch_pages.is_some_and(|pages| pages < 1) {
      return Err(format!("{}: a budget reads one page at least", key.key));
    }
    *kept = KeyDefaults {
      branches: key.branches,
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

/// A references run's instructions: the references task with the web namespace's listing, the references
/// rulebook whole, then plain writing and self-preservation. It is handed that one rulebook, and no search
/// rulebook binds it.
pub fn references_instructions(tools: &str) -> Result<String, String> {
  Ok(
    [
      fill(words::REFERENCE_TASK, &[("tools", tools)]).trim().to_string(),
      rulebook_whole("references")?,
      rulebook_whole(&words::runner().search.plain_writing)?,
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
    return fill(words::BROWSE_REQUEST, &[("task", ask.query.trim())]).trim().to_string();
  }
  let branches = match kept.branches {
    0 => "no branches".to_string(),
    1 => "one branch".to_string(),
    many => format!("up to {many} branches"),
  };
  let asked_at = chrono::Utc::now().to_rfc3339();
  let options = options_text(&ask.options).unwrap_or_else(|| "none".to_owned());
  fill(
    words::SEARCH_REQUEST,
    &[
      ("query", ask.query.trim()),
      ("asker", said(&ask.asker).unwrap_or("the owner")),
      ("askedAt", &asked_at),
      ("options", &options),
      ("depth", depth_of(&ask.key)),
      ("branches", &branches),
      ("budget", &budget_line(kept)),
    ],
  )
  .trim()
  .to_string()
}

/// The request a references run is put to, its template filled with the ask and the options it names.
pub fn references_request(ask: &ResearchRunAsk, reference: &ReferenceAsk, kept: KeyDefaults) -> String {
  let asked_at = chrono::Utc::now().to_rfc3339();
  fill(
    words::REFERENCE_REQUEST,
    &[
      ("query", ask.query.trim()),
      ("subjectKey", &reference.subject_key),
      ("kind", &reference.kind),
      ("asker", said(&ask.asker).unwrap_or("the owner")),
      ("askedAt", &asked_at),
      ("language", &reference.language),
      ("candidateLimit", &reference.candidate_limit.to_string()),
      ("engines", &reference.image_engines.join(", ")),
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
  pub provider: &'a str,
  pub model: &'a str,
  pub rulebooks: &'a [String],
}

/// The run's record, which its first answer's session carries under `labels.research` (`docs/runner.md`).
pub fn record(app: &App, made: &Recorded<'_>) -> Value {
  let mut record = json!({
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

// Agency's role slots and role sessions.

/// What a run holds of agency under this kind: a slot request's token, or a session's id.
pub const SLOT: &str = "slot";
const SESSION: &str = "session";

/// A slot request as agency answers it: its token, its state, the road it is let in on, empty until then
/// unless its ask named one, and who gave it back and why, where it was given back.
#[derive(Clone, Debug, Default)]
pub struct Slot {
  pub token: String,
  pub state: String,
  pub provider: String,
  pub model: String,
  pub effort: Option<String>,
  pub ended_by: Option<String>,
  pub outcome: Option<String>,
}

// The state a slot request is let in in, and those it no longer waits in.
const ADMITTED: [&str; 1] = ["active"];
const ENDED: [&str; 2] = ["released", "cancelled"];

impl Slot {
  pub fn of(said: &Value) -> Option<Slot> {
    let text = |field: &str| said[field].as_str().filter(|value| !value.is_empty());
    Some(Slot {
      token: text("token")?.to_owned(),
      state: text("state").unwrap_or_default().to_owned(),
      provider: text("provider").unwrap_or_default().to_owned(),
      model: text("model").unwrap_or_default().to_owned(),
      effort: text("effort").map(str::to_owned),
      ended_by: text("endedBy").map(str::to_owned),
      outcome: text("outcome").map(str::to_owned),
    })
  }

  /// The slot as an admission answers it: the request let in under `request` where it is `active`, else this
  /// slot in the `state` the answer names, with its `endedBy` and `outcome` where it was given back.
  pub fn admission(&self, said: &Value) -> Slot {
    if let Some(request) = said.get("request").and_then(Slot::of) {
      return request;
    }
    let text = |field: &str| said[field].as_str().filter(|value| !value.is_empty()).map(str::to_owned);
    let mut now = self.clone();
    if let Some(state) = text("state") {
      now.state = state;
    }
    now.ended_by = text("endedBy");
    now.outcome = text("outcome");
    now
  }

  pub fn admitted(&self) -> bool {
    ADMITTED.contains(&self.state.as_str())
  }

  pub fn ended(&self) -> bool {
    ENDED.contains(&self.state.as_str())
  }

  /// Why a slot given back before it got a place ended what waited for it: who gave it back and why, as
  /// agency answers them.
  pub fn given_back(&self, what: &str) -> String {
    let by = self.ended_by.as_deref().unwrap_or("agency");
    match self.outcome.as_deref() {
      Some(outcome) => format!("{what} was {} by {by} before it got a place: {outcome}", self.state),
      None => format!("{what} was {} by {by} before it got a place", self.state),
    }
  }
}

/// Whether a run still stands running: a branch of it waits for its slot and its card only while it does.
pub fn runs(app: &App, run: i64) -> bool {
  app.store.research_run(run).is_ok_and(|row| row.state == ResearchRunState::Running)
}

/// A slot given back with what its work came to; cancelled where it never ran. Answers whether agency still
/// held it: false where its finish says it was no longer held, or the finish was lost on the way.
pub async fn give_back(app: &App, run: i64, token: &str, outcome: &str, cancel: bool) -> bool {
  give_back_held((&app.peers, &app.store), (run, SLOT), token, outcome, cancel).await
}

/// A slot given back as `give_back` gives one, its hold let go under the owner and the kind it is kept under.
pub async fn give_back_held(
  (peers, store): (&Peers, &Store),
  (run, kind): (i64, &str),
  token: &str,
  outcome: &str,
  cancel: bool,
) -> bool {
  let outcome: String = outcome.chars().take(300).collect();
  let mut change = json!({ "state": "finished", "outcome": outcome });
  if cancel {
    change["cancel"] = json!(true);
  }
  let held = match peers.move_slot(token, &change).await {
    Ok(said) => said["held"].as_bool() != Some(false),
    Err(fault) => {
      eprintln!("bmo-research: the {kind} {token} held under {run} was not given back: {fault}");
      !fault.lost
    }
  };
  let _ = store.let_go(run, kind, token);
  held
}

/// What a slot is asked with (`docs/runner.md`, 「One run, step by step」): the request key, the run that
/// owns it, its role and its line, and the road it is let in on where one is named, its provider and model
/// together; where none is, the agent of its role it is let in on gives it.
pub fn slot_ask(request_key: &str, run: i64, role: &str, line: &str, road: Option<(&str, &str)>) -> Value {
  let mut asked = json!({
    "requestKey": request_key,
    "owner": format!("research:{run}"),
    "role": role,
    "source": line,
  });
  if let Some((provider, model)) = road {
    asked["provider"] = json!(provider);
    asked["model"] = json!(model);
  }
  asked
}

/// A slot asked and kept as the run's hold: the slot, or why agency gave none.
pub async fn take_slot(app: &App, run: i64, asked: &Value) -> Result<Slot, Fault> {
  let said = app.peers.ask_slot(asked).await?;
  let slot = Slot::of(&said).ok_or_else(|| Fault {
    why: "agency answered the slot with no token".to_string(),
    lost: false,
  })?;
  let _ = app.store.hold(run, SLOT, &slot.token);
  Ok(slot)
}

/// A session ended once its turn is over: an end answered busy asked again until it goes through, however
/// long the turn it is in takes.
pub async fn end_session(app: &App, run: i64, session: &str) {
  end_session_held((&app.peers, &app.store), (run, SESSION), session).await;
}

/// A session ended as `end_session` ends one, its hold let go under the owner and the kind it is kept under.
pub async fn end_session_held((peers, store): (&Peers, &Store), (run, kind): (i64, &str), session: &str) {
  loop {
    match peers.end_session(session).await {
      Ok(()) => break,
      Err(Ending::Busy) => tokio::time::sleep(END_EVERY).await,
      Err(Ending::Failed(why)) => {
        eprintln!("bmo-research: the {kind} {session} held under {run} was not ended: {why}");
        break;
      }
    }
  }
  let _ = store.let_go(run, kind, session);
}

/// What a session is started with.
pub struct Start<'a> {
  pub line: &'a str,
  /// The token of the active slot the session runs on: its role, its road and its path are the slot's.
  pub slot: &'a str,
  pub system: &'a str,
  pub record: &'a Value,
  pub capabilities: &'a [String],
  /// The arbiter demand every turn of the session stands on, where its run or branch holds one.
  pub demand: Option<i64>,
}

/// A role's session started on agency's line on the slot it was let in on, its id kept as the run's hold. It
/// names no role, road or folder, which are the slot's, and no bound, so agency cuts none of its turns
/// (`docs/runner.md`, 「Nothing of a run is clocked」); agency moves the slot's lease onto it as it starts it
/// (「The slots' children」).
pub async fn start_session(app: &App, run: i64, start: &Start<'_>) -> Result<String, Fault> {
  let mut body = json!({
    "scope": "role",
    "slot": start.slot,
    "line": start.line,
    "systemPrompt": start.system,
    "labels": { "research": start.record },
    "toolsAllowed": start.capabilities,
  });
  match start.demand {
    Some(demand) => body["demandId"] = json!(demand),
    None => body
      .as_object_mut()
      .expect("a start is an object")
      .extend(app.settings.engine.agency_terms()),
  }
  let session = app.peers.start_session(&body).await?;
  let _ = app.store.hold(run, SESSION, &session);
  Ok(session)
}

/// What a turn is sent with beside its prompt.
pub struct Turn<'a> {
  pub name: &'a str,
  pub title: &'a str,
  pub chat: i64,
}

/// A turn's body: its prompt and the parts that are the turn's own — a fresh resume, its run's name, its
/// title, its room, no words heard and no variables. Its road, its effort and its folder are its session's.
pub fn turn_body(prompt: &str, turn: &Turn<'_>) -> Value {
  json!({
    "prompt": prompt,
    "resume": { "kind": "fresh" },
    "name": turn.name,
    "title": turn.title,
    "chat": turn.chat,
    "hearsWords": false,
    "variables": {},
  })
}

// The ending of a turn agency stopped because the demand it stood on lost its card.
const CARD_TAKEN: &str = "card-taken";

// The lines a block opens a result's head with.
const HEAD_LINES: [&str; 2] = ["ANSWERED:", "TITLE:"];

/// What a turn wrote, whole (`docs/runner.md`, 「What a turn wrote」): where its text blocks hold more than its
/// answer, which may be its last block alone, the blocks joined in order from the last one opening a result's
/// head, or every block where none opens one; its answer otherwise, and its blocks where it names no answer.
pub fn written(turn: &Value) -> String {
  let blocks: Vec<&str> = turn["text"]
    .as_array()
    .map(|blocks| blocks.iter().filter_map(Value::as_str).filter(|b| !b.trim().is_empty()).collect())
    .unwrap_or_default();
  let opens_head = |block: &&str| {
    block
      .lines()
      .any(|line| HEAD_LINES.iter().any(|head| line.trim_start().starts_with(head)))
  };
  let from = blocks.iter().rposition(opens_head).unwrap_or(0);
  let joined = blocks[from..].join("\n");
  match turn["answer"].as_str().filter(|answer| !answer.trim().is_empty()) {
    Some(answer) if joined.trim().len() > answer.trim().len() && joined.contains(answer.trim()) => joined,
    Some(answer) => answer.to_owned(),
    None => joined,
  }
}

// The run.

/// Why a research is not sent out: the ask does not read, nothing can carry it now (a unit it needs could not
/// be reached or failed on its side), a unit it needs refused what it was asked, or the runner failed.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Refused {
  Ask(String),
  Unavailable(String),
  Upstream(String),
  Failed(String),
}

impl Refused {
  /// The refusal's sentence.
  pub fn why(&self) -> &str {
    match self {
      Refused::Ask(why) | Refused::Unavailable(why) | Refused::Upstream(why) | Refused::Failed(why) => why,
    }
  }
}

/// What a deep dive's run is handed (`docs/design.md`, 「A node pulls」): the slot its place was let in on,
/// and the arbiter demand that place was granted where its road is watched.
#[derive(Debug, Clone)]
pub struct Handed {
  pub slot: Slot,
  pub demand: Option<i64>,
}

/// A place's slot given back cancelled, its work never run on it, with why.
pub async fn give_back_unrun(app: &App, token: &str, why: &str) {
  let outcome: String = why.chars().take(300).collect();
  let change = json!({ "state": "finished", "cancel": true, "outcome": outcome });
  if let Err(fault) = app.peers.move_slot(token, &change).await {
    eprintln!("bmo-research: the place's slot {token} was not given back: {fault}");
  }
}

/// Why an ask is refused before anything is looked up.
pub fn ask_fault(ask: &ResearchRunAsk) -> Option<String> {
  if !KEYS.contains(&ask.key.as_str()) {
    return Some(format!("{} is no research key: {}", ask.key, KEYS.join(", ")));
  }
  if ask.query.trim().is_empty() {
    return Some("a research asks a question".to_string());
  }
  if ask.key == references::KEY
    && let Err(why) = references::ask(ask)
  {
    return Some(why);
  }
  (said(&ask.provider).is_some() && said(&ask.model).is_none())
    .then(|| "a research names its provider only with its model".to_string())
}

/// The road a run's slot is asked on (`docs/runner.md`, 「One run, step by step」): the ask's provider and
/// model where it names both; where it names a model alone, that model on the provider an agent of the role
/// holds it under, read off agency's role; none where it names no model, the agent the slot is let in on
/// giving it.
pub async fn named_road(
  app: &App,
  role: &str,
  ask: &ResearchRunAsk,
) -> Result<Option<(String, String)>, Fault> {
  let Some(model) = said(&ask.model) else {
    return Ok(None);
  };
  if let Some(provider) = said(&ask.provider) {
    return Ok(Some((provider.to_owned(), model.to_owned())));
  }
  let held = app.peers.role(role).await?;
  Ok(Some((provider_holding(&held, model).unwrap_or_default(), model.to_owned())))
}

/// The provider a role holds a model under: that of the first of its agents that is switched on and names
/// that model, else the provider the role stands on now, else its first switched-on agent's.
pub fn provider_holding(role: &Value, model: &str) -> Option<String> {
  let agents = role["agents"].as_array().map(Vec::as_slice).unwrap_or_default();
  let seated = || agents.iter().filter(|agent| agent["enabled"].as_bool() != Some(false));
  let provider = |agent: &Value| agent["provider"].as_str().filter(|p| !p.is_empty()).map(str::to_owned);
  seated()
    .find(|agent| agent["model"].as_str() == Some(model))
    .and_then(provider)
    .or_else(|| role["provider"].as_str().filter(|p| !p.is_empty()).map(str::to_owned))
    .or_else(|| seated().find_map(provider))
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
  /// A reference dossier: filed as a `search` document for the drawing service, with no room told.
  References,
}

/// What a deep dive asks of its run's page: the section headings its shape holds and its opening's, in every
/// language a result may be written in, the line under its title saying where the research came from, in
/// English markdown, and the earlier run whose result the run continues from, where it continues one
/// (`docs/runner.md`, 「A run that continues」).
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct PageAsk {
  /// The archive's category for the page: the item's shape.
  pub category: String,
  pub sections: Vec<String>,
  pub opening: Vec<String>,
  pub origin: Option<String>,
  pub continues: Option<i64>,
  /// The platform's age floor an advisory is judged against, as the run's question names it; none where no
  /// floor stands or the shape is not an advisory.
  pub floor: Option<String>,
}

/// The result a run continues from: the earlier run's, as the ledger keeps it, and the index of the first of
/// the key's rounds still to ask.
#[derive(Debug, Clone)]
struct Continued {
  from: i64,
  written: String,
  round: usize,
}

/// The index of the first of `rounds` the chain ending in `run` has left to ask: the one after the last any
/// run of the chain answered, each run's own answers read before those of the run it continued; none of them
/// where none answered.
pub fn rounds_left(app: &App, run: i64, rounds: &[String]) -> usize {
  let mut at = Some(run);
  let mut seen = Vec::new();
  while let Some(id) = at.filter(|id| !seen.contains(id)) {
    seen.push(id);
    let Ok(row) = app.store.research_run(id) else {
      break;
    };
    if let Some(last) = row.answered.iter().rev().find_map(|name| rounds.iter().position(|r| r == name)) {
      return last + 1;
    }
    at = row.continues;
  }
  0
}

/// What a deep dive's run continues from, where the run it names wrote a result to the same question.
fn continued(app: &App, ask: &ResearchRunAsk, filing: &Filing) -> Option<Continued> {
  let Filing::Page(page) = filing else {
    return None;
  };
  let row = app.store.research_run(page.continues?).ok()?;
  let written = row.written.clone().filter(|w| !w.trim().is_empty())?;
  if row.state != ResearchRunState::Closed || row.query.trim() != ask.query.trim() {
    return None;
  }
  let rounds = words::runner().rounds.get(&ask.key).map(|p| p.rounds.as_slice()).unwrap_or_default();
  Some(Continued {
    from: row.id,
    written,
    round: rounds_left(app, row.id, rounds),
  })
}

/// A run under way: what it was composed of and what it holds.
struct Going {
  run: i64,
  ask: ResearchRunAsk,
  filing: Filing,
  line: &'static str,
  /// Its slot as it was asked, whose token it is given back by.
  slot: Slot,
  system: String,
  prompt: String,
  held: Held,
  kept: KeyDefaults,
  rulebooks: Vec<String>,
  capabilities: Vec<String>,
  /// The subject a references run builds its dossier for, or none.
  reference: Option<ReferenceAsk>,
  demand: Option<i64>,
  /// The earlier result the run continues from, or none.
  continued: Option<Continued>,
  /// The first loss the run met, what it stands on going away (「A cut run」), or none.
  lost: Mutex<Option<String>>,
}

impl Going {
  /// A loss the run met, kept where it is the first.
  fn lose(&self, why: &str) {
    let mut lost = self.lost.lock().unwrap_or_else(|e| e.into_inner());
    if lost.is_none() {
      eprintln!("bmo-research: research {}: what it stands on went away: {why}", self.run);
      *lost = Some(why.to_owned());
    }
  }

  /// A failure of agency's said after what it stopped, kept as a loss where it is one; the sentence either
  /// way.
  fn met(&self, fault: Fault, what: &str) -> String {
    let why = format!("{what}: {fault}");
    if fault.lost {
      self.lose(&why);
    }
    why
  }

  fn loss(&self) -> Option<String> {
    self.lost.lock().unwrap_or_else(|e| e.into_inner()).clone()
  }
}

/// The run's road once its slot is let in.
struct Road {
  slot: Slot,
  model: String,
  record: Value,
}

/// The change that asks a waiting slot for its place, naming no state, standing on the demand its run or
/// branch holds where it holds one, so agency asks no card of the slot's own.
pub fn admission(demand: Option<i64>, terms: serde_json::Map<String, Value>) -> Value {
  match demand {
    Some(demand) => json!({ "demandId": demand }),
    None => Value::Object(terms),
  }
}

/// A turn sent into a session and answered once it ends, sent again for as long as it ends with its card taken:
/// what such a turn wrote is unfinished, and agency holds the next until its demand is granted again
/// (`docs/runner.md`, 「A turn's send」).
pub async fn sent_turn(app: &App, run: i64, session: &str, body: &Value) -> Result<Value, Fault> {
  loop {
    let answered = app.peers.send_turn(session, body).await?;
    if answered["ending"].as_str() != Some(CARD_TAKEN) {
      return Ok(answered);
    }
    note(
      app,
      run,
      "turn",
      "The turn's card was taken: it is asked again once its card is granted again.",
    );
    crate::engine::card_taken(app, run);
  }
}

/// One research sent out (`docs/runner.md`, 「One run, step by step」): composed, its slot asked — or the
/// slot it was handed taken as its own — and the rest carried on in the background; answered at once with
/// the road it names so far. A run asked for a room queues its end for it; a deep dive's tells nothing. A run
/// handed its place's slot and demand stands on them: its sessions stand on the demand. A handed slot the run
/// is refused before it takes is given back.
pub async fn launch(
  app: Arc<App>,
  ask: ResearchRunAsk,
  filing: Filing,
  handed: Option<Handed>,
) -> Result<ResearchRun, Refused> {
  let token = handed.as_ref().map(|handed| handed.slot.token.clone());
  let launched = sent_out(app.clone(), ask, filing, handed).await;
  if let (Err(refused), Some(token)) = (&launched, token) {
    give_back_unrun(&app, &token, &format!("the run was not sent out: {}", refused.why())).await;
  }
  launched
}

async fn sent_out(
  app: Arc<App>,
  ask: ResearchRunAsk,
  filing: Filing,
  handed: Option<Handed>,
) -> Result<ResearchRun, Refused> {
  let demand = handed.as_ref().and_then(|handed| handed.demand);
  let tells = filing == Filing::Room;
  if let Some(why) = ask_fault(&ask) {
    return Err(Refused::Ask(why));
  }
  let reference = if ask.key == references::KEY {
    Some(references::ask(&ask).map_err(Refused::Ask)?)
  } else {
    None
  };
  let line = line_of(&ask.key);
  let (held, _) = held(&app).map_err(Refused::Failed)?;
  let kept = held.key(&ask.key);
  let (system, rulebooks) = if reference.is_some() {
    let tools = ToolHost::new(&app.settings).listing("web").await.map_err(|why| {
      Refused::Unavailable(format!("the references run's instructions are not composed: {why}"))
    })?;
    (
      references_instructions(&tools).map_err(Refused::Failed)?,
      vec![references::KEY.to_owned()],
    )
  } else if line == "search" {
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
  let prompt = match &reference {
    Some(reference) => references_request(&ask, reference, kept),
    None => request(&ask, kept),
  };
  let options = options_text(&ask.options);
  let run = app
    .store
    .open_research_run(&NewResearchRun {
      key: &ask.key,
      line,
      query: ask.query.trim(),
      asker: said(&ask.asker),
      options: options.as_deref(),
      room_id: ask.room_id.filter(|room| *room != 0),
      provider: said(&ask.provider).unwrap_or_default(),
      model: said(&ask.model).unwrap_or_default(),
      branches_allowed: kept.branches,
      tells,
    })
    .map_err(Refused::Failed)?;
  note(&app, run, "asked", &format!("Research on {}: {}", ask.key, ask.query.trim()));
  if let Some(demand) = demand {
    let _ = app.store.demand_ran(demand, run);
  }
  let continued = continued(&app, &ask, &filing);
  if let Some(earlier) = &continued {
    let _ = app.store.keep_continues(run, earlier.from);
    let rounds = words::runner().rounds.get(&ask.key).map(|p| p.rounds.as_slice()).unwrap_or_default();
    let from = match rounds.get(earlier.round) {
      Some(round) => format!("from the round {round}"),
      None => "with no round left".to_string(),
    };
    note(
      &app,
      run,
      "asked",
      &format!("The research continues from what run {} wrote, {from}.", earlier.from),
    );
  }
  let refuse = |why: String, lost: bool| {
    note(&app, run, "ended", &format!("The research was refused: {why}"));
    let _ = app.store.close_research_run(run, "failed", Some(&why), None);
    if lost { Refused::Unavailable(why) } else { Refused::Upstream(why) }
  };
  let slot = match handed {
    Some(handed) => {
      let _ = app.store.hold(run, SLOT, &handed.slot.token);
      handed.slot
    }
    None => {
      let role = app.settings.runner.role(line).to_owned();
      let named = match named_road(&app, &role, &ask).await {
        Ok(named) => named,
        Err(fault) => {
          return Err(refuse(format!("agency's role {role} was not read: {fault}"), fault.lost));
        }
      };
      let road = named.as_ref().map(|(provider, model)| (provider.as_str(), model.as_str()));
      let asked = slot_ask(&format!("research:{run}"), run, &role, line, road);
      match take_slot(&app, run, &asked).await {
        Ok(slot) => slot,
        Err(fault) => return Err(refuse(format!("agency gave the run no slot: {fault}"), fault.lost)),
      }
    }
  };
  let state = if slot.admitted() { ResearchRunState::Running } else { ResearchRunState::Waiting };
  let launched = ResearchRun {
    run_id: run,
    key: ask.key.clone(),
    state,
    provider: slot.provider.clone(),
    model: slot.model.clone(),
  };
  let capabilities = capabilities(&app, kept.branches > 0, !rulebooks.is_empty());
  let going = Going {
    run,
    ask,
    filing,
    line,
    slot,
    system,
    prompt,
    held,
    kept,
    rulebooks,
    capabilities,
    reference,
    demand,
    continued,
    lost: Mutex::new(None),
  };
  tokio::spawn(go(app, going));
  Ok(launched)
}

/// The run to its end: let in, its session started on its road, its first answer, its rounds, its slot
/// given back, its document filed, closed in the ledger and told to its room.
async fn go(app: Arc<App>, going: Going) {
  let run = going.run;
  let admitted = admitted(&app, &going).await;
  // A slot that never ran is cancelled rather than finished.
  let never_ran = admitted.is_err();
  let road = match admitted {
    Ok(slot) => on_road(&app, &going, slot).await,
    Err(why) => Err(why),
  };
  let model = match &road {
    Ok(road) => road.model.clone(),
    Err(_) => said(&going.ask.model).unwrap_or_default().to_owned(),
  };
  // The session the run's slot is bound to now (`docs/runner.md`, 「The slots' children」).
  let mut bound = None;
  let result = match &road {
    Ok(road) => carried(&app, &going, road, &mut bound).await,
    Err(why) => Err(why.clone()),
  };
  let outcome = match &result {
    Ok(Finished::Written(_)) => "answered".to_string(),
    Ok(Finished::Skipped(_)) => "skipped".to_string(),
    Err(why) => format!("failed: {why}"),
  };
  // The slot is given back before the session it is bound to is ended, so its finish says whether agency
  // still held it.
  let held = give_back(&app, run, &going.slot.token, &outcome, never_ran).await;
  if let Some(session) = &bound {
    end_session(&app, run, session).await;
  }
  if !held {
    going.lose("agency no longer held the run's slot when it was given back");
  }
  ended(&app, &going, &model, result).await;
}

/// The slot let in, asked for its place every half second however long its role's line takes, and asked no
/// more once agency answers it given back; or why it was not let in. Agency binds it to each session
/// started on it in turn (`docs/runner.md`, 「The slots' children」).
async fn admitted(app: &App, going: &Going) -> Result<Slot, String> {
  let mut slot = going.slot.clone();
  loop {
    if slot.admitted() {
      return Ok(slot);
    }
    if slot.ended() {
      let why = slot.given_back("the run's slot request");
      going.lose(&why);
      return Err(why);
    }
    tokio::time::sleep(LOOK_EVERY).await;
    match app
      .peers
      .move_slot(&slot.token, &admission(going.demand, app.settings.engine.agency_terms()))
      .await
    {
      Ok(said) => slot = slot.admission(&said),
      Err(fault) => {
        return Err(going.met(fault, "the run's slot could not be let in"));
      }
    }
  }
}

/// The let-in run on its road (`docs/runner.md`, 「One run, step by step」): its road read off the ask and
/// the slot, and its record composed on it and kept; or why it could not go out.
async fn on_road(app: &App, going: &Going, slot: Slot) -> Result<Road, String> {
  let run = going.run;
  let _ = app.store.research_running(run);
  let (provider, model) = (slot.provider.clone(), slot.model.clone());
  if model.is_empty() {
    return Err("the run's slot was let in with no model".to_string());
  }
  note(app, run, "admitted", &format!("Let in on {provider} · {model}."));
  let record = record(
    app,
    &Recorded {
      run,
      ask: &going.ask,
      held: &going.held,
      kept: going.kept,
      provider: &provider,
      model: &model,
      rulebooks: &going.rulebooks,
    },
  );
  let _ = app.store.keep_research_road(run, &provider, &model, &record);
  Ok(Road {
    slot,
    model,
    record,
  })
}

/// A session started on the run's slot, which moves the slot's lease onto it, and the session the slot was
/// bound to before then ended (`docs/runner.md`, 「The slots' children」); `bound` names the new one.
async fn moved_onto(
  app: &App,
  going: &Going,
  start: &Start<'_>,
  bound: &mut Option<String>,
) -> Result<String, Fault> {
  let session = start_session(app, going.run, start).await?;
  if let Some(before) = bound.replace(session.clone()) {
    end_session(app, going.run, &before).await;
  }
  Ok(session)
}

/// The first answer's session: the run's record and its every capability, on the run's slot.
async fn first_session(
  app: &App,
  going: &Going,
  road: &Road,
  bound: &mut Option<String>,
) -> Result<String, String> {
  let start = Start {
    line: going.line,
    slot: &road.slot.token,
    system: &going.system,
    record: &road.record,
    capabilities: &going.capabilities,
    demand: going.demand,
  };
  moved_onto(app, going, &start, bound)
    .await
    .map_err(|fault| going.met(fault, "agency started no session for the run"))
}

/// What a run's work came to: the result written, or an advisory the scope check judged older than the
/// platform's age floor, which files no document and tells no one.
enum Finished {
  Written(String),
  Skipped(String),
}

/// The run's work on its slot, each session started on it before the one before is ended: the scope check an
/// advisory's deep dive gets before any research, the first answer, or the earlier result a continuing run
/// takes as its own, then the rounds, the whole document and the narration. Answers the run's outcome.
async fn carried(
  app: &App,
  going: &Going,
  road: &Road,
  bound: &mut Option<String>,
) -> Result<Finished, String> {
  let first = match &going.continued {
    Some(earlier) => earlier.written.clone(),
    None => {
      let session = first_session(app, going, road, bound).await?;
      if let Some(floor) = scope_skip(app, going, &session).await {
        return Ok(Finished::Skipped(floor));
      }
      first_answer(app, going, &session).await?
    }
  };
  if words::runner().rounds.contains_key(&going.ask.key) {
    return Ok(Finished::Written(rounds(app, going, road, first, bound).await));
  }
  let session = match bound.clone() {
    Some(session) => session,
    None => first_session(app, going, road, bound).await?,
  };
  let answer = whole(app, going, &session, first).await;
  Ok(Finished::Written(narrated(app, going, &session, answer).await))
}

/// The floor an advisory's run is judged older than before its research begins, where its scope turn answers
/// the marker; none for another shape, a run with no floor, or a turn that did not come back, which researches
/// as it stands.
async fn scope_skip(app: &App, going: &Going, session: &str) -> Option<String> {
  let Filing::Page(page) = &going.filing else {
    return None;
  };
  if page.category != Shape::Advisory.as_str() {
    return None;
  }
  let floor = page.floor.as_deref()?;
  let ask = &words::runner().scope;
  let question = fill(&ask.question, &[("outside", &ask.outside), ("inside", &ask.inside)]);
  let prompt = format!("{question}\n\n## The task\n\n{}", going.ask.query);
  let name = format!("research-{}-scope", going.run);
  match turn(app, going, session, &name, &prompt).await {
    Ok(text) if scope_marker(&text, &ask.outside) => Some(floor.to_owned()),
    Ok(_) => None,
    Err(why) => {
      note(
        app,
        going.run,
        "scope",
        &format!("The scope check did not answer, so the research goes on: {why}"),
      );
      None
    }
  }
}

/// Whether a scope turn's text opens with the marker saying the advisory is older than the floor: its first
/// non-empty line, so a document that merely mentions the words is never read as one.
pub fn scope_marker(text: &str, marker: &str) -> bool {
  text
    .lines()
    .map(str::trim)
    .find(|line| !line.is_empty())
    .is_some_and(|line| line.starts_with(marker))
}

/// One turn into a session, answered once it ends however long that takes (`docs/runner.md`, 「A turn's
/// send」): what it wrote, or why it wrote nothing.
async fn turn(app: &App, going: &Going, session: &str, name: &str, prompt: &str) -> Result<String, String> {
  let title = format!("{} research {}", going.line, going.run);
  let body = turn_body(
    prompt,
    &Turn {
      name,
      title: &title,
      chat: going.ask.room_id.unwrap_or(0),
    },
  );
  let answered = sent_turn(app, going.run, session, &body)
    .await
    .map_err(|fault| going.met(fault, &format!("the turn {name} did not come back")))?;
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

async fn first_answer(app: &App, going: &Going, session: &str) -> Result<String, String> {
  let name = format!("research-{}", going.run);
  let answer = turn(app, going, session, &name, &going.prompt).await;
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

/// The key's rounds after the first answer, in a session of their own started on the run's slot, the session
/// before it then ended, each asked in turn to the last; a round that does not answer leaves the last answer
/// standing. A deep dive's whole document and its narration are asked of the same session, which the slot
/// stays bound to until the run's end. Answers the run's result.
async fn rounds(
  app: &App,
  going: &Going,
  road: &Road,
  first: String,
  bound: &mut Option<String>,
) -> String {
  let run = going.run;
  let runner = words::runner();
  let Some(process) = runner.rounds.get(&going.ask.key) else {
    return first;
  };
  let record = round_record(&road.record);
  let capabilities: Vec<String> = going
    .capabilities
    .iter()
    .filter(|c| c.as_str() != BRANCHES && c.as_str() != RULEBOOKS)
    .cloned()
    .collect();
  let start = Start {
    line: going.line,
    slot: &road.slot.token,
    system: &going.system,
    record: &record,
    capabilities: &capabilities,
    demand: going.demand,
  };
  let session = match moved_onto(app, going, &start, bound).await {
    Ok(session) => session,
    Err(fault) => {
      let why = going.met(fault, "agency started no session for the rounds");
      note(
        app,
        run,
        "round",
        &format!("The rounds' session did not start, so the first answer stands: {why}"),
      );
      return first;
    }
  };
  let mut last = first;
  let mut asking = going.continued.as_ref().map_or(0, |earlier| earlier.round);
  while asking < process.rounds.len() {
    let name = process.rounds[asking].as_str();
    let Some(question) = runner.questions.get(name) else {
      asking += 1;
      continue;
    };
    let prompt = round_prompt(question, &going.prompt, &last);
    match turn(app, going, &session, &format!("research-{run}-{name}"), &prompt).await {
      Ok(answer) => {
        note(app, run, "round", &format!("The round {name} answered."));
        let _ = app.store.keep_round_answered(run, name);
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
  let last = whole(app, going, &session, last).await;
  if going.reference.is_some() {
    return reference_whole(app, going, &session, last).await;
  }
  narrated(app, going, &session, last).await
}

/// A references run's result held to the dossier's shape: where its last fenced block is not the JSON the
/// drawing service reads, `session` is asked once for the whole dossier, its problems set in, and that turn's
/// text is the result judged. A turn that writes nothing leaves the result as it stands, and the filing
/// judges it then.
async fn reference_whole(app: &App, going: &Going, session: &str, result: String) -> String {
  let run = going.run;
  let Some(reference) = &going.reference else {
    return result;
  };
  let Err(problems) = references::dossier(&result, reference, now_ms()) else {
    return result;
  };
  note(
    app,
    run,
    "whole",
    &format!("The dossier is not whole, so it is asked again whole: {problems}."),
  );
  let question = fill(&words::runner().references.whole, &[("problems", &problems)]);
  let prompt = round_prompt(&question, &going.prompt, &result);
  let name = format!("research-{run}-whole");
  match turn(app, going, session, &name, &prompt).await {
    Ok(text) => {
      note(app, run, "whole", "The whole dossier was asked again and came back.");
      text
    }
    Err(why) => {
      note(app, run, "whole", &format!("The whole dossier did not come: {why}"));
      result
    }
  }
}

/// The prompt a run's missing narration is asked with: the narration question and the rulebook's section on
/// the narration, the task as the run was given it, and the answer the run wrote last.
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

/// A run's result with its narration (`docs/runner.md`, 「The narration」): where it carries no `LISTEN:`
/// script, `session` is asked one more turn for the narration alone, and its text set after the result as
/// the script and kept as what the run wrote. A result that has its script, or a turn that writes none,
/// leaves the result as it stands.
async fn narrated(app: &App, going: &Going, session: &str, result: String) -> String {
  let run = going.run;
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
  let script = match turn(app, going, session, &name, &prompt).await {
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

/// The instructions a deep dive's review session is started with: the deep-dive rulebook's section on the
/// review, and the rulebook against destroying the system.
pub fn review_instructions() -> Result<String, String> {
  let ask = &words::runner().review;
  let rule = rulebook_sections(&ask.rulebook, std::slice::from_ref(&ask.section))?;
  let guard = rulebook_whole(&words::runner().search.self_preservation)?;
  Ok(format!("{rule}\n\n{guard}"))
}

/// The prompt a deep dive's review is asked with: the review question, then the finished page.
pub fn review_prompt(page: &str) -> String {
  format!("{}\n\n{page}", words::runner().review.question)
}

/// One review of a run's finished page (`docs/runner.md`, 「The review」): what the closed run still holds of
/// agency given back by its owner first, then a slot of the run's role on the run's road and, once it is let
/// in, a session of its own, both held under the run, the slot waited on for its place however long that
/// takes, one turn asked with `prompt` and waited on until it ends, the session ended and the slot given back.
/// Answers what the turn wrote, or why it wrote nothing.
pub async fn review(app: &App, run: i64, prompt: &str) -> Result<String, String> {
  let row = app.store.research_run(run).map_err(|why| why.to_string())?;
  // An earlier ask whose admission or give-back was lost stands in the role's line until it is given back.
  app
    .peers
    .release_owner(&format!("research:{run}"))
    .await
    .map_err(|fault| format!("what the run still held of agency was not given back: {fault}"))?;
  let line = line_of(&row.key);
  let role = app.settings.runner.role(line);
  let system = review_instructions()?;
  // A key is spent once its request is given back, and a stop stands over a turn's name until it is taken
  // back, so each ask goes under a count of its own: its slot's key and its turn's name alike.
  let ask = app
    .store
    .count_review_ask(run)
    .map_err(|why| format!("the review's ask was not counted: {why}"))?;
  let named = Some((row.provider.as_str(), row.model.as_str()))
    .filter(|(provider, model)| !provider.is_empty() && !model.is_empty());
  let asked = slot_ask(&format!("research:{run}:review:{ask}"), run, role, line, named);
  let mut slot = take_slot(app, run, &asked)
    .await
    .map_err(|fault| format!("agency gave the review no slot: {fault}"))?;
  let mut session = None;
  let answered = async {
    while !slot.admitted() {
      if slot.ended() {
        return Err(slot.given_back("the review's slot request"));
      }
      tokio::time::sleep(LOOK_EVERY).await;
      let said = app
        .peers
        .move_slot(&slot.token, &admission(None, app.settings.engine.agency_terms()))
        .await;
      match said {
        Ok(said) => slot = slot.admission(&said),
        Err(fault) => return Err(format!("the review's slot could not be let in: {fault}")),
      }
    }
    let start = Start {
      line,
      slot: &slot.token,
      system: &system,
      record: &row.record,
      capabilities: &[],
      demand: None,
    };
    let started = start_session(app, run, &start)
      .await
      .map_err(|fault| format!("agency started no session for the review: {fault}"))?;
    let started: &str = session.insert(started);
    let name = format!("research-{run}-review-{ask}");
    let title = format!("{line} research {run} review");
    let body = turn_body(
      prompt,
      &Turn {
        name: &name,
        title: &title,
        chat: 0,
      },
    );
    let turned = sent_turn(app, run, started, &body).await;
    let _ = app.store.keep_research_turn(run, &name, None);
    let turned = turned.map_err(|fault| format!("the turn {name} did not come back: {fault}"))?;
    let text = written(&turned);
    if text.trim().is_empty() {
      let outcome = turned["outcome"].as_str().unwrap_or("no outcome");
      return Err(format!("the turn {name} wrote nothing ({outcome})"));
    }
    Ok(text)
  }
  .await;
  if let Some(session) = &session {
    end_session(app, run, session).await;
  }
  let outcome = match &answered {
    Ok(_) => "answered".to_string(),
    Err(why) => format!("failed: {why}"),
  };
  give_back(app, run, &slot.token, &outcome, !slot.admitted()).await;
  answered
}

/// Why a deep dive's result is short of the shape its page asks (`docs/runner.md`, 「The deep dive's page」),
/// the answer's heading read in the question's language; none where it is whole.
fn short_of_shape(ask: &ResearchRunAsk, page: &PageAsk, result: &str) -> Option<String> {
  let words = words::search_document(document::language_of(&ask.query));
  document::short_of(&document::draw(result, words), &page.sections, &page.opening, &words.answer)
}

/// The prompt a deep dive's result short of its shape is asked again with: the whole-document question with
/// what it lacks set in, the task as the run was given it, and the answer the run wrote last.
pub fn whole_prompt(missing: &str, task: &str, last: &str) -> String {
  let question = words::fill(&words::runner().whole.question, &[("missing", missing)]);
  round_prompt(&question, task, last)
}

/// A deep dive's result whole (`docs/runner.md`, 「The deep dive's page」): where it is short of its shape,
/// `session` is asked one more turn for the whole document, and that turn's text is the result judged. A run
/// told to a room, a whole result, or a turn that writes nothing leaves the result as it stands.
async fn whole(app: &App, going: &Going, session: &str, result: String) -> String {
  let run = going.run;
  let Filing::Page(page) = &going.filing else {
    return result;
  };
  let Some(short) = short_of_shape(&going.ask, page, &result) else {
    return result;
  };
  note(
    app,
    run,
    "whole",
    &format!("The result is not whole, so it is asked again whole: {short}."),
  );
  let prompt = whole_prompt(&short, &going.prompt, &result);
  let name = format!("research-{run}-whole");
  match turn(app, going, session, &name, &prompt).await {
    Ok(text) => {
      note(app, run, "whole", "The whole document was asked again and came back.");
      text
    }
    Err(why) => {
      note(app, run, "whole", &format!("The whole document did not come: {why}"));
      result
    }
  }
}

/// A deep dive's result held to its shape and filed as its page: the outcome, the error and the document.
async fn file_page(
  app: &App,
  going: &Going,
  page: &PageAsk,
  text: &str,
) -> (&'static str, Option<String>, Option<RunDocument>) {
  let (run, ask) = (going.run, &going.ask);
  if let Some(short) = short_of_shape(ask, page, text) {
    let why = format!("the result was not filed: {short}");
    note(app, run, "filed", &format!("The result was not filed, since {short}."));
    return ("failed", Some(why), None);
  }
  let drawn = document::draw(text, words::search_document(document::language_of(&ask.query)));
  let title = drawn.title.clone().unwrap_or_default();
  let kept = document::fetch_pictures(app, &drawn, run).await;
  let figures = document::figures(&drawn, &kept);
  let filing = document::page_filing(&drawn, run, &ask.key, &ask.query, &title, page, &figures);
  match document::file_page(app, &filing, &kept).await {
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
async fn ended(app: &App, going: &Going, model: &str, result: Result<Finished, String>) {
  let run = going.run;
  let ask = &going.ask;
  if let Ok(Finished::Skipped(floor)) = &result {
    let said = format!("older than the age floor, {floor}");
    note(
      app,
      run,
      "ended",
      &format!("The advisory is skipped: {said}; no document and no notice."),
    );
    match app.store.close_research_run(run, "skipped", Some(&said), None) {
      Ok(true) => {}
      Ok(false) => {}
      Err(why) => eprintln!("bmo-research: research {run} was not closed: {why}"),
    }
    return;
  }
  let result = match result {
    Ok(Finished::Written(text)) => Ok(text),
    Ok(Finished::Skipped(_)) => unreachable!("the skipped outcome returns above"),
    Err(why) => Err(why),
  };
  if let (Filing::Page(page), Ok(text)) = (&going.filing, &result) {
    let (outcome, error, filed) = match file_page(app, going, page, text).await {
      ("failed", Some(why), None) => match going.loss() {
        Some(loss) => ("cut", Some(cut_error(&why, &loss)), None),
        None => ("failed", Some(why), None),
      },
      other => other,
    };
    match app.store.close_research_run(run, outcome, error.as_deref(), filed.as_ref()) {
      Ok(true) => note(app, run, "ended", &format!("The research ended {outcome}.")),
      Ok(false) => {}
      Err(why) => eprintln!("bmo-research: research {run} was not closed: {why}"),
    }
    return;
  }
  if matches!(going.filing, Filing::References) {
    let reference = going.reference.as_ref().expect("a references filing holds its ask");
    let (outcome, error, filed) = match &result {
      Ok(text) => match references::file(app, run, ask, reference, text).await {
        Ok(filed) => {
          note(
            app,
            run,
            "filed",
            &format!("The dossier was filed as document {}: {}", filed.id, filed.link),
          );
          ("answered", None, Some(filed))
        }
        Err(why) => {
          let why = format!("the dossier was not filed: {why}");
          note(app, run, "filed", &why);
          match going.loss() {
            Some(loss) => ("cut", Some(cut_error(&why, &loss)), None),
            None => ("failed", Some(why), None),
          }
        }
      },
      Err(why) => match going.loss() {
        Some(loss) => ("cut", Some(cut_error(why, &loss)), None),
        None => ("failed", Some(why.clone()), None),
      },
    };
    match app.store.close_research_run(run, outcome, error.as_deref(), filed.as_ref()) {
      Ok(true) => note(app, run, "ended", &format!("The research ended {outcome}.")),
      Ok(false) => {}
      Err(why) => eprintln!("bmo-research: research {run} was not closed: {why}"),
    }
    return;
  }
  let mut pending = Vec::new();
  let (told, outcome, error, filed) = match &result {
    Ok(text) => {
      let code = document::language_of(&ask.query);
      let drawn = document::draw(text, words::search_document(code));
      let filing = document::filing(&drawn, run, &ask.key, &ask.query, ask.room_id);
      match document::file(app, &drawn, &filing, run).await {
        Ok(filed) => {
          note(
            app,
            run,
            "filed",
            &format!("The result was filed as document {}: {}", filed.id, filed.link),
          );
          // The document's page is opened before the run closes, so a restart finds what it lacks.
          pending = document::room_pending(&drawn, &ask.query);
          let retry = (!pending.is_empty()).then(|| now_ms() + app.settings.retry_minutes * 60_000);
          if let Err(why) = app.store.open_page(filed.id, None, &[code.to_owned()], &pending, retry) {
            eprintln!("bmo-research: research {run}'s document {} was not kept as a page: {why}", filed.id);
          }
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
          (whole_instruction(run, ask, model, &result), "answered", None, None)
        }
      }
    }
    Err(why) => match going.loss() {
      Some(loss) => {
        let told = match app.store.research_run(run) {
          Ok(row) => cut_instruction(&row),
          Err(_) => whole_instruction(run, ask, model, &result),
        };
        (told, "cut", Some(cut_error(why, &loss)), None)
      }
      None => (whole_instruction(run, ask, model, &result), "failed", Some(why.clone()), None),
    },
  };
  match app.store.close_research_run(run, outcome, error.as_deref(), filed.as_ref()) {
    Ok(true) => note(app, run, "ended", &format!("The research ended {outcome}.")),
    Ok(false) => return,
    Err(why) => eprintln!("bmo-research: research {run} was not closed: {why}"),
  }
  let Ok(row) = app.store.research_run(run) else {
    return tell(app, run, &told).await;
  };
  if !row.tells {
    return;
  }
  match (&filed, pending.first()) {
    (Some(_), Some(lacks)) => {
      let every = app.settings.retry_minutes;
      let said = format!(
        "The room is told once the document has its narration ({lacks}), asked again every {every} minutes \
         until it is attached."
      );
      note(app, run, "filed", &said);
      finish_told(app, &row).await;
    }
    _ => tell(app, run, &told).await,
  }
}

/// A room's document finished as far as it can be now (`docs/runner.md`, 「The narration's sound」): its
/// narration, where its result has a script and it lacks the sound, spoken as written in the question's
/// language and attached at the document's head; what it still lacks kept, to be asked again
/// `retry_minutes` from now; and, once it lacks nothing, its labels set without `pending` and its room told,
/// once, with the document's live link. A document being finished now is left to that finishing.
pub async fn finish_told(app: &App, row: &ResearchRunRow) {
  let (run, Some(document_id)) = (row.id, row.document_id) else {
    return;
  };
  let Some(_held) = app.pages.hold(document_id) else {
    return;
  };
  let written = row.written.clone().unwrap_or_default();
  let ask = ResearchRunAsk {
    key: row.key.clone(),
    query: row.query.clone(),
    room_id: row.room_id,
    ..ResearchRunAsk::default()
  };
  // A document gone before its room was told leaves the result to be told whole, once.
  if dive::gone(app, run, document_id).await {
    note(app, run, "filed", "The document is gone, so its room is told the result whole.");
    return tell(app, run, &whole_instruction(run, &ask, &row.model, &Ok(written))).await;
  }
  let state = match app.store.page_state(document_id) {
    Ok(state) => state,
    Err(why) => {
      eprintln!("bmo-research: research {run}'s document {document_id} could not be read: {why}");
      return;
    }
  };
  let code = document::language_of(&row.query);
  let drawn = document::draw(&written, words::search_document(code));
  let script = drawn.narration.clone().filter(|script| !script.trim().is_empty());
  let lacking = script.is_some() && !state.narrations.iter().any(|had| had == code);
  let next = now_ms() + app.settings.retry_minutes * 60_000;
  if let Err(why) = app.store.claim_page(document_id, next, false, lacking) {
    eprintln!("bmo-research: research {run}'s document {document_id} could not be claimed: {why}");
    return;
  }
  let mut narrations = state.narrations.clone();
  if let (true, Some(script)) = (lacking, script.as_deref()) {
    let name = dive::sound_name(code);
    match dive::narration(app, (run, document_id), (code, Speech::Written(script)), &name, Some("head"))
      .await
    {
      Ok(()) => {
        narrations.push(code.to_owned());
        let _ = app.store.keep_narrations(document_id, &narrations);
        note(app, run, "narration", &format!("The {code} narration was attached."));
      }
      Err(why) => {
        eprintln!("bmo-research: research {run}'s {code} narration was not made: {why}");
        note(app, run, "narration", &format!("The {code} narration was not made: {why}"));
      }
    }
  }
  let pending = match &script {
    Some(_) if !narrations.iter().any(|had| had == code) => vec![format!("narration-{code}")],
    _ => Vec::new(),
  };
  let _ = app.store.pend_page(document_id, &pending, (!pending.is_empty()).then_some(next));
  if pending == state.pending {
    return;
  }
  let labels = document::room_labels(&drawn, run, &row.key, &row.query, &pending);
  let set = app
    .peers
    .artifact_send(
      reqwest::Method::PATCH,
      &format!("/documents/{document_id}"),
      &json!({ "labels": labels }),
    )
    .await;
  if let Err(why) = set {
    eprintln!("bmo-research: research {run}'s document {document_id}'s labels were not set: {why}");
  }
  if !pending.is_empty() {
    return;
  }
  let link = match app.peers.share(document_id).await {
    Ok(link) => link,
    Err(why) => {
      eprintln!("bmo-research: research {run}'s document {document_id} has no live link now: {why}");
      row.link.clone().unwrap_or_default()
    }
  };
  let brief = brief_of(&written).unwrap_or_else(|| written.clone());
  note(app, run, "filed", "The document has its narration, so its room is told.");
  tell(app, run, &linked_instruction(run, &ask, &brief, &link)).await;
}

/// Why a run that met a loss was cut: its own failure, and the loss where that was not the failure itself.
fn cut_error(why: &str, loss: &str) -> String {
  if why == loss { why.to_owned() } else { format!("{why}; {loss}") }
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

/// The answer in brief a result gives the room: its verdict, else a summary it still writes, else its title.
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

/// Everything an owner held of agency given back by the owner's release; the sessions agency ended with it,
/// none where the release did not go through.
async fn release_owner(peers: &Peers, owner: &str) -> Vec<String> {
  match peers.release_owner(owner).await {
    Ok(ended) => ended,
    Err(fault) => {
      eprintln!("bmo-research: what {owner} held of agency was not given back: {fault}");
      vec![]
    }
  }
}

/// A daemon's start (`docs/runner.md`, 「A restart」): every run its store holds open closed as cut, what it
/// held of agency given back by its owner, and its room told. Answers how many were cut.
pub async fn sweep(app: &Arc<App>) -> usize {
  let open = match app.store.open_research_runs() {
    Ok(open) => open,
    Err(why) => {
      eprintln!("bmo-research: the open research runs could not be read: {why}");
      return 0;
    }
  };
  // Each run held open, and each closed run whose review held something at the restart, is given back by its
  // owner: its slots and the sessions agency bound them to. A session agency did not end — the rounds' beside
  // the first, or one still in a turn — is ended once its turn is over; the review is asked again on its
  // page's retry wait.
  let mut owned: BTreeMap<i64, Vec<(String, String)>> = BTreeMap::new();
  for (run, kind, value) in app.store.closed_holds().unwrap_or_default() {
    owned.entry(run).or_default().push((kind, value));
  }
  for row in &open {
    owned.entry(row.id).or_default().extend(app.store.holds(row.id).unwrap_or_default());
  }
  // Every place the daemon had asked, waiting or let in, is given back by the places' owner
  // (`docs/design.md`, 「A node pulls」): a run that held one is closed below.
  let places = release_owner(&app.peers, crate::pull::OWNER).await;
  for (run, held) in owned {
    let mut ended = release_owner(&app.peers, &format!("research:{run}")).await;
    ended.extend(places.iter().cloned());
    for (kind, value) in held {
      if kind == SESSION && !ended.contains(&value) {
        let app = app.clone();
        tokio::spawn(async move { end_session(&app, run, &value).await });
      } else {
        let _ = app.store.let_go(run, &kind, &value);
      }
    }
  }
  // A notice to an agent under way holds a slot and a session under its item: the session is ended once its
  // turn is over, and only then is everything its owner holds given back, so the checkout is not let go under
  // a turn still going.
  let mut notices: BTreeMap<i64, Vec<(String, String)>> = BTreeMap::new();
  for (item, kind, value) in app.store.notice_holds().unwrap_or_default() {
    notices.entry(item).or_default().push((kind, value));
  }
  for (item, held) in notices {
    let app = app.clone();
    tokio::spawn(async move {
      let ground = (&app.peers, &app.store);
      for (kind, session) in held.iter().filter(|(kind, _)| kind == dive::NOTICE_SESSION) {
        end_session_held(ground, (item, kind), session).await;
      }
      release_owner(&app.peers, &format!("research:notice:{item}")).await;
      for (kind, token) in held.iter().filter(|(kind, _)| kind == dive::NOTICE_SLOT) {
        let _ = app.store.let_go(item, kind, token);
      }
    });
  }
  for row in &open {
    let run = row.id;
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
