//! The tool host's control socket (`docs/api.md`, 「Calls this unit makes」): HTTP/1.1 over a Unix socket,
//! under the issuer token the host keeps, read afresh at every call. The runner reads the web namespace's
//! listing from it, the research settings the web tools' search engines' state, and the scan a `page`
//! source's listing through explorer's `web/web_fetch`.

use std::collections::BTreeMap;
use std::time::Duration;

use http_body_util::{BodyExt, Full};
use hyper::body::Bytes;
use hyper_util::rt::TokioIo;
use serde_json::{Value, json};

use crate::config::Settings;

/// How long one request of the control socket may take.
const WITHIN: Duration = Duration::from_secs(30);

/// How long the record of one system call stands where nothing takes its token back.
const SYSTEM_CALL_TTL_MS: i64 = 60_000;

/// How long one slice of a page read through explorer may take: Chrome started and the page drawn.
const PAGE_WITHIN: Duration = Duration::from_secs(120);

/// How long the record a page is read under stands where nothing takes its token back.
const PAGE_TTL_MS: i64 = 900_000;

/// The most characters of a page one slice holds, explorer's own ceiling.
const SLICE_CHARS: usize = 100_000;

/// The most slices of one page read; a page longer than these is read as far as they go.
const SLICES: usize = 16;

/// The holder a system call's record names.
const HOLDER: &str = "bmo-research";

/// The tool host as the settings name it: its socket and the file its issuer token stands in.
pub struct ToolHost<'a> {
  settings: &'a Settings,
}

impl<'a> ToolHost<'a> {
  pub fn new(settings: &'a Settings) -> ToolHost<'a> {
    ToolHost { settings }
  }

  fn issuer(&self) -> Result<String, String> {
    let path = self.settings.tool_host_token_path();
    std::fs::read_to_string(&path)
      .map(|t| t.trim().to_string())
      .map_err(|e| format!("the tool host's issuer token, {}: {e}", path.display()))
  }

  /// One request of the control socket under `token`: the status and the body read as JSON, null where it
  /// answers none.
  async fn ask(
    &self,
    method: hyper::Method,
    path: &str,
    token: &str,
    body: Option<&Value>,
    within: Duration,
  ) -> Result<(u16, Value), String> {
    let socket = self.settings.tool_host_socket();
    let what = format!("the tool host {method} {path}");
    let asked = async {
      let stream = tokio::net::UnixStream::connect(&socket)
        .await
        .map_err(|e| format!("{what}: {}: {e}", socket.display()))?;
      let (mut sender, connection) = hyper::client::conn::http1::handshake(TokioIo::new(stream))
        .await
        .map_err(|e| format!("{what}: {e}"))?;
      tokio::spawn(async move {
        let _ = connection.await;
      });
      let bytes = body.map(|b| Bytes::from(b.to_string())).unwrap_or_default();
      let request = hyper::Request::builder()
        .method(method.clone())
        .uri(path)
        .header(hyper::header::HOST, "localhost")
        .header(hyper::header::AUTHORIZATION, format!("Bearer {token}"))
        .header(hyper::header::CONTENT_TYPE, "application/json")
        .header(hyper::header::ACCEPT, "application/json")
        .body(Full::new(bytes))
        .map_err(|e| format!("{what}: {e}"))?;
      let answer = sender.send_request(request).await.map_err(|e| format!("{what}: {e}"))?;
      let status = answer.status().as_u16();
      let read = answer.into_body().collect().await.map_err(|e| format!("{what}: {e}"))?.to_bytes();
      let value = if read.iter().all(u8::is_ascii_whitespace) {
        Value::Null
      } else {
        serde_json::from_slice(&read).map_err(|e| format!("{what}: the answer did not read: {e}"))?
      };
      Ok((status, value))
    };
    tokio::time::timeout(within, asked)
      .await
      .map_err(|_| format!("{what}: no answer within {within:?}"))?
  }

  /// A request answered with a success, its body; any other answer is the failure's sentence.
  async fn called(
    &self,
    method: hyper::Method,
    path: &str,
    token: &str,
    body: Option<&Value>,
  ) -> Result<Value, String> {
    self.called_within(method, path, token, body, WITHIN).await
  }

  async fn called_within(
    &self,
    method: hyper::Method,
    path: &str,
    token: &str,
    body: Option<&Value>,
    within: Duration,
  ) -> Result<Value, String> {
    let (status, value) = self.ask(method.clone(), path, token, body, within).await?;
    if !(200..300).contains(&status) {
      let why = value["error"].as_str().map(str::to_string).unwrap_or_else(|| value.to_string());
      return Err(format!("the tool host answered {method} {path} with {status}: {why}"));
    }
    Ok(value)
  }

  /// A namespace's listing: the `listing` line of every tool under it, in the order each names.
  pub async fn listing(&self, namespace: &str) -> Result<String, String> {
    let issuer = self.issuer()?;
    let mut listed: Vec<(i64, String)> = Vec::new();
    let mut after: Option<String> = None;
    loop {
      let mut path = format!("/namespaces/{namespace}/tools?limit=200");
      if let Some(name) = &after {
        let named: String = url::form_urlencoded::byte_serialize(name.as_bytes()).collect();
        path.push_str(&format!("&after={named}"));
      }
      let page = self.called(hyper::Method::GET, &path, &issuer, None).await?;
      let items = page["items"].as_array().cloned().unwrap_or_default();
      let mut last = None;
      for tool in &items {
        last = tool["name"].as_str().map(str::to_string);
        if let (Some(line), order) = (tool["listing"]["line"].as_str(), tool["listing"]["order"].as_i64()) {
          listed.push((order.unwrap_or(i64::MAX), line.to_string()));
        }
      }
      match (page["more"].as_bool().unwrap_or(false), last) {
        (true, Some(name)) => after = Some(name),
        _ => break,
      }
    }
    if listed.is_empty() {
      return Err(format!("the tool host lists no {namespace} tool with a listing line"));
    }
    listed.sort_by_key(|(order, _)| *order);
    Ok(listed.into_iter().map(|(_, line)| line).collect::<Vec<_>>().join("\n"))
  }

  /// One call of a system's entry under a record of its own naming the namespace alone and the data root,
  /// the record taken back after: the call's structured content.
  pub async fn system_call(&self, namespace: &str, name: &str, input: Value) -> Result<Value, String> {
    let issuer = self.issuer()?;
    let record = json!({
      "holderName": HOLDER,
      "namespaces": [namespace],
      "capabilities": [],
      "grants": [],
      "variables": BTreeMap::from([("BMO_DATA_DIR", self.settings.data_root.to_string_lossy())]),
      "labels": { "line": "system" },
      "ttlMs": SYSTEM_CALL_TTL_MS,
    });
    let issued = self.called(hyper::Method::POST, "/tool-sessions", &issuer, Some(&record)).await?;
    let token = issued["token"]
      .as_str()
      .ok_or("the tool host answered the session with no token")?
      .to_string();
    let call = json!({ "namespace": namespace, "name": name, "input": input });
    let called = self.called(hyper::Method::POST, "/tool-calls", &token, Some(&call)).await;
    let revoked = format!("/tool-sessions/{token}");
    if let Err(why) = self.called(hyper::Method::DELETE, &revoked, &issuer, None).await {
      eprintln!("bmo-research: a system call's session was not taken back: {why}");
    }
    let result = &called?["result"];
    if result["isError"].as_bool() == Some(true) {
      return Err(said(result, " "));
    }
    result
      .get("structuredContent")
      .filter(|v| !v.is_null())
      .cloned()
      .ok_or_else(|| format!("{namespace}/{name} answered no structured content"))
  }

  /// A page's markup read through explorer's `web/web_fetch` under a record of its own, held to the strategy
  /// explorer's word names, slice by slice to its end: the address the reading ended at, and the markup.
  pub async fn read_page(&self, address: &str, strategy: &str) -> Result<(String, String), String> {
    let issuer = self.issuer()?;
    let record = json!({
      "holderName": HOLDER,
      "namespaces": ["web"],
      "capabilities": ["web.read"],
      "grants": [],
      "variables": BTreeMap::from([("BMO_DATA_DIR", self.settings.data_root.to_string_lossy())]),
      "labels": { "line": "system", "research": { "strategy": strategy } },
      "ttlMs": PAGE_TTL_MS,
    });
    let issued = self.called(hyper::Method::POST, "/tool-sessions", &issuer, Some(&record)).await?;
    let token = issued["token"]
      .as_str()
      .ok_or("the tool host answered the session with no token")?
      .to_string();
    let read = self.slices(address, &token).await;
    let revoked = format!("/tool-sessions/{token}");
    if let Err(why) = self.called(hyper::Method::DELETE, &revoked, &issuer, None).await {
      eprintln!("bmo-research: a page reading's session was not taken back: {why}");
    }
    read
  }

  async fn slices(&self, address: &str, token: &str) -> Result<(String, String), String> {
    let mut markup = String::new();
    let mut at = address.to_string();
    let mut offset = 0;
    for _ in 0..SLICES {
      let input = json!({ "url": address, "raw": true, "max_chars": SLICE_CHARS, "offset": offset });
      let call = json!({ "namespace": "web", "name": "web_fetch", "input": input });
      let called = self
        .called_within(hyper::Method::POST, "/tool-calls", token, Some(&call), PAGE_WITHIN)
        .await?;
      let result = &called["result"];
      let text = said(result, "\n");
      if result["isError"].as_bool() == Some(true) {
        return Err(text);
      }
      let slice = Slice::read(&text)?;
      if let Some(status) = slice.status.filter(|s| !(200..300).contains(s)) {
        return Err(format!("{} answered {status}", slice.url));
      }
      at = slice.url;
      markup.push_str(&slice.content);
      match slice.next {
        Some(next) if next > offset => offset = next,
        _ => break,
      }
    }
    Ok((at, markup))
  }
}

/// The text blocks of a tool result, joined.
fn said(result: &Value, between: &str) -> String {
  let blocks: Vec<&str> = result["content"]
    .as_array()
    .into_iter()
    .flatten()
    .filter_map(|block| block["text"].as_str())
    .collect();
  blocks.join(between)
}

/// One slice of a page as `web_fetch` answers it: its head's lines, `URL`, `HTTP-STATUS` and `CHARS` with
/// the offset to read on from where more stands, and the content between its marks.
struct Slice {
  url: String,
  status: Option<u16>,
  next: Option<usize>,
  content: String,
}

impl Slice {
  fn read(text: &str) -> Result<Slice, String> {
    let first = text.lines().next().unwrap_or_default();
    let Some((head, rest)) = text.split_once("\nCONTENT (") else {
      return Err(format!("explorer answered no page: {first}"));
    };
    let content = rest.split_once('\n').map(|(_, content)| content).unwrap_or_default();
    let content = content
      .strip_suffix("END OF CONTENT")
      .map(|content| content.strip_suffix('\n').unwrap_or(content))
      .unwrap_or(content);
    let field = |name: &str| {
      head
        .lines()
        .find_map(|line| line.strip_prefix(name).and_then(|rest| rest.strip_prefix(": ")))
    };
    let url = field("URL")
      .ok_or_else(|| format!("explorer answered no page: {first}"))?
      .trim()
      .to_string();
    let next = field("CHARS")
      .and_then(|chars| chars.split_once("(more: offset "))
      .and_then(|(_, more)| more.trim_end_matches(')').trim().parse().ok());
    Ok(Slice {
      url,
      status: field("HTTP-STATUS").and_then(|status| status.trim().parse().ok()),
      next,
      content: content.to_string(),
    })
  }
}
