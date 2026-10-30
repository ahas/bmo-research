//! The research unit's types and client, as `docs/api.md` states them.
//!
//! The daemon keeps a queue of things to research by category: the categories and their recipients, the
//! sources it reads, the items it queued, the tries it asked of agency's runner, the stack an advisory is
//! judged on and the facts the briefs kept.

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
  pub cap: i64,
}

/// The daemon's state: stopped where every category is stopped, and each category's.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct DaemonState {
  pub state: Running,
  /// epoch milliseconds; the latest change of any category's state, null where none was changed
  pub changed_at: Option<i64>,
  pub categories: Vec<CategoryState>,
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
  /// agency's ledger run whose brief held it
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
  pub filter: Filter,
  pub every_minutes: Option<i64>,
  pub limit: i64,
  pub failure_count: i64,
  pub last_error: Option<String>,
  /// epoch milliseconds
  pub read_at: Option<i64>,
  pub created_at: i64,
  pub changed_at: i64,
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
  Reported,
  Fit,
  Failed,
  Taken,
  Dismissed,
}

impl ItemState {
  pub const ALL: [ItemState; 7] = [
    ItemState::Waiting,
    ItemState::Running,
    ItemState::Reported,
    ItemState::Fit,
    ItemState::Failed,
    ItemState::Taken,
    ItemState::Dismissed,
  ];

  pub fn as_str(self) -> &'static str {
    match self {
      ItemState::Waiting => "waiting",
      ItemState::Running => "running",
      ItemState::Reported => "reported",
      ItemState::Fit => "fit",
      ItemState::Failed => "failed",
      ItemState::Taken => "taken",
      ItemState::Dismissed => "dismissed",
    }
  }

  pub fn parse(word: &str) -> Option<ItemState> {
    ItemState::ALL.into_iter().find(|state| state.as_str() == word)
  }
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
  pub priority: i64,
  pub state: ItemState,
  pub tries: i64,
  pub retry_at: Option<i64>,
  /// agency's ledger run of the latest try
  pub run_id: Option<i64>,
  pub document_id: Option<i64>,
  pub title: Option<String>,
  pub description: Option<String>,
  pub share_url: Option<String>,
  /// the Korean rendering's document, null until it is filed
  pub ko_document_id: Option<i64>,
  /// the Korean rendering's live link, null until it is filed
  pub ko_share_url: Option<String>,
  pub fit: bool,
  /// the settings' model nodes a manual's fit names
  pub fit_nodes: Vec<String>,
  /// the repositories and hosts an advisory touches
  pub touches: Option<String>,
  /// the heading of the document's plan section
  pub plan: Option<String>,
  /// the job item the chat agent filed for it
  pub job_item_id: Option<i64>,
  pub error: Option<String>,
  pub found_at: i64,
  pub created_at: i64,
  pub changed_at: i64,
}

/// A thing the owner asks for by its name and its maker's address.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct Ask {
  pub name: String,
  pub address: String,
  #[serde(default, skip_serializing_if = "Option::is_none")]
  pub category_name: Option<String>,
  #[serde(default, skip_serializing_if = "Option::is_none")]
  pub shape: Option<Shape>,
}

/// A change of an item: dismiss it or mark it taken by `state`, or move it to the front.
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
