//! A research run's document (`docs/runner.md`, 「The document」 and 「The deep dive's page」): the searcher's
//! structure lines made into a document written for its reader — its title, description, kind and labels from
//! the head, its body as headed sections in the question's language with the sources as links last, its
//! pictures kept beside it — and nothing of the structure lines themselves; a deep dive's held to its shape
//! and filed as a page; and the filing of it in the artifact service.

use std::net::IpAddr;
use std::time::Duration;

use base64::Engine;
use serde_json::{Value, json};

use crate::App;
use crate::page::{self, Side};
use crate::reading;
use crate::store::{RunDocument, now_ms};
use crate::words::SearchDocumentWords as Words;

// The name the product rulebook gives its news section, which a result writes in English.
const NEWS_SECTION: &str = "New products and prices";

/// Who a run's document is filed by.
pub const ACTOR: &str = "research";

/// How long a run's document's share link opens.
pub const SHARE_LIFE: Duration = Duration::from_secs(24 * 3_600);

/// The language a question is asked in, as the words file names it: `ko` where it is written in Hangul, `en`
/// otherwise.
pub fn language_of(question: &str) -> &'static str {
  let hangul = |c: char| ('\u{AC00}'..='\u{D7A3}').contains(&c) || ('\u{1100}'..='\u{11FF}').contains(&c);
  if question.chars().any(hangul) { "ko" } else { "en" }
}

/// A picture the result names, as the document draws it: its address, its caption and the name it is kept
/// under beside the document.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Picture {
  pub url: String,
  pub caption: String,
  pub name: String,
}

/// A result made into its document.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Drawn {
  pub title: Option<String>,
  pub description: Option<String>,
  pub learning: bool,
  pub keywords: Option<String>,
  pub rule: Option<String>,
  pub body: String,
  pub pictures: Vec<Picture>,
  /// A deep dive's script for the ear, lifted out of the body.
  pub narration: Option<String>,
}

// The lines that open a block of the body.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Mode {
  Body,
  Narration,
  Menu,
}

fn after<'a>(line: &'a str, word: &str) -> Option<&'a str> {
  line.trim().strip_prefix(word).map(str::trim)
}

fn parts(text: &str) -> Vec<&str> {
  text.split("::").map(str::trim).collect()
}

// Whether a line opens a block or an entry line of its own, ending a menu.
fn opens(line: &str) -> bool {
  const OPENERS: [&str; 17] = [
    "VERDICT:",
    "SUMMARY:",
    "SECTION:",
    "UNCONFIRMED:",
    "===ITEM===",
    "SOURCE",
    "NAME:",
    "ADDRESS:",
    "MAP:",
    "ROADVIEW:",
    "MENU:",
    "RELEASED:",
    "NEWER:",
    "STOCK:",
    "DEAL:",
    "REVIEW:",
    "IMAGE:",
  ];
  let line = line.trim_start();
  OPENERS.iter().any(|opener| line.starts_with(opener))
}

/// A result's structure lines made into its document in `words`.
pub fn draw(result: &str, words: &Words) -> Drawn {
  let mut drawn = Drawn::default();
  let mut body: Vec<String> = Vec::new();
  let mut sources: Vec<(String, String, String)> = Vec::new();
  let mut narration: Vec<String> = Vec::new();
  let mut mode = Mode::Body;
  let brief = !result.lines().any(|line| line.trim() == "VERDICT:");
  let heading = |body: &mut Vec<String>, text: &str| {
    body.push(String::new());
    body.push(format!("## {text}"));
    body.push(String::new());
  };
  for raw in result.lines() {
    let line = raw.trim_end();
    let trimmed = line.trim();
    if mode == Mode::Narration && !opens(trimmed) {
      narration.push(line.to_owned());
      continue;
    }
    if mode == Mode::Menu && !opens(trimmed) {
      if trimmed.is_empty() {
        continue;
      }
      let item = parts(trimmed);
      body.push(match item.as_slice() {
        [dish, price] => format!("- {dish} — {price}"),
        [dish, price, note, ..] => format!("- {dish} — {price} ({note})"),
        _ => format!("- **{trimmed}**"),
      });
      continue;
    }
    mode = Mode::Body;
    if let Some(value) = after(trimmed, "TITLE:") {
      drawn.title = Some(value.to_owned()).filter(|value| !value.is_empty());
    } else if let Some(value) = after(trimmed, "TOPIC:") {
      drawn.description = Some(value.to_owned()).filter(|value| !value.is_empty());
    } else if let Some(value) = after(trimmed, "KEYWORDS:") {
      drawn.keywords = Some(value.to_owned()).filter(|value| !value.is_empty());
    } else if let Some(value) = after(trimmed, "RULE:") {
      drawn.rule = Some(value.to_owned()).filter(|value| !value.is_empty());
    } else if let Some(value) = after(trimmed, "KIND:") {
      drawn.learning = value == "learning";
    } else if after(trimmed, "ANSWERED:").is_some() || after(trimmed, "DOCUMENT:").is_some() {
    } else if trimmed == "LISTEN:" {
      mode = Mode::Narration;
    } else if trimmed == "VERDICT:" {
      heading(&mut body, &words.answer);
    } else if trimmed == "SUMMARY:" {
      heading(&mut body, if brief { &words.brief_answer } else { &words.summary });
    } else if let Some(name) = after(trimmed, "SECTION:") {
      let grounds = name.eq_ignore_ascii_case("grounds") || name == words.grounds;
      // The product rulebook names its news section in English, the one name a result writes as it stands.
      let news = name.eq_ignore_ascii_case(NEWS_SECTION);
      heading(
        &mut body,
        if grounds {
          &words.grounds
        } else if news {
          &words.news
        } else {
          name
        },
      );
    } else if trimmed == "UNCONFIRMED:" {
      heading(&mut body, &words.unconfirmed);
    } else if trimmed == "===ITEM===" {
      body.push(String::new());
    } else if let Some(name) = after(trimmed, "NAME:") {
      body.push(String::new());
      body.push(format!("### {name}"));
      body.push(String::new());
    } else if let Some(value) = after(trimmed, "ADDRESS:") {
      body.push(format!("- {}: {value}", words.address));
    } else if let Some(value) = after(trimmed, "MAP:") {
      body.push(format!("- [{}]({value})", words.map));
    } else if let Some(value) = after(trimmed, "ROADVIEW:") {
      body.push(format!("- [{}]({value})", words.roadview));
    } else if let Some(value) = after(trimmed, "MENU:") {
      body.push(String::new());
      body.push(format!("#### {}", words.menu));
      body.push(String::new());
      if !value.is_empty() {
        body.push(format!("_{value}_"));
      }
      mode = Mode::Menu;
    } else if let Some(value) = after(trimmed, "RELEASED:") {
      body.push(format!("- {}: {value}", words.released));
    } else if let Some(value) = after(trimmed, "NEWER:") {
      body.push(format!("- {}: {value}", words.newer));
    } else if let Some(value) = after(trimmed, "STOCK:") {
      body.push(format!("- {}: {}", words.stock, parts(value).join(" · ")));
    } else if let Some(value) = after(trimmed, "DEAL:") {
      body.push(format!("- {}: {}", words.deal, parts(value).join(" · ")));
    } else if let Some(value) = after(trimmed, "REVIEW:") {
      let review = parts(value);
      body.push(match review.as_slice() {
        [rating, said, page, ..] => format!("> {said}\n> — {}: {rating}, [↗]({page})", words.review),
        [rating, said] => format!("> {said}\n> — {}: {rating}", words.review),
        _ => format!("> {value}"),
      });
    } else if let Some(value) = after(trimmed, "IMAGE:") {
      let picture = parts(value);
      let url = picture.first().copied().unwrap_or_default().to_owned();
      let caption = picture[1..]
        .iter()
        .filter(|part| !part.is_empty())
        .copied()
        .collect::<Vec<_>>()
        .join(" · ");
      let name = format!("picture-{:02}", drawn.pictures.len() + 1);
      body.push(format!("![{caption}]({name})"));
      drawn.pictures.push(Picture { url, caption, name });
    } else if let Some(value) = after(trimmed, "MORE:") {
      if !value.is_empty() {
        body.push(value.to_owned());
      }
    } else if let Some(rest) = trimmed.strip_prefix("SOURCE") {
      let (mark, cited) = match rest.trim_start().strip_prefix('[') {
        Some(marked) => match marked.split_once("]:") {
          Some((mark, cited)) => (mark.trim().to_owned(), cited),
          None => (String::new(), rest),
        },
        None => (String::new(), rest.trim_start().trim_start_matches(':')),
      };
      let cited = parts(cited);
      let url = cited.first().copied().unwrap_or_default().to_owned();
      let what = cited.get(1).copied().filter(|what| !what.is_empty()).unwrap_or(url.as_str()).to_owned();
      if !url.is_empty() && !sources.iter().any(|(_, kept, _)| *kept == url) {
        sources.push((mark, url, what));
      }
    } else {
      body.push(line.to_owned());
    }
  }
  if !sources.is_empty() {
    heading(&mut body, &words.sources);
    for (mark, url, what) in &sources {
      let mark = if mark.is_empty() { String::new() } else { format!("[{mark}] ") };
      body.push(format!("- {mark}[{what}]({url})"));
    }
  }
  drawn.body = tidy(&body);
  drawn.narration = Some(narration.join("\n").trim().to_owned()).filter(|text| !text.is_empty());
  drawn
}

// The body's lines with runs of blank lines folded to one, none at either end.
fn tidy(lines: &[String]) -> String {
  let mut out: Vec<&str> = Vec::new();
  for line in lines {
    if line.trim().is_empty() && out.last().is_none_or(|last| last.trim().is_empty()) {
      continue;
    }
    out.push(line);
  }
  while out.last().is_some_and(|last| last.trim().is_empty()) {
    out.pop();
  }
  out.join("\n")
}

/// The labels a run's document carries: the run, its key, its question, and the keywords, the rule and the
/// topic where the result writes them.
pub fn labels(drawn: &Drawn, run: i64, key: &str, query: &str) -> Value {
  let mut labels = json!({ "research": run.to_string(), "key": key, "query": query.trim() });
  if let Some(keywords) = &drawn.keywords {
    labels["keywords"] = keywords.clone().into();
  }
  if let Some(rule) = &drawn.rule {
    labels["rule"] = rule.clone().into();
  }
  if let Some(topic) = &drawn.description {
    labels["topic"] = topic.clone().into();
  }
  labels
}

/// The document as the artifact service files it: its kind — `search`, or `learning` for study material —
/// title, description (the topic), body, scope and labels.
pub fn filing(drawn: &Drawn, run: i64, key: &str, query: &str, room: Option<i64>) -> Value {
  let labels = labels(drawn, run, key, query);
  json!({
    "kind": if drawn.learning { "learning" } else { "search" },
    "title": drawn.title.clone().unwrap_or_else(|| query.trim().to_owned()),
    "description": drawn.description,
    "body": drawn.body,
    "scope": room.filter(|room| *room != 0).map(|room| room.to_string()),
    "actorName": ACTOR,
    "labels": labels,
  })
}

// How many pictures one document keeps, how long one fetch and the whole keeping may take, and the most
// bytes one picture may be.
const MOST_PICTURES: usize = 12;
const FETCH_WITHIN: Duration = Duration::from_secs(10);
const KEEPING_WITHIN: Duration = Duration::from_secs(40);
const MOST_PICTURE_BYTES: usize = 15 * 1024 * 1024;
const MOST_REDIRECTS: usize = 3;

// Whether an address is one the outward fetch may reach: none on this machine or its networks.
fn public(ip: IpAddr) -> bool {
  match ip {
    IpAddr::V4(v4) => {
      !(v4.is_private()
        || v4.is_loopback()
        || v4.is_link_local()
        || v4.is_unspecified()
        || v4.is_broadcast()
        || v4.is_multicast()
        || v4.octets()[0] == 100 && (64..128).contains(&v4.octets()[1]))
    }
    IpAddr::V6(v6) => {
      !(v6.is_loopback()
        || v6.is_unspecified()
        || v6.is_multicast()
        || (v6.segments()[0] & 0xfe00) == 0xfc00
        || (v6.segments()[0] & 0xffc0) == 0xfe80)
    }
  }
}

// Whether a web address names a host the outward fetch may reach, every address it resolves to public.
async fn reachable(url: &url::Url) -> bool {
  if !matches!(url.scheme(), "http" | "https") {
    return false;
  }
  let Some(host) = url.host_str() else { return false };
  let port = url.port_or_known_default().unwrap_or(443);
  match tokio::net::lookup_host((host, port)).await {
    Ok(addresses) => {
      let addresses: Vec<_> = addresses.collect();
      !addresses.is_empty() && addresses.iter().all(|address| public(address.ip()))
    }
    Err(_) => false,
  }
}

/// One picture fetched through the guarded direct fetch: every hop's host checked, at most a few redirects,
/// a picture's type and size; none where any check fails.
pub async fn fetch_picture(address: &str) -> Option<(Vec<u8>, String)> {
  let client = reqwest::Client::builder()
    .redirect(reqwest::redirect::Policy::none())
    .timeout(FETCH_WITHIN)
    .build()
    .ok()?;
  let mut at = url::Url::parse(address).ok()?;
  for _ in 0..=MOST_REDIRECTS {
    if !reachable(&at).await {
      return None;
    }
    let answer = client.get(at.clone()).send().await.ok()?;
    if answer.status().is_redirection() {
      let next = answer.headers().get(reqwest::header::LOCATION)?.to_str().ok()?;
      at = at.join(next).ok()?;
      continue;
    }
    if !answer.status().is_success() {
      return None;
    }
    let media = answer.headers().get(reqwest::header::CONTENT_TYPE)?.to_str().ok()?.to_owned();
    if !media.starts_with("image/")
      || answer.content_length().is_some_and(|length| length as usize > MOST_PICTURE_BYTES)
    {
      return None;
    }
    let bytes = answer.bytes().await.ok()?;
    return (bytes.len() <= MOST_PICTURE_BYTES && !bytes.is_empty()).then(|| (bytes.to_vec(), media));
  }
  None
}

/// The pictures kept beside a filed document, at most the bound and within the keeping's time; a picture not
/// kept is drawn from its address in the body. Answers the body as it is to stand, and how many were kept.
pub async fn keep_pictures(app: &App, document: i64, drawn: &Drawn) -> (String, usize) {
  let mut body = drawn.body.clone();
  let mut kept = 0;
  let began = std::time::Instant::now();
  for (at, picture) in drawn.pictures.iter().enumerate() {
    let fetched = if at < MOST_PICTURES && began.elapsed() < KEEPING_WITHIN {
      fetch_picture(&picture.url).await
    } else {
      None
    };
    let attached = match fetched {
      Some((bytes, media)) => app
        .peers
        .artifact_send(
          reqwest::Method::POST,
          &format!("/documents/{document}/attachments"),
          &json!({
            "name": picture.name,
            "mediaType": media,
            "content": base64::engine::general_purpose::STANDARD.encode(bytes),
          }),
        )
        .await
        .is_ok(),
      None => false,
    };
    if attached {
      kept += 1;
    } else {
      body = body.replace(&format!("]({})", picture.name), &format!("]({})", picture.url));
    }
  }
  (body, kept)
}

/// A result's document filed in the artifact service and given its share link, which ends a day after it is
/// made; the document stays in the archive.
pub async fn file(app: &App, drawn: &Drawn, filing: &Value) -> Result<RunDocument, String> {
  let filed = app.peers.artifact_send(reqwest::Method::POST, "/documents", filing).await?;
  let id = filed["id"].as_i64().ok_or("the artifact service named no document for the result")?;
  let (body, _) = keep_pictures(app, id, drawn).await;
  if body != drawn.body {
    app
      .peers
      .artifact_send(
        reqwest::Method::PATCH,
        &format!("/documents/{id}"),
        &json!({ "body": body, "labels": filing["labels"] }),
      )
      .await?;
  }
  let link = share_for_a_day(app, id).await?;
  Ok(RunDocument {
    id,
    title: filing["title"].as_str().unwrap_or_default().to_owned(),
    description: filing["description"].as_str().map(str::to_owned),
    link,
  })
}

/// Why a deep dive's result is not filed: a body that is empty, or one holding none of the section headings
/// its shape holds; none where it may be filed.
pub fn short_of(drawn: &Drawn, sections: &[String]) -> Option<String> {
  if drawn.body.trim().is_empty() {
    return Some("its body is empty".into());
  }
  if !sections.is_empty() && !reading::holds_heading(&drawn.body, sections) {
    return Some(format!(
      "its body holds none of the section headings its shape holds: {}",
      sections.join(", ")
    ));
  }
  None
}

/// A deep dive's page's title: the `TITLE:` line, else the body's first top-level heading, else the title the
/// deep dive falls back on.
pub fn page_title(drawn: &Drawn, fallback: &str) -> String {
  drawn
    .title
    .clone()
    .filter(|title| !title.trim().is_empty())
    .or_else(|| reading::top_heading(&drawn.body))
    .unwrap_or_else(|| fallback.to_owned())
}

/// A deep dive's page's English body: every picture drawn from its address, and the heading it opens with
/// taken out where it says the title, since the page's head says it.
pub fn page_body(drawn: &Drawn, title: &str) -> String {
  let mut body = reading::without_opening_heading(&drawn.body, title).to_owned();
  for picture in &drawn.pictures {
    body = body.replace(&format!("]({})", picture.name), &format!("]({})", picture.url));
  }
  body
}

/// A deep dive's page as the artifact service files it: kind `page`, the page drawn in English alone, its
/// head and its labels as a room's document's, with the languages it holds.
pub fn page_filing(drawn: &Drawn, run: i64, key: &str, query: &str, title: &str) -> Value {
  let body = page_body(drawn, title);
  let html = page::compose(
    &[Side {
      code: "en",
      title,
      description: drawn.description.as_deref(),
      body: &body,
    }],
    &[],
  );
  let mut labels = labels(drawn, run, key, query);
  labels["languages"] = "en".into();
  json!({
    "kind": "page",
    "title": title,
    "description": drawn.description,
    "html": html,
    "actorName": ACTOR,
    "labels": labels,
  })
}

// A filed document's share link, ending a day after it is made.
async fn share_for_a_day(app: &App, id: i64) -> Result<String, String> {
  let ends = now_ms() + SHARE_LIFE.as_millis() as i64;
  let shared = app
    .peers
    .artifact_send(reqwest::Method::POST, "/shares", &json!({ "documentId": id, "expiresAt": ends }))
    .await?;
  Ok(shared["url"].as_str().ok_or("the artifact service named no link")?.to_owned())
}

/// A deep dive's page filed in the artifact service, its pictures kept beside it, and given its share link,
/// which ends a day after it is made.
pub async fn file_page(app: &App, drawn: &Drawn, filing: &Value) -> Result<RunDocument, String> {
  let filed = app.peers.artifact_send(reqwest::Method::POST, "/documents", filing).await?;
  let id = filed["id"].as_i64().ok_or("the artifact service named no document for the result")?;
  keep_pictures(app, id, drawn).await;
  let link = share_for_a_day(app, id).await?;
  Ok(RunDocument {
    id,
    title: filing["title"].as_str().unwrap_or_default().to_owned(),
    description: filing["description"].as_str().map(str::to_owned),
    link,
  })
}
