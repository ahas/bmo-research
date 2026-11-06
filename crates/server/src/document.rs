//! A research run's document (`docs/runner.md`, 「The document」 and 「The deep dive's page」): the voyager's
//! structure lines made into a document written for its reader — its title, description, kind and labels from
//! the head, its body as headed sections in the question's language with the sources as links last, its
//! pictures kept beside it — and nothing of the structure lines themselves; a deep dive's held to its shape
//! and filed as a page; and the filing of it in the artifact service.

use std::time::Duration;

use base64::Engine;
use serde_json::{Value, json};

use crate::App;
use crate::page::{self, Figure, Side};
use crate::reading;
use crate::runner::PageAsk;
use crate::store::{RunDocument, now_ms};
use crate::tool_host::ToolHost;
use crate::words::SearchDocumentWords as Words;

// The name the product rulebook gives its news section, which a result writes in English.
const NEWS_SECTION: &str = "New products and prices";

/// Who a run's document is filed by.
pub const ACTOR: &str = "research";

/// The name a page's pending Korean side goes by, in the queue's `pending` and the document's label of it.
pub const PENDING_KOREAN: &str = "korean";

/// How long a run's document's share link opens.
pub const SHARE_LIFE: Duration = Duration::from_secs(24 * 3_600);

/// The language a question is asked in, as the words file names it: `ko` where it is written in Hangul, `en`
/// otherwise.
pub fn language_of(question: &str) -> &'static str {
  let hangul = |c: char| ('\u{AC00}'..='\u{D7A3}').contains(&c) || ('\u{1100}'..='\u{11FF}').contains(&c);
  if question.chars().any(hangul) { "ko" } else { "en" }
}

/// The source entry a picture belongs to: its mark, the page it links and the words saying what it supports.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PictureSource {
  pub mark: String,
  pub url: String,
  pub what: String,
}

/// A picture the result names, as the document draws it: its address, its caption, the name it is kept
/// under beside the document, and the source entry its block closes with.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Picture {
  pub url: String,
  pub caption: String,
  pub name: String,
  pub source: Option<PictureSource>,
}

/// A picture fetched and kept beside a document: the name it is kept under, the type explorer answered and
/// its bytes.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct KeptPicture {
  pub name: String,
  pub media_type: String,
  pub bytes: Vec<u8>,
}

/// A result made into its document.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Drawn {
  /// Whether the result writes its `ANSWERED:` line, the run's own check, which stands nowhere in the document.
  pub answered: bool,
  pub title: Option<String>,
  pub description: Option<String>,
  pub learning: bool,
  pub keywords: Option<String>,
  pub rule: Option<String>,
  pub body: String,
  pub pictures: Vec<Picture>,
  /// The result's script for the ear, lifted out of the body.
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

// A `SOURCE` line read as its mark, the page it links and what it supports, none for any other line.
fn source_at(line: &str) -> Option<(String, String, String)> {
  let rest = line.trim().strip_prefix("SOURCE")?;
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
  (!url.is_empty()).then_some((mark, url, what))
}

// Whether a line opens a block of its own, so a picture and its source entry stand in one block.
fn boundary(line: &str) -> bool {
  let line = line.trim();
  ["VERDICT:", "SECTION:", "NAME:", "===ITEM===", "UNCONFIRMED:", "MENU:", "LISTEN:"]
    .iter()
    .any(|opener| line.starts_with(opener))
}

// A picture's caption as its body writes it: what it shows, then the source entry it came from, so it reads
// on its own where the picture stands.
fn captioned(caption: &str, source: &PictureSource, words: &Words) -> String {
  let source = format!("{}: {}", words.picture_source, source.what.trim());
  match caption.trim().is_empty() {
    true => source,
    false => format!("{} — {source}", caption.trim()),
  }
}

// A picture's caption made to stand inside a markdown image's brackets, brackets and backslashes taken as
// text.
fn picture_text(text: &str) -> String {
  let mut out = String::with_capacity(text.len());
  for c in text.chars() {
    if matches!(c, '\\' | '[' | ']') {
      out.push('\\');
    }
    out.push(c);
  }
  out
}

// The name a kept picture is kept under without its extension, the name its body target gives it.
fn base_name(name: &str) -> &str {
  name.rsplit_once('.').map_or(name, |(base, _)| base)
}

// The extension a kept picture's name carries, by the type explorer answered.
fn extension(media_type: &str) -> &'static str {
  match media_type.split(';').next().unwrap_or_default().trim().to_ascii_lowercase().as_str() {
    "image/jpeg" | "image/jpg" => "jpg",
    "image/png" => "png",
    "image/gif" => "gif",
    "image/webp" => "webp",
    "image/avif" => "avif",
    "image/tiff" => "tiff",
    "image/svg+xml" => "svg",
    _ => "img",
  }
}

/// A result's structure lines made into its document in `words`.
pub fn draw(result: &str, words: &Words) -> Drawn {
  let mut drawn = Drawn::default();
  let mut body: Vec<String> = Vec::new();
  let mut sources: Vec<(String, String, String)> = Vec::new();
  let mut narration: Vec<String> = Vec::new();
  let mut mode = Mode::Body;
  let mut answered = false;
  let heading = |body: &mut Vec<String>, text: &str| {
    body.push(String::new());
    body.push(format!("## {text}"));
    body.push(String::new());
  };
  let lines: Vec<&str> = result.lines().collect();
  let source_lines: Vec<(usize, PictureSource)> = lines
    .iter()
    .enumerate()
    .filter_map(|(at, raw)| source_at(raw).map(|(mark, url, what)| (at, PictureSource { mark, url, what })))
    .collect();
  for (index, raw) in lines.iter().enumerate() {
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
    } else if after(trimmed, "ANSWERED:").is_some() {
      drawn.answered = true;
    } else if after(trimmed, "DOCUMENT:").is_some() {
    } else if trimmed == "LISTEN:" {
      mode = Mode::Narration;
    } else if trimmed == "VERDICT:" || trimmed == "SUMMARY:" {
      // A summary a result still writes is the answer's text where it stands, and the answer itself where no
      // verdict came before it.
      if answered {
        body.push(String::new());
      } else {
        heading(&mut body, &words.answer);
        answered = true;
      }
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
      // A picture belongs to the block whose source entry names where it came from, and one that block
      // names no source for is left out.
      let source = source_lines
        .iter()
        .find(|(at, _)| *at > index && !lines[index + 1..*at].iter().any(|line| boundary(line)))
        .map(|(_, source)| source.clone());
      let Some(source) = source.filter(|_| !url.is_empty() && !caption.is_empty()) else {
        continue;
      };
      let name = format!("picture-{:02}", drawn.pictures.len() + 1);
      body.push(format!("![{}]({name})", picture_text(&captioned(&caption, &source, words))));
      drawn.pictures.push(Picture {
        url,
        caption,
        name,
        source: Some(source),
      });
    } else if let Some(value) = after(trimmed, "MORE:") {
      if !value.is_empty() {
        body.push(value.to_owned());
      }
    } else if let Some((mark, url, what)) = source_at(trimmed) {
      if !sources.iter().any(|(_, kept, _)| *kept == url) {
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

/// The labels a run's document carries: the run, its key, its question, the key again as the archive's
/// category for the document, and the keywords, the rule and the topic where the result writes them.
pub fn labels(drawn: &Drawn, run: i64, key: &str, query: &str) -> Value {
  let mut labels =
    json!({ "research": run.to_string(), "key": key, "query": query.trim(), "category": key });
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

/// What a room's document lacks as it is filed (`docs/runner.md`, 「The narration's sound」): its narration's
/// sound in the question's language, where the result has a script; nothing where it has none.
pub fn room_pending(drawn: &Drawn, query: &str) -> Vec<String> {
  match drawn.narration.as_deref().map(str::trim).filter(|script| !script.is_empty()) {
    Some(_) => vec![format!("narration-{}", language_of(query))],
    None => Vec::new(),
  }
}

/// The labels a room's document carries: a run's document's, with the language it is written in as
/// `languages`, and what it still lacks as `pending`, left out where it lacks nothing.
pub fn room_labels(drawn: &Drawn, run: i64, key: &str, query: &str, pending: &[String]) -> Value {
  let mut labels = labels(drawn, run, key, query);
  labels["languages"] = language_of(query).into();
  if !pending.is_empty() {
    labels["pending"] = pending.join(",").into();
  }
  labels
}

/// The document as the artifact service files it: its kind — `search`, or `learning` for study material —
/// title, description (the topic), body, scope and labels, its narration's sound pending where it has a
/// script.
pub fn filing(drawn: &Drawn, run: i64, key: &str, query: &str, room: Option<i64>) -> Value {
  let labels = room_labels(drawn, run, key, query, &room_pending(drawn, query));
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

// How many pictures one document keeps.
const MOST_PICTURES: usize = 12;

/// The pictures a result names, fetched through the tool host's `explorer/fetch_raster` under the run's
/// pacing name, at most the bound: the name each is kept under beside its document, the type explorer
/// answered and its bytes. A picture explorer answers none for is not kept.
pub async fn fetch_pictures(app: &App, drawn: &Drawn, run: i64) -> Vec<KeptPicture> {
  let pacing = format!("research:{run}");
  let mut kept = Vec::new();
  for picture in drawn.pictures.iter().take(MOST_PICTURES) {
    let Ok((bytes, media_type)) = ToolHost::new(&app.settings).fetch_raster(&picture.url, &pacing).await
    else {
      continue;
    };
    if bytes.is_empty() || !media_type.starts_with("image/") {
      continue;
    }
    kept.push(KeptPicture {
      name: format!("{}.{}", picture.name, extension(&media_type)),
      media_type,
      bytes,
    });
  }
  kept
}

/// The figures a page draws from the pictures it names: each from the copy kept beside it as a data address,
/// else from the address the result gave, so a page needs no outside host for a picture it kept.
pub fn figures(drawn: &Drawn, kept: &[KeptPicture]) -> Vec<Figure> {
  drawn
    .pictures
    .iter()
    .map(|picture| {
      let one = kept.iter().find(|one| base_name(&one.name) == picture.name);
      let address = one.map_or_else(
        || picture.url.clone(),
        |one| {
          format!(
            "data:{};base64,{}",
            one.media_type,
            base64::engine::general_purpose::STANDARD.encode(&one.bytes)
          )
        },
      );
      Figure {
        target: picture.name.clone(),
        address,
        name: one.map_or_else(|| picture.name.clone(), |one| one.name.clone()),
      }
    })
    .collect()
}

/// The pictures kept beside a filed document, read back for a page drawn again: each picture the document
/// keeps by the name the result gave it, at most the bound. None where the archive cannot be read; a
/// picture whose bytes are not had is left out and drawn from its address.
pub async fn kept_pictures(app: &App, document: i64, drawn: &Drawn) -> Vec<KeptPicture> {
  if drawn.pictures.is_empty() {
    return Vec::new();
  }
  let Ok(attachments) = app.peers.attachments(document).await else {
    return Vec::new();
  };
  let mut kept = Vec::new();
  for picture in drawn.pictures.iter().take(MOST_PICTURES) {
    let Some(one) = attachments.iter().find(|one| base_name(&one.name) == picture.name) else {
      continue;
    };
    let Ok((bytes, media_type)) = app.peers.attachment_bytes(document, one.id).await else {
      continue;
    };
    kept.push(KeptPicture {
      name: one.name.clone(),
      media_type,
      bytes,
    });
  }
  kept
}

// The body a room's document files with its pictures named by the files kept beside it, each picture not
// kept drawn from its address.
fn pictured_body(drawn: &Drawn, kept: &[KeptPicture]) -> String {
  let mut body = drawn.body.clone();
  for picture in &drawn.pictures {
    let name = kept
      .iter()
      .find(|one| base_name(&one.name) == picture.name)
      .map_or(picture.url.as_str(), |one| one.name.as_str());
    body = body.replace(&format!("]({})", picture.name), &format!("]({name})"));
  }
  body
}

/// A result's document filed in the artifact service and given its share link, which ends a day after it is
/// made; the document stays in the archive. Its pictures are fetched and kept beside it before it is filed,
/// and one the archive refuses is drawn from its address instead.
pub async fn file(app: &App, drawn: &Drawn, filing: &Value, run: i64) -> Result<RunDocument, String> {
  let kept = fetch_pictures(app, drawn, run).await;
  let mut filing = filing.clone();
  let mut body = pictured_body(drawn, &kept);
  filing["body"] = body.clone().into();
  let filed = app.peers.artifact_send(reqwest::Method::POST, "/documents", &filing).await?;
  let id = filed["id"].as_i64().ok_or("the artifact service named no document for the result")?;
  let mut patched = false;
  for picture in &drawn.pictures {
    let Some(one) = kept.iter().find(|one| base_name(&one.name) == picture.name) else {
      continue;
    };
    if app.peers.attach(id, &one.name, &one.media_type, None, &one.bytes).await.is_err() {
      body = body.replace(&format!("]({})", one.name), &format!("]({})", picture.url));
      patched = true;
    }
  }
  if patched {
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

/// Why a deep dive's result is not whole (`docs/runner.md`, 「The deep dive's page」), each thing it lacks: an
/// empty body, the `ANSWERED:` or the `TITLE:` line, a body opening with `answer`, the heading of the answer
/// in the question's language, a section under one of the `opening` headings where its shape names any, or
/// half the section headings its shape holds in one language; none where it may be filed.
pub fn short_of(drawn: &Drawn, sections: &[String], opening: &[String], answer: &str) -> Option<String> {
  if drawn.body.trim().is_empty() {
    return Some("its body is empty".into());
  }
  let mut short = Vec::new();
  if !drawn.answered {
    short.push("it has no ANSWERED: line".to_string());
  }
  if drawn.title.as_deref().is_none_or(|title| title.trim().is_empty()) {
    short.push("it has no TITLE: line".to_string());
  }
  match reading::first_heading(&drawn.body) {
    Some(first) if reading::heading_holds(&first, answer) => {}
    Some(first) => {
      short.push(format!("its body opens with \"{first}\", not with the answer, \"{answer}\""))
    }
    None => short.push(format!("its body has no heading, not even the answer, \"{answer}\"")),
  }
  if !opening.is_empty() && reading::headings_held(&drawn.body, opening) == 0 {
    short.push(format!(
      "its body has no opening section saying what the subject is and what it is for, under one of: {}",
      opening.join(", ")
    ));
  }
  let (korean, english): (Vec<String>, Vec<String>) =
    sections.iter().cloned().partition(|heading| language_of(heading) == "ko");
  let half_held = |listed: &[String]| {
    !listed.is_empty() && reading::headings_held(&drawn.body, listed) * 2 >= listed.len()
  };
  if !sections.is_empty() && !half_held(&english) && !half_held(&korean) {
    short.push(format!(
      "its body holds fewer than half of the section headings its shape holds in any one language: {}",
      sections.join(", ")
    ));
  }
  (!short.is_empty()).then(|| short.join("; "))
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

/// A deep dive's page's English body: each picture named where the body names it, the heading it opens with
/// taken out where it says the title, since the page's head says it, and the first section under one of the
/// `opening` headings set before the rest, so the page opens by saying what its subject is.
pub fn page_body(drawn: &Drawn, title: &str, opening: &[String]) -> String {
  let rest = reading::without_opening_heading(&drawn.body, title);
  reading::lifted(rest, opening)
}

/// A deep dive's page as the artifact service files it: kind `page`, the page drawn in English alone with the
/// line saying where the research came from and its opening first, the pictures `figures` names drawn where
/// the text names them, its head and its labels as a room's document's, with the languages it holds, its
/// item's shape as its category, and its Korean side pending.
pub fn page_filing(
  drawn: &Drawn,
  run: i64,
  key: &str,
  query: &str,
  title: &str,
  ask: &PageAsk,
  figures: &[Figure],
) -> Value {
  let body = page_body(drawn, title, &ask.opening);
  let html = page::compose_with(
    &[Side {
      code: "en",
      title,
      description: drawn.description.as_deref(),
      origin: ask.origin.as_deref(),
      body: &body,
    }],
    &[],
    figures,
  );
  let mut labels = labels(drawn, run, key, query);
  labels["languages"] = "en".into();
  labels["category"] = ask.category.clone().into();
  labels["pending"] = PENDING_KOREAN.into();
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
pub async fn file_page(app: &App, filing: &Value, kept: &[KeptPicture]) -> Result<RunDocument, String> {
  let filed = app.peers.artifact_send(reqwest::Method::POST, "/documents", filing).await?;
  let id = filed["id"].as_i64().ok_or("the artifact service named no document for the result")?;
  for picture in kept {
    let _ = app.peers.attach(id, &picture.name, &picture.media_type, None, &picture.bytes).await;
  }
  let link = share_for_a_day(app, id).await?;
  Ok(RunDocument {
    id,
    title: filing["title"].as_str().unwrap_or_default().to_owned(),
    description: filing["description"].as_str().map(str::to_owned),
    link,
  })
}
