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

/// How a research run's ledger row stands.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RunState {
  pub state: String,
  pub outcome: Option<String>,
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

  /// A research run asked of agency on the road named, with no room: its ledger run's id.
  pub async fn ask_run(&self, query: &str, provider: &str, model: &str) -> Result<i64, String> {
    let agency = self.agency()?;
    let body =
      json!({ "key": self.settings.road.key, "query": query, "provider": provider, "model": model });
    let answer = self
      .call(
        &agency,
        self.request(&agency, reqwest::Method::POST, "/research-runs").json(&body),
        "POST /research-runs",
      )
      .await?;
    answer["runId"].as_i64().ok_or_else(|| "agency answered no runId".to_string())
  }

  /// A ledger run's state and outcome.
  pub async fn run_state(&self, run_id: i64) -> Result<RunState, String> {
    let agency = self.agency()?;
    let path = format!("/runs/{run_id}");
    let answer = self
      .call(&agency, self.request(&agency, reqwest::Method::GET, &path), &format!("GET {path}"))
      .await?;
    Ok(RunState {
      state: answer["state"].as_str().unwrap_or_default().to_string(),
      outcome: answer["outcome"].as_str().map(str::to_string),
    })
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

  /// The document agency filed for a run, under the label `research` with the run's id. A Korean rendering
  /// carries the same label with its item's id, so a document labelled as a translation is passed over.
  pub async fn filed_document(&self, run_id: i64) -> Result<Option<Document>, String> {
    let label = format!("research:{run_id}");
    let listed = self.documents(&[("label", label.as_str()), ("limit", "20")]).await?;
    Ok(listed.into_iter().find(|d| !d.labels.contains_key("translationOf")))
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

  /// A notice queued for a room's answerer: the instruction's id.
  pub async fn instruct(
    &self,
    room: i64,
    item_id: i64,
    notice: &str,
    summary: &str,
  ) -> Result<Option<i64>, String> {
    let receiver = self.receiver()?;
    let body = json!({
      "room": room,
      "sort": "notice",
      "kind": format!("research-report:{item_id}"),
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
}
