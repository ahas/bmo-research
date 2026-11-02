//! The research unit's types and client, as `docs/api.md` states them.
//!
//! The daemon keeps a queue of things to research by category: the categories and their recipients, the
//! sources it reads, the items it queued, the tries it asked of the runner, the stack an advisory is judged
//! on and the facts the briefs kept; and the runner's research runs, their branches and their settings.

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

/// A failure, in the wire format's one error shape.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct Failure {
  pub code: String,
  pub error: String,
}

/// The daemon's readiness and version.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct Health {
  pub ready: bool,
  pub reason: Option<String>,
  pub version: String,
}

/// A list, one page of it: the items and whether more stand past the last.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct Page<T> {
  pub items: Vec<T>,
  pub more: bool,
}

/// Whether the loop runs.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "lowercase")]
pub enum Running {
  Running,
  Stopped,
}

impl Running {
  pub fn as_str(self) -> &'static str {
    match self {
      Running::Running => "running",
      Running::Stopped => "stopped",
    }
  }

  pub fn parse(word: &str) -> Option<Running> {
    match word {
      "running" => Some(Running::Running),
      "stopped" => Some(Running::Stopped),
      _ => None,
    }
  }
}

/// One category's state and its part of the queue.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct CategoryState {
  pub category_name: String,
  pub state: Running,
  pub waiting_count: i64,
  pub running_count: i64,
  /// its items whose page is finished before they are told
  pub finishing_count: i64,
  /// its items stuck past their retries, still asked on every retry wait
  pub stuck_count: i64,
  pub cap: i64,
}

/// Why a run asked does not go out now: it waits for the arbiter to grant its engine's card.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "kebab-case")]
pub enum HoldReason {
  WaitingCard,
}

impl HoldReason {
  pub fn as_str(self) -> &'static str {
    match self {
      HoldReason::WaitingCard => "waiting-card",
    }
  }
}

/// A run asked that waits for its engine now (`docs/api.md`, 「The engine」).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct Hold {
  pub reason: HoldReason,
  /// the try that waits
  pub run_id: i64,
  pub category_name: String,
  /// the registration the try's road goes out on
  pub registration: String,
  /// the arbiter service whose card it waits for
  pub service_name: String,
  /// the arbiter demand it waits on; null until the master has answered the ask
  pub demand_id: Option<i64>,
  /// the master's reason the card waits (`starting`, `in-use`, `held` or `start-failed`); null where it gave
  /// none
  pub cause: Option<String>,
  /// the master's sentence for that reason
  pub cause_text: Option<String>,
  /// epoch milliseconds; when the demand was asked
  pub since: i64,
  /// whether the owner's room was told the engine has not come
  pub reported: bool,
}

impl Hold {
  /// The hold in one English line: which try waits for which service's card, and why where the master said.
  pub fn line(&self) -> String {
    let why = match (&self.cause_text, &self.cause) {
      (Some(text), _) if !text.trim().is_empty() => format!(": {}", text.trim()),
      (_, Some(cause)) => format!(" ({cause})"),
      _ => String::new(),
    };
    let told = if self.reported { "; the owner's room was told it has not come" } else { "" };
    format!(
      "run {} of {} waits for a {} card for {}{why}{told}",
      self.run_id, self.category_name, self.service_name, self.registration
    )
  }
}

/// The daemon's state: stopped where every category is stopped, each category's, and the runs that wait for
/// their engine.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct DaemonState {
  pub state: Running,
  /// epoch milliseconds; the latest change of any category's state, null where none was changed
  pub changed_at: Option<i64>,
  pub categories: Vec<CategoryState>,
  pub holds: Vec<Hold>,
}

/// A change of the daemon's state, for every category or for the one named.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct StateChange {
  pub state: Running,
  #[serde(default, skip_serializing_if = "Option::is_none")]
  pub category_name: Option<String>,
}

/// What a run of a category's items is asked for.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "kebab-case")]
pub enum CategoryShape {
  ManualOrInvestigation,
  Advisory,
  Brief,
}

impl CategoryShape {
  pub fn as_str(self) -> &'static str {
    match self {
      CategoryShape::ManualOrInvestigation => "manual-or-investigation",
      CategoryShape::Advisory => "advisory",
      CategoryShape::Brief => "brief",
    }
  }

  pub fn parse(word: &str) -> Option<CategoryShape> {
    [CategoryShape::ManualOrInvestigation, CategoryShape::Advisory, CategoryShape::Brief]
      .into_iter()
      .find(|shape| shape.as_str() == word)
  }
}

/// A category (`docs/design.md`, 「Categories」).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct Category {
  pub name: String,
  pub purpose: String,
  pub shape: CategoryShape,
  pub cap: i64,
  pub cadence_minutes: i64,
  /// for a `brief` category, how often its items are gathered into one run
  pub gather_minutes: Option<i64>,
  /// its road; null where the settings' stands
  pub provider: Option<String>,
  pub model: Option<String>,
  pub state: Running,
  pub state_at: Option<i64>,
  pub created_at: i64,
}

/// A category asked to be registered.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct NewCategory {
  pub name: String,
  pub purpose: String,
  pub shape: CategoryShape,
  #[serde(default, skip_serializing_if = "Option::is_none")]
  pub cap: Option<i64>,
  #[serde(default, skip_serializing_if = "Option::is_none")]
  pub cadence_minutes: Option<i64>,
  #[serde(default, skip_serializing_if = "Option::is_none")]
  pub gather_minutes: Option<i64>,
  #[serde(default, skip_serializing_if = "Option::is_none")]
  pub provider: Option<String>,
  #[serde(default, skip_serializing_if = "Option::is_none")]
  pub model: Option<String>,
}

/// A change of a category; a field left out keeps what it holds, and an empty road gives the settings' back.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct CategoryChange {
  #[serde(default, skip_serializing_if = "Option::is_none")]
  pub purpose: Option<String>,
  #[serde(default, skip_serializing_if = "Option::is_none")]
  pub cap: Option<i64>,
  #[serde(default, skip_serializing_if = "Option::is_none")]
  pub cadence_minutes: Option<i64>,
  #[serde(default, skip_serializing_if = "Option::is_none")]
  pub gather_minutes: Option<i64>,
  #[serde(default, skip_serializing_if = "Option::is_none")]
  pub provider: Option<String>,
  #[serde(default, skip_serializing_if = "Option::is_none")]
  pub model: Option<String>,
}

/// Whether a recipient is a room or an agent.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "lowercase")]
pub enum RecipientKind {
  Room,
  Agent,
}

impl RecipientKind {
  pub fn as_str(self) -> &'static str {
    match self {
      RecipientKind::Room => "room",
      RecipientKind::Agent => "agent",
    }
  }
}

/// Who hears a category's notices.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct Recipient {
  pub id: i64,
  pub category_name: String,
  pub kind: RecipientKind,
  pub room_id: Option<i64>,
  pub agent_name: Option<String>,
  /// `owner` or `added`
  pub origin: String,
  pub added_at: i64,
}

/// A recipient asked to be added.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct NewRecipient {
  pub kind: RecipientKind,
  #[serde(default, skip_serializing_if = "Option::is_none")]
  pub room_id: Option<i64>,
  #[serde(default, skip_serializing_if = "Option::is_none")]
  pub agent_name: Option<String>,
}

/// A row of the platform's stack inventory.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct StackRow {
  pub id: i64,
  pub component: String,
  pub version: Option<String>,
  pub place: Option<String>,
  pub repository: Option<String>,
  pub worker: Option<String>,
  pub added_at: i64,
}

/// A stack row asked to be added.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct NewStackRow {
  pub component: String,
  #[serde(default, skip_serializing_if = "Option::is_none")]
  pub version: Option<String>,
  #[serde(default, skip_serializing_if = "Option::is_none")]
  pub place: Option<String>,
  #[serde(default, skip_serializing_if = "Option::is_none")]
  pub repository: Option<String>,
  #[serde(default, skip_serializing_if = "Option::is_none")]
  pub worker: Option<String>,
}

/// A figure a brief kept.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct Fact {
  pub id: i64,
  pub name: String,
  pub value: String,
  pub unit: Option<String>,
  pub as_of: Option<String>,
  pub source_address: Option<String>,
  /// the research run whose brief held it
  pub run_id: Option<i64>,
  pub document_id: Option<i64>,
  pub created_at: i64,
}

/// The words an item must carry and the words that rule it out.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct Filter {
  #[serde(default)]
  pub require: Vec<String>,
  #[serde(default)]
  pub exclude: Vec<String>,
}

/// A registered source (`docs/sources.md`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct Source {
  pub id: i64,
  pub category_name: String,
  pub kind: String,
  pub address: String,
  pub query: Option<String>,
  /// a `page` source's link pattern, null for another kind
  pub pattern: Option<String>,
  /// how a `page` source's listing is read, `direct` or `browser`; null for another kind
  pub strategy: Option<String>,
  pub filter: Filter,
  pub every_minutes: Option<i64>,
  pub limit: i64,
  /// readings failed since the last that did not
  pub failure_count: i64,
  /// the last reading's failure sentence, null when it succeeded
  pub last_error: Option<String>,
  /// epoch milliseconds of the last reading, failed or not
  pub read_at: Option<i64>,
  pub created_at: i64,
  pub changed_at: i64,
  /// readings made, failed or not
  pub scan_count: i64,
  /// candidates the last reading that succeeded turned up past the filter
  pub last_yield: Option<i64>,
  /// of those, the ones kept as new, each waiting for a slot in the queue
  pub last_taken: Option<i64>,
}

/// A source asked to be registered.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct NewSource {
  pub category_name: String,
  pub kind: String,
  #[serde(default, skip_serializing_if = "Option::is_none")]
  pub address: Option<String>,
  #[serde(default, skip_serializing_if = "Option::is_none")]
  pub query: Option<String>,
  /// which links of a `page` source's listing are posts: a path prefix, else a regular expression
  #[serde(default, skip_serializing_if = "Option::is_none")]
  pub pattern: Option<String>,
  /// how a `page` source's listing is read: `direct` or `browser`
  #[serde(default, skip_serializing_if = "Option::is_none")]
  pub strategy: Option<String>,
  #[serde(default, skip_serializing_if = "Option::is_none")]
  pub filter: Option<Filter>,
  #[serde(default, skip_serializing_if = "Option::is_none")]
  pub every_minutes: Option<i64>,
  #[serde(default, skip_serializing_if = "Option::is_none")]
  pub limit: Option<i64>,
}

/// A change of a source; a field left out keeps what it holds.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct SourceChange {
  #[serde(default, skip_serializing_if = "Option::is_none")]
  pub address: Option<String>,
  #[serde(default, skip_serializing_if = "Option::is_none")]
  pub query: Option<String>,
  /// which links of a `page` source's listing are posts: a path prefix, else a regular expression
  #[serde(default, skip_serializing_if = "Option::is_none")]
  pub pattern: Option<String>,
  /// how a `page` source's listing is read: `direct` or `browser`
  #[serde(default, skip_serializing_if = "Option::is_none")]
  pub strategy: Option<String>,
  #[serde(default, skip_serializing_if = "Option::is_none")]
  pub filter: Option<Filter>,
  #[serde(default, skip_serializing_if = "Option::is_none")]
  pub every_minutes: Option<i64>,
  #[serde(default, skip_serializing_if = "Option::is_none")]
  pub limit: Option<i64>,
}

/// The shape an item's research run is asked in.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "lowercase")]
pub enum Shape {
  Manual,
  Investigation,
  Advisory,
  Brief,
}

impl Shape {
  pub fn as_str(self) -> &'static str {
    match self {
      Shape::Manual => "manual",
      Shape::Investigation => "investigation",
      Shape::Advisory => "advisory",
      Shape::Brief => "brief",
    }
  }

  pub fn parse(word: &str) -> Option<Shape> {
    [Shape::Manual, Shape::Investigation, Shape::Advisory, Shape::Brief]
      .into_iter()
      .find(|shape| shape.as_str() == word)
  }
}

/// An item's place in its life (`docs/api.md`, 「queue」).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "lowercase")]
pub enum ItemState {
  Waiting,
  Running,
  Finishing,
  Reported,
  Fit,
  Stuck,
  Taken,
  Dismissed,
}

impl ItemState {
  pub const ALL: [ItemState; 8] = [
    ItemState::Waiting,
    ItemState::Running,
    ItemState::Finishing,
    ItemState::Reported,
    ItemState::Fit,
    ItemState::Stuck,
    ItemState::Taken,
    ItemState::Dismissed,
  ];

  pub fn as_str(self) -> &'static str {
    match self {
      ItemState::Waiting => "waiting",
      ItemState::Running => "running",
      ItemState::Finishing => "finishing",
      ItemState::Reported => "reported",
      ItemState::Fit => "fit",
      ItemState::Stuck => "stuck",
      ItemState::Taken => "taken",
      ItemState::Dismissed => "dismissed",
    }
  }

  pub fn parse(word: &str) -> Option<ItemState> {
    ItemState::ALL.into_iter().find(|state| state.as_str() == word)
  }
}

/// What an item came from (`docs/api.md`, 「queue」): a source's scan, the owner's ask, or an ask made from an
/// earlier research.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "kebab-case")]
pub enum OriginKind {
  Source,
  Asked,
  FollowUp,
}

impl OriginKind {
  pub fn as_str(self) -> &'static str {
    match self {
      OriginKind::Source => "source",
      OriginKind::Asked => "asked",
      OriginKind::FollowUp => "follow-up",
    }
  }

  pub fn parse(word: &str) -> Option<OriginKind> {
    [OriginKind::Source, OriginKind::Asked, OriginKind::FollowUp]
      .into_iter()
      .find(|kind| kind.as_str() == word)
  }
}

/// Where an item came from; the fields its kind does not use are null.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct Origin {
  pub kind: OriginKind,
  /// for `source`: the source that found it, by its kind and what it reads; null where it no longer stands
  pub source: Option<String>,
  /// the words the ask was made with: the owner's for `asked`, why it was asked for `follow-up`
  pub because: Option<String>,
  /// for `follow-up`: the item it follows from
  pub item_id: Option<i64>,
  /// for `follow-up`: the document it follows from
  pub document_id: Option<i64>,
  /// for `follow-up`: that document's title, as it stood when the ask was made
  pub title: Option<String>,
  /// for `follow-up`: that document's live link, as it stood when the ask was made
  pub share_url: Option<String>,
}

/// One thing in the queue.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct Item {
  pub id: i64,
  pub category_name: String,
  pub key: String,
  pub name: String,
  pub maker: Option<String>,
  pub address: Option<String>,
  /// `model`, `paper`, `release`, `entry`, or `asked` for an ask
  pub kind: String,
  pub shape: Shape,
  pub source_id: Option<i64>,
  pub asked: bool,
  pub origin: Origin,
  pub priority: i64,
  pub state: ItemState,
  pub tries: i64,
  pub retry_at: Option<i64>,
  /// the research run of the latest try
  pub run_id: Option<i64>,
  pub document_id: Option<i64>,
  /// the head the run gave its document, in English
  pub title: Option<String>,
  pub description: Option<String>,
  /// the document's live link
  pub share_url: Option<String>,
  /// the languages the document's page holds, Korean first; empty before a document is filed
  pub languages: Vec<String>,
  /// the languages whose narration the document's page plays, Korean first
  pub narrations: Vec<String>,
  /// what the document's page still lacks before its report is told: `korean`, `narration-ko`,
  /// `narration-en`; empty where it lacks nothing
  pub pending: Vec<String>,
  /// when what the page lacks is asked again, or none where nothing is pending
  pub pending_retry_at: Option<i64>,

  pub fit: bool,
  /// the repositories and hosts an advisory touches
  pub touches: Option<String>,
  /// the heading of the document's plan section
  pub plan: Option<String>,
  /// the job item the chat agent filed for it
  pub job_item_id: Option<i64>,
  /// the job item filed for the helper while it is stuck, to fix the cause
  pub repair_item_id: Option<i64>,
  /// why its latest try failed or was interrupted: that try's outcome, not how the item stands now
  pub error: Option<String>,
  /// when the try `error` names ended
  pub error_at: Option<i64>,
  pub found_at: i64,
  pub created_at: i64,
  pub changed_at: i64,
}

/// A thing the owner asks for by its name and its maker's address, and, for a follow-up, the earlier research
/// it follows from.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct Ask {
  pub name: String,
  pub address: String,
  #[serde(default, skip_serializing_if = "Option::is_none")]
  pub category_name: Option<String>,
  #[serde(default, skip_serializing_if = "Option::is_none")]
  pub shape: Option<Shape>,
  /// the words the ask is made with: why it is asked
  #[serde(default, skip_serializing_if = "Option::is_none")]
  pub because: Option<String>,
  /// for a follow-up: the item it follows from
  #[serde(default, skip_serializing_if = "Option::is_none")]
  pub follows_item_id: Option<i64>,
  /// for a follow-up: the document it follows from
  #[serde(default, skip_serializing_if = "Option::is_none")]
  pub follows_document_id: Option<i64>,
}

/// A change of an item: dismiss it, mark it taken, or ask a stuck one again by `state`, or move it to the front.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct ItemChange {
  #[serde(default, skip_serializing_if = "Option::is_none")]
  pub state: Option<ItemState>,
  /// with `taken`: the job item the chat agent filed
  #[serde(default, skip_serializing_if = "Option::is_none")]
  pub job_item_id: Option<i64>,
  #[serde(default, skip_serializing_if = "Option::is_none")]
  pub front: Option<bool>,
}

/// The daemon's record of one try of an item.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct Run {
  pub id: i64,
  pub category_name: String,
  pub item_id: i64,
  pub item_ids: Vec<i64>,
  #[serde(rename = "try")]
  pub attempt: i64,
  pub run_id: Option<i64>,
  /// `running`, `closed`, `refused`, `cut` or `failed`
  pub state: String,
  pub outcome: Option<String>,
  pub error: Option<String>,
  pub started_at: i64,
  pub ended_at: Option<i64>,
}

/// A research asked of the runner (`docs/api.md`, 「research runs」).
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct ResearchRunAsk {
  /// `search:basic`, `search:thorough`, `search:exhaustive` or `browse`
  pub key: String,
  pub query: String,
  #[serde(default, skip_serializing_if = "Option::is_none")]
  pub asker: Option<String>,
  #[serde(default, skip_serializing_if = "Option::is_none")]
  pub options: Option<String>,
  /// the room the result is told to; the owner's private room where none is named
  #[serde(default, skip_serializing_if = "Option::is_none")]
  pub room_id: Option<i64>,
  /// the provider the run goes out on, named only with its model; where a model alone is named, the one its
  /// role's order holds that model under; its slot's where none is named
  #[serde(default, skip_serializing_if = "Option::is_none")]
  pub provider: Option<String>,
  /// the model the run goes out on; its slot's where none is named
  #[serde(default, skip_serializing_if = "Option::is_none")]
  pub model: Option<String>,
}

/// Where a research run stands.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "lowercase")]
pub enum ResearchRunState {
  Waiting,
  Running,
  Closed,
}

impl ResearchRunState {
  pub fn as_str(self) -> &'static str {
    match self {
      ResearchRunState::Waiting => "waiting",
      ResearchRunState::Running => "running",
      ResearchRunState::Closed => "closed",
    }
  }

  pub fn parse(word: &str) -> Option<ResearchRunState> {
    [ResearchRunState::Waiting, ResearchRunState::Running, ResearchRunState::Closed]
      .into_iter()
      .find(|state| state.as_str() == word)
  }
}

/// A research run as it was asked: its id, its key, its slot's state and the road its slot stands on.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct ResearchRun {
  pub run_id: i64,
  pub key: String,
  pub state: ResearchRunState,
  pub provider: String,
  pub model: String,
}

/// One research run as the runner's ledger holds it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct ResearchRunView {
  pub run_id: i64,
  pub key: String,
  pub query: String,
  pub asker: Option<String>,
  pub room_id: Option<i64>,
  pub provider: String,
  pub model: String,
  pub state: ResearchRunState,
  /// `answered`, `failed` or `cut`; null while the run is open
  pub outcome: Option<String>,
  /// epoch milliseconds
  pub started_at: i64,
  /// epoch milliseconds
  pub finished_at: Option<i64>,
  pub document_id: Option<i64>,
  /// the filed document's share link
  pub link: Option<String>,
  pub error: Option<String>,
  /// how many branches it opened
  pub branches: i64,
  /// the names of the agency turns it went out under
  pub turns: Vec<String>,
  /// the languages whose narration its document plays
  #[serde(default)]
  pub narrations: Vec<String>,
  /// what its document still lacks, its room told once it lacks nothing
  #[serde(default)]
  pub pending: Vec<String>,
}

/// One line of a research run's log.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct ResearchLogLine {
  pub id: i64,
  pub run_id: i64,
  /// epoch milliseconds
  pub at: i64,
  /// `asked`, `admitted`, `answer`, `round`, `branch`, `whole`, `narration`, `filed`, `told` or `ended`
  pub kind: String,
  pub text: String,
}

/// A research run's log, oldest first.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct ResearchLog {
  pub items: Vec<ResearchLogLine>,
}

/// A branch asked of a research run.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct BranchAsk {
  /// the research run's id
  pub run: i64,
  pub angle: String,
}

/// How a branch ended.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "lowercase")]
pub enum BranchState {
  Completed,
  Failed,
}

/// A branch at its end: its index in its run, how it ended, what it wrote and why it failed.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct BranchEnd {
  /// counted from 1 per run
  pub index: i64,
  pub state: BranchState,
  pub notes: String,
  pub reason: Option<String>,
}

/// One research key's settings: its branches and its budget.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct ResearchKeySettings {
  pub key: String,
  pub branches: i64,
  /// the most pages the first answer fetches; none for no ceiling. Left out of a change, it keeps what the
  /// key holds.
  #[serde(default)]
  pub pages: Option<i64>,
  /// the most pages each branch fetches; none for no ceiling. Left out of a change, it keeps what the key
  /// holds.
  #[serde(default)]
  pub branch_pages: Option<i64>,
}

/// One search engine's state, as the web tools answer it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct SearchEngineState {
  pub name: String,
  #[serde(default)]
  pub category: Option<String>,
  pub state: String,
  #[serde(default)]
  pub reason: Option<String>,
  #[serde(default)]
  pub observed_at: Option<i64>,
  #[serde(default)]
  pub suspended_until: Option<i64>,
}

/// What every research run is held to, and the search engines' state.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct ResearchSettings {
  /// the role a branch's slot is asked under and its session holds
  pub branch_role: String,
  /// `direct-then-browser`, `browser-first` or `direct-only`
  pub strategy: String,
  /// null where the web tools' default stands
  pub host_interval_ms: Option<i64>,
  /// null where the web tools' default stands
  pub engine_interval_ms: Option<i64>,
  pub keys: Vec<ResearchKeySettings>,
  /// `default` where the settings file's stand, `set` once changed
  pub source: String,
  /// null where they could not be read
  pub engines: Option<Vec<SearchEngineState>>,
  /// why the engines' state could not be read, where it could not
  pub engines_unread: Option<String>,
}

fn present<'de, D: serde::Deserializer<'de>, T: Deserialize<'de>>(
  given: D,
) -> Result<Option<Option<T>>, D::Error> {
  Option::<T>::deserialize(given).map(Some)
}

/// A change to the research settings: a field left out keeps what it holds, and a gap given as null returns
/// to the web tools' default.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct ResearchSettingsChange {
  #[serde(default, skip_serializing_if = "Option::is_none")]
  pub branch_role: Option<String>,
  #[serde(default, skip_serializing_if = "Option::is_none")]
  pub strategy: Option<String>,
  #[serde(default, deserialize_with = "present", skip_serializing_if = "Option::is_none")]
  pub host_interval_ms: Option<Option<i64>>,
  #[serde(default, deserialize_with = "present", skip_serializing_if = "Option::is_none")]
  pub engine_interval_ms: Option<Option<i64>>,
  #[serde(default, skip_serializing_if = "Option::is_none")]
  pub keys: Option<Vec<ResearchKeySettings>>,
}

/// The unit's client, calling the routes `docs/api.md` states.
#[derive(Debug, Clone)]
pub struct ResearchClient {
  base: String,
  http: reqwest::Client,
  token: Option<String>,
}

impl ResearchClient {
  pub fn new(base: impl Into<String>) -> ResearchClient {
    ResearchClient {
      base: base.into().trim_end_matches('/').to_string(),
      http: reqwest::Client::new(),
      token: None,
    }
  }

  pub fn with_token(mut self, token: impl Into<String>) -> ResearchClient {
    self.token = Some(token.into());
    self
  }

  fn req(&self, method: reqwest::Method, path: &str) -> reqwest::RequestBuilder {
    let mut r = self.http.request(method, format!("{}{}", self.base, path));
    if let Some(t) = &self.token {
      r = r.bearer_auth(t);
    }
    r
  }

  pub async fn health(&self) -> Result<Health, String> {
    self.call(self.req(reqwest::Method::GET, "/health")).await
  }

  pub async fn state(&self) -> Result<DaemonState, String> {
    self.call(self.req(reqwest::Method::GET, "/state")).await
  }

  pub async fn set_state(&self, change: &StateChange) -> Result<DaemonState, String> {
    self.call(self.req(reqwest::Method::PATCH, "/state").json(change)).await
  }

  pub async fn categories(&self) -> Result<Page<Category>, String> {
    self.call(self.req(reqwest::Method::GET, "/categories")).await
  }

  pub async fn category(&self, name: &str) -> Result<Category, String> {
    self.call(self.req(reqwest::Method::GET, &format!("/categories/{name}"))).await
  }

  pub async fn add_category(&self, category: &NewCategory) -> Result<Category, String> {
    self.call(self.req(reqwest::Method::POST, "/categories").json(category)).await
  }

  pub async fn change_category(&self, name: &str, change: &CategoryChange) -> Result<Category, String> {
    self
      .call(self.req(reqwest::Method::PATCH, &format!("/categories/{name}")).json(change))
      .await
  }

  pub async fn remove_category(&self, name: &str) -> Result<(), String> {
    self.empty(self.req(reqwest::Method::DELETE, &format!("/categories/{name}"))).await
  }

  pub async fn recipients(&self, category: &str) -> Result<Page<Recipient>, String> {
    self
      .call(self.req(reqwest::Method::GET, &format!("/categories/{category}/recipients")))
      .await
  }

  pub async fn add_recipient(&self, category: &str, recipient: &NewRecipient) -> Result<Recipient, String> {
    self
      .call(
        self
          .req(reqwest::Method::POST, &format!("/categories/{category}/recipients"))
          .json(recipient),
      )
      .await
  }

  pub async fn remove_recipient(&self, category: &str, id: i64) -> Result<(), String> {
    self
      .empty(self.req(reqwest::Method::DELETE, &format!("/categories/{category}/recipients/{id}")))
      .await
  }

  pub async fn sources(
    &self,
    category: Option<&str>,
    after: Option<i64>,
    limit: i64,
  ) -> Result<Page<Source>, String> {
    let mut query = vec![("limit", limit.to_string())];
    if let Some(category) = category {
      query.push(("categoryName", category.to_string()));
    }
    if let Some(after) = after {
      query.push(("after", after.to_string()));
    }
    self.call(self.req(reqwest::Method::GET, "/sources").query(&query)).await
  }

  pub async fn source(&self, id: i64) -> Result<Source, String> {
    self.call(self.req(reqwest::Method::GET, &format!("/sources/{id}"))).await
  }

  pub async fn add_source(&self, source: &NewSource) -> Result<Source, String> {
    self.call(self.req(reqwest::Method::POST, "/sources").json(source)).await
  }

  pub async fn change_source(&self, id: i64, change: &SourceChange) -> Result<Source, String> {
    self
      .call(self.req(reqwest::Method::PATCH, &format!("/sources/{id}")).json(change))
      .await
  }

  pub async fn remove_source(&self, id: i64) -> Result<(), String> {
    self.empty(self.req(reqwest::Method::DELETE, &format!("/sources/{id}"))).await
  }

  /// The queue in its order, narrowed to the category and the states named where any are.
  pub async fn queue(
    &self,
    category: Option<&str>,
    states: &[ItemState],
    after: Option<i64>,
    limit: i64,
  ) -> Result<Page<Item>, String> {
    let mut query = vec![("limit", limit.to_string())];
    if let Some(category) = category {
      query.push(("categoryName", category.to_string()));
    }
    if !states.is_empty() {
      let named: Vec<&str> = states.iter().map(|s| s.as_str()).collect();
      query.push(("state", named.join(",")));
    }
    if let Some(after) = after {
      query.push(("after", after.to_string()));
    }
    self.call(self.req(reqwest::Method::GET, "/queue").query(&query)).await
  }

  pub async fn ask(&self, ask: &Ask) -> Result<Item, String> {
    self.call(self.req(reqwest::Method::POST, "/queue").json(ask)).await
  }

  pub async fn item(&self, id: i64) -> Result<Item, String> {
    self.call(self.req(reqwest::Method::GET, &format!("/queue/{id}"))).await
  }

  pub async fn change_item(&self, id: i64, change: &ItemChange) -> Result<Item, String> {
    self.call(self.req(reqwest::Method::PATCH, &format!("/queue/{id}")).json(change)).await
  }

  pub async fn runs(
    &self,
    item_id: Option<i64>,
    category: Option<&str>,
    before: Option<i64>,
    limit: i64,
  ) -> Result<Page<Run>, String> {
    let mut query = vec![("limit", limit.to_string())];
    if let Some(item_id) = item_id {
      query.push(("itemId", item_id.to_string()));
    }
    if let Some(category) = category {
      query.push(("categoryName", category.to_string()));
    }
    if let Some(before) = before {
      query.push(("before", before.to_string()));
    }
    self.call(self.req(reqwest::Method::GET, "/runs").query(&query)).await
  }

  pub async fn run(&self, id: i64) -> Result<Run, String> {
    self.call(self.req(reqwest::Method::GET, &format!("/runs/{id}"))).await
  }

  pub async fn stack(&self) -> Result<Page<StackRow>, String> {
    self.call(self.req(reqwest::Method::GET, "/stack")).await
  }

  pub async fn add_stack_row(&self, row: &NewStackRow) -> Result<StackRow, String> {
    self.call(self.req(reqwest::Method::POST, "/stack").json(row)).await
  }

  pub async fn remove_stack_row(&self, id: i64) -> Result<(), String> {
    self.empty(self.req(reqwest::Method::DELETE, &format!("/stack/{id}"))).await
  }

  pub async fn facts(
    &self,
    name: Option<&str>,
    before: Option<i64>,
    limit: i64,
  ) -> Result<Page<Fact>, String> {
    let mut query = vec![("limit", limit.to_string())];
    if let Some(name) = name {
      query.push(("name", name.to_string()));
    }
    if let Some(before) = before {
      query.push(("before", before.to_string()));
    }
    self.call(self.req(reqwest::Method::GET, "/facts").query(&query)).await
  }

  pub async fn ask_research(&self, ask: &ResearchRunAsk) -> Result<ResearchRun, String> {
    self.call(self.req(reqwest::Method::POST, "/research-runs").json(ask)).await
  }

  pub async fn research_run(&self, id: i64) -> Result<ResearchRunView, String> {
    self.call(self.req(reqwest::Method::GET, &format!("/research-runs/{id}"))).await
  }

  pub async fn research_log(&self, id: i64) -> Result<ResearchLog, String> {
    self
      .call(self.req(reqwest::Method::GET, &format!("/research-runs/{id}/log-lines")))
      .await
  }

  pub async fn branch(&self, ask: &BranchAsk) -> Result<BranchEnd, String> {
    self.call(self.req(reqwest::Method::POST, "/branches").json(ask)).await
  }

  pub async fn research_settings(&self) -> Result<ResearchSettings, String> {
    self.call(self.req(reqwest::Method::GET, "/research-settings")).await
  }

  pub async fn set_research_settings(
    &self,
    change: &ResearchSettingsChange,
  ) -> Result<ResearchSettings, String> {
    self.call(self.req(reqwest::Method::PUT, "/research-settings").json(change)).await
  }

  async fn empty(&self, r: reqwest::RequestBuilder) -> Result<(), String> {
    let resp = r.send().await.map_err(|e| e.to_string())?;
    let status = resp.status();
    if status.is_success() {
      return Ok(());
    }
    let body = resp.text().await.unwrap_or_default();
    Err(refusal(status.as_u16(), &body))
  }

  async fn call<T: serde::de::DeserializeOwned>(&self, r: reqwest::RequestBuilder) -> Result<T, String> {
    let resp = r.send().await.map_err(|e| e.to_string())?;
    let status = resp.status();
    let body = resp.text().await.map_err(|e| e.to_string())?;
    if !status.is_success() {
      return Err(refusal(status.as_u16(), &body));
    }
    serde_json::from_str(&body).map_err(|e| format!("the answer did not read: {e}: {body}"))
  }
}

/// A failure's sentence: its status, its code and its error where the body reads as the error shape.
fn refusal(status: u16, body: &str) -> String {
  let why = serde_json::from_str::<Failure>(body)
    .map(|f| format!("{}: {}", f.code, f.error))
    .unwrap_or_else(|_| body.to_string());
  format!("{status} {why}")
}
