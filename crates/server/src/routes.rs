//! The routes `docs/api.md` states.

use std::collections::BTreeMap;
use std::sync::Arc;

use axum::body::Bytes;
use axum::extract::{Path, Query, State};
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use axum::routing::{delete, get, post};
use axum::{Json, Router};
use bmo_research_client::*;
use serde::de::DeserializeOwned;

use crate::App;
use crate::branches::Refusal as Branching;
use crate::runner::Refused as Launch;
use crate::sources::{LinkPattern, SourceKind, Strategy, address_key};
use crate::store::{Asked, Refusal, item_shape};

type Shared = State<Arc<App>>;
type Queries = Query<BTreeMap<String, String>>;

pub fn router(app: Arc<App>) -> Router {
  Router::new()
    .route("/health", get(health))
    .route("/state", get(get_state).patch(patch_state))
    .route("/categories", get(list_categories).post(create_category))
    .route(
      "/categories/{categoryName}",
      get(get_category).patch(patch_category).delete(delete_category),
    )
    .route("/categories/{categoryName}/recipients", get(list_recipients).post(create_recipient))
    .route("/categories/{categoryName}/recipients/{recipientId}", delete(delete_recipient))
    .route("/sources", get(list_sources).post(create_source))
    .route("/sources/{sourceId}", get(get_source).patch(patch_source).delete(delete_source))
    .route("/queue", get(list_queue).post(create_ask))
    .route("/queue/{itemId}", get(get_item).patch(patch_item))
    .route("/runs", get(list_runs))
    .route("/runs/{id}", get(get_run))
    .route("/stack", get(list_stack).post(create_stack_row))
    .route("/stack/{id}", delete(delete_stack_row))
    .route("/facts", get(list_facts))
    .route("/research-runs", post(create_research_run))
    .route("/research-runs/{runId}", get(get_research_run))
    .route("/research-runs/{runId}/log-lines", get(get_research_log))
    .route("/branches", post(create_branch))
    .route("/research-settings", get(get_research_settings).put(put_research_settings))
    .fallback(no_route)
    .layer(axum::middleware::from_fn_with_state(app.clone(), auth))
    .with_state(app)
}

async fn auth(State(app): Shared, req: axum::extract::Request, next: axum::middleware::Next) -> Response {
  let ok = req
    .headers()
    .get(axum::http::header::AUTHORIZATION)
    .and_then(|v| v.to_str().ok())
    .and_then(|v| v.strip_prefix("Bearer "))
    .is_some_and(|t| !app.token.is_empty() && t == app.token);
  if !ok {
    return failure(StatusCode::UNAUTHORIZED, "unauthorized", "the token is missing or wrong");
  }
  next.run(req).await
}

pub fn failure(status: StatusCode, code: &str, error: impl Into<String>) -> Response {
  (
    status,
    Json(Failure {
      code: code.into(),
      error: error.into(),
    }),
  )
    .into_response()
}

fn bad(why: impl Into<String>) -> Response {
  failure(StatusCode::BAD_REQUEST, "bad-request", why)
}

fn failed(why: impl Into<String>) -> Response {
  failure(StatusCode::INTERNAL_SERVER_ERROR, "failed", why)
}

fn refused(refusal: Refusal) -> Response {
  match refusal {
    Refusal::NotFound(why) => failure(StatusCode::NOT_FOUND, "not-found", why),
    Refusal::Conflict(why) => failure(StatusCode::CONFLICT, "conflict", why),
    Refusal::Failed(why) => failed(why),
  }
}

fn answered<T: serde::Serialize>(result: Result<T, Refusal>, status: StatusCode) -> Response {
  match result {
    Ok(member) => (status, Json(member)).into_response(),
    Err(refusal) => refused(refusal),
  }
}

fn listed<T: serde::Serialize>(result: Result<Page<T>, String>) -> Response {
  match result {
    Ok(page) => Json(page).into_response(),
    Err(why) => failed(why),
  }
}

/// A body read as the route's request type, or the sentence saying what is wrong with it.
fn body<T: DeserializeOwned>(bytes: &Bytes) -> Result<T, String> {
  serde_json::from_slice(bytes).map_err(|e| format!("the body does not read: {e}"))
}

fn number(word: &str, what: &str) -> Result<i64, String> {
  word.parse().map_err(|_| format!("{what} {word} is not a number"))
}

fn query_number(q: &BTreeMap<String, String>, key: &str) -> Result<Option<i64>, String> {
  q.get(key).map(|v| number(v, key)).transpose()
}

fn limit(q: &BTreeMap<String, String>) -> Result<i64, String> {
  Ok(query_number(q, "limit")?.unwrap_or(50).clamp(1, 200))
}

async fn no_route() -> Response {
  failure(StatusCode::NOT_FOUND, "no-route", "no route answers this method and path")
}

async fn health() -> Response {
  Json(Health {
    ready: true,
    reason: None,
    version: env!("CARGO_PKG_VERSION").into(),
  })
  .into_response()
}

// state

async fn get_state(State(app): Shared) -> Response {
  match app.store.daemon_state() {
    Ok(mut state) => {
      state.holds = crate::engine::holds(&app);
      Json(state).into_response()
    }
    Err(why) => failed(why),
  }
}

async fn patch_state(State(app): Shared, bytes: Bytes) -> Response {
  let change: StateChange = match body(&bytes) {
    Ok(c) => c,
    Err(r) => return bad(r),
  };
  if let Err(refusal) = app.store.set_running(change.category_name.as_deref(), change.state) {
    return refused(refusal);
  }
  app.places.wake();
  get_state(State(app)).await
}

// categories

fn valid_name(name: &str) -> bool {
  let mut chars = name.chars();
  name.len() <= 32
    && chars.next().is_some_and(|c| c.is_ascii_lowercase())
    && chars.all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-')
}

async fn list_categories(State(app): Shared) -> Response {
  listed(app.store.categories().map(|items| Page { items, more: false }))
}

async fn create_category(State(app): Shared, bytes: Bytes) -> Response {
  let new: NewCategory = match body(&bytes) {
    Ok(c) => c,
    Err(r) => return bad(r),
  };
  if !valid_name(&new.name) {
    return bad(
      "a category's name is a lower-case letter, then lower-case letters, digits and -, at most 32",
    );
  }
  if new.purpose.trim().is_empty() {
    return bad("a category needs a purpose");
  }
  if [new.cadence_minutes, new.gather_minutes].into_iter().flatten().any(|n| n < 1) {
    return bad("a category's numbers are 1 or more");
  }
  if new.gather_minutes.is_some() && !new.shape.gathers() {
    return bad("only a brief or a topic category gathers");
  }
  let topic = new.shape == CategoryShape::Topic;
  if topic && new.instruction.as_deref().is_none_or(|instruction| instruction.trim().is_empty()) {
    return bad("a topic category needs its instruction: what to look for and how to report it");
  }
  if !topic && (new.instruction.is_some() || new.calendar.is_some()) {
    return bad("only a topic category takes an instruction or a calendar");
  }
  if let Some(why) = new.calendar.as_ref().and_then(calendar_fault) {
    return bad(why);
  }
  let category = match app.store.add_category(&new) {
    Ok(c) => c,
    Err(refusal) => return refused(refusal),
  };
  crate::dive::give_owner_room(&app).await;
  (StatusCode::CREATED, Json(category)).into_response()
}

// Why a calendar a topic names does not stand, or none where it does.
fn calendar_fault(calendar: &CalendarTarget) -> Option<&'static str> {
  if !["mine", "organization"].contains(&calendar.realm.as_str()) {
    return Some("a calendar's realm is mine or organization");
  }
  calendar.calendar_id.trim().is_empty().then_some("a calendar names its calendarId")
}

async fn get_category(State(app): Shared, Path(name): Path<String>) -> Response {
  answered(app.store.category(&name), StatusCode::OK)
}

async fn patch_category(State(app): Shared, Path(name): Path<String>, bytes: Bytes) -> Response {
  let change: CategoryChange = match body(&bytes) {
    Ok(c) => c,
    Err(r) => return bad(r),
  };
  if [change.cadence_minutes, change.gather_minutes].into_iter().flatten().any(|n| n < 1) {
    return bad("a category's numbers are 1 or more");
  }
  if let Ok(category) = app.store.category(&name) {
    if change.gather_minutes.is_some() && !category.shape.gathers() {
      return bad("only a brief or a topic category gathers");
    }
    if category.shape != CategoryShape::Topic && (change.instruction.is_some() || change.calendar.is_some())
    {
      return bad("only a topic category takes an instruction or a calendar");
    }
  }
  if change.instruction.as_deref().is_some_and(|instruction| instruction.trim().is_empty()) {
    return bad("a topic's instruction is never emptied");
  }
  if let Some(why) =
    change.calendar.as_ref().and_then(|calendar| calendar.as_ref()).and_then(calendar_fault)
  {
    return bad(why);
  }
  let changed = app.store.change_category(&name, &change);
  app.places.wake();
  answered(changed, StatusCode::OK)
}

async fn delete_category(State(app): Shared, Path(name): Path<String>) -> Response {
  match app.store.remove_category(&name) {
    Ok(()) => StatusCode::NO_CONTENT.into_response(),
    Err(refusal) => refused(refusal),
  }
}

// recipients

async fn list_recipients(State(app): Shared, Path(name): Path<String>) -> Response {
  if let Err(refusal) = app.store.category(&name) {
    return refused(refusal);
  }
  listed(app.store.recipients(&name).map(|items| Page { items, more: false }))
}

async fn create_recipient(State(app): Shared, Path(name): Path<String>, bytes: Bytes) -> Response {
  let new: NewRecipient = match body(&bytes) {
    Ok(r) => r,
    Err(r) => return bad(r),
  };
  let agent = new.agent_name.as_deref().map(str::trim).filter(|a| !a.is_empty());
  let added = match (new.kind, new.room_id, agent) {
    (RecipientKind::Room, Some(room), None) => {
      app.store.add_recipient(&name, RecipientKind::Room, Some(room), None, "added")
    }
    (RecipientKind::Agent, None, Some(agent)) => {
      app.store.add_recipient(&name, RecipientKind::Agent, None, Some(agent), "added")
    }
    _ => return bad("a room names its roomId alone, an agent its agentName alone"),
  };
  answered(added, StatusCode::CREATED)
}

async fn delete_recipient(State(app): Shared, Path((name, id)): Path<(String, String)>) -> Response {
  let id = match number(&id, "recipient") {
    Ok(i) => i,
    Err(r) => return bad(r),
  };
  match app.store.remove_recipient(&name, id) {
    Ok(()) => StatusCode::NO_CONTENT.into_response(),
    Err(refusal) => refused(refusal),
  }
}

// sources

async fn list_sources(State(app): Shared, Query(q): Queries) -> Response {
  let (after, limit) = match (query_number(&q, "after"), limit(&q)) {
    (Ok(a), Ok(l)) => (a, l),
    (Err(r), _) | (_, Err(r)) => return bad(r),
  };
  listed(app.store.sources(q.get("categoryName").map(String::as_str), after, limit))
}

async fn create_source(State(app): Shared, bytes: Bytes) -> Response {
  let mut new: NewSource = match body(&bytes) {
    Ok(s) => s,
    Err(r) => return bad(r),
  };
  let Some(kind) = SourceKind::parse(&new.kind) else {
    let kinds: Vec<&str> = SourceKind::ALL.iter().map(|k| k.as_str()).collect();
    return bad(format!("{} is no source kind; the kinds are {}", new.kind, kinds.join(", ")));
  };
  if kind.takes_query() && new.query.as_deref().is_none_or(|q| q.trim().is_empty()) {
    return bad(format!("a {} source needs a query", new.kind));
  }
  match page_fields(kind, new.pattern.as_deref().or(Some("")), new.strategy.as_deref()) {
    Ok((pattern, strategy)) => {
      new.pattern = pattern;
      new.strategy =
        strategy.or_else(|| (kind == SourceKind::Page).then(|| Strategy::Direct.as_str().into()));
    }
    Err(r) => return bad(r),
  }
  if [new.every_minutes, new.limit].into_iter().flatten().any(|n| n < 1) {
    return bad("a source's numbers are 1 or more");
  }
  let address = match new
    .address
    .clone()
    .filter(|a| !a.trim().is_empty())
    .or(kind.default_address().map(String::from))
  {
    Some(a) => a,
    None => return bad(format!("a {} source needs an address", new.kind)),
  };
  answered(app.store.add_source(&new, &address, new.limit.unwrap_or(30)), StatusCode::CREATED)
}

async fn get_source(State(app): Shared, Path(id): Path<String>) -> Response {
  match number(&id, "source") {
    Ok(id) => answered(app.store.source(id), StatusCode::OK),
    Err(r) => bad(r),
  }
}

async fn patch_source(State(app): Shared, Path(id): Path<String>, bytes: Bytes) -> Response {
  let (id, change) = match (number(&id, "source"), body::<SourceChange>(&bytes)) {
    (Ok(i), Ok(c)) => (i, c),
    (Err(r), _) | (_, Err(r)) => return bad(r),
  };
  if [change.every_minutes, change.limit].into_iter().flatten().any(|n| n < 1) {
    return bad("a source's numbers are 1 or more");
  }
  let mut change = change;
  if change.pattern.is_some() || change.strategy.is_some() {
    let kind = match app.store.source(id) {
      Ok(source) => SourceKind::parse(&source.kind),
      Err(refusal) => return refused(refusal),
    };
    match kind.map(|kind| page_fields(kind, change.pattern.as_deref(), change.strategy.as_deref())) {
      Some(Ok((pattern, strategy))) => (change.pattern, change.strategy) = (pattern, strategy),
      Some(Err(r)) => return bad(r),
      None => return bad("a pattern and a strategy are given to a page source alone"),
    }
  }
  answered(app.store.change_source(id, &change), StatusCode::OK)
}

/// A `page` source's pattern and strategy as they are kept, a pattern of none where none is named; for
/// another kind, both refused where either is named.
fn page_fields(
  kind: SourceKind,
  pattern: Option<&str>,
  strategy: Option<&str>,
) -> Result<(Option<String>, Option<String>), String> {
  if kind != SourceKind::Page {
    return match (pattern.filter(|p| !p.is_empty()), strategy) {
      (None, None) => Ok((None, None)),
      _ => Err(format!("a {} source takes no pattern and no strategy", kind.as_str())),
    };
  }
  let pattern = match pattern {
    Some(pattern) => Some(LinkPattern::parse(pattern)?.as_str().to_string()),
    None => None,
  };
  let strategy = match strategy {
    Some(word) => Some(
      Strategy::parse(word.trim())
        .ok_or_else(|| format!("{word} is no strategy; the strategies are direct and browser"))?
        .as_str()
        .to_string(),
    ),
    None => None,
  };
  Ok((pattern, strategy))
}

async fn delete_source(State(app): Shared, Path(id): Path<String>) -> Response {
  let id = match number(&id, "source") {
    Ok(i) => i,
    Err(r) => return bad(r),
  };
  match app.store.remove_source(id) {
    Ok(()) => StatusCode::NO_CONTENT.into_response(),
    Err(refusal) => refused(refusal),
  }
}

// queue

async fn list_queue(State(app): Shared, Query(q): Queries) -> Response {
  let (after, limit) = match (query_number(&q, "after"), limit(&q)) {
    (Ok(a), Ok(l)) => (a, l),
    (Err(r), _) | (_, Err(r)) => return bad(r),
  };
  let mut states = Vec::new();
  for word in q
    .get("state")
    .map(String::as_str)
    .unwrap_or_default()
    .split(',')
    .filter(|w| !w.is_empty())
  {
    match ItemState::parse(word.trim()) {
      Some(state) => states.push(state),
      None => return bad(format!("{word} is no item state")),
    }
  }
  listed(app.store.queue(q.get("categoryName").map(String::as_str), &states, after, limit))
}

async fn create_ask(State(app): Shared, bytes: Bytes) -> Response {
  let ask: Ask = match body(&bytes) {
    Ok(a) => a,
    Err(r) => return bad(r),
  };
  let (name, address) = (ask.name.trim(), ask.address.trim());
  if name.is_empty() || address.is_empty() {
    return bad("an ask names the thing and its maker's address");
  }
  let category = match app.store.category(ask.category_name.as_deref().unwrap_or("it")) {
    Ok(c) => c,
    Err(refusal) => return refused(refusal),
  };
  let shape = match (category.shape, ask.shape) {
    (CategoryShape::ManualOrInvestigation, Some(shape @ (Shape::Manual | Shape::Investigation))) => shape,
    (_, Some(shape)) => {
      return bad(format!("category {} does not take the shape {}", category.name, shape.as_str()));
    }
    (shape, None) => item_shape(shape, None),
  };
  let parent = match (ask.follows_item_id, ask.follows_document_id) {
    (None, None) => None,
    (item, document) => match app.store.parent(item, document) {
      Ok(parent) if document.is_some() && item.is_some() && parent.document_id != document => {
        return bad(format!(
          "item {} filed no document {}",
          item.unwrap_or_default(),
          document.unwrap_or_default()
        ));
      }
      Ok(parent) => Some(parent),
      Err(refusal) => return refused(refusal),
    },
  };
  let asked = Asked {
    because: ask.because.as_deref().map(str::trim).filter(|b| !b.is_empty()).map(str::to_owned),
    parent,
  };
  let made = app.store.ask_from(&category.name, &address_key(address), name, address, shape, &asked);
  app.places.wake();
  answered(made, StatusCode::CREATED)
}

async fn get_item(State(app): Shared, Path(id): Path<String>) -> Response {
  match number(&id, "item") {
    Ok(id) => answered(app.store.item(id), StatusCode::OK),
    Err(r) => bad(r),
  }
}

async fn patch_item(State(app): Shared, Path(id): Path<String>, bytes: Bytes) -> Response {
  let (id, change) = match (number(&id, "item"), body::<ItemChange>(&bytes)) {
    (Ok(i), Ok(c)) => (i, c),
    (Err(r), _) | (_, Err(r)) => return bad(r),
  };
  let answer = match (change.state, change.front) {
    (Some(ItemState::Dismissed), None) => answered(app.store.dismiss(id), StatusCode::OK),
    (Some(ItemState::Taken), None) => match change.job_item_id {
      Some(job) => answered(app.store.take(id, job), StatusCode::OK),
      None => bad("taken names the jobItemId the chat agent filed"),
    },
    (Some(ItemState::Waiting), None) => answered(app.store.ask_again(id), StatusCode::OK),
    (Some(state), None) => match app.store.item(id) {
      Ok(_) => {
        failure(StatusCode::CONFLICT, "conflict", format!("an item is not moved to {}", state.as_str()))
      }
      Err(refusal) => refused(refusal),
    },
    (None, Some(true)) => answered(app.store.front(id), StatusCode::OK),
    _ => bad("a change names a state, dismissed, taken or waiting, or front"),
  };
  app.places.wake();
  answer
}

// runs

async fn list_runs(State(app): Shared, Query(q): Queries) -> Response {
  let (item, before, limit) = match (query_number(&q, "itemId"), query_number(&q, "before"), limit(&q)) {
    (Ok(i), Ok(b), Ok(l)) => (i, b, l),
    (Err(r), _, _) | (_, Err(r), _) | (_, _, Err(r)) => return bad(r),
  };
  listed(app.store.runs(item, q.get("categoryName").map(String::as_str), before, limit))
}

async fn get_run(State(app): Shared, Path(id): Path<String>) -> Response {
  match number(&id, "run") {
    Ok(id) => answered(app.store.run(id), StatusCode::OK),
    Err(r) => bad(r),
  }
}

// stack

async fn list_stack(State(app): Shared) -> Response {
  listed(app.store.stack().map(|items| Page { items, more: false }))
}

async fn create_stack_row(State(app): Shared, bytes: Bytes) -> Response {
  let row: NewStackRow = match body(&bytes) {
    Ok(r) => r,
    Err(r) => return bad(r),
  };
  let named = |v: &Option<String>| v.as_deref().is_some_and(|v| !v.trim().is_empty());
  if row.component.trim().is_empty() || !(named(&row.place) || named(&row.repository)) {
    return bad("a stack row names its component and a place or a repository");
  }
  answered(app.store.add_stack_row(&row), StatusCode::CREATED)
}

async fn delete_stack_row(State(app): Shared, Path(id): Path<String>) -> Response {
  let id = match number(&id, "stack row") {
    Ok(i) => i,
    Err(r) => return bad(r),
  };
  match app.store.remove_stack_row(id) {
    Ok(()) => StatusCode::NO_CONTENT.into_response(),
    Err(refusal) => refused(refusal),
  }
}

// facts

async fn list_facts(State(app): Shared, Query(q): Queries) -> Response {
  let (before, limit) = match (query_number(&q, "before"), limit(&q)) {
    (Ok(b), Ok(l)) => (b, l),
    (Err(r), _) | (_, Err(r)) => return bad(r),
  };
  listed(app.store.facts(q.get("name").map(String::as_str), before, limit))
}

// research runs

async fn create_research_run(State(app): Shared, bytes: Bytes) -> Response {
  let ask: ResearchRunAsk = match body(&bytes) {
    Ok(a) => a,
    Err(r) => return bad(r),
  };
  let filing = if ask.key == crate::references::KEY {
    crate::runner::Filing::References
  } else {
    crate::runner::Filing::Room
  };
  match crate::runner::launch(app, ask, filing, None).await {
    Ok(run) => (StatusCode::ACCEPTED, Json(run)).into_response(),
    Err(Launch::Ask(why)) => bad(why),
    Err(Launch::Unavailable(why)) => failure(StatusCode::SERVICE_UNAVAILABLE, "unavailable", why),
    Err(Launch::Upstream(why)) => failure(StatusCode::BAD_GATEWAY, "upstream-failed", why),
    Err(Launch::Failed(why)) => failed(why),
  }
}

async fn get_research_run(State(app): Shared, Path(id): Path<String>) -> Response {
  match number(&id, "research run") {
    Ok(id) => answered(app.store.research_run(id).map(|row| row.view()), StatusCode::OK),
    Err(r) => bad(r),
  }
}

async fn get_research_log(State(app): Shared, Path(id): Path<String>) -> Response {
  let id = match number(&id, "research run") {
    Ok(i) => i,
    Err(r) => return bad(r),
  };
  if let Err(refusal) = app.store.research_run(id) {
    return refused(refusal);
  }
  match app.store.research_log(id) {
    Ok(items) => Json(ResearchLog { items }).into_response(),
    Err(why) => failed(why),
  }
}

async fn create_branch(State(app): Shared, bytes: Bytes) -> Response {
  let ask: BranchAsk = match body(&bytes) {
    Ok(a) => a,
    Err(r) => return bad(r),
  };
  match crate::branches::open(app, ask).await {
    Ok(end) => Json(end).into_response(),
    Err(Branching::NotFound(why)) => failure(StatusCode::NOT_FOUND, "not-found", why),
    Err(Branching::Bad(why)) => bad(why),
    Err(Branching::Refused(why)) => failure(StatusCode::CONFLICT, "branch-refused", why),
  }
}

// research settings

async fn get_research_settings(State(app): Shared) -> Response {
  match crate::runner::held(&app) {
    Ok((held, set)) => Json(crate::runner::view(&app, &held, set).await).into_response(),
    Err(why) => failed(why),
  }
}

async fn put_research_settings(State(app): Shared, bytes: Bytes) -> Response {
  let change: ResearchSettingsChange = match body(&bytes) {
    Ok(c) => c,
    Err(r) => return bad(r),
  };
  let held = match crate::runner::held(&app) {
    Ok((held, _)) => held,
    Err(why) => return failed(why),
  };
  let roles = match &change.branch_role {
    Some(_) => match app.peers.roles().await {
      Ok(roles) => roles,
      Err(why) => return failure(StatusCode::BAD_GATEWAY, "upstream-failed", why),
    },
    None => Vec::new(),
  };
  let held = match crate::runner::changed(held, &change, &roles) {
    Ok(held) => held,
    Err(why) => return bad(why),
  };
  if let Err(why) = app.store.keep_research_settings(&serde_json::to_string(&held).unwrap_or_default()) {
    return failed(why);
  }
  Json(crate::runner::view(&app, &held, true).await).into_response()
}
