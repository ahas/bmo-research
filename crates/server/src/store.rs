//! The daemon's store: the categories and their recipients, the sources, the candidates seen, the queue, the
//! tries, the reports, the facts and the stack, the arbiter demands held, and the runner's research runs, their
//! log, what they hold of agency and the research settings, in SQLite under the data root (`docs/store.md`).

use std::path::Path;
use std::sync::{Arc, Mutex};

use bmo_research_client::{
  Category, CategoryChange, CategoryShape, CategoryState, DaemonState, Fact, Filter, Item, ItemState,
  NewCategory, NewSource, NewStackRow, Origin, OriginKind, Page, Recipient, RecipientKind, ResearchLogLine,
  ResearchRunState, ResearchRunView, Run, Running, Shape, Source, SourceChange, StackRow,
};
use rusqlite::{Connection, OptionalExtension, Transaction, params};

use crate::sources::{Candidate, ItemKind};
use crate::translation::Rendering;

/// The schema's steps, each bringing the store one version up; the version is how many have been taken. The
/// first registers the three categories the unit starts with, so a category taken away is not brought back;
/// the second keeps an item's Korean rendering and its pending tries; the third holds the runner's research
/// runs, their log, what they hold of agency and the owner's research settings; the fourth keeps what a
/// deep dive's one page holds — its languages, its narrations and its Korean side — and what it still
/// lacks, a document filed before it holding English alone and lacking nothing it can be given; the fifth
/// keeps what the readings left on each source: how many were made, and what the last that succeeded turned
/// up and kept; the sixth holds a `page` source's link pattern and strategy; the seventh keeps the arbiter
/// demands the daemon holds and takes the fit's node list away; the eighth keeps what a page still lacks
/// before its report is told, stands an item whose page was being finished and whose report was never told
/// `finishing`, and sets every page finished since the fourth that still lacks a part due on the retry
/// wait; the ninth keeps where an item came from — a source, an ask with its words, or a follow-up with the
/// document it follows from — an item standing before it a source's where no ask made it, and an ask's with
/// no words otherwise; the tenth keeps the repair a stuck item's worker is asked for, stands every item the
/// store held given up `stuck` with its retry due and no repair yet, and names their notices `stuck`; the
/// eleventh keeps the synthesis asked of the voice unit for each narration a page still lacks, and sets
/// every filed page that plays a narration to be finished again at once by the item its try was asked for,
/// with none counted as had, so each sound is found attached and the page drawn again whole from what the
/// store keeps; the twelfth keeps, for a run that continues, the run it continues and the rounds each run
/// answered, and, for a deep dive's page, each part of its Korean side as it is had, the texts its review
/// rewrote and its Korean narration's words, its one review, and how often each narration was asked anew;
/// the thirteenth keeps when the try an item's error names ended, an error standing before it dated by the
/// latest try that carried the item, else by the item's last change; the fourteenth counts each run's
/// page's review asks; the fifteenth takes a run's closing time away and the research settings' minutes
/// with it; the sixteenth keeps what a page holds and lacks by its document, for a deep dive's page and a
/// room's document alike, each filed page taking what the item its run was asked for kept; the seventeenth
/// names every hold of a slot request `slot`; the eighteenth keeps when a page's document was found gone; the
/// nineteenth takes each category's cap away, the queue holding none.
const MIGRATIONS: &[&str] = &[
  "
  create table categories (
    name text primary key, purpose text not null, shape text not null, cap integer not null,
    cadence_minutes integer not null, gather_minutes integer, provider text, model text,
    state text not null default 'running', state_at integer, created_at integer not null);
  insert into categories (name, purpose, shape, cap, cadence_minutes, gather_minutes, created_at) values
    ('it', 'New AI technology: models, engines, datasets, tools and papers the lab may run or measure.',
     'manual-or-investigation', 5, 60, null, cast(strftime('%s', 'now') as integer) * 1000),
    ('security', 'Vulnerability news that bears on the platform''s own stack.',
     'advisory', 3, 60, null, cast(strftime('%s', 'now') as integer) * 1000),
    ('economy', 'Economic news and indicators, gathered into a daily brief whose figures are kept.',
     'brief', 30, 60, 1440, cast(strftime('%s', 'now') as integer) * 1000);
  create table sources (
    id integer primary key autoincrement, category text not null, kind text not null, address text not null,
    query text, require text not null default '[]', exclude text not null default '[]', every_minutes integer,
    item_limit integer not null, failure_count integer not null default 0, last_error text, read_at integer,
    created_at integer not null, changed_at integer not null);
  create table seen (
    key text primary key, category text not null, name text not null, maker text, address text,
    kind text not null, published_at integer, source_id integer, state text not null, item_id integer,
    found_at integer not null);
  create index seen_unqueued on seen (category, state, found_at);
  create table queue (
    id integer primary key autoincrement, category text not null, key text not null, name text not null,
    maker text, address text, kind text not null, shape text not null, source_id integer, asked integer not null,
    priority integer not null, state text not null, tries integer not null default 0, retry_at integer,
    run_id integer, document_id integer, title text, description text, share_url text,
    fit integer not null default 0, fit_nodes text not null default '[]', touches text, plan text,
    job_item_id integer, error text, found_at integer not null, created_at integer not null,
    changed_at integer not null);
  create index queue_state on queue (category, state);
  create table runs (
    id integer primary key autoincrement, category text not null, item_id integer not null,
    item_ids text not null, try integer not null, run_id integer, state text not null, outcome text, error text,
    started_at integer not null, ended_at integer);
  create table reports (
    id integer primary key autoincrement, item_id integer not null, recipient text not null, kind text not null,
    instruction_id integer, state text not null, error text, at integer not null);
  create table recipients (
    id integer primary key autoincrement, category text not null, kind text not null, room_id integer,
    agent_name text, origin text not null, added_at integer not null);
  create unique index recipients_one on recipients (category, kind, coalesce(room_id, 0), coalesce(agent_name, ''));
  create table facts (
    id integer primary key autoincrement, name text not null, value text not null, unit text, as_of text,
    source_address text, run_id integer, document_id integer, created_at integer not null);
  create index facts_name on facts (name, id);
  create table stack (
    id integer primary key autoincrement, component text not null, version text, place text, repository text,
    worker text, added_at integer not null);
",
  "
  alter table queue add column ko_document_id integer;
  alter table queue add column ko_share_url text;
  alter table queue add column translation_tries integer not null default 0;
  alter table queue add column translation_retry_at integer;
",
  "
  create table research_runs (
    id integer primary key autoincrement, key text not null, line text not null, query text not null,
    asker text, options text, room_id integer, provider text not null, model text not null,
    state text not null, outcome text, error text, started_at integer not null, closes_at integer,
    finished_at integer, branches_allowed integer not null, branches integer not null default 0,
    record text not null, tells integer not null, turns text not null default '[]', written text,
    document_id integer, title text, description text, link text);
  create index research_runs_state on research_runs (state);
  create table research_log (
    id integer primary key autoincrement, run_id integer not null, at integer not null, kind text not null,
    text text not null);
  create index research_log_run on research_log (run_id, id);
  create table research_holds (
    id integer primary key autoincrement, run_id integer not null, kind text not null, value text not null);
  create index research_holds_run on research_holds (run_id);
  create table research_settings (id integer primary key, value text not null);
",
  "
  alter table queue drop column ko_document_id;
  alter table queue drop column ko_share_url;
  alter table queue rename column translation_retry_at to pending_retry_at;
  alter table queue add column languages text not null default '[]';
  alter table queue add column narrations text not null default '[]';
  alter table queue add column narration_tries integer not null default 0;
  update queue set languages = '[\"en\"]' where document_id is not null;
  update queue set pending_retry_at = null;
  create table renderings (
    document_id integer primary key, title text not null, description text, body text not null,
    created_at integer not null);
",
  "
  alter table sources add column scan_count integer not null default 0;
  alter table sources add column last_yield integer;
  alter table sources add column last_taken integer;
",
  "
  alter table sources add column pattern text;
  alter table sources add column strategy text;
",
  "
  alter table queue drop column fit_nodes;
  create table demands (
    id integer primary key, try_id integer, run_id integer, within integer, service text not null,
    asked_at integer not null);
",
  "
  alter table queue add column pending text not null default '[]';
  update queue set state = 'finishing'
    where state in ('reported', 'fit') and pending_retry_at is not null
      and not exists (select 1 from reports where reports.item_id = queue.id);
  update queue set pending_retry_at = 0
    where state in ('reported', 'fit', 'taken') and document_id is not null and pending_retry_at is null
      and (translation_tries > 0 or narration_tries > 0)
      and (languages <> '[\"ko\",\"en\"]' or narrations <> '[\"ko\",\"en\"]');
",
  "
  alter table queue add column origin text not null default 'source';
  alter table queue add column because text;
  alter table queue add column parent_item_id integer;
  alter table queue add column parent_document_id integer;
  alter table queue add column parent_title text;
  alter table queue add column parent_share_url text;
  update queue set origin = 'asked' where asked = 1;
",
  "
  alter table queue add column repair_item_id integer;
  update queue set state = 'stuck', retry_at = 0 where state = 'failed';
  update reports set kind = 'stuck' where kind = 'failed';
",
  "
  create table syntheses (
    document_id integer not null, language text not null, synthesis_id integer not null,
    characters integer not null, asked_at integer not null, primary key (document_id, language));
  update queue set narrations = '[]', pending_retry_at = 0
    where narrations <> '[]' and document_id is not null
      and state in ('finishing', 'reported', 'fit', 'taken')
      and id in (select runs.item_id from runs where runs.run_id = queue.run_id);
  update queue set narrations = '[]'
    where narrations <> '[]' and document_id in (select document_id from queue where pending_retry_at = 0);
",
  "
  alter table research_runs add column continues integer;
  alter table research_runs add column answered text not null default '[]';
  create table translated_parts (
    document_id integer not null, english text not null, korean text not null,
    primary key (document_id, english));
  create table page_texts (
    document_id integer not null, name text not null, text text not null, primary key (document_id, name));
  create table page_reviews (
    document_id integer primary key, answer text not null, applied text not null default '[]',
    remade text not null default '[]', reviewed_at integer not null);
  create table narration_asks (
    document_id integer not null, language text not null, renewed integer not null,
    primary key (document_id, language));
",
  "
  alter table queue add column error_at integer;
  update queue set error_at = coalesce(
      (select max(runs.ended_at) from runs
        where runs.item_id = queue.id or (queue.run_id is not null and runs.run_id = queue.run_id)),
      changed_at)
    where error is not null;
",
  "
  alter table research_runs add column review_asks integer not null default 0;
",
  "
  alter table research_runs drop column closes_at;
  update research_settings set value = json_remove(value, '$.keys.\"search:basic\".minutes',
    '$.keys.\"search:thorough\".minutes', '$.keys.\"search:exhaustive\".minutes', '$.keys.browse.minutes');
",
  "
  create table pages (
    document_id integer primary key, item_id integer, languages text not null default '[]',
    narrations text not null default '[]', pending text not null default '[]', pending_retry_at integer,
    translation_tries integer not null default 0, narration_tries integer not null default 0);
  insert or ignore into pages (document_id, item_id, languages, narrations, pending, pending_retry_at,
      translation_tries, narration_tries)
    select document_id, id, languages, narrations, pending, pending_retry_at, translation_tries, narration_tries
      from queue where document_id is not null
      order by document_id,
        exists (select 1 from runs where runs.run_id = queue.run_id and runs.item_id = queue.id) desc,
        pending_retry_at is null, id;
  alter table queue drop column languages;
  alter table queue drop column narrations;
  alter table queue drop column pending;
  alter table queue drop column pending_retry_at;
  alter table queue drop column translation_tries;
  alter table queue drop column narration_tries;
",
  "
  update research_holds set kind = 'slot' where kind = 'seat';
",
  "
  alter table pages add column gone_at integer;
",
  "
  alter table categories drop column cap;
",
];

/// A page's one review (`docs/store.md`, `page_reviews`): what it wrote, the indices of its fixes applied or
/// left out, and the languages whose narration it had made again.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Review {
  pub answer: String,
  pub applied: Vec<usize>,
  pub remade: Vec<String>,
}

/// What an ask says of where it came from: the words it was made with, and for a follow-up, the earlier
/// research it follows from.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Asked {
  pub because: Option<String>,
  pub parent: Option<Parent>,
}

/// The earlier research a follow-up follows from: its item where one filed it, its document, and that
/// document's title and live link.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Parent {
  pub item_id: Option<i64>,
  pub document_id: Option<i64>,
  pub title: Option<String>,
  pub share_url: Option<String>,
}

/// The schema version this binary holds.
pub fn schema_version() -> i64 {
  MIGRATIONS.len() as i64
}

pub fn now_ms() -> i64 {
  std::time::SystemTime::now()
    .duration_since(std::time::UNIX_EPOCH)
    .map(|d| d.as_millis() as i64)
    .unwrap_or(0)
}

const CATEGORY_COLUMNS: &str =
  "name, purpose, shape, cadence_minutes, gather_minutes, provider, model, state, state_at, created_at";

const ITEM_COLUMNS: &str =
  "id, category, key, name, maker, address, kind, shape, source_id, asked, priority,
  state, tries, retry_at, run_id, document_id, title, description, share_url, fit, touches, plan,
  job_item_id, error, found_at, created_at, changed_at,
  coalesce((select pages.languages from pages where pages.document_id = queue.document_id), '[]'),
  coalesce((select pages.narrations from pages where pages.document_id = queue.document_id), '[]'),
  coalesce((select pages.pending from pages where pages.document_id = queue.document_id), '[]'),
  (select pages.pending_retry_at from pages where pages.document_id = queue.document_id),
  origin, because, parent_item_id, parent_document_id, parent_title, parent_share_url,
  (select sources.kind || ' (' || coalesce(sources.query, sources.address) || ')' from sources
    where sources.id = queue.source_id), repair_item_id, error_at";

const SOURCE_COLUMNS: &str =
  "id, category, kind, address, query, require, exclude, every_minutes, item_limit,
  failure_count, last_error, read_at, created_at, changed_at, scan_count, last_yield, last_taken, pattern, strategy";

const RUN_COLUMNS: &str =
  "id, category, item_id, item_ids, try, run_id, state, outcome, error, started_at, ended_at";

const RECIPIENT_COLUMNS: &str = "id, category, kind, room_id, agent_name, origin, added_at";

/// Why a change of the store was refused: nothing stood at the key, the state or a key did not allow it, or
/// the store failed.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Refusal {
  NotFound(String),
  Conflict(String),
  Failed(String),
}

impl From<rusqlite::Error> for Refusal {
  fn from(e: rusqlite::Error) -> Self {
    Refusal::Failed(e.to_string())
  }
}

impl std::fmt::Display for Refusal {
  fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    match self {
      Refusal::NotFound(why) | Refusal::Conflict(why) | Refusal::Failed(why) => f.write_str(why),
    }
  }
}

/// What a reading that succeeded turned up: the candidates the filter let through, and those kept as new.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Yield {
  pub candidates: i64,
  pub taken: i64,
}

/// What a finished try leaves on an item.
#[derive(Debug, Clone, Default)]
pub struct Filed {
  pub document_id: i64,
  pub title: String,
  pub description: Option<String>,
  pub share_url: String,
  pub fit: bool,
  pub touches: Option<String>,
  pub plan: Option<String>,
}

/// How a document's page stands (`docs/store.md`, `pages`): the languages and narrations it holds, Korean
/// first, what it was last found lacking, the Korean sides and narrations asked, and when what it lacks is
/// asked again.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct PageState {
  pub languages: Vec<String>,
  pub narrations: Vec<String>,
  pub pending: Vec<String>,
  pub translation_tries: i64,
  pub narration_tries: i64,
  pub retry_at: Option<i64>,
}

/// A research run as the runner opens it.
#[derive(Debug, Clone)]
pub struct NewResearchRun<'a> {
  pub key: &'a str,
  pub line: &'a str,
  pub query: &'a str,
  pub asker: Option<&'a str>,
  pub options: Option<&'a str>,
  pub room_id: Option<i64>,
  pub provider: &'a str,
  pub model: &'a str,
  pub branches_allowed: i64,
  pub tells: bool,
}

/// A research run's row, whole (`docs/store.md`, `research_runs`).
#[derive(Debug, Clone)]
pub struct ResearchRunRow {
  pub id: i64,
  pub key: String,
  pub line: String,
  pub query: String,
  pub asker: Option<String>,
  pub options: Option<String>,
  pub room_id: Option<i64>,
  pub provider: String,
  pub model: String,
  pub state: ResearchRunState,
  pub outcome: Option<String>,
  pub error: Option<String>,
  pub started_at: i64,
  pub finished_at: Option<i64>,
  pub branches_allowed: i64,
  pub branches: i64,
  pub record: serde_json::Value,
  pub tells: bool,
  pub turns: Vec<String>,
  pub written: Option<String>,
  pub document_id: Option<i64>,
  pub title: Option<String>,
  pub description: Option<String>,
  pub link: Option<String>,
  /// The run whose result it continues from, or none.
  pub continues: Option<i64>,
  /// The rounds that answered, in the order they were asked.
  pub answered: Vec<String>,
  /// The languages whose narration its document plays, from its document's page.
  pub narrations: Vec<String>,
  /// What its document still lacks, from its document's page.
  pub pending: Vec<String>,
}

impl ResearchRunRow {
  /// The run as its route answers it.
  pub fn view(&self) -> ResearchRunView {
    ResearchRunView {
      run_id: self.id,
      key: self.key.clone(),
      query: self.query.clone(),
      asker: self.asker.clone(),
      room_id: self.room_id,
      provider: self.provider.clone(),
      model: self.model.clone(),
      state: self.state,
      outcome: self.outcome.clone(),
      started_at: self.started_at,
      finished_at: self.finished_at,
      document_id: self.document_id,
      link: self.link.clone(),
      error: self.error.clone(),
      branches: self.branches,
      turns: self.turns.clone(),
      narrations: self.narrations.clone(),
      pending: self.pending.clone(),
    }
  }
}

/// The document a closing run filed: its id, its head and its share link.
#[derive(Debug, Clone, Default)]
pub struct RunDocument {
  pub id: i64,
  pub title: String,
  pub description: Option<String>,
  pub link: String,
}

const RESEARCH_RUN_COLUMNS: &str = "id, key, line, query, asker, options, room_id, provider, model, state,
  outcome, error, started_at, finished_at, branches_allowed, branches, record, tells, turns, written,
  document_id, title, description, link, continues, answered,
  coalesce((select pages.narrations from pages where pages.document_id = research_runs.document_id), '[]'),
  coalesce((select pages.pending from pages where pages.document_id = research_runs.document_id), '[]')";

/// The shape an item of a category is asked in: the category's, and in a `manual-or-investigation`
/// category the one its item kind takes.
pub fn item_shape(category: CategoryShape, kind: Option<ItemKind>) -> Shape {
  match category {
    CategoryShape::ManualOrInvestigation => kind.map(ItemKind::shape).unwrap_or(Shape::Manual),
    CategoryShape::Advisory => Shape::Advisory,
    CategoryShape::Brief => Shape::Brief,
  }
}

#[derive(Clone)]
pub struct Store {
  conn: Arc<Mutex<Connection>>,
}

impl Store {
  /// The store in `dir`, its schema brought to this binary's version.
  pub fn open(dir: &Path) -> Result<Store, String> {
    std::fs::create_dir_all(dir).map_err(|e| format!("{}: {e}", dir.display()))?;
    let conn = Connection::open(dir.join("research.db")).map_err(|e| e.to_string())?;
    conn.execute_batch("pragma journal_mode=wal;").map_err(|e| e.to_string())?;
    conn.busy_handler(Some(wait_for_the_lock)).map_err(|e| e.to_string())?;
    migrate(&conn)?;
    Ok(Store {
      conn: Arc::new(Mutex::new(conn)),
    })
  }

  fn lock(&self) -> std::sync::MutexGuard<'_, Connection> {
    self.conn.lock().unwrap_or_else(|e| e.into_inner())
  }

  pub fn version(&self) -> Result<i64, String> {
    self
      .lock()
      .query_row("pragma user_version", [], |r| r.get(0))
      .map_err(|e| e.to_string())
  }

  // The categories.

  pub fn categories(&self) -> Result<Vec<Category>, String> {
    let conn = self.lock();
    let mut stmt = conn
      .prepare(&format!("select {CATEGORY_COLUMNS} from categories order by name"))
      .map_err(|e| e.to_string())?;
    stmt
      .query_map([], read_category)
      .map_err(|e| e.to_string())?
      .collect::<Result<Vec<_>, _>>()
      .map_err(|e| e.to_string())
  }

  pub fn category(&self, name: &str) -> Result<Category, Refusal> {
    self
      .lock()
      .query_row(
        &format!("select {CATEGORY_COLUMNS} from categories where name = ?1"),
        params![name],
        read_category,
      )
      .optional()?
      .ok_or_else(|| Refusal::NotFound(format!("no category {name}")))
  }

  pub fn add_category(&self, new: &NewCategory) -> Result<Category, Refusal> {
    let gather = match new.shape {
      CategoryShape::Brief => Some(new.gather_minutes.unwrap_or(1440)),
      _ => None,
    };
    let n = self.lock().execute(
      "insert or ignore into categories (name, purpose, shape, cadence_minutes, gather_minutes, provider, model,
         created_at) values (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
      params![
        new.name,
        new.purpose,
        new.shape.as_str(),
        new.cadence_minutes.unwrap_or(60),
        gather,
        new.provider.clone().filter(|p| !p.is_empty()),
        new.model.clone().filter(|m| !m.is_empty()),
        now_ms()
      ],
    )?;
    if n == 0 {
      return Err(Refusal::Conflict(format!("a category {} already stands", new.name)));
    }
    self.category(&new.name)
  }

  pub fn change_category(&self, name: &str, change: &CategoryChange) -> Result<Category, Refusal> {
    let mut category = self.category(name)?;
    if let Some(purpose) = &change.purpose {
      category.purpose = purpose.clone();
    }
    if let Some(cadence) = change.cadence_minutes {
      category.cadence_minutes = cadence;
    }
    if let Some(gather) = change.gather_minutes {
      category.gather_minutes = Some(gather);
    }
    if let Some(provider) = &change.provider {
      category.provider = Some(provider.clone()).filter(|p| !p.is_empty());
    }
    if let Some(model) = &change.model {
      category.model = Some(model.clone()).filter(|m| !m.is_empty());
    }
    self.lock().execute(
      "update categories set purpose = ?2, cadence_minutes = ?3, gather_minutes = ?4, provider = ?5, model = ?6
         where name = ?1",
      params![
        name,
        category.purpose,
        category.cadence_minutes,
        category.gather_minutes,
        category.provider,
        category.model
      ],
    )?;
    self.category(name)
  }

  /// A category taken away with its recipients, where no source and no waiting, running or stuck item names
  /// it.
  pub fn remove_category(&self, name: &str) -> Result<(), Refusal> {
    self.category(name)?;
    let mut conn = self.lock();
    let tx = conn.transaction()?;
    let named: i64 = tx.query_row(
      "select (select count(*) from sources where category = ?1)
            + (select count(*) from queue where category = ?1 and state in ('waiting', 'running', 'stuck'))",
      params![name],
      |r| r.get(0),
    )?;
    if named > 0 {
      return Err(Refusal::Conflict(format!("category {name} still has sources or items under way")));
    }
    tx.execute("delete from recipients where category = ?1", params![name])?;
    tx.execute("delete from categories where name = ?1", params![name])?;
    tx.commit()?;
    Ok(())
  }

  /// A change of state, for the category named or for every one.
  pub fn set_running(&self, category: Option<&str>, state: Running) -> Result<(), Refusal> {
    if let Some(name) = category {
      self.category(name)?;
    }
    self.lock().execute(
      "update categories set state = ?2, state_at = ?3 where ?1 is null or name = ?1",
      params![category, state.as_str(), now_ms()],
    )?;
    Ok(())
  }

  pub fn daemon_state(&self) -> Result<DaemonState, String> {
    let categories = self.categories()?;
    let conn = self.lock();
    let count = |category: &str, state: &str| -> Result<i64, String> {
      conn
        .query_row(
          "select count(*) from queue where category = ?1 and state = ?2",
          params![category, state],
          |r| r.get(0),
        )
        .map_err(|e| e.to_string())
    };
    let mut states = Vec::new();
    for c in &categories {
      states.push(CategoryState {
        category_name: c.name.clone(),
        state: c.state,
        waiting_count: count(&c.name, "waiting")?,
        running_count: count(&c.name, "running")?,
        finishing_count: count(&c.name, "finishing")?,
        stuck_count: count(&c.name, "stuck")?,
      });
    }
    let stopped = !categories.is_empty() && categories.iter().all(|c| c.state == Running::Stopped);
    Ok(DaemonState {
      state: if stopped { Running::Stopped } else { Running::Running },
      changed_at: categories.iter().filter_map(|c| c.state_at).max(),
      categories: states,
      holds: Vec::new(),
    })
  }

  // The recipients.

  pub fn recipients(&self, category: &str) -> Result<Vec<Recipient>, String> {
    let conn = self.lock();
    let mut stmt = conn
      .prepare(&format!(
        "select {RECIPIENT_COLUMNS} from recipients where category = ?1
         order by case origin when 'owner' then 0 else 1 end, id"
      ))
      .map_err(|e| e.to_string())?;
    stmt
      .query_map(params![category], read_recipient)
      .map_err(|e| e.to_string())?
      .collect::<Result<Vec<_>, _>>()
      .map_err(|e| e.to_string())
  }

  pub fn add_recipient(
    &self,
    category: &str,
    kind: RecipientKind,
    room_id: Option<i64>,
    agent_name: Option<&str>,
    origin: &str,
  ) -> Result<Recipient, Refusal> {
    self.category(category)?;
    let conn = self.lock();
    let n = conn.execute(
      "insert or ignore into recipients (category, kind, room_id, agent_name, origin, added_at)
       values (?1, ?2, ?3, ?4, ?5, ?6)",
      params![category, kind.as_str(), room_id, agent_name, origin, now_ms()],
    )?;
    if n == 0 {
      return Err(Refusal::Conflict(format!("category {category} already holds this recipient")));
    }
    let id = conn.last_insert_rowid();
    conn
      .query_row(
        &format!("select {RECIPIENT_COLUMNS} from recipients where id = ?1"),
        params![id],
        read_recipient,
      )
      .map_err(Refusal::from)
  }

  pub fn remove_recipient(&self, category: &str, id: i64) -> Result<(), Refusal> {
    let n = self
      .lock()
      .execute("delete from recipients where id = ?1 and category = ?2", params![id, category])?;
    if n == 0 {
      Err(Refusal::NotFound(format!("no recipient {id} of {category}")))
    } else {
      Ok(())
    }
  }

  /// The categories holding no recipient, which the owner's room is given as the daemon starts.
  pub fn categories_without_recipients(&self) -> Result<Vec<String>, String> {
    let conn = self.lock();
    let mut stmt = conn
      .prepare(
        "select name from categories where not exists (select 1 from recipients where category = name)
         order by name",
      )
      .map_err(|e| e.to_string())?;
    stmt
      .query_map([], |r| r.get(0))
      .map_err(|e| e.to_string())?
      .collect::<Result<Vec<_>, _>>()
      .map_err(|e| e.to_string())
  }

  // The sources.

  pub fn add_source(&self, new: &NewSource, address: &str, limit: i64) -> Result<Source, Refusal> {
    self.category(&new.category_name)?;
    let now = now_ms();
    let filter = new.filter.clone().unwrap_or_default();
    let conn = self.lock();
    conn.execute(
      "insert into sources (category, kind, address, query, require, exclude, every_minutes, item_limit,
         created_at, changed_at, pattern, strategy) values (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?9, ?10, ?11)",
      params![
        new.category_name,
        new.kind,
        address,
        new.query,
        words_json(&filter.require),
        words_json(&filter.exclude),
        new.every_minutes,
        limit,
        now,
        new.pattern,
        new.strategy
      ],
    )?;
    let id = conn.last_insert_rowid();
    drop(conn);
    self.source(id)
  }

  pub fn source(&self, id: i64) -> Result<Source, Refusal> {
    self
      .lock()
      .query_row(&format!("select {SOURCE_COLUMNS} from sources where id = ?1"), params![id], read_source)
      .optional()?
      .ok_or_else(|| Refusal::NotFound(format!("no source {id}")))
  }

  pub fn sources(
    &self,
    category: Option<&str>,
    after: Option<i64>,
    limit: i64,
  ) -> Result<Page<Source>, String> {
    let conn = self.lock();
    let mut stmt = conn
      .prepare(&format!(
        "select {SOURCE_COLUMNS} from sources where id > ?1 and (?2 is null or category = ?2) order by id limit ?3"
      ))
      .map_err(|e| e.to_string())?;
    let rows = stmt
      .query_map(params![after.unwrap_or(i64::MIN), category, limit + 1], read_source)
      .map_err(|e| e.to_string())?
      .collect::<Result<Vec<_>, _>>()
      .map_err(|e| e.to_string())?;
    Ok(page(rows, limit))
  }

  /// Every source of a category, for the scan.
  pub fn category_sources(&self, category: &str) -> Result<Vec<Source>, String> {
    Ok(self.sources(Some(category), None, i64::MAX - 1)?.items)
  }

  pub fn change_source(&self, id: i64, change: &SourceChange) -> Result<Source, Refusal> {
    let mut source = self.source(id)?;
    if let Some(address) = &change.address {
      source.address = address.clone();
    }
    if let Some(query) = &change.query {
      source.query = Some(query.clone());
    }
    if let Some(pattern) = &change.pattern {
      source.pattern = Some(pattern.clone());
    }
    if let Some(strategy) = &change.strategy {
      source.strategy = Some(strategy.clone());
    }
    if let Some(filter) = &change.filter {
      source.filter = filter.clone();
    }
    if let Some(every) = change.every_minutes {
      source.every_minutes = Some(every);
    }
    if let Some(limit) = change.limit {
      source.limit = limit;
    }
    self.lock().execute(
      "update sources set address = ?2, query = ?3, require = ?4, exclude = ?5, every_minutes = ?6,
         item_limit = ?7, changed_at = ?8, pattern = ?9, strategy = ?10 where id = ?1",
      params![
        id,
        source.address,
        source.query,
        words_json(&source.filter.require),
        words_json(&source.filter.exclude),
        source.every_minutes,
        source.limit,
        now_ms(),
        source.pattern,
        source.strategy
      ],
    )?;
    self.source(id)
  }

  pub fn remove_source(&self, id: i64) -> Result<(), Refusal> {
    let n = self.lock().execute("delete from sources where id = ?1", params![id])?;
    if n == 0 { Err(Refusal::NotFound(format!("no source {id}"))) } else { Ok(()) }
  }

  /// A reading's end: the time it was read and the reading counted; what it turned up and kept, and the
  /// failure count set back, or its failure counted and what the last reading that succeeded left kept.
  pub fn source_read(&self, id: i64, reading: Result<Yield, &str>) -> Result<(), String> {
    let now = now_ms();
    let conn = self.lock();
    match reading {
      Ok(found) => conn.execute(
        "update sources set read_at = ?2, scan_count = scan_count + 1, last_yield = ?3, last_taken = ?4,
           failure_count = 0, last_error = null where id = ?1",
        params![id, now, found.candidates, found.taken],
      ),
      Err(why) => conn.execute(
        "update sources set read_at = ?2, scan_count = scan_count + 1, failure_count = failure_count + 1,
           last_error = ?3 where id = ?1",
        params![id, now, why],
      ),
    }
    .map_err(|e| e.to_string())?;
    Ok(())
  }

  // The candidates seen.

  pub fn is_seen(&self, key: &str) -> Result<bool, String> {
    self
      .lock()
      .query_row("select 1 from seen where key = ?1", params![key], |_| Ok(()))
      .optional()
      .map(|r| r.is_some())
      .map_err(|e| e.to_string())
  }

  /// Keeps a candidate as seen, `unqueued` or `archived`; a key already seen is left as it stands. Answers
  /// whether a row was made.
  pub fn keep_seen(
    &self,
    candidate: &Candidate,
    category: &str,
    source_id: Option<i64>,
    archived: bool,
  ) -> Result<bool, String> {
    let made = self
      .lock()
      .execute(
        "insert or ignore into seen (key, category, name, maker, address, kind, published_at, source_id, state,
           found_at) values (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)",
        params![
          candidate.key(),
          category,
          candidate.name,
          candidate.maker,
          candidate.address,
          candidate.kind.as_str(),
          candidate.published_at,
          source_id,
          if archived { "archived" } else { "unqueued" },
          now_ms()
        ],
      )
      .map_err(|e| e.to_string())?;
    Ok(made > 0)
  }

  /// The state a seen row stands in, where one stands for the key.
  pub fn seen_state(&self, key: &str) -> Result<Option<String>, String> {
    self
      .lock()
      .query_row("select state from seen where key = ?1", params![key], |r| r.get(0))
      .optional()
      .map_err(|e| e.to_string())
  }

  /// Every unqueued seen row of a category queued, oldest found first, the queue holding no cap; answers the
  /// items made.
  pub fn fill(&self, category: &Category) -> Result<Vec<i64>, String> {
    self.fill_with(category, None).map(|(made, _)| made)
  }

  /// The same, judged against the platform's age floor: an advisory whose fixed builds predate the oldest
  /// recorded component is marked `stale` and never queued, and its name is answered so the pass can say it.
  /// A floor that does not stand, or an advisory with no date, skips nothing.
  pub fn fill_with(
    &self,
    category: &Category,
    floor: Option<&crate::stack::AgeFloor>,
  ) -> Result<(Vec<i64>, Vec<String>), String> {
    let mut conn = self.lock();
    let tx = conn.transaction().map_err(|e| e.to_string())?;
    let made = take_unqueued(&tx, category, floor).map_err(|e| e.to_string())?;
    tx.commit().map_err(|e| e.to_string())?;
    Ok(made)
  }

  // The queue.

  /// An ask queued at once, above every item the scan took in, naming no words and no parent; refused where an
  /// item of its key still waits or runs.
  pub fn ask(
    &self,
    category: &str,
    key: &str,
    name: &str,
    address: &str,
    shape: Shape,
  ) -> Result<Item, Refusal> {
    self.ask_from(category, key, name, address, shape, &Asked::default())
  }

  /// The earlier research a follow-up follows from, read as it stands now: a named item's document, title (its
  /// name where it has none) and live link; a named document's from the item that filed it, else from the
  /// research run that filed it, else its number alone. An item that does not stand, or has filed no document
  /// yet, is refused.
  pub fn parent(&self, item: Option<i64>, document: Option<i64>) -> Result<Parent, Refusal> {
    let of_item = |item: Item| Parent {
      item_id: Some(item.id),
      document_id: item.document_id,
      title: item.title.clone().filter(|t| !t.trim().is_empty()).or(Some(item.name.clone())),
      share_url: item.share_url.clone(),
    };
    if let Some(id) = item {
      let item = self.item(id)?;
      if item.document_id.is_none() {
        return Err(Refusal::Conflict(format!("item {id} has filed no document yet")));
      }
      return Ok(of_item(item));
    }
    let Some(document) = document else {
      return Ok(Parent::default());
    };
    let filer: Option<i64> = self
      .lock()
      .query_row(
        "select id from queue where document_id = ?1 order by id desc limit 1",
        params![document],
        |r| r.get(0),
      )
      .optional()?;
    if let Some(id) = filer {
      return Ok(of_item(self.item(id)?));
    }
    let run: Option<(Option<String>, Option<String>)> = self
      .lock()
      .query_row(
        "select title, link from research_runs where document_id = ?1 order by id desc limit 1",
        params![document],
        |r| Ok((r.get(0)?, r.get(1)?)),
      )
      .optional()?;
    let (title, share_url) = run.unwrap_or_default();
    Ok(Parent {
      item_id: None,
      document_id: Some(document),
      title: title.filter(|t| !t.trim().is_empty()),
      share_url,
    })
  }

  /// An ask queued at once, above every item the scan took in, with the words it was made with and, for a
  /// follow-up, the earlier research it follows from; refused where an item of its key still waits or runs.
  pub fn ask_from(
    &self,
    category: &str,
    key: &str,
    name: &str,
    address: &str,
    shape: Shape,
    asked: &Asked,
  ) -> Result<Item, Refusal> {
    let now = now_ms();
    let parent = asked.parent.clone().unwrap_or_default();
    let origin = if asked.parent.is_some() { OriginKind::FollowUp } else { OriginKind::Asked };
    let mut conn = self.lock();
    let tx = conn.transaction()?;
    let standing: Option<i64> = tx
      .query_row(
        "select id from queue where key = ?1 and state in ('waiting', 'running', 'stuck') limit 1",
        params![key],
        |r| r.get(0),
      )
      .optional()?;
    if let Some(id) = standing {
      return Err(Refusal::Conflict(format!("item {id} already stands for {key}")));
    }
    tx.execute(
      "insert into queue (category, key, name, address, kind, shape, asked, priority, state, found_at,
         created_at, changed_at, origin, because, parent_item_id, parent_document_id, parent_title,
         parent_share_url)
       values (?1, ?2, ?3, ?4, 'asked', ?5, 1, 1, 'waiting', ?6, ?6, ?6, ?7, ?8, ?9, ?10, ?11, ?12)",
      params![
        category,
        key,
        name,
        address,
        shape.as_str(),
        now,
        origin.as_str(),
        asked.because,
        parent.item_id,
        parent.document_id,
        parent.title,
        parent.share_url
      ],
    )?;
    let id = tx.last_insert_rowid();
    tx.execute(
      "insert into seen (key, category, name, address, kind, state, item_id, found_at)
       values (?1, ?2, ?3, ?4, 'asked', 'queued', ?5, ?6)
       on conflict (key) do update set state = 'queued', item_id = ?5",
      params![key, category, name, address, id, now],
    )?;
    tx.commit()?;
    drop(conn);
    self.item(id)
  }

  pub fn item(&self, id: i64) -> Result<Item, Refusal> {
    self
      .lock()
      .query_row(&format!("select {ITEM_COLUMNS} from queue where id = ?1"), params![id], read_item)
      .optional()?
      .ok_or_else(|| Refusal::NotFound(format!("no item {id}")))
  }

  /// The items in the queue's order: the running ones, the waiting and stuck ones in their category's order,
  /// then the rest newest first; narrowed to a category and to `states` where named, continued past `after`.
  pub fn queue(
    &self,
    category: Option<&str>,
    states: &[ItemState],
    after: Option<i64>,
    limit: i64,
  ) -> Result<Page<Item>, String> {
    let conn = self.lock();
    let mut stmt = conn
      .prepare(&format!(
        "select {ITEM_COLUMNS} from queue where ?1 is null or category = ?1 order by
           case state when 'running' then 0 when 'waiting' then 1 when 'stuck' then 1 else 2 end,
           case when state in ('running', 'waiting', 'stuck') then -priority else 0 end,
           case when state in ('running', 'waiting', 'stuck') then found_at else -id end,
           id"
      ))
      .map_err(|e| e.to_string())?;
    let all = stmt
      .query_map(params![category], read_item)
      .map_err(|e| e.to_string())?
      .collect::<Result<Vec<_>, _>>()
      .map_err(|e| e.to_string())?;
    let named: Vec<Item> = all
      .into_iter()
      .filter(|item| states.is_empty() || states.contains(&item.state))
      .collect();
    let start = match after {
      Some(after) => named.iter().position(|item| item.id == after).map(|at| at + 1).unwrap_or(named.len()),
      None => 0,
    };
    Ok(page(named.into_iter().skip(start).collect(), limit))
  }

  /// A category's waiting and stuck items whose retry is due, in its order, at most `limit`; a limit below 0
  /// holds every one.
  pub fn waiting(&self, category: &str, now: i64, limit: i64) -> Result<Vec<Item>, String> {
    let conn = self.lock();
    let mut stmt = conn
      .prepare(&format!(
        "select {ITEM_COLUMNS} from queue where category = ?1 and state in ('waiting', 'stuck')
           and (retry_at is null or retry_at <= ?2)
         order by priority desc, found_at, id limit ?3"
      ))
      .map_err(|e| e.to_string())?;
    stmt
      .query_map(params![category, now, limit], read_item)
      .map_err(|e| e.to_string())?
      .collect::<Result<Vec<_>, _>>()
      .map_err(|e| e.to_string())
  }

  /// A category's reports: the items that filed a document and were reported, the whole history an
  /// investigation's brief picks the nearest of (`docs/api.md`, 「The questions」), newest first.
  pub fn reported(&self, category: &str) -> Result<Vec<Item>, String> {
    let conn = self.lock();
    let mut stmt = conn
      .prepare(&format!(
        "select {ITEM_COLUMNS} from queue where category = ?1 and document_id is not null
           and state in ('reported', 'fit', 'taken')
         order by id desc"
      ))
      .map_err(|e| e.to_string())?;
    stmt
      .query_map(params![category], read_item)
      .map_err(|e| e.to_string())?
      .collect::<Result<Vec<_>, _>>()
      .map_err(|e| e.to_string())
  }

  /// When a category's latest try was asked, or none.
  pub fn last_try_at(&self, category: &str) -> Result<Option<i64>, String> {
    self
      .lock()
      .query_row("select max(started_at) from runs where category = ?1", params![category], |r| r.get(0))
      .map_err(|e| e.to_string())
  }

  /// When the latest of a category's tries that filed its document was asked, or none.
  pub fn last_filed_at(&self, category: &str) -> Result<Option<i64>, String> {
    self
      .lock()
      .query_row(
        "select max(started_at) from runs where category = ?1 and state = 'closed'",
        params![category],
        |r| r.get(0),
      )
      .map_err(|e| e.to_string())
  }

  /// Whether a category holds a waiting or stuck item whose retry is still to come.
  pub fn retry_ahead(&self, category: &str, now: i64) -> Result<bool, String> {
    self
      .lock()
      .query_row(
        "select exists (select 1 from queue where category = ?1 and state in ('waiting', 'stuck')
           and retry_at is not null and retry_at > ?2)",
        params![category, now],
        |r| r.get(0),
      )
      .map_err(|e| e.to_string())
  }

  /// How many of a category's waiting and stuck items have their retry due.
  pub fn due_count(&self, category: &str, now: i64) -> Result<i64, String> {
    self
      .lock()
      .query_row(
        "select count(*) from queue where category = ?1 and state in ('waiting', 'stuck')
           and (retry_at is null or retry_at <= ?2)",
        params![category, now],
        |r| r.get(0),
      )
      .map_err(|e| e.to_string())
  }

  /// How many tries are running: each one a place took, its run holding the place's slot.
  pub fn running_count(&self) -> Result<i64, String> {
    self
      .lock()
      .query_row("select count(*) from runs where state = 'running'", [], |r| r.get(0))
      .map_err(|e| e.to_string())
  }

  /// Waiting or stuck items taken for one try: each running with its tries counted, and a run row opened
  /// carrying them all. Answers the run's id and the try's number, or none where the first item no longer
  /// waits.
  pub fn start_try(&self, category: &str, items: &[i64]) -> Result<Option<(i64, i64)>, String> {
    let Some(&lead) = items.first() else {
      return Ok(None);
    };
    let now = now_ms();
    let mut conn = self.lock();
    let tx = conn.transaction().map_err(|e| e.to_string())?;
    let mut taken = Vec::new();
    for id in items {
      let n = tx
        .execute(
          "update queue set state = 'running', tries = tries + 1, retry_at = null, run_id = null, error = null,
             error_at = null, changed_at = ?2 where id = ?1 and state in ('waiting', 'stuck')",
          params![id, now],
        )
        .map_err(|e| e.to_string())?;
      if n > 0 {
        taken.push(*id);
      }
    }
    if !taken.contains(&lead) {
      return Ok(None);
    }
    let tries: i64 = tx
      .query_row("select tries from queue where id = ?1", params![lead], |r| r.get(0))
      .map_err(|e| e.to_string())?;
    tx.execute(
      "insert into runs (category, item_id, item_ids, try, state, started_at) values (?1, ?2, ?3, ?4, 'running', ?5)",
      params![category, lead, ids_json(&taken), tries, now],
    )
    .map_err(|e| e.to_string())?;
    let run = tx.last_insert_rowid();
    tx.commit().map_err(|e| e.to_string())?;
    Ok(Some((run, tries)))
  }

  /// The ledger run a try's ask was answered with, kept on the try and on its items.
  pub fn keep_run_id(&self, run: i64, run_id: i64) -> Result<(), String> {
    let items = self.run(run).map_err(|e| e.to_string())?.item_ids;
    let conn = self.lock();
    conn
      .execute("update runs set run_id = ?2 where id = ?1", params![run, run_id])
      .map_err(|e| e.to_string())?;
    for id in items {
      conn
        .execute(
          "update queue set run_id = ?2, changed_at = ?3 where id = ?1",
          params![id, run_id, now_ms()],
        )
        .map_err(|e| e.to_string())?;
    }
    Ok(())
  }

  /// The research run the last try before `try_id` that asked one asked for the same lead item, or none.
  pub fn earlier_research_run(&self, item_id: i64, try_id: i64) -> Result<Option<i64>, String> {
    self
      .lock()
      .query_row(
        "select run_id from runs where item_id = ?1 and id < ?2 and run_id is not null order by id desc limit 1",
        params![item_id, try_id],
        |r| r.get(0),
      )
      .optional()
      .map_err(|e| e.to_string())
  }

  /// A try's end in its run row.
  pub fn end_run(
    &self,
    run: i64,
    state: &str,
    outcome: Option<&str>,
    error: Option<&str>,
  ) -> Result<(), String> {
    self
      .lock()
      .execute(
        "update runs set state = ?2, outcome = ?3, error = ?4, ended_at = ?5 where id = ?1",
        params![run, state, outcome, error, now_ms()],
      )
      .map_err(|e| e.to_string())?;
    Ok(())
  }

  /// A failed try: its items still running wait again until `retry_at`, `stuck` where its tries passed the
  /// retries. Answers the items it moved, so a dismissed one is left as it stands.
  pub fn fail_try(&self, run: i64, error: &str, retry_at: i64, stuck: bool) -> Result<Vec<i64>, String> {
    let items = self.run(run).map_err(|e| e.to_string())?.item_ids;
    let conn = self.lock();
    let mut moved = Vec::new();
    for id in items {
      let n = conn
        .execute(
          "update queue set state = ?2, retry_at = ?3, error = ?4, error_at = ?5, changed_at = ?5
           where id = ?1 and state = 'running'",
          params![id, if stuck { "stuck" } else { "waiting" }, retry_at, error, now_ms()],
        )
        .map_err(|e| e.to_string())?;
      if n > 0 {
        moved.push(id);
      }
    }
    Ok(moved)
  }

  /// A try that filed its document: the item `finishing` until its document's page is whole. Answers whether
  /// it was still running.
  pub fn file(&self, item_id: i64, filed: &Filed) -> Result<bool, String> {
    let n = self
      .lock()
      .execute(
        "update queue set state = 'finishing', document_id = ?2, title = ?3, description = ?4, share_url = ?5,
           fit = ?6, touches = ?7, plan = ?8, error = null, error_at = null, changed_at = ?9
         where id = ?1 and state = 'running'",
        params![
          item_id,
          filed.document_id,
          filed.title,
          filed.description,
          filed.share_url,
          filed.fit,
          filed.touches,
          filed.plan,
          now_ms()
        ],
      )
      .map_err(|e| e.to_string())?;
    Ok(n > 0)
  }

  /// A filed document's page opened (`docs/store.md`, `pages`): finished for `item_id`, or for the run told to
  /// its room that filed it where none is named; holding `languages` and no narration, lacking `pending`, and
  /// asked at `retry_at` where the daemon does not finish it first. A page opened again starts over, its tries
  /// kept.
  pub fn open_page(
    &self,
    document_id: i64,
    item_id: Option<i64>,
    languages: &[String],
    pending: &[String],
    retry_at: Option<i64>,
  ) -> Result<(), String> {
    self
      .lock()
      .execute(
        "insert into pages (document_id, item_id, languages, narrations, pending, pending_retry_at)
           values (?1, ?2, ?3, '[]', ?4, ?5)
         on conflict (document_id) do update set item_id = excluded.item_id, languages = excluded.languages,
           narrations = '[]', pending = excluded.pending, pending_retry_at = excluded.pending_retry_at",
        params![document_id, item_id, words_json(languages), words_json(pending), retry_at],
      )
      .map(|_| ())
      .map_err(|e| e.to_string())
  }

  /// What a document's page is about to be asked for — its Korean side, a narration, or both — counted, and
  /// its next try set at `retry_at`, so what the daemon never finished is asked again. Answers how it stands.
  pub fn claim_page(
    &self,
    document_id: i64,
    retry_at: i64,
    korean: bool,
    narration: bool,
  ) -> Result<PageState, String> {
    self
      .lock()
      .execute(
        "update pages set translation_tries = translation_tries + ?2, narration_tries = narration_tries + ?3,
           pending_retry_at = ?4 where document_id = ?1",
        params![document_id, korean as i64, narration as i64, retry_at],
      )
      .map_err(|e| e.to_string())?;
    self.page_state(document_id)
  }

  /// How a document's page stands: the languages and narrations it holds, what it was last found lacking, the
  /// tries asked of each, and when what it lacks is asked again; nothing held for a document with no page.
  pub fn page_state(&self, document_id: i64) -> Result<PageState, String> {
    self
      .lock()
      .query_row(
        "select languages, narrations, translation_tries, narration_tries, pending_retry_at, pending from pages
         where document_id = ?1",
        params![document_id],
        |r| {
          let languages: String = r.get(0)?;
          let narrations: String = r.get(1)?;
          let pending: String = r.get(5)?;
          Ok(PageState {
            languages: serde_json::from_str(&languages).unwrap_or_default(),
            narrations: serde_json::from_str(&narrations).unwrap_or_default(),
            pending: serde_json::from_str(&pending).unwrap_or_default(),
            translation_tries: r.get(2)?,
            narration_tries: r.get(3)?,
            retry_at: r.get(4)?,
          })
        },
      )
      .optional()
      .map(Option::unwrap_or_default)
      .map_err(|e| e.to_string())
  }

  /// A page's Korean side had: kept, and its document's page holding both languages.
  pub fn keep_rendering(&self, document_id: i64, rendering: &Rendering) -> Result<(), String> {
    let conn = self.lock();
    conn
      .execute(
        "insert or replace into renderings (document_id, title, description, body, created_at)
         values (?1, ?2, ?3, ?4, ?5)",
        params![document_id, rendering.title, rendering.description, rendering.body, now_ms()],
      )
      .map_err(|e| e.to_string())?;
    conn
      .execute(
        "update pages set languages = ?2 where document_id = ?1",
        params![document_id, words_json(&["ko".to_string(), "en".to_string()])],
      )
      .map(|_| ())
      .map_err(|e| e.to_string())
  }

  /// A page's Korean side set aside (`docs/api.md`, 「The page」): its document's page holding `languages` `en`
  /// alone, `pending` naming `korean` among what it lacks, and its retry wait due at `retry_at`, so the Korean
  /// side is asked again as any pending part.
  pub fn set_aside_korean(&self, document_id: i64, retry_at: i64) -> Result<(), String> {
    let mut pending = self.page_state(document_id)?.pending;
    if !pending.iter().any(|lacks| lacks == "korean") {
      pending.insert(0, "korean".to_string());
    }
    self
      .lock()
      .execute(
        "update pages set languages = ?2, pending = ?3, pending_retry_at = ?4 where document_id = ?1",
        params![document_id, words_json(&["en".to_string()]), words_json(&pending), retry_at],
      )
      .map(|_| ())
      .map_err(|e| e.to_string())
  }

  /// A page's Korean side as it was kept, or none where it has none.
  pub fn rendering(&self, document_id: i64) -> Result<Option<Rendering>, String> {
    self
      .lock()
      .query_row(
        "select title, description, body from renderings where document_id = ?1",
        params![document_id],
        |r| {
          Ok(Rendering {
            title: r.get(0)?,
            description: r.get(1)?,
            body: r.get(2)?,
          })
        },
      )
      .optional()
      .map_err(|e| e.to_string())
  }

  /// A part of a page's Korean side as it was had, by its English text, or none where it was not.
  pub fn translated_part(&self, document_id: i64, english: &str) -> Result<Option<String>, String> {
    self
      .lock()
      .query_row(
        "select korean from translated_parts where document_id = ?1 and english = ?2",
        params![document_id, english],
        |r| r.get(0),
      )
      .optional()
      .map_err(|e| e.to_string())
  }

  /// A part of a page's Korean side had, kept by its English text.
  pub fn keep_translated_part(&self, document_id: i64, english: &str, korean: &str) -> Result<(), String> {
    self
      .lock()
      .execute(
        "insert or replace into translated_parts (document_id, english, korean) values (?1, ?2, ?3)",
        params![document_id, english, korean],
      )
      .map_err(|e| e.to_string())?;
    Ok(())
  }

  /// A text a page holds beside its run's result (`docs/store.md`, `page_texts`), or none.
  pub fn page_text(&self, document_id: i64, name: &str) -> Result<Option<String>, String> {
    self
      .lock()
      .query_row(
        "select text from page_texts where document_id = ?1 and name = ?2",
        params![document_id, name],
        |r| r.get(0),
      )
      .optional()
      .map_err(|e| e.to_string())
  }

  /// A text a page holds, kept in place of the one it held.
  pub fn keep_page_text(&self, document_id: i64, name: &str, text: &str) -> Result<(), String> {
    self
      .lock()
      .execute(
        "insert or replace into page_texts (document_id, name, text) values (?1, ?2, ?3)",
        params![document_id, name, text],
      )
      .map_err(|e| e.to_string())?;
    Ok(())
  }

  /// A text a page held, dropped.
  pub fn drop_page_text(&self, document_id: i64, name: &str) -> Result<(), String> {
    self
      .lock()
      .execute(
        "delete from page_texts where document_id = ?1 and name = ?2",
        params![document_id, name],
      )
      .map_err(|e| e.to_string())?;
    Ok(())
  }

  /// A page's one review, or none where it was not had.
  pub fn review(&self, document_id: i64) -> Result<Option<Review>, String> {
    self
      .lock()
      .query_row(
        "select answer, applied, remade from page_reviews where document_id = ?1",
        params![document_id],
        |r| {
          let applied: String = r.get(1)?;
          let remade: String = r.get(2)?;
          Ok(Review {
            answer: r.get(0)?,
            applied: serde_json::from_str(&applied).unwrap_or_default(),
            remade: serde_json::from_str(&remade).unwrap_or_default(),
          })
        },
      )
      .optional()
      .map_err(|e| e.to_string())
  }

  /// A page's review kept as it answered, none of its fixes applied; a review kept before stands.
  pub fn keep_review(&self, document_id: i64, answer: &str) -> Result<(), String> {
    self
      .lock()
      .execute(
        "insert or ignore into page_reviews (document_id, answer, reviewed_at) values (?1, ?2, ?3)",
        params![document_id, answer, now_ms()],
      )
      .map_err(|e| e.to_string())?;
    Ok(())
  }

  /// The fixes of a page's review applied or left out so far, and the narrations it had made again.
  pub fn keep_review_applied(
    &self,
    document_id: i64,
    applied: &[usize],
    remade: &[String],
  ) -> Result<(), String> {
    self
      .lock()
      .execute(
        "update page_reviews set applied = ?2, remade = ?3 where document_id = ?1",
        params![document_id, serde_json::to_string(applied).unwrap_or_default(), words_json(remade)],
      )
      .map_err(|e| e.to_string())?;
    Ok(())
  }

  /// The narrations a document's page plays.
  pub fn keep_narrations(&self, document_id: i64, narrations: &[String]) -> Result<(), String> {
    self
      .lock()
      .execute(
        "update pages set narrations = ?2 where document_id = ?1",
        params![document_id, words_json(narrations)],
      )
      .map(|_| ())
      .map_err(|e| e.to_string())
  }

  /// The synthesis kept for a page's narration in `language` and how many characters it was asked to speak,
  /// or none where none is on its way.
  pub fn synthesis(&self, document_id: i64, language: &str) -> Result<Option<(i64, i64)>, String> {
    self
      .lock()
      .query_row(
        "select synthesis_id, characters from syntheses where document_id = ?1 and language = ?2",
        params![document_id, language],
        |r| Ok((r.get(0)?, r.get(1)?)),
      )
      .optional()
      .map_err(|e| e.to_string())
  }

  /// The synthesis just asked for a page's narration in `language`, of a text of `characters`, kept until
  /// its sound is attached.
  pub fn keep_synthesis(
    &self,
    document_id: i64,
    language: &str,
    synthesis_id: i64,
    characters: i64,
  ) -> Result<(), String> {
    self
      .lock()
      .execute(
        "insert or replace into syntheses (document_id, language, synthesis_id, characters, asked_at)
         values (?1, ?2, ?3, ?4, ?5)",
        params![document_id, language, synthesis_id, characters, now_ms()],
      )
      .map(|_| ())
      .map_err(|e| e.to_string())
  }

  /// A page's narration in `language` no longer on its way: its sound attached, or its synthesis ended
  /// without one.
  pub fn forget_synthesis(&self, document_id: i64, language: &str) -> Result<(), String> {
    self
      .lock()
      .execute(
        "delete from syntheses where document_id = ?1 and language = ?2",
        params![document_id, language],
      )
      .map(|_| ())
      .map_err(|e| e.to_string())
  }

  /// How often a page's narration in `language` was asked anew, after a synthesis that ended without its sound
  /// or a review that rewrote it; none where it never was.
  pub fn narration_renewed(&self, document_id: i64, language: &str) -> Result<i64, String> {
    self
      .lock()
      .query_row(
        "select renewed from narration_asks where document_id = ?1 and language = ?2",
        params![document_id, language],
        |r| r.get(0),
      )
      .optional()
      .map(Option::unwrap_or_default)
      .map_err(|e| e.to_string())
  }

  /// A page's narration in `language` asked anew: its count bumped, so its next synthesis is a new one.
  pub fn renew_narration(&self, document_id: i64, language: &str) -> Result<(), String> {
    self
      .lock()
      .execute(
        "insert into narration_asks (document_id, language, renewed) values (?1, ?2, 1)
           on conflict (document_id, language) do update set renewed = renewed + 1",
        params![document_id, language],
      )
      .map(|_| ())
      .map_err(|e| e.to_string())
  }

  /// A page whose document the archive no longer holds closed as failed (`docs/runner.md`, 「The page」): lacking
  /// nothing and never asked again, every item its document was filed for keeping `error`, and one still
  /// `finishing` waiting again until `retry_at`. Answers the items that wait again.
  pub fn close_gone_page(&self, document_id: i64, error: &str, retry_at: i64) -> Result<Vec<i64>, String> {
    let mut conn = self.lock();
    let tx = conn.transaction().map_err(|e| e.to_string())?;
    let now = now_ms();
    tx.execute(
      "insert into pages (document_id, gone_at) values (?1, ?2)
       on conflict (document_id) do update set gone_at = excluded.gone_at, pending = '[]',
         pending_retry_at = null",
      params![document_id, now],
    )
    .map_err(|e| e.to_string())?;
    let waiting: Vec<i64> = {
      let mut stmt = tx
        .prepare("select id from queue where document_id = ?1 and state = 'finishing' order by id")
        .map_err(|e| e.to_string())?;
      stmt
        .query_map(params![document_id], |r| r.get(0))
        .map_err(|e| e.to_string())?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|e| e.to_string())?
    };
    tx.execute(
      "update queue set state = case when state = 'finishing' then 'waiting' else state end,
         retry_at = case when state = 'finishing' then ?3 else retry_at end,
         error = ?2, error_at = ?4, changed_at = ?4
       where document_id = ?1",
      params![document_id, error, retry_at, now],
    )
    .map_err(|e| e.to_string())?;
    tx.commit().map_err(|e| e.to_string())?;
    Ok(waiting)
  }

  /// What a document's page still lacks, and when it is asked again, or none where it lacks nothing.
  pub fn pend_page(
    &self,
    document_id: i64,
    pending: &[String],
    retry_at: Option<i64>,
  ) -> Result<(), String> {
    self
      .lock()
      .execute(
        "update pages set pending = ?2, pending_retry_at = ?3 where document_id = ?1",
        params![document_id, words_json(pending), retry_at],
      )
      .map(|_| ())
      .map_err(|e| e.to_string())
  }

  /// A whole page's report about to be told: every item its document was filed for that stands
  /// `finishing` is `reported`, or `fit` where its document asks the owner's word. Answers whether `item_id`
  /// was among them, so a report is told once and a dismissed item's not at all.
  pub fn finished(&self, item_id: i64, document_id: i64) -> Result<bool, String> {
    let mut conn = self.lock();
    let tx = conn.transaction().map_err(|e| e.to_string())?;
    let finishing: bool = tx
      .query_row(
        "select exists (select 1 from queue where id = ?1 and state = 'finishing')",
        params![item_id],
        |r| r.get(0),
      )
      .map_err(|e| e.to_string())?;
    tx.execute(
      "update queue set state = case when fit then 'fit' else 'reported' end, changed_at = ?2
       where document_id = ?1 and state = 'finishing'",
      params![document_id, now_ms()],
    )
    .map_err(|e| e.to_string())?;
    tx.commit().map_err(|e| e.to_string())?;
    Ok(finishing)
  }

  /// The items whose document's page has a pending part due and is finished for them, while they stand
  /// finishing, reported, fit or taken.
  pub fn pages_due(&self, now: i64) -> Result<Vec<Item>, String> {
    let conn = self.lock();
    let mut stmt = conn
      .prepare(&format!(
        "select {ITEM_COLUMNS} from queue where state in ('finishing', 'reported', 'fit', 'taken')
           and exists (select 1 from pages where pages.document_id = queue.document_id
             and pages.item_id = queue.id and pages.gone_at is null
             and pages.pending_retry_at is not null and pages.pending_retry_at <= ?1)
           order by (select pages.pending_retry_at from pages where pages.document_id = queue.document_id), id"
      ))
      .map_err(|e| e.to_string())?;
    stmt
      .query_map(params![now], read_item)
      .map_err(|e| e.to_string())?
      .collect::<Result<Vec<_>, _>>()
      .map_err(|e| e.to_string())
  }

  /// The runs told to their room whose document's page has a pending part due (`docs/runner.md`, 「The
  /// narration's sound」), whatever their outcome.
  pub fn run_pages_due(&self, now: i64) -> Result<Vec<ResearchRunRow>, String> {
    let conn = self.lock();
    let mut stmt = conn
      .prepare(&format!(
        "select {RESEARCH_RUN_COLUMNS} from research_runs where tells = 1 and state = 'closed'
           and exists (select 1 from pages where pages.document_id = research_runs.document_id
             and pages.item_id is null and pages.gone_at is null
             and pages.pending_retry_at is not null and pages.pending_retry_at <= ?1)
           order by id"
      ))
      .map_err(|e| e.to_string())?;
    stmt
      .query_map(params![now], read_research_run)
      .map_err(|e| e.to_string())?
      .collect::<Result<Vec<_>, _>>()
      .map_err(|e| e.to_string())
  }

  /// Every item holding a page, for the item its document's run was asked for, in the order of the documents.
  pub fn filed_pages(&self) -> Result<Vec<Item>, String> {
    let conn = self.lock();
    let mut stmt = conn
      .prepare(&format!(
        "select {ITEM_COLUMNS} from queue where document_id is not null and run_id is not null
           and exists (select 1 from runs where runs.run_id = queue.run_id and runs.item_id = queue.id)
           and not exists (select 1 from pages where pages.document_id = queue.document_id
             and pages.gone_at is not null)
           order by document_id, id"
      ))
      .map_err(|e| e.to_string())?;
    stmt
      .query_map([], read_item)
      .map_err(|e| e.to_string())?
      .collect::<Result<Vec<_>, _>>()
      .map_err(|e| e.to_string())
  }

  /// The tries left running, for a daemon taking them up again.
  pub fn open_runs(&self) -> Result<Vec<Run>, String> {
    let conn = self.lock();
    let mut stmt = conn
      .prepare(&format!("select {RUN_COLUMNS} from runs where state = 'running' order by id"))
      .map_err(|e| e.to_string())?;
    stmt
      .query_map([], read_run)
      .map_err(|e| e.to_string())?
      .collect::<Result<Vec<_>, _>>()
      .map_err(|e| e.to_string())
  }

  /// A try that is not counted — an interrupted one, or one a stopped daemon never asked: ended in the state
  /// given, and its items put back as they stood, to be asked at once with the try taken off their count:
  /// `stuck` where their tries still pass `retries`, `waiting` otherwise.
  pub fn requeue(
    &self,
    run: i64,
    state: &str,
    outcome: Option<&str>,
    error: &str,
    retries: i64,
  ) -> Result<(), String> {
    let items = self.run(run).map_err(|e| e.to_string())?.item_ids;
    self.end_run(run, state, outcome, Some(error))?;
    let conn = self.lock();
    for id in items {
      conn
        .execute(
          "update queue set state = case when max(tries - 1, 0) > ?4 then 'stuck' else 'waiting' end,
             tries = max(tries - 1, 0), retry_at = null, error = ?3, error_at = ?2, changed_at = ?2
           where id = ?1 and state = 'running'",
          params![id, now_ms(), error, retries],
        )
        .map_err(|e| e.to_string())?;
    }
    Ok(())
  }

  /// A stuck item asked again once its cause is fixed: waiting, its tries counted from none, with no retry
  /// wait and no repair item.
  pub fn ask_again(&self, id: i64) -> Result<Item, Refusal> {
    let item = self.item(id)?;
    if item.state != ItemState::Stuck {
      return Err(Refusal::Conflict(format!("item {id} is {}, not stuck", item.state.as_str())));
    }
    self.lock().execute(
      "update queue set state = 'waiting', tries = 0, retry_at = null, repair_item_id = null, changed_at = ?2
       where id = ?1 and state = 'stuck'",
      params![id, now_ms()],
    )?;
    self.item(id)
  }

  /// The stuck items holding no repair yet, in the order they became stuck.
  pub fn unrepaired(&self) -> Result<Vec<Item>, String> {
    let conn = self.lock();
    let mut stmt = conn
      .prepare(&format!(
        "select {ITEM_COLUMNS} from queue where state = 'stuck' and repair_item_id is null
         order by changed_at, id"
      ))
      .map_err(|e| e.to_string())?;
    stmt
      .query_map([], read_item)
      .map_err(|e| e.to_string())?
      .collect::<Result<Vec<_>, _>>()
      .map_err(|e| e.to_string())
  }

  /// The repair filed for stuck items kept on each that is still stuck.
  pub fn keep_repair(&self, items: &[i64], job_item_id: i64) -> Result<(), String> {
    let conn = self.lock();
    for id in items {
      conn
        .execute(
          "update queue set repair_item_id = ?2, changed_at = ?3 where id = ?1 and state = 'stuck'",
          params![id, job_item_id, now_ms()],
        )
        .map_err(|e| e.to_string())?;
    }
    Ok(())
  }

  pub fn dismiss(&self, id: i64) -> Result<Item, Refusal> {
    let item = self.item(id)?;
    if matches!(item.state, ItemState::Taken | ItemState::Dismissed) {
      return Err(Refusal::Conflict(format!("item {id} is {} and is not dismissed", item.state.as_str())));
    }
    self.lock().execute(
      "update queue set state = 'dismissed', changed_at = ?2 where id = ?1",
      params![id, now_ms()],
    )?;
    self.item(id)
  }

  /// The job item the chat agent filed kept, and the item taken, where it stands reported or fit.
  pub fn take(&self, id: i64, job_item_id: i64) -> Result<Item, Refusal> {
    let item = self.item(id)?;
    if !matches!(item.state, ItemState::Reported | ItemState::Fit) {
      return Err(Refusal::Conflict(format!("item {id} is {}, not reported", item.state.as_str())));
    }
    self.lock().execute(
      "update queue set state = 'taken', job_item_id = ?2, changed_at = ?3 where id = ?1",
      params![id, job_item_id, now_ms()],
    )?;
    self.item(id)
  }

  /// A waiting or stuck item given a priority one above the highest any item holds, a stuck one's retry wait
  /// cleared with it.
  pub fn front(&self, id: i64) -> Result<Item, Refusal> {
    let item = self.item(id)?;
    if !matches!(item.state, ItemState::Waiting | ItemState::Stuck) {
      return Err(Refusal::Conflict(format!("item {id} is {}, not waiting", item.state.as_str())));
    }
    self.lock().execute(
      "update queue set priority = (select coalesce(max(priority), 0) + 1 from queue), changed_at = ?2,
         retry_at = case when state = 'stuck' then null else retry_at end
       where id = ?1",
      params![id, now_ms()],
    )?;
    self.item(id)
  }

  // The tries.

  pub fn runs(
    &self,
    item_id: Option<i64>,
    category: Option<&str>,
    before: Option<i64>,
    limit: i64,
  ) -> Result<Page<Run>, String> {
    let conn = self.lock();
    let mut stmt = conn
      .prepare(&format!(
        "select {RUN_COLUMNS} from runs where id < ?1
           and (?2 is null or exists (select 1 from json_each(runs.item_ids) where value = ?2))
           and (?3 is null or category = ?3)
         order by id desc limit ?4"
      ))
      .map_err(|e| e.to_string())?;
    let rows = stmt
      .query_map(params![before.unwrap_or(i64::MAX), item_id, category, limit + 1], read_run)
      .map_err(|e| e.to_string())?
      .collect::<Result<Vec<_>, _>>()
      .map_err(|e| e.to_string())?;
    Ok(page(rows, limit))
  }

  pub fn run(&self, id: i64) -> Result<Run, Refusal> {
    self
      .lock()
      .query_row(&format!("select {RUN_COLUMNS} from runs where id = ?1"), params![id], read_run)
      .optional()?
      .ok_or_else(|| Refusal::NotFound(format!("no run {id}")))
  }

  // The reports.

  pub fn keep_report(
    &self,
    item_id: i64,
    recipient: &str,
    kind: &str,
    instruction_id: Option<i64>,
    error: Option<&str>,
  ) -> Result<(), String> {
    self
      .lock()
      .execute(
        "insert into reports (item_id, recipient, kind, instruction_id, state, error, at)
         values (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
        params![
          item_id,
          recipient,
          kind,
          instruction_id,
          if error.is_none() { "sent" } else { "refused" },
          error,
          now_ms()
        ],
      )
      .map_err(|e| e.to_string())?;
    Ok(())
  }

  /// The reports kept for an item: the recipient, the kind and the state of each, in order.
  pub fn reports(&self, item_id: i64) -> Result<Vec<(String, String, String)>, String> {
    let conn = self.lock();
    let mut stmt = conn
      .prepare("select recipient, kind, state from reports where item_id = ?1 order by id")
      .map_err(|e| e.to_string())?;
    stmt
      .query_map(params![item_id], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)))
      .map_err(|e| e.to_string())?
      .collect::<Result<Vec<_>, _>>()
      .map_err(|e| e.to_string())
  }

  // The stack.

  pub fn stack(&self) -> Result<Vec<StackRow>, String> {
    let conn = self.lock();
    let mut stmt = conn
      .prepare("select id, component, version, place, repository, worker, added_at from stack order by component, id")
      .map_err(|e| e.to_string())?;
    stmt
      .query_map([], read_stack_row)
      .map_err(|e| e.to_string())?
      .collect::<Result<Vec<_>, _>>()
      .map_err(|e| e.to_string())
  }

  pub fn add_stack_row(&self, row: &NewStackRow) -> Result<StackRow, Refusal> {
    let conn = self.lock();
    conn.execute(
      "insert into stack (component, version, place, repository, worker, added_at) values (?1, ?2, ?3, ?4, ?5, ?6)",
      params![row.component, row.version, row.place, row.repository, row.worker, now_ms()],
    )?;
    let id = conn.last_insert_rowid();
    conn
      .query_row(
        "select id, component, version, place, repository, worker, added_at from stack where id = ?1",
        params![id],
        read_stack_row,
      )
      .map_err(Refusal::from)
  }

  pub fn remove_stack_row(&self, id: i64) -> Result<(), Refusal> {
    let n = self.lock().execute("delete from stack where id = ?1", params![id])?;
    if n == 0 { Err(Refusal::NotFound(format!("no stack row {id}"))) } else { Ok(()) }
  }

  // The facts.

  #[allow(clippy::too_many_arguments)]
  pub fn keep_fact(
    &self,
    name: &str,
    value: &str,
    unit: Option<&str>,
    as_of: Option<&str>,
    source_address: Option<&str>,
    run_id: Option<i64>,
    document_id: Option<i64>,
  ) -> Result<(), String> {
    self
      .lock()
      .execute(
        "insert into facts (name, value, unit, as_of, source_address, run_id, document_id, created_at)
         values (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
        params![name, value, unit, as_of, source_address, run_id, document_id, now_ms()],
      )
      .map_err(|e| e.to_string())?;
    Ok(())
  }

  pub fn facts(&self, name: Option<&str>, before: Option<i64>, limit: i64) -> Result<Page<Fact>, String> {
    let conn = self.lock();
    let mut stmt = conn
      .prepare(
        "select id, name, value, unit, as_of, source_address, run_id, document_id, created_at from facts
         where id < ?1 and (?2 is null or name = ?2) order by id desc limit ?3",
      )
      .map_err(|e| e.to_string())?;
    let rows = stmt
      .query_map(params![before.unwrap_or(i64::MAX), name, limit + 1], |r| {
        Ok(Fact {
          id: r.get(0)?,
          name: r.get(1)?,
          value: r.get(2)?,
          unit: r.get(3)?,
          as_of: r.get(4)?,
          source_address: r.get(5)?,
          run_id: r.get(6)?,
          document_id: r.get(7)?,
          created_at: r.get(8)?,
        })
      })
      .map_err(|e| e.to_string())?
      .collect::<Result<Vec<_>, _>>()
      .map_err(|e| e.to_string())?;
    Ok(page(rows, limit))
  }

  // The runner's research runs.

  /// A research run opened, waiting for its slot, with the record its sessions carry: its id.
  pub fn open_research_run(&self, new: &NewResearchRun<'_>) -> Result<i64, String> {
    let conn = self.lock();
    conn
      .execute(
        "insert into research_runs (key, line, query, asker, options, room_id, provider, model, state,
           started_at, branches_allowed, record, tells)
         values (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, 'waiting', ?9, ?10, '{}', ?11)",
        params![
          new.key,
          new.line,
          new.query,
          new.asker,
          new.options,
          new.room_id,
          new.provider,
          new.model,
          now_ms(),
          new.branches_allowed,
          new.tells
        ],
      )
      .map_err(|e| e.to_string())?;
    Ok(conn.last_insert_rowid())
  }

  /// The run's road and the record its sessions carry, once its slot is let in.
  pub fn keep_research_road(
    &self,
    id: i64,
    provider: &str,
    model: &str,
    record: &serde_json::Value,
  ) -> Result<(), String> {
    self
      .lock()
      .execute(
        "update research_runs set provider = ?2, model = ?3, record = ?4 where id = ?1",
        params![id, provider, model, record.to_string()],
      )
      .map_err(|e| e.to_string())?;
    Ok(())
  }

  pub fn research_run(&self, id: i64) -> Result<ResearchRunRow, Refusal> {
    self
      .lock()
      .query_row(
        &format!("select {RESEARCH_RUN_COLUMNS} from research_runs where id = ?1"),
        params![id],
        read_research_run,
      )
      .optional()?
      .ok_or_else(|| Refusal::NotFound(format!("no research run {id}")))
  }

  /// Every research run not yet closed.
  pub fn open_research_runs(&self) -> Result<Vec<ResearchRunRow>, String> {
    let conn = self.lock();
    let mut stmt = conn
      .prepare(&format!(
        "select {RESEARCH_RUN_COLUMNS} from research_runs where state != 'closed' order by id"
      ))
      .map_err(|e| e.to_string())?;
    stmt
      .query_map([], read_research_run)
      .map_err(|e| e.to_string())?
      .collect::<Result<Vec<_>, _>>()
      .map_err(|e| e.to_string())
  }

  /// A waiting run let in: running from now.
  pub fn research_running(&self, id: i64) -> Result<(), String> {
    self
      .lock()
      .execute(
        "update research_runs set state = 'running' where id = ?1 and state = 'waiting'",
        params![id],
      )
      .map_err(|e| e.to_string())?;
    Ok(())
  }

  /// A turn the run went out under, by its name, and the answer it wrote last where it wrote one.
  pub fn keep_research_turn(&self, id: i64, name: &str, written: Option<&str>) -> Result<(), String> {
    let mut conn = self.lock();
    let tx = conn.transaction().map_err(|e| e.to_string())?;
    let turns: String = tx
      .query_row("select turns from research_runs where id = ?1", params![id], |r| r.get(0))
      .map_err(|e| e.to_string())?;
    let mut turns: Vec<String> = serde_json::from_str(&turns).unwrap_or_default();
    turns.push(name.to_string());
    tx.execute(
      "update research_runs set turns = ?2, written = coalesce(?3, written) where id = ?1",
      params![id, serde_json::to_string(&turns).unwrap_or_default(), written],
    )
    .map_err(|e| e.to_string())?;
    tx.commit().map_err(|e| e.to_string())
  }

  /// A run kept as continuing from the result of `from`.
  pub fn keep_continues(&self, id: i64, from: i64) -> Result<(), String> {
    self
      .lock()
      .execute("update research_runs set continues = ?2 where id = ?1", params![id, from])
      .map_err(|e| e.to_string())?;
    Ok(())
  }

  /// A round the run asked counted as answered.
  pub fn keep_round_answered(&self, id: i64, round: &str) -> Result<(), String> {
    let mut conn = self.lock();
    let tx = conn.transaction().map_err(|e| e.to_string())?;
    let answered: String = tx
      .query_row("select answered from research_runs where id = ?1", params![id], |r| r.get(0))
      .map_err(|e| e.to_string())?;
    let mut answered: Vec<String> = serde_json::from_str(&answered).unwrap_or_default();
    answered.push(round.to_string());
    tx.execute(
      "update research_runs set answered = ?2 where id = ?1",
      params![id, serde_json::to_string(&answered).unwrap_or_default()],
    )
    .map_err(|e| e.to_string())?;
    tx.commit().map_err(|e| e.to_string())
  }

  /// What a run wrote, set as its result stands now.
  pub fn keep_research_written(&self, id: i64, written: &str) -> Result<(), String> {
    self
      .lock()
      .execute("update research_runs set written = ?2 where id = ?1", params![id, written])
      .map_err(|e| e.to_string())?;
    Ok(())
  }

  /// One more ask of a run's page's review counted: the count it now stands at, which the review's slot is
  /// asked under.
  pub fn count_review_ask(&self, id: i64) -> Result<i64, String> {
    self
      .lock()
      .query_row(
        "update research_runs set review_asks = review_asks + 1 where id = ?1 returning review_asks",
        params![id],
        |r| r.get(0),
      )
      .map_err(|e| e.to_string())
  }

  /// One branch counted against a running run's allowance: its index, or none where the allowance is spent.
  pub fn count_branch(&self, id: i64) -> Result<Option<i64>, String> {
    let conn = self.lock();
    let changed = conn
      .execute(
        "update research_runs set branches = branches + 1 where id = ?1 and branches < branches_allowed",
        params![id],
      )
      .map_err(|e| e.to_string())?;
    if changed == 0 {
      return Ok(None);
    }
    conn
      .query_row("select branches from research_runs where id = ?1", params![id], |r| r.get(0))
      .map(Some)
      .map_err(|e| e.to_string())
  }

  /// A run closed with its outcome, why it failed, and the document it filed; false where it was closed
  /// already.
  pub fn close_research_run(
    &self,
    id: i64,
    outcome: &str,
    error: Option<&str>,
    document: Option<&RunDocument>,
  ) -> Result<bool, String> {
    let changed = self
      .lock()
      .execute(
        "update research_runs set state = 'closed', outcome = ?2, error = ?3, finished_at = ?4,
           document_id = ?5, title = ?6, description = ?7, link = ?8
         where id = ?1 and state != 'closed'",
        params![
          id,
          outcome,
          error,
          now_ms(),
          document.map(|d| d.id),
          document.map(|d| d.title.as_str()),
          document.and_then(|d| d.description.as_deref()),
          document.map(|d| d.link.as_str())
        ],
      )
      .map_err(|e| e.to_string())?;
    Ok(changed > 0)
  }

  /// A line of a run's log.
  pub fn log_research(&self, run_id: i64, kind: &str, text: &str) -> Result<(), String> {
    self
      .lock()
      .execute(
        "insert into research_log (run_id, at, kind, text) values (?1, ?2, ?3, ?4)",
        params![run_id, now_ms(), kind, text],
      )
      .map_err(|e| e.to_string())?;
    Ok(())
  }

  pub fn research_log(&self, run_id: i64) -> Result<Vec<ResearchLogLine>, String> {
    let conn = self.lock();
    let mut stmt = conn
      .prepare("select id, run_id, at, kind, text from research_log where run_id = ?1 order by id")
      .map_err(|e| e.to_string())?;
    stmt
      .query_map(params![run_id], |r| {
        Ok(ResearchLogLine {
          id: r.get(0)?,
          run_id: r.get(1)?,
          at: r.get(2)?,
          kind: r.get(3)?,
          text: r.get(4)?,
        })
      })
      .map_err(|e| e.to_string())?
      .collect::<Result<Vec<_>, _>>()
      .map_err(|e| e.to_string())
  }

  /// A slot or a session the run holds of agency, kept until it is given back.
  pub fn hold(&self, run_id: i64, kind: &str, value: &str) -> Result<(), String> {
    self
      .lock()
      .execute(
        "insert into research_holds (run_id, kind, value) values (?1, ?2, ?3)",
        params![run_id, kind, value],
      )
      .map_err(|e| e.to_string())?;
    Ok(())
  }

  /// A hold given back.
  pub fn let_go(&self, run_id: i64, kind: &str, value: &str) -> Result<(), String> {
    self
      .lock()
      .execute(
        "delete from research_holds where run_id = ?1 and kind = ?2 and value = ?3",
        params![run_id, kind, value],
      )
      .map_err(|e| e.to_string())?;
    Ok(())
  }

  /// What closed runs still hold, each run, kind and value: a review's slot and session left at a restart.
  pub fn closed_holds(&self) -> Result<Vec<(i64, String, String)>, String> {
    let conn = self.lock();
    let mut stmt = conn
      .prepare(
        "select h.run_id, h.kind, h.value from research_holds h
           join research_runs r on r.id = h.run_id
         where r.state = 'closed' and h.kind in ('slot', 'session') order by h.id",
      )
      .map_err(|e| e.to_string())?;
    stmt
      .query_map([], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)))
      .map_err(|e| e.to_string())?
      .collect::<Result<Vec<_>, _>>()
      .map_err(|e| e.to_string())
  }

  /// What the notices to agents still hold, each item, kind and value, in the order it was taken: a notice's
  /// slot and session left at a restart.
  pub fn notice_holds(&self) -> Result<Vec<(i64, String, String)>, String> {
    let conn = self.lock();
    let mut stmt = conn
      .prepare(
        "select run_id, kind, value from research_holds
         where kind in ('notice-slot', 'notice-session') order by id",
      )
      .map_err(|e| e.to_string())?;
    stmt
      .query_map([], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)))
      .map_err(|e| e.to_string())?
      .collect::<Result<Vec<_>, _>>()
      .map_err(|e| e.to_string())
  }

  /// What a run still holds, each kind and value, in the order it was taken.
  pub fn holds(&self, run_id: i64) -> Result<Vec<(String, String)>, String> {
    let conn = self.lock();
    let mut stmt = conn
      .prepare(
        "select kind, value from research_holds where run_id = ?1 and kind in ('slot', 'session') order by id",
      )
      .map_err(|e| e.to_string())?;
    stmt
      .query_map(params![run_id], |r| Ok((r.get(0)?, r.get(1)?)))
      .map_err(|e| e.to_string())?
      .collect::<Result<Vec<_>, _>>()
      .map_err(|e| e.to_string())
  }

  /// A demand asked of the arbiter's master, kept until it is given back: for a try, or for a branch of the
  /// run `run_id` names within the demand `within` names.
  pub fn keep_demand(
    &self,
    id: i64,
    try_id: Option<i64>,
    run_id: Option<i64>,
    within: Option<i64>,
    service: &str,
  ) -> Result<(), String> {
    self
      .lock()
      .execute(
        "insert or replace into demands (id, try_id, run_id, within, service, asked_at)
         values (?1, ?2, ?3, ?4, ?5, ?6)",
        params![id, try_id, run_id, within, service, now_ms()],
      )
      .map_err(|e| e.to_string())?;
    Ok(())
  }

  /// The try a place's demand is held by, once the place has taken one.
  pub fn demand_tried(&self, id: i64, try_id: i64) -> Result<(), String> {
    self
      .lock()
      .execute("update demands set try_id = ?2 where id = ?1", params![id, try_id])
      .map_err(|e| e.to_string())?;
    Ok(())
  }

  /// The research run a try's demand stands for, once the try is asked of the runner.
  pub fn demand_ran(&self, id: i64, run_id: i64) -> Result<(), String> {
    self
      .lock()
      .execute("update demands set run_id = ?2 where id = ?1", params![id, run_id])
      .map_err(|e| e.to_string())?;
    Ok(())
  }

  /// The demand a try holds, or none.
  pub fn try_demand(&self, try_id: i64) -> Result<Option<i64>, String> {
    self
      .lock()
      .query_row("select id from demands where try_id = ?1 order by id desc", params![try_id], |r| {
        r.get(0)
      })
      .optional()
      .map_err(|e| e.to_string())
  }

  /// The demand a research run stands on, its try's, or none.
  pub fn run_demand(&self, run_id: i64) -> Result<Option<i64>, String> {
    self
      .lock()
      .query_row(
        "select id from demands where run_id = ?1 and try_id is not null order by id desc",
        params![run_id],
        |r| r.get(0),
      )
      .optional()
      .map_err(|e| e.to_string())
  }

  /// Every demand held, in the order they were asked.
  pub fn demands(&self) -> Result<Vec<i64>, String> {
    let conn = self.lock();
    let mut stmt = conn
      .prepare("select id from demands order by asked_at, id")
      .map_err(|e| e.to_string())?;
    stmt
      .query_map([], |r| r.get(0))
      .map_err(|e| e.to_string())?
      .collect::<Result<Vec<_>, _>>()
      .map_err(|e| e.to_string())
  }

  /// The demands left behind: a try's whose try is no longer running, a branch's whose run has closed.
  pub fn demands_left(&self) -> Result<Vec<i64>, String> {
    let conn = self.lock();
    let mut stmt = conn
      .prepare(
        "select d.id from demands d
           left join runs r on r.id = d.try_id
           left join research_runs rr on rr.id = d.run_id
         where (d.try_id is not null and (r.id is null or r.state != 'running'))
            or (d.try_id is null and d.run_id is not null and (rr.id is null or rr.state = 'closed'))
         order by d.asked_at, d.id",
      )
      .map_err(|e| e.to_string())?;
    stmt
      .query_map([], |r| r.get(0))
      .map_err(|e| e.to_string())?
      .collect::<Result<Vec<_>, _>>()
      .map_err(|e| e.to_string())
  }

  /// A demand given back.
  pub fn drop_demand(&self, id: i64) -> Result<(), String> {
    self
      .lock()
      .execute("delete from demands where id = ?1", params![id])
      .map_err(|e| e.to_string())?;
    Ok(())
  }

  /// The research settings the owner last set, as kept; none where he set none.
  pub fn kept_research_settings(&self) -> Result<Option<String>, String> {
    self
      .lock()
      .query_row("select value from research_settings where id = 1", [], |r| r.get(0))
      .optional()
      .map_err(|e| e.to_string())
  }

  pub fn keep_research_settings(&self, value: &str) -> Result<(), String> {
    self
      .lock()
      .execute(
        "insert into research_settings (id, value) values (1, ?1)
         on conflict (id) do update set value = excluded.value",
        params![value],
      )
      .map_err(|e| e.to_string())?;
    Ok(())
  }
}

fn read_research_run(r: &rusqlite::Row<'_>) -> rusqlite::Result<ResearchRunRow> {
  let state: String = r.get(9)?;
  let record: String = r.get(16)?;
  let turns: String = r.get(18)?;
  Ok(ResearchRunRow {
    id: r.get(0)?,
    key: r.get(1)?,
    line: r.get(2)?,
    query: r.get(3)?,
    asker: r.get(4)?,
    options: r.get(5)?,
    room_id: r.get(6)?,
    provider: r.get(7)?,
    model: r.get(8)?,
    state: ResearchRunState::parse(&state).unwrap_or(ResearchRunState::Closed),
    outcome: r.get(10)?,
    error: r.get(11)?,
    started_at: r.get(12)?,
    finished_at: r.get(13)?,
    branches_allowed: r.get(14)?,
    branches: r.get(15)?,
    record: serde_json::from_str(&record).unwrap_or_default(),
    tells: r.get(17)?,
    turns: serde_json::from_str(&turns).unwrap_or_default(),
    written: r.get(19)?,
    document_id: r.get(20)?,
    title: r.get(21)?,
    description: r.get(22)?,
    link: r.get(23)?,
    continues: r.get(24)?,
    answered: serde_json::from_str(&r.get::<_, String>(25)?).unwrap_or_default(),
    narrations: serde_json::from_str(&r.get::<_, String>(26)?).unwrap_or_default(),
    pending: serde_json::from_str(&r.get::<_, String>(27)?).unwrap_or_default(),
  })
}

/// Up to `free` of a category's unqueued seen rows made waiting items, oldest found first; an advisory older
/// than the age floor is marked `stale` instead and its name answered.
fn take_unqueued(
  tx: &Transaction<'_>,
  category: &Category,
  floor: Option<&crate::stack::AgeFloor>,
) -> rusqlite::Result<(Vec<i64>, Vec<String>)> {
  type Row = (String, String, Option<String>, Option<String>, String, Option<i64>, i64, Option<i64>);
  let rows: Vec<Row> = {
    let mut stmt = tx.prepare(
      "select key, name, maker, address, kind, source_id, found_at, published_at from seen
       where category = ?1 and state = 'unqueued' order by found_at, rowid",
    )?;
    stmt
      .query_map(params![category.name], |r| {
        Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?, r.get(4)?, r.get(5)?, r.get(6)?, r.get(7)?))
      })?
      .collect::<Result<Vec<_>, _>>()?
  };
  let now = now_ms();
  let mut made = Vec::new();
  let mut skipped = Vec::new();
  for (key, name, maker, address, kind, source_id, found_at, published_at) in rows {
    if category.shape == CategoryShape::Advisory && crate::stack::stale(published_at, floor) {
      tx.execute("update seen set state = 'stale' where key = ?1", params![key])?;
      skipped.push(name);
      continue;
    }
    let shape = item_shape(category.shape, ItemKind::parse(&kind));
    tx.execute(
      "insert into queue (category, key, name, maker, address, kind, shape, source_id, asked, priority, state,
         found_at, created_at, changed_at)
       values (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, 0, 0, 'waiting', ?9, ?10, ?10)",
      params![category.name, key, name, maker, address, kind, shape.as_str(), source_id, found_at, now],
    )?;
    let id = tx.last_insert_rowid();
    tx.execute("update seen set state = 'queued', item_id = ?2 where key = ?1", params![key, id])?;
    made.push(id);
  }
  Ok((made, skipped))
}

/// How often a write that finds the file locked by another process looks again whether the lock is free.
const LOCK_LOOK_EVERY: std::time::Duration = std::time::Duration::from_millis(5);

/// A write that finds the file locked waits for the lock however long its holder keeps it: SQLite asks again
/// for as long as this answers true (`docs/store.md`).
fn wait_for_the_lock(_asked: i32) -> bool {
  std::thread::sleep(LOCK_LOOK_EVERY);
  true
}

/// The store's schema brought to this binary's version, a step at a time; a newer store is refused.
fn migrate(conn: &Connection) -> Result<(), String> {
  let version: i64 = conn.query_row("pragma user_version", [], |r| r.get(0)).map_err(|e| e.to_string())?;
  if version > schema_version() {
    return Err(format!(
      "the store is at schema version {version}, newer than this binary's {}",
      schema_version()
    ));
  }
  for (at, step) in MIGRATIONS.iter().enumerate().skip(version as usize) {
    let next = at as i64 + 1;
    conn
      .execute_batch(&format!("begin; {step} pragma user_version = {next}; commit;"))
      .map_err(|e| format!("the schema's step to version {next}: {e}"))?;
  }
  Ok(())
}

fn page<T>(mut rows: Vec<T>, limit: i64) -> Page<T> {
  let more = rows.len() as i64 > limit;
  rows.truncate(limit.max(0) as usize);
  Page { items: rows, more }
}

fn words_json(words: &[String]) -> String {
  serde_json::to_string(words).unwrap_or_else(|_| "[]".into())
}

fn ids_json(ids: &[i64]) -> String {
  serde_json::to_string(ids).unwrap_or_else(|_| "[]".into())
}

fn read_category(r: &rusqlite::Row<'_>) -> rusqlite::Result<Category> {
  let shape: String = r.get(2)?;
  let state: String = r.get(7)?;
  Ok(Category {
    name: r.get(0)?,
    purpose: r.get(1)?,
    shape: CategoryShape::parse(&shape).unwrap_or(CategoryShape::ManualOrInvestigation),
    cadence_minutes: r.get(3)?,
    gather_minutes: r.get(4)?,
    provider: r.get(5)?,
    model: r.get(6)?,
    state: Running::parse(&state).unwrap_or(Running::Running),
    state_at: r.get(8)?,
    created_at: r.get(9)?,
  })
}

fn read_recipient(r: &rusqlite::Row<'_>) -> rusqlite::Result<Recipient> {
  let kind: String = r.get(2)?;
  Ok(Recipient {
    id: r.get(0)?,
    category_name: r.get(1)?,
    kind: if kind == "agent" { RecipientKind::Agent } else { RecipientKind::Room },
    room_id: r.get(3)?,
    agent_name: r.get(4)?,
    origin: r.get(5)?,
    added_at: r.get(6)?,
  })
}

fn read_source(r: &rusqlite::Row<'_>) -> rusqlite::Result<Source> {
  let words = |i: usize| -> rusqlite::Result<Vec<String>> {
    Ok(serde_json::from_str(&r.get::<_, String>(i)?).unwrap_or_default())
  };
  Ok(Source {
    id: r.get(0)?,
    category_name: r.get(1)?,
    kind: r.get(2)?,
    address: r.get(3)?,
    query: r.get(4)?,
    pattern: r.get(17)?,
    strategy: r.get(18)?,
    filter: Filter {
      require: words(5)?,
      exclude: words(6)?,
    },
    every_minutes: r.get(7)?,
    limit: r.get(8)?,
    failure_count: r.get(9)?,
    last_error: r.get(10)?,
    read_at: r.get(11)?,
    created_at: r.get(12)?,
    changed_at: r.get(13)?,
    scan_count: r.get(14)?,
    last_yield: r.get(15)?,
    last_taken: r.get(16)?,
  })
}

fn read_item(r: &rusqlite::Row<'_>) -> rusqlite::Result<Item> {
  let shape: String = r.get(7)?;
  let state: String = r.get(11)?;
  let languages: String = r.get(27)?;
  let narrations: String = r.get(28)?;
  let pending: String = r.get(29)?;
  let origin: String = r.get(31)?;
  Ok(Item {
    id: r.get(0)?,
    category_name: r.get(1)?,
    key: r.get(2)?,
    name: r.get(3)?,
    maker: r.get(4)?,
    address: r.get(5)?,
    kind: r.get(6)?,
    shape: Shape::parse(&shape).unwrap_or(Shape::Investigation),
    source_id: r.get(8)?,
    asked: r.get(9)?,
    priority: r.get(10)?,
    state: ItemState::parse(&state).unwrap_or(ItemState::Stuck),
    tries: r.get(12)?,
    retry_at: r.get(13)?,
    run_id: r.get(14)?,
    document_id: r.get(15)?,
    title: r.get(16)?,
    description: r.get(17)?,
    share_url: r.get(18)?,
    fit: r.get(19)?,
    touches: r.get(20)?,
    plan: r.get(21)?,
    job_item_id: r.get(22)?,
    repair_item_id: r.get(38)?,
    error: r.get(23)?,
    error_at: r.get(39)?,
    found_at: r.get(24)?,
    created_at: r.get(25)?,
    changed_at: r.get(26)?,
    languages: serde_json::from_str(&languages).unwrap_or_default(),
    narrations: serde_json::from_str(&narrations).unwrap_or_default(),
    pending: serde_json::from_str(&pending).unwrap_or_default(),
    pending_retry_at: r.get(30)?,
    origin: Origin {
      kind: OriginKind::parse(&origin).unwrap_or(OriginKind::Source),
      source: r.get(37)?,
      because: r.get(32)?,
      item_id: r.get(33)?,
      document_id: r.get(34)?,
      title: r.get(35)?,
      share_url: r.get(36)?,
    },
  })
}

fn read_run(r: &rusqlite::Row<'_>) -> rusqlite::Result<Run> {
  let ids: String = r.get(3)?;
  Ok(Run {
    id: r.get(0)?,
    category_name: r.get(1)?,
    item_id: r.get(2)?,
    item_ids: serde_json::from_str(&ids).unwrap_or_default(),
    attempt: r.get(4)?,
    run_id: r.get(5)?,
    state: r.get(6)?,
    outcome: r.get(7)?,
    error: r.get(8)?,
    started_at: r.get(9)?,
    ended_at: r.get(10)?,
  })
}

fn read_stack_row(r: &rusqlite::Row<'_>) -> rusqlite::Result<StackRow> {
  Ok(StackRow {
    id: r.get(0)?,
    component: r.get(1)?,
    version: r.get(2)?,
    place: r.get(3)?,
    repository: r.get(4)?,
    worker: r.get(5)?,
    added_at: r.get(6)?,
  })
}
