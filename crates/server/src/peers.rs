//! The calls this unit makes into other units (`docs/api.md`, 「Calls this unit makes」), each at the address
//! and with the token that unit's own files under the data root's `env/` hold, read afresh at every call.

use std::collections::BTreeMap;
use std::path::Path;
use std::sync::Arc;
use std::time::Duration;

use serde::Deserialize;
use serde_json::{Value, json};
use tokio::sync::Semaphore;

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

/// Why a call into agency for a run's seat, its session or a turn brought no answer: the failure's sentence,
/// and whether what the run stands on went away (`docs/runner.md`, 「A cut run」) — the call did not reach
/// agency, agency answered with a failure of its own (5xx), or, for a call naming a seat or a session, agency
/// answered that it no longer holds it (404).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Fault {
  pub why: String,
  pub lost: bool,
}

impl Fault {
  fn kept(why: String) -> Fault {
    Fault { why, lost: false }
  }
}

impl std::fmt::Display for Fault {
  fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    f.write_str(&self.why)
  }
}

impl From<Fault> for String {
  fn from(fault: Fault) -> String {
    fault.why
  }
}

/// How a synthesis of the voice unit stands as a reading finds it: waiting for its card or being spoken, with
/// the voice unit's sentence on what it waits on where it says one; spoken; failed; or gone, the voice unit
/// holding none under its id.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Synthesis {
  Going(Option<String>),
  Done,
  Failed,
  Gone,
}

/// The job service's own settings file, the part the daemon reads: where it listens and the file its token
/// stands in.
#[derive(Deserialize)]
struct JobFile {
  server: JobServer,
}

#[derive(Deserialize)]
struct JobServer {
  listen: String,
  #[serde(default)]
  token_file: Option<String>,
}

/// The voice unit's own settings file, the parts the daemon reads: where it answers and the file under `env/`
/// its token stands in.
#[derive(Deserialize)]
struct VoiceFile {
  address: String,
  #[serde(default)]
  token_file: Option<String>,
}

/// An attachment of a document, as the artifact service lists it: its name, all that is read of it.
#[derive(Deserialize)]
struct Attachment {
  name: String,
}

#[derive(Deserialize)]
struct Listed<T> {
  items: Vec<T>,
}

#[derive(Deserialize)]
struct Share {
  url: String,
}

/// A demand as the arbiter's master answers it, the parts the daemon reads: its id, its state (`waiting`,
/// `granted`, `partly` or `lent`), who took its card where one did, and why a card waits.
#[derive(Debug, Clone, Default, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Demand {
  pub id: i64,
  #[serde(default)]
  pub state: String,
  #[serde(default)]
  pub taken: Option<Value>,
  #[serde(default)]
  pub reason: Option<CardWait>,
}

/// Why a demand's card waits, as the master says it: its code and its sentence.
#[derive(Debug, Clone, Default, Deserialize)]
pub struct CardWait {
  #[serde(default)]
  pub code: Option<String>,
  #[serde(default)]
  pub text: Option<String>,
}

impl Demand {
  pub fn granted(&self) -> bool {
    self.state == "granted"
  }

  /// Whether a claimant or a lower tier took its card: the master names who did, or its card is no longer
  /// granted or lent.
  pub fn taken(&self) -> bool {
    self.taken.as_ref().is_some_and(|who| !who.is_null())
      || !matches!(self.state.as_str(), "granted" | "lent")
  }
}

#[derive(Clone)]
pub struct Peers {
  http: reqwest::Client,
  settings: Arc<Settings>,
  /// The translations out with agency's translator at once, across every page: at most
  /// `translation.parts_out_total`, every other waiting its turn here in the order it was asked.
  translating: Arc<Semaphore>,
}

impl Peers {
  pub fn new(settings: Arc<Settings>) -> Peers {
    Peers {
      http: reqwest::Client::new(),
      translating: Arc::new(Semaphore::new(settings.translation.parts_out_total.max(1))),
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

  /// The arbiter's master at the engine watch's address, with the token its file holds where one is named.
  pub fn arbiter(&self) -> Result<Peer, String> {
    let watch = &self.settings.engine;
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

  /// The voice unit at the address its own settings file names, with the token of the file it names.
  pub fn voice(&self) -> Result<Peer, String> {
    let path = self.settings.env_file(&self.settings.voice.settings);
    let text = std::fs::read_to_string(&path).map_err(|e| format!("{}: {e}", path.display()))?;
    let file: VoiceFile = toml::from_str(&text).map_err(|e| format!("{}: {e}", path.display()))?;
    let token = match file.token_file.as_deref().map(str::trim).filter(|name| !name.is_empty()) {
      Some(name) => Some(token_file(&self.settings.env_file(name))?),
      None => None,
    };
    Ok(Peer {
      name: "the voice unit",
      address: file.address,
      token,
    })
  }

  /// The job service at the address its own settings file listens on, the loopback where it listens on every
  /// address, with the token of the file it names.
  pub fn job(&self) -> Result<Peer, String> {
    let path = self.settings.env_file(&self.settings.job.settings);
    let text = std::fs::read_to_string(&path).map_err(|e| format!("{}: {e}", path.display()))?;
    let file: JobFile = toml::from_str(&text).map_err(|e| format!("{}: {e}", path.display()))?;
    let listen = file.server.listen.trim();
    let address = match listen.rsplit_once(':') {
      Some(("0.0.0.0" | "[::]", port)) => format!("http://127.0.0.1:{port}"),
      _ => format!("http://{listen}"),
    };
    let token = match file.server.token_file.as_deref().map(str::trim).filter(|name| !name.is_empty()) {
      Some(name) => Some(token_file(&self.settings.env_file(name))?),
      None => None,
    };
    Ok(Peer {
      name: "the job service",
      address,
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
    self.unclocked(peer, method, path).timeout(Duration::from_secs(60))
  }

  /// A request with no clock of this unit's, waited on until its unit answers.
  fn unclocked(&self, peer: &Peer, method: reqwest::Method, path: &str) -> reqwest::RequestBuilder {
    let request = self.http.request(method, format!("{}{path}", peer.address.trim_end_matches('/')));
    match &peer.token {
      Some(token) => request.bearer_auth(token),
      None => request,
    }
  }

  /// One call answered with a success, its body read as JSON; any other answer is the failure's sentence.
  async fn call(&self, peer: &Peer, request: reqwest::RequestBuilder, what: &str) -> Result<Value, String> {
    self.held_call(peer, request, what, false).await.map_err(String::from)
  }

  /// One call answered with a success, its body read as JSON; any other answer is the failure, lost where
  /// the call or its answer did not come through, the unit answered 5xx, or — for a call that `names_held`,
  /// naming something the unit holds — 404.
  async fn held_call(
    &self,
    peer: &Peer,
    request: reqwest::RequestBuilder,
    what: &str,
    names_held: bool,
  ) -> Result<Value, Fault> {
    let lost = |why: String| Fault { why, lost: true };
    let answer = request.send().await.map_err(|e| lost(format!("{} {what}: {e}", peer.name)))?;
    let status = answer.status().as_u16();
    let succeeded = (200..300).contains(&status);
    let body = match answer.text().await {
      Ok(body) => body,
      Err(e) if succeeded => {
        return Err(lost(format!("{} {what}: the answer did not come: {e}", peer.name)));
      }
      Err(_) => String::new(),
    };
    if !succeeded {
      return Err(Fault {
        why: format!("{} {what} answered {status}: {body}", peer.name),
        lost: status >= 500 || (names_held && status == 404),
      });
    }
    if body.trim().is_empty() {
      return Ok(Value::Null);
    }
    serde_json::from_str(&body)
      .map_err(|e| Fault::kept(format!("{} {what}: the answer did not read: {e}", peer.name)))
  }

  /// A demand asked of the arbiter's master, answered as it stands: granted, or waiting.
  pub async fn ask_demand(&self, body: &Value) -> Result<Demand, String> {
    let arbiter = self.arbiter()?;
    let answer = self
      .call(
        &arbiter,
        self.request(&arbiter, reqwest::Method::POST, "/demands").json(body),
        "POST /demands",
      )
      .await?;
    serde_json::from_value(answer).map_err(|e| format!("the arbiter's demand did not read: {e}"))
  }

  /// A demand read, its answer held open up to `wait_ms` for a change; none where the master no longer
  /// holds it.
  pub async fn read_demand(&self, id: i64, wait_ms: u64) -> Result<Option<Demand>, String> {
    let arbiter = self.arbiter()?;
    let path = format!("/demands/{id}");
    let answer = self
      .request(&arbiter, reqwest::Method::GET, &path)
      .query(&[("waitMs", wait_ms)])
      .timeout(Duration::from_millis(wait_ms) + Duration::from_secs(30))
      .send()
      .await
      .map_err(|e| format!("the arbiter GET {path}: {e}"))?;
    let status = answer.status().as_u16();
    let body = answer.text().await.unwrap_or_default();
    match status {
      404 => Ok(None),
      200..300 => serde_json::from_str(&body)
        .map(Some)
        .map_err(|e| format!("the arbiter GET {path}: the answer did not read: {e}")),
      _ => Err(format!("the arbiter GET {path} answered {status}: {body}")),
    }
  }

  /// A demand given back; one the master no longer holds is given back already.
  pub async fn give_back_demand(&self, id: i64) -> Result<(), String> {
    let arbiter = self.arbiter()?;
    let path = format!("/demands/{id}");
    let answer = self
      .request(&arbiter, reqwest::Method::DELETE, &path)
      .send()
      .await
      .map_err(|e| format!("the arbiter DELETE {path}: {e}"))?;
    match answer.status().as_u16() {
      200..300 | 404 => Ok(()),
      status => Err(format!(
        "the arbiter DELETE {path} answered {status}: {}",
        answer.text().await.unwrap_or_default()
      )),
    }
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

  /// One text translated by agency's translator, with the note that gives it its context and the terms it keeps
  /// exactly as written: sent once fewer than `translation.parts_out_total` others are out, and waited on until
  /// agency answers, however long the card the translations share takes to be granted.
  pub async fn translate(
    &self,
    language: &str,
    text: &str,
    (note, terms): (&str, &[String]),
  ) -> Result<String, Untranslated> {
    let _out = self
      .translating
      .acquire()
      .await
      .map_err(|e| Untranslated::Failed(format!("the translator's turns are closed: {e}")))?;
    let agency = self.agency().map_err(Untranslated::Failed)?;
    let terms: Vec<Value> = terms.iter().map(|term| json!({ "source": term })).collect();
    let body = json!({
      "purpose": "document", "language": language, "text": text, "note": note, "terms": terms,
    });
    let answer = self
      .unclocked(&agency, reqwest::Method::POST, "/translation-runs")
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

  /// One text asked of the voice unit to speak in the settings' voice, else the first it lists, as a line of
  /// `language`: the synthesis's id, answered at once while it waits for its card.
  pub async fn ask_synthesis(&self, text: &str, language: &str, key: &str) -> Result<i64, String> {
    let voice = self.voice()?;
    let name = match self.settings.voice.name.trim() {
      "" => {
        let listed = self
          .call(&voice, self.request(&voice, reqwest::Method::GET, "/voices"), "GET /voices")
          .await?;
        listed["items"][0]["name"]
          .as_str()
          .map(str::to_owned)
          .ok_or_else(|| "the voice unit lists no voice".to_string())?
      }
      named => named.to_owned(),
    };
    let made = self
      .call(
        &voice,
        self
          .request(&voice, reqwest::Method::POST, "/syntheses")
          .json(&json!({ "voiceName": name, "text": text, "language": language, "key": key })),
        "POST /syntheses",
      )
      .await?;
    made["id"]
      .as_i64()
      .ok_or_else(|| "the voice unit answered the synthesis with no id".to_string())
  }

  /// One reading of a synthesis: how it stands, or the failure, lost where the voice unit was not reached or
  /// answered 5xx.
  pub async fn read_synthesis(&self, id: i64) -> Result<Synthesis, Fault> {
    let voice = self.voice().map_err(Fault::kept)?;
    let path = format!("/syntheses/{id}");
    let lost = |why: String| Fault { why, lost: true };
    let answer = self
      .request(&voice, reqwest::Method::GET, &path)
      .send()
      .await
      .map_err(|e| lost(format!("{} GET {path}: {e}", voice.name)))?;
    let status = answer.status().as_u16();
    if status == 404 {
      return Ok(Synthesis::Gone);
    }
    let body = answer
      .text()
      .await
      .map_err(|e| lost(format!("{} GET {path}: the answer did not come: {e}", voice.name)))?;
    if !(200..300).contains(&status) {
      let why = format!("{} GET {path} answered {status}: {body}", voice.name);
      return Err(Fault {
        why,
        lost: status >= 500,
      });
    }
    let read: Value = serde_json::from_str(&body)
      .map_err(|e| Fault::kept(format!("{} GET {path}: the answer did not read: {e}", voice.name)))?;
    Ok(match read["state"].as_str() {
      Some("done") => Synthesis::Done,
      Some("failed") => Synthesis::Failed,
      _ => Synthesis::Going(
        read["waitingOn"].as_str().map(str::trim).filter(|on| !on.is_empty()).map(str::to_owned),
      ),
    })
  }

  /// A synthesis followed until it is spoken, has failed or is gone, however long that takes: read every
  /// second, a reading that did not come through read again a second later. `waiting` is handed what it waits
  /// on each time the voice unit's sentence for it changes. Answers how it ended; a refusal other than a lost
  /// one is the failure.
  pub async fn follow_synthesis(
    &self,
    id: i64,
    mut waiting: impl FnMut(&str),
  ) -> Result<Synthesis, String> {
    let mut told: Option<String> = None;
    loop {
      match self.read_synthesis(id).await {
        Ok(Synthesis::Going(on)) => {
          if let Some(on) = on
            && told.as_deref() != Some(on.as_str())
          {
            waiting(&on);
            told = Some(on);
          }
        }
        Ok(ended) => return Ok(ended),
        Err(fault) if fault.lost => {}
        Err(fault) => return Err(fault.why),
      }
      tokio::time::sleep(Duration::from_secs(1)).await;
    }
  }

  /// A spoken synthesis's sound and its media type.
  pub async fn synthesis_sound(&self, id: i64) -> Result<(Vec<u8>, String), String> {
    let voice = self.voice()?;
    let sound = format!("/syntheses/{id}/sound");
    self
      .bytes(&voice, self.request(&voice, reqwest::Method::GET, &sound), &format!("GET {sound}"))
      .await
  }

  // One call answered with a success, its body read as bytes with its media type.
  async fn bytes(
    &self,
    peer: &Peer,
    request: reqwest::RequestBuilder,
    what: &str,
  ) -> Result<(Vec<u8>, String), String> {
    let answer = request.send().await.map_err(|e| format!("{} {what}: {e}", peer.name))?;
    let status = answer.status().as_u16();
    let media = answer
      .headers()
      .get(reqwest::header::CONTENT_TYPE)
      .and_then(|value| value.to_str().ok())
      .unwrap_or("application/octet-stream")
      .to_owned();
    let body = answer.bytes().await.map_err(|e| format!("{} {what}: {e}", peer.name))?;
    if !(200..300).contains(&status) {
      return Err(format!("{} {what} answered {status}: {}", peer.name, String::from_utf8_lossy(&body)));
    }
    Ok((body.to_vec(), media))
  }

  /// A file attached to a document under its name, with the role a reader is shown it under where one is
  /// named.
  pub async fn attach(
    &self,
    document_id: i64,
    name: &str,
    media_type: &str,
    role: Option<&str>,
    content: &[u8],
  ) -> Result<(), String> {
    use base64::Engine;
    let mut body = json!({
      "name": name,
      "mediaType": media_type,
      "content": base64::engine::general_purpose::STANDARD.encode(content),
    });
    if let Some(role) = role {
      body["role"] = json!(role);
    }
    self
      .artifact_send(reqwest::Method::POST, &format!("/documents/{document_id}/attachments"), &body)
      .await
      .map(|_| ())
  }

  /// A page document's HTML as the archive keeps it.
  pub async fn kept_page(&self, document_id: i64) -> Result<String, String> {
    let artifact = self.artifact()?;
    let path = format!("/documents/{document_id}/content");
    let request = self.request(&artifact, reqwest::Method::GET, &path).query(&[("download", "true")]);
    let (bytes, _) = self.bytes(&artifact, request, &format!("GET {path}")).await?;
    String::from_utf8(bytes).map_err(|e| format!("the artifact service's page did not read as text: {e}"))
  }

  /// A document's labels as the archive keeps them.
  pub async fn kept_labels(&self, document_id: i64) -> Result<Value, String> {
    let artifact = self.artifact()?;
    let path = format!("/documents/{document_id}");
    let answer = self
      .call(&artifact, self.request(&artifact, reqwest::Method::GET, &path), &format!("GET {path}"))
      .await?;
    Ok(answer["labels"].clone())
  }

  /// Whether a document keeps a file under a name.
  pub async fn attached(&self, document_id: i64, name: &str) -> Result<bool, String> {
    let artifact = self.artifact()?;
    let path = format!("/documents/{document_id}/attachments");
    let answer = self
      .call(
        &artifact,
        self.request(&artifact, reqwest::Method::GET, &path).query(&[("limit", "1000")]),
        &format!("GET {path}"),
      )
      .await?;
    let listed: Listed<Attachment> = serde_json::from_value(answer)
      .map_err(|e| format!("the artifact service's attachments did not read: {e}"))?;
    Ok(listed.items.iter().any(|kept| kept.name == name))
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

  // Agency's seats and thrown-away sessions, as the runner asks them (`docs/runner.md`): each call waited on
  // until agency answers, however long a card it waits on takes, a turn's send alone bounded by its session's
  // bound.

  /// A seat asked of the co-worker pool: the request as agency answers it.
  pub async fn ask_seat(&self, ask: &Value) -> Result<Value, Fault> {
    let agency = self.agency().map_err(Fault::kept)?;
    let request = self.unclocked(&agency, reqwest::Method::POST, "/seat-requests").json(ask);
    self.held_call(&agency, request, "POST /seat-requests", false).await
  }

  /// A seat moved: asked for its place, bound to its session, or given back.
  pub async fn move_seat(&self, token: &str, change: &Value) -> Result<Value, Fault> {
    let agency = self.agency().map_err(Fault::kept)?;
    let path = format!("/seat-requests/{}", segment(token));
    let request = self.unclocked(&agency, reqwest::Method::PATCH, &path).json(change);
    self.held_call(&agency, request, &format!("PATCH {path}"), true).await
  }

  /// A thrown-away session started: its id.
  pub async fn start_session(&self, start: &Value) -> Result<String, Fault> {
    let agency = self.agency().map_err(Fault::kept)?;
    let request = self.unclocked(&agency, reqwest::Method::POST, "/sessions").json(start);
    let answer = self.held_call(&agency, request, "POST /sessions", false).await?;
    answer["id"]
      .as_str()
      .filter(|id| !id.is_empty())
      .map(str::to_string)
      .ok_or_else(|| Fault::kept("agency answered the session with no id".to_string()))
  }

  /// One turn into a session, answered once it ends: the turn's end as agency answers it.
  pub async fn send_turn(&self, session: &str, body: &Value, within: Duration) -> Result<Value, Fault> {
    let agency = self.agency().map_err(Fault::kept)?;
    let path = format!("/sessions/{}/turns", segment(session));
    let request = self.unclocked(&agency, reqwest::Method::POST, &path).timeout(within).json(body);
    self.held_call(&agency, request, &format!("POST {path}"), true).await
  }

  /// An item raised in the job service: its number.
  pub async fn file_item(&self, item: &Value) -> Result<i64, String> {
    let job = self.job()?;
    let request = self.request(&job, reqwest::Method::POST, "/items").json(item);
    let answer = self.call(&job, request, "POST /items").await?;
    answer["id"]
      .as_i64()
      .ok_or_else(|| "the job service answered the item with no id".to_string())
  }

  /// A session ended; `Busy` while a turn of it still runs.
  pub async fn end_session(&self, session: &str) -> Result<(), Ending> {
    let agency = self.agency().map_err(Ending::Failed)?;
    let path = format!("/sessions/{}", segment(session));
    let answer = self
      .unclocked(&agency, reqwest::Method::DELETE, &path)
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
      .call(&agency, self.request(&agency, reqwest::Method::GET, "/roles"), "GET /roles")
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

/// An identifier agency gave, written into a path as one segment: what a segment cannot hold encoded, and the
/// rest — a session id's `@` and `:` among it — as given, which is how agency's routes match it.
fn segment(identifier: &str) -> String {
  let mut address = url::Url::parse("http://agency/").expect("the base reads");
  address.path_segments_mut().expect("the base has a path").push(identifier);
  address.path().trim_start_matches('/').to_owned()
}
