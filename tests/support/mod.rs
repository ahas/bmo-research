//! What the tests share: a scratch data root, the daemon's app over it, and stand-ins for agency, the artifact
//! service, the receiver and the sources, each keeping every request it was asked.

#![allow(dead_code)]

use std::collections::{BTreeMap, VecDeque};
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};

use axum::Router;
use axum::body::Bytes;
use axum::extract::{Path, Query, State};
use axum::http::{Method, StatusCode, Uri};
use axum::response::{IntoResponse, Response};
use axum::routing::{get, post};
use research_server::App;
use research_server::config::Settings;
use research_server::store::Store;
use serde_json::{Value, json};

/// One request a stand-in was asked: the unit, the method, the path with its query, and the body.
#[derive(Debug, Clone)]
pub struct Asked {
  pub unit: &'static str,
  pub method: String,
  pub path: String,
  pub body: Value,
}

/// A document the stand-in archive holds for a run.
#[derive(Debug, Clone)]
pub struct Filed {
  pub id: i64,
  pub title: String,
  pub description: String,
  pub content: String,
}

/// How the stand-ins answer, and what they were asked.
#[derive(Default)]
pub struct World {
  pub asked: Vec<Asked>,
  /// The statuses the next research-run asks are answered with; 202 once these run out.
  pub run_answers: VecDeque<u16>,
  pub next_run: i64,
  /// Each run's outcome as its ledger closes it; `answered` where none is named.
  pub outcomes: BTreeMap<i64, String>,
  /// How many readings a run stays open for before it closes.
  pub open_polls: u32,
  pub polls: BTreeMap<i64, u32>,
  /// The document filed under each run.
  pub filed: BTreeMap<i64, Filed>,
  /// The names the archive already holds.
  pub archived: Vec<String>,
  pub owner_room: i64,
  pub shares_made: i64,
  /// The statuses the next translation asks are answered with; 200 once these run out.
  pub translation_answers: VecDeque<u16>,
  /// A text longer than this many characters is refused 413 `too-large`.
  pub too_large_over: Option<usize>,
  /// The translations out at once now, and the most that ever were.
  pub translating: u32,
  pub translating_most: u32,
  /// The documents filed with `POST /documents`, as the archive answers them.
  pub posted: Vec<Value>,
  pub next_document: i64,
}

pub type Shared = Arc<Mutex<World>>;

pub fn world() -> Shared {
  Arc::new(Mutex::new(World {
    next_run: 100,
    owner_room: 7,
    next_document: 900,
    ..World::default()
  }))
}

/// What the stand-in translator answers for a text: the text marked as translated.
pub fn translated(text: &str) -> String {
  format!("[ko] {text}")
}

fn keep(world: &Shared, unit: &'static str, method: &Method, uri: &Uri, body: &Bytes) {
  let body = serde_json::from_slice(body).unwrap_or(Value::Null);
  world.lock().unwrap().asked.push(Asked {
    unit,
    method: method.to_string(),
    path: uri.to_string(),
    body,
  });
}

/// Serves a router on a loopback port of its own: its address.
pub async fn serve(router: Router) -> String {
  let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
  let addr = listener.local_addr().unwrap();
  tokio::spawn(async move { axum::serve(listener, router).await.unwrap() });
  format!("http://{addr}")
}

async fn agency_research_run(State(w): State<Shared>, method: Method, uri: Uri, body: Bytes) -> Response {
  keep(&w, "agency", &method, &uri, &body);
  let mut world = w.lock().unwrap();
  let status = world.run_answers.pop_front().unwrap_or(202);
  if status != 202 {
    return (
      StatusCode::from_u16(status).unwrap(),
      axum::Json(json!({"code": "no-login-open", "error": "no road carries it"})),
    )
      .into_response();
  }
  world.next_run += 1;
  let run = world.next_run;
  (StatusCode::ACCEPTED, axum::Json(json!({"runId": run, "key": "search:exhaustive"}))).into_response()
}

async fn agency_run(
  State(w): State<Shared>,
  Path(run): Path<i64>,
  method: Method,
  uri: Uri,
  body: Bytes,
) -> Response {
  keep(&w, "agency", &method, &uri, &body);
  let mut world = w.lock().unwrap();
  let open = world.open_polls;
  let polls = world.polls.entry(run).or_insert(0);
  *polls += 1;
  if *polls <= open {
    return axum::Json(json!({"id": run, "state": "open", "outcome": null})).into_response();
  }
  let outcome = world.outcomes.get(&run).cloned().unwrap_or_else(|| "answered".into());
  axum::Json(json!({"id": run, "state": "closed", "outcome": outcome})).into_response()
}

async fn agency_turn(State(w): State<Shared>, method: Method, uri: Uri, body: Bytes) -> Response {
  keep(&w, "agency", &method, &uri, &body);
  (StatusCode::CREATED, axum::Json(json!({"id": 1, "state": "ended"}))).into_response()
}

async fn agency_translation(State(w): State<Shared>, method: Method, uri: Uri, body: Bytes) -> Response {
  keep(&w, "agency", &method, &uri, &body);
  let asked: Value = serde_json::from_slice(&body).unwrap_or_default();
  let text = asked["text"].as_str().unwrap_or_default().to_string();
  {
    let mut world = w.lock().unwrap();
    let status = world.translation_answers.pop_front().unwrap_or(200);
    if status != 200 {
      let code = if status == 413 { "too-large" } else { "no-login-open" };
      return (StatusCode::from_u16(status).unwrap(), axum::Json(json!({"code": code, "error": code})))
        .into_response();
    }
    if world.too_large_over.is_some_and(|most| text.chars().count() > most) {
      return (
        StatusCode::PAYLOAD_TOO_LARGE,
        axum::Json(json!({"code": "too-large", "error": "longer than the window"})),
      )
        .into_response();
    }
    world.translating += 1;
    world.translating_most = world.translating_most.max(world.translating);
  }
  tokio::time::sleep(std::time::Duration::from_millis(20)).await;
  w.lock().unwrap().translating -= 1;
  axum::Json(json!({"text": translated(&text), "provider": "registered", "model": "translator"}))
    .into_response()
}

pub fn agency(world: Shared) -> Router {
  Router::new()
    .route("/research-runs", post(agency_research_run))
    .route("/translation-runs", post(agency_translation))
    .route("/runs/{run}", get(agency_run))
    .route("/sessions/{session}/turns", post(agency_turn))
    .with_state(world)
}

async fn artifact_documents(
  State(w): State<Shared>,
  Query(q): Query<BTreeMap<String, String>>,
  method: Method,
  uri: Uri,
  body: Bytes,
) -> Response {
  keep(&w, "artifact", &method, &uri, &body);
  let mut world = w.lock().unwrap();
  if method == Method::POST {
    world.next_document += 1;
    let mut document: Value = serde_json::from_slice(&body).unwrap_or_default();
    document["id"] = json!(world.next_document);
    world.posted.push(document.clone());
    return (StatusCode::CREATED, axum::Json(document)).into_response();
  }
  let mut items = Vec::new();
  if let Some(label) = q.get("label")
    && let Some(run) = label.strip_prefix("research:")
  {
    // The newest first: a rendering labelled with the same value stands ahead of the run's document.
    items.extend(world.posted.iter().rev().filter(|d| d["labels"]["research"] == run).cloned());
    if let Some(doc) = run.parse::<i64>().ok().and_then(|r| world.filed.get(&r)) {
      items.push(json!({
        "id": doc.id, "title": doc.title, "description": doc.description, "labels": {"research": run},
      }));
    }
  }
  if let Some(phrase) = q.get("q")
    && world.archived.iter().any(|a| a.eq_ignore_ascii_case(phrase))
  {
    items.push(json!({"id": 1, "title": phrase, "description": null}));
  }
  axum::Json(json!({"items": items, "more": false})).into_response()
}

async fn artifact_content(
  State(w): State<Shared>,
  Path(id): Path<i64>,
  method: Method,
  uri: Uri,
  body: Bytes,
) -> Response {
  keep(&w, "artifact", &method, &uri, &body);
  let world = w.lock().unwrap();
  match world.filed.values().find(|d| d.id == id) {
    Some(doc) => doc.content.clone().into_response(),
    None => {
      (StatusCode::NOT_FOUND, axum::Json(json!({"code": "not-found", "error": "none"}))).into_response()
    }
  }
}

async fn artifact_shares(State(w): State<Shared>, method: Method, uri: Uri, body: Bytes) -> Response {
  keep(&w, "artifact", &method, &uri, &body);
  if method == Method::GET {
    return axum::Json(json!({"items": [], "more": false})).into_response();
  }
  let mut world = w.lock().unwrap();
  world.shares_made += 1;
  let document = serde_json::from_slice::<Value>(&body).unwrap_or_default()["documentId"]
    .as_i64()
    .unwrap_or(0);
  (
    StatusCode::CREATED,
    axum::Json(json!({"token": "abcdefgh", "url": format!("https://share.example/d{document}")})),
  )
    .into_response()
}

pub fn artifact(world: Shared) -> Router {
  Router::new()
    .route("/documents", get(artifact_documents).post(artifact_documents))
    .route("/documents/{id}/content", get(artifact_content))
    .route("/shares", get(artifact_shares).post(artifact_shares))
    .with_state(world)
}

async fn receiver_owner(State(w): State<Shared>, method: Method, uri: Uri, body: Bytes) -> Response {
  keep(&w, "receiver", &method, &uri, &body);
  let room = w.lock().unwrap().owner_room;
  axum::Json(json!({"senderId": 1, "room": room})).into_response()
}

async fn receiver_instruction(State(w): State<Shared>, method: Method, uri: Uri, body: Bytes) -> Response {
  keep(&w, "receiver", &method, &uri, &body);
  let n = w.lock().unwrap().asked.len() as i64;
  (StatusCode::CREATED, axum::Json(json!({"id": n}))).into_response()
}

pub fn receiver(world: Shared) -> Router {
  Router::new()
    .route("/owner", get(receiver_owner))
    .route("/instructions", post(receiver_instruction))
    .with_state(world)
}

/// A fixture's text, from `tests/fixtures/`.
pub fn fixture(name: &str) -> String {
  std::fs::read_to_string(PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures").join(name))
    .expect("the fixture reads")
}

/// A stand-in for the sources: each fixture served at the path its kind asks.
pub fn sources(world: Shared) -> Router {
  async fn answer(
    State((w, body, kind)): State<(Shared, String, &'static str)>,
    method: Method,
    uri: Uri,
  ) -> Response {
    keep(&w, "source", &method, &uri, &Bytes::new());
    ([(axum::http::header::CONTENT_TYPE, kind)], body).into_response()
  }
  Router::new()
    .route(
      "/api/models",
      get(answer).with_state((world.clone(), fixture("huggingface.json"), "application/json")),
    )
    .route(
      "/api/query",
      get(answer).with_state((world.clone(), fixture("arxiv.xml"), "application/atom+xml")),
    )
    .route(
      "/repos/example-org/example-engine/releases",
      get(answer).with_state((world.clone(), fixture("releases.json"), "application/json")),
    )
    .route("/feed.xml", get(answer).with_state((world, fixture("feed.xml"), "application/rss+xml")))
}

/// A scratch data root, its own for each call.
pub fn scratch() -> PathBuf {
  static NEXT: AtomicU64 = AtomicU64::new(0);
  let n = NEXT.fetch_add(1, Ordering::SeqCst);
  let dir = std::env::temp_dir().join(format!("bmo-research-test-{}-{n}", std::process::id()));
  let _ = std::fs::remove_dir_all(&dir);
  std::fs::create_dir_all(dir.join("env")).unwrap();
  dir
}

/// The ground a test stands on: the stand-ins, the data root naming them, and the daemon's app over it.
pub struct Ground {
  pub world: Shared,
  pub app: Arc<App>,
  pub root: PathBuf,
  pub sources: String,
}

impl Ground {
  pub fn asked(&self, unit: &str) -> Vec<Asked> {
    self.world.lock().unwrap().asked.iter().filter(|a| a.unit == unit).cloned().collect()
  }

  pub fn asked_path(&self, unit: &str, method: &str, path: &str) -> Vec<Asked> {
    self
      .asked(unit)
      .into_iter()
      .filter(|a| a.method == method && a.path.starts_with(path))
      .collect()
  }
}

/// The stand-ins served, their addresses and tokens written into the data root's `env/` files the way each
/// unit keeps them, and the daemon's app over a fresh store.
pub async fn ground(change: impl FnOnce(&mut Settings)) -> Ground {
  let world = world();
  let agency = serve(agency(world.clone())).await;
  let artifact = serve(artifact(world.clone())).await;
  let receiver = serve(receiver(world.clone())).await;
  let sources = serve(sources(world.clone())).await;
  let root = scratch();
  let port = agency.rsplit(':').next().unwrap();
  std::fs::write(
    root.join("env/agency.env"),
    format!("AGENCY_CONTROL_PORT={port}\nAGENCY_CONTROL_TOKEN=agency-test\n"),
  )
  .unwrap();
  std::fs::write(
    root.join("env/receiver.env"),
    format!("RECEIVER_ADDRESS={receiver}\nRECEIVER_TOKEN=receiver-test\n"),
  )
  .unwrap();
  std::fs::write(root.join("env/artifact.token"), "artifact-test\n").unwrap();
  let mut settings = Settings::read("").unwrap();
  settings.data_root = root.clone();
  settings.artifact.address = artifact;
  settings.poll_seconds = 0;
  settings.turn_wait_seconds = 5;
  change(&mut settings);
  let store = Store::open(&settings.store_dir()).unwrap();
  let app = App::new(settings, store, "test-token".into());
  Ground {
    world,
    app,
    root,
    sources,
  }
}
