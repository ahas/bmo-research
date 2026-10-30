//! The calls this unit makes into other units (`docs/api.md`, 「Calls this unit makes」), each at the address
//! and with the token that unit's own files under the data root's `env/` hold, read afresh at every call.

use std::collections::BTreeMap;
use std::path::Path;
use std::sync::Arc;
use std::time::Duration;

use serde::Deserialize;
use serde_json::{Value, json};

use crate::config::Settings;

/// A unit's address and the bearer it takes.
#[derive(Debug, Clone)]
pub struct Peer {
  pub name: &'static str,
  pub address: String,
  pub token: Option<String>,
}

/// The variables an env file holds, read as the unit that keeps it writes it.
pub fn env_file(path: &Path) -> Result<BTreeMap<String, String>, String> {
  let lines = dotenvy::from_path_iter(path).map_err(|e| format!("{}: {e}", path.display()))?;
  lines.map(|line| line.map_err(|e| format!("{}: {e}", path.display()))).collect()
}

fn token_file(path: &Path) -> Result<String, String> {
  std::fs::read_to_string(path)
    .map(|t| t.trim().to_string())
    .map_err(|e| format!("{}: {e}", path.display()))
}

/// A document of the artifact service, the fields the daemon reads.
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Document {
  pub id: i64,
  pub title: String,
  #[serde(default)]
  pub description: Option<String>,
  #[serde(default)]
  pub labels: BTreeMap<String, Value>,
}

/// Why a part came back untranslated: the translator found it too long, so it is cut smaller; or anything
/// else, which leaves the rendering pending.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Untranslated {
  TooLarge,
  Failed(String),
}

/// A Korean rendering to file beside its English document.
#[derive(Debug, Clone)]
pub struct NewRendering<'a> {
  pub item_id: i64,
  pub translation_of: i64,
  pub title: &'a str,
  pub description: Option<&'a str>,
  pub body: &'a str,
}

/// Who the daemon files a document as.
const ACTOR: &str = "bmo-research";

#[derive(Deserialize)]
struct Listed<T> {
  items: Vec<T>,
}

#[derive(Deserialize)]
struct Share {
  url: String,
}

/// The parts of the arbiter master's cluster picture the bench watch reads: each node's cards, each card's
/// turn and the tickets waiting in its queue, by the service each stands for.
#[derive(Deserialize)]
struct Cluster {
  nodes: Vec<ClusterNode>,
}

#[derive(Deserialize)]
struct ClusterNode {
  name: String,
  #[serde(default)]
  cards: Vec<ClusterCard>,
}

#[derive(Deserialize)]
struct ClusterCard {
  #[serde(default)]
  turn: Option<Ticket>,
  #[serde(default)]
  queue: Vec<Ticket>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct Ticket {
  service_name: String,
}

#[derive(Clone)]
pub struct Peers {
  http: reqwest::Client,
  settings: Arc<Settings>,
}

impl Peers {
  pub fn new(settings: Arc<Settings>) -> Peers {
    Peers {
      http: reqwest::Client::new(),
      settings,
    }
  }

  /// Agency at the loopback port its env file names, 3220 where it names none.
  pub fn agency(&self) -> Result<Peer, String> {
    let vars = env_file(&self.settings.env_file(&self.settings.agency.env))?;
    let port = vars
      .get("AGENCY_CONTROL_PORT")
      .map(|p| p.trim().to_string())
      .unwrap_or_else(|| "3220".into());
    Ok(Peer {
      name: "agency",
      address: format!("http://127.0.0.1:{port}"),
      token: vars.get("AGENCY_CONTROL_TOKEN").cloned(),
    })
  }

  pub fn receiver(&self) -> Result<Peer, String> {
    let path = self.settings.env_file(&self.settings.receiver.env);
    let vars = env_file(&path)?;
    let address = vars
      .get("RECEIVER_ADDRESS")
      .cloned()
      .ok_or_else(|| format!("{} names no RECEIVER_ADDRESS", path.display()))?;
    Ok(Peer {
      name: "the receiver",
      address,
      token: vars.get("RECEIVER_TOKEN").cloned(),
    })
  }

  /// The arbiter's master at the bench watch's address, with the token its file holds where one is named.
  pub fn arbiter(&self) -> Result<Peer, String> {
    let watch = &self.settings.bench_watch;
    let token = match watch.token.trim() {
      "" => None,
      name => Some(token_file(&self.settings.env_file(name))?),
    };
    Ok(Peer {
      name: "the arbiter",
      address: watch.address.clone(),
      token,
    })
  }

  pub fn artifact(&self) -> Result<Peer, String> {
    Ok(Peer {
      name: "the artifact service",
      address: self.settings.artifact.address.clone(),
      token: Some(token_file(&self.settings.env_file(&self.settings.artifact.token))?),
    })
  }

  fn request(&self, peer: &Peer, method: reqwest::Method, path: &str) -> reqwest::RequestBuilder {
    let mut request = self
      .http
      .request(method, format!("{}{path}", peer.address.trim_end_matches('/')))
      .timeout(Duration::from_secs(60));
    if let Some(token) = &peer.token {
      request = request.bearer_auth(token);
    }
    request
  }

  /// One call answered with a success, its body read as JSON; any other answer is the failure's sentence.
  async fn call(&self, peer: &Peer, request: reqwest::RequestBuilder, what: &str) -> Result<Value, String> {
    let answer = request.send().await.map_err(|e| format!("{} {what}: {e}", peer.name))?;
    let status = answer.status().as_u16();
    let body = answer.text().await.unwrap_or_default();
    if !(200..300).contains(&status) {
      return Err(format!("{} {what} answered {status}: {body}", peer.name));
    }
    if body.trim().is_empty() {
      return Ok(Value::Null);
    }
    serde_json::from_str(&body).map_err(|e| format!("{} {what}: the answer did not read: {e}", peer.name))
  }

  /// Each node's occupants holding a ticket now, granted or waiting, as the arbiter master's cluster
  /// picture shows them, by node.
  pub async fn tickets(&self) -> Result<BTreeMap<String, Vec<String>>, String> {
    let arbiter = self.arbiter()?;
    let answer = self
      .call(&arbiter, self.request(&arbiter, reqwest::Method::GET, "/cluster"), "GET /cluster")
      .await?;
    let cluster: Cluster =
      serde_json::from_value(answer).map_err(|e| format!("the arbiter's cluster did not read: {e}"))?;
    Ok(
      cluster
        .nodes
        .into_iter()
        .map(|node| {
          let services = node
            .cards
            .into_iter()
            .flat_map(|card| card.turn.into_iter().chain(card.queue))
            .map(|ticket| ticket.service_name)
            .collect();
          (node.name, services)
        })
        .collect(),
    )
  }

  /// Whether a registration of agency's registry answers its health now.
  pub async fn healthy(&self, provider: &str, registration: &str) -> Result<bool, String> {
    let agency = self.agency()?;
    let path = format!("/models/{registration}");
    let answer = self
      .call(
        &agency,
        self.request(&agency, reqwest::Method::GET, &path).query(&[("provider", provider)]),
        &format!("GET {path}"),
      )
      .await?;
    Ok(answer["healthy"].as_bool().unwrap_or(false))
  }

  /// A notice told to an agent by a turn into its worker session. A turn still working after the wait is
  /// handed: agency carries a sent turn to its end.
  pub async fn turn(&self, agent: &str, prompt: &str) -> Result<(), String> {
    let agency = self.agency()?;
    let path = format!("/sessions/{agent}@worker/turns");
    let request = self
      .request(&agency, reqwest::Method::POST, &path)
      .timeout(Duration::from_secs(self.settings.turn_wait_seconds.max(1)))
      .json(&json!({ "prompt": prompt }));
    match request.send().await {
      Ok(answer) if answer.status().is_success() => Ok(()),
      Ok(answer) => {
        let status = answer.status().as_u16();
        Err(format!(
          "agency POST {path} answered {status}: {}",
          answer.text().await.unwrap_or_default()
        ))
      }
      Err(e) if e.is_timeout() => Ok(()),
      Err(e) => Err(format!("agency POST {path}: {e}")),
    }
  }

  /// One text translated by agency's translator, with the note that gives it its context.
  pub async fn translate(&self, language: &str, text: &str, note: &str) -> Result<String, Untranslated> {
    let agency = self.agency().map_err(Untranslated::Failed)?;
    let body = json!({ "purpose": "document", "language": language, "text": text, "note": note });
    let answer = self
      .request(&agency, reqwest::Method::POST, "/translation-runs")
      .timeout(Duration::from_secs(600))
      .json(&body)
      .send()
      .await
      .map_err(|e| Untranslated::Failed(format!("agency POST /translation-runs: {e}")))?;
    let status = answer.status().as_u16();
    let said = answer.text().await.unwrap_or_default();
    if status == 413 {
      return Err(Untranslated::TooLarge);
    }
    if !(200..300).contains(&status) {
      return Err(Untranslated::Failed(format!("agency POST /translation-runs answered {status}: {said}")));
    }
    let read: Value = serde_json::from_str(&said).map_err(|e| {
      Untranslated::Failed(format!("agency POST /translation-runs: the answer did not read: {e}"))
    })?;
    read["text"]
      .as_str()
      .map(str::to_string)
      .ok_or_else(|| Untranslated::Failed("agency's translation answered no text".into()))
  }

  /// A Korean rendering filed as a record of its own, labelled as its English document's translation: its id.
  pub async fn file_rendering(&self, rendering: &NewRendering<'_>) -> Result<i64, String> {
    let artifact = self.artifact()?;
    let mut body = json!({
      "kind": "record",
      "title": rendering.title,
      "body": rendering.body,
      "actorName": ACTOR,
      "labels": {
        "research": rendering.item_id.to_string(),
        "translationOf": rendering.translation_of.to_string(),
        "language": "ko",
      },
    });
    if let Some(description) = rendering.description {
      body["description"] = json!(description);
    }
    let answer = self
      .call(
        &artifact,
        self.request(&artifact, reqwest::Method::POST, "/documents").json(&body),
        "POST /documents",
      )
      .await?;
    answer["id"]
      .as_i64()
      .ok_or_else(|| "the artifact service answered no document id".to_string())
  }

  async fn documents(&self, query: &[(&str, &str)]) -> Result<Vec<Document>, String> {
    let artifact = self.artifact()?;
    let answer = self
      .call(
        &artifact,
        self.request(&artifact, reqwest::Method::GET, "/documents").query(query),
        "GET /documents",
      )
      .await?;
    let listed: Listed<Document> = serde_json::from_value(answer)
      .map_err(|e| format!("the artifact service's documents did not read: {e}"))?;
    Ok(listed.items)
  }

  /// Whether the archive holds a document saying the name.
  pub async fn archive_holds(&self, name: &str) -> Result<bool, String> {
    let phrase: String = name.chars().take(200).collect();
    Ok(!self.documents(&[("q", phrase.as_str()), ("limit", "1")]).await?.is_empty())
  }

  pub async fn content(&self, document_id: i64) -> Result<String, String> {
    let artifact = self.artifact()?;
    let path = format!("/documents/{document_id}/content");
    let answer = self
      .request(&artifact, reqwest::Method::GET, &path)
      .send()
      .await
      .map_err(|e| format!("the artifact service GET {path}: {e}"))?;
    let status = answer.status().as_u16();
    let body = answer.text().await.map_err(|e| format!("the artifact service GET {path}: {e}"))?;
    if !(200..300).contains(&status) {
      return Err(format!("the artifact service GET {path} answered {status}: {body}"));
    }
    Ok(body)
  }

  /// The document's live share link, made where none stands.
  pub async fn share(&self, document_id: i64) -> Result<String, String> {
    let artifact = self.artifact()?;
    let id = document_id.to_string();
    let standing = self
      .call(
        &artifact,
        self
          .request(&artifact, reqwest::Method::GET, "/shares")
          .query(&[("documentId", id.as_str())]),
        "GET /shares",
      )
      .await?;
    let listed: Listed<Share> = serde_json::from_value(standing)
      .map_err(|e| format!("the artifact service's shares did not read: {e}"))?;
    if let Some(share) = listed.items.into_iter().next() {
      return Ok(share.url);
    }
    let made = self
      .call(
        &artifact,
        self
          .request(&artifact, reqwest::Method::POST, "/shares")
          .json(&json!({ "documentId": document_id })),
        "POST /shares",
      )
      .await?;
    let share: Share = serde_json::from_value(made)
      .map_err(|e| format!("the artifact service's share did not read: {e}"))?;
    Ok(share.url)
  }

  /// The owner's private room, as the receiver answers it.
  pub async fn owner_room(&self) -> Result<i64, String> {
    let receiver = self.receiver()?;
    let answer = self
      .call(&receiver, self.request(&receiver, reqwest::Method::GET, "/owner"), "GET /owner")
      .await?;
    answer["room"].as_i64().ok_or_else(|| "the receiver's owner names no room".to_string())
  }

  /// A notice queued for a room's answerer under its kind: the instruction's id.
  pub async fn instruct(
    &self,
    room: i64,
    kind: &str,
    notice: &str,
    summary: &str,
  ) -> Result<Option<i64>, String> {
    let receiver = self.receiver()?;
    let body = json!({
      "room": room,
      "sort": "notice",
      "kind": kind,
      "said": { "order": notice },
      "summary": summary,
    });
    let answer = self
      .call(
        &receiver,
        self.request(&receiver, reqwest::Method::POST, "/instructions").json(&body),
        "POST /instructions",
      )
      .await?;
    Ok(answer["id"].as_i64())
  }

  // Agency's seats and thrown-away sessions, as the runner asks them (`docs/runner.md`).

  /// A seat asked of the co-worker pool: the request as agency answers it.
  pub async fn ask_seat(&self, ask: &Value) -> Result<Value, String> {
    let agency = self.agency()?;
    let request = self.request(&agency, reqwest::Method::POST, "/seat-requests").timeout(WITHIN).json(ask);
    self.call(&agency, request, "POST /seat-requests").await
  }

  /// A seat moved: asked for its place, bound to its session, or given back.
  pub async fn move_seat(&self, token: &str, change: &Value) -> Result<Value, String> {
    let agency = self.agency()?;
    let path = format!("/seat-requests/{}", segment(token));
    let request = self.request(&agency, reqwest::Method::PATCH, &path).timeout(WITHIN).json(change);
    self.call(&agency, request, &format!("PATCH {path}")).await
  }

  /// A thrown-away session started: its id.
  pub async fn start_session(&self, start: &Value) -> Result<String, String> {
    let agency = self.agency()?;
    let request = self.request(&agency, reqwest::Method::POST, "/sessions").timeout(WITHIN).json(start);
    let answer = self.call(&agency, request, "POST /sessions").await?;
    answer["id"]
      .as_str()
      .filter(|id| !id.is_empty())
      .map(str::to_string)
      .ok_or_else(|| "agency answered the session with no id".to_string())
  }

  /// One turn into a session, answered once it ends: the turn's end as agency answers it.
  pub async fn send_turn(&self, session: &str, body: &Value, within: Duration) -> Result<Value, String> {
    let agency = self.agency()?;
    let path = format!("/sessions/{}/turns", segment(session));
    let request = self.request(&agency, reqwest::Method::POST, &path).timeout(within).json(body);
    self.call(&agency, request, &format!("POST {path}")).await
  }

  /// A session ended; `Busy` while a turn of it still runs.
  pub async fn end_session(&self, session: &str) -> Result<(), Ending> {
    let agency = self.agency().map_err(Ending::Failed)?;
    let path = format!("/sessions/{}", segment(session));
    let answer = self
      .request(&agency, reqwest::Method::DELETE, &path)
      .timeout(WITHIN)
      .send()
      .await
      .map_err(|e| Ending::Failed(format!("agency DELETE {path}: {e}")))?;
    match answer.status().as_u16() {
      200..=299 | 404 => Ok(()),
      409 => Err(Ending::Busy),
      status => Err(Ending::Failed(format!(
        "agency DELETE {path} answered {status}: {}",
        answer.text().await.unwrap_or_default()
      ))),
    }
  }

  /// The co-worker roles agency's pool can lend, by name.
  pub async fn roles(&self) -> Result<Vec<String>, String> {
    let agency = self.agency()?;
    let answer = self
      .call(
        &agency,
        self.request(&agency, reqwest::Method::GET, "/roles").timeout(WITHIN),
        "GET /roles",
      )
      .await?;
    Ok(
      answer["items"]
        .as_array()
        .map(|roles| roles.iter().filter_map(|r| r["name"].as_str().map(str::to_string)).collect())
        .unwrap_or_default(),
    )
  }

  /// A research run's end queued for its room's answerer, the body as the runner composed it.
  pub async fn instruct_end(&self, body: &Value) -> Result<(), String> {
    let receiver = self.receiver()?;
    let request = self.request(&receiver, reqwest::Method::POST, "/instructions").json(body);
    self.call(&receiver, request, "POST /instructions").await.map(|_| ())
  }

  /// One call of the artifact service's routes with a JSON body: its answer.
  pub async fn artifact_send(
    &self,
    method: reqwest::Method,
    path: &str,
    body: &Value,
  ) -> Result<Value, String> {
    let artifact = self.artifact()?;
    let what = format!("{method} {path}");
    self.call(&artifact, self.request(&artifact, method, path).json(body), &what).await
  }
}

/// Why a session was not ended: a turn of it still runs, or the call failed.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Ending {
  Busy,
  Failed(String),
}

/// How long one call of agency's seat and session routes may take, a turn's send aside.
const WITHIN: Duration = Duration::from_secs(30);

/// An identifier agency gave, written into a path as one segment.
fn segment(identifier: &str) -> String {
  url::form_urlencoded::byte_serialize(identifier.as_bytes())
    .collect::<String>()
    .replace('+', "%20")
}
