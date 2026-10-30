//! The Korean rendering of a filed document (`docs/api.md`, 「The Korean rendering」): its body cut at its
//! headings into parts the translator takes, each part translated with the note that gives it its context,
//! a part the translator finds too long halved and asked again, and the rendering filed beside the English
//! document with a share link of its own.

use tokio::task::JoinSet;

use crate::App;
use crate::peers::{NewRendering, Peers, Untranslated};
use crate::reading;
use crate::words::{self, fill};

/// A Korean rendering filed: its document, its title and description, and its live link.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Rendering {
  pub document_id: i64,
  pub title: String,
  pub description: Option<String>,
  pub share_url: String,
}

/// The English document a rendering is made of, and the item it was filed for.
#[derive(Debug, Clone)]
pub struct English<'a> {
  pub item_id: i64,
  pub name: &'a str,
  pub document_id: i64,
  pub title: &'a str,
  pub description: Option<&'a str>,
  pub body: &'a str,
}

fn chars(text: &str) -> usize {
  text.chars().count()
}

/// The note every part of a report about `name` carries.
pub fn note(name: &str) -> String {
  fill(&words::translation().note, &[("name", name)])
}

/// The note a report's title carries.
pub fn title_note(name: &str) -> String {
  fill(&words::translation().title_note, &[("name", name)])
}

/// A text halved nearest its middle: at a heading, else after a blank line, else after a line's end, else
/// after a space, each looked for in the text's middle half, else at its middle character. None where the
/// text holds fewer than two characters.
pub fn halve(text: &str) -> Option<(&str, &str)> {
  let count = chars(text);
  if count < 2 {
    return None;
  }
  let middle = text.char_indices().nth(count / 2).map(|(at, _)| at).unwrap_or(text.len());
  let (low, high) = (text.len() / 4, text.len() - text.len() / 4);
  let nearest = |cuts: Vec<usize>| {
    cuts
      .into_iter()
      .filter(|at| *at > 0 && *at < text.len() && *at >= low && *at <= high)
      .min_by_key(|at| at.abs_diff(middle))
  };
  let after = |mark: &str| text.match_indices(mark).map(|(at, m)| at + m.len()).collect::<Vec<_>>();
  let at = nearest(reading::heading_starts(text))
    .or_else(|| nearest(after("\n\n")))
    .or_else(|| nearest(after("\n")))
    .or_else(|| nearest(after(" ")))
    .unwrap_or(middle);
  Some(text.split_at(at))
}

/// A text halved until every piece holds at most `limit` characters, in order.
fn fit(text: &str, limit: usize) -> Vec<String> {
  if chars(text) <= limit {
    return vec![text.to_string()];
  }
  match halve(text) {
    Some((first, second)) => {
      let mut pieces = fit(first, limit);
      pieces.extend(fit(second, limit));
      pieces
    }
    None => vec![text.to_string()],
  }
}

/// A body cut at its headings into parts of at most `limit` characters: whole sections gathered into a part
/// while they fit, and a section longer than the limit halved until each piece fits.
pub fn cut(body: &str, limit: usize) -> Vec<String> {
  let limit = limit.max(1);
  let mut sections = Vec::new();
  let mut from = 0;
  for at in reading::heading_starts(body).into_iter().filter(|at| *at > 0) {
    sections.push(&body[from..at]);
    from = at;
  }
  sections.push(&body[from..]);
  let mut parts: Vec<String> = Vec::new();
  let mut open = String::new();
  for section in sections {
    if chars(&open) + chars(section) <= limit {
      open.push_str(section);
      continue;
    }
    if !open.is_empty() {
      parts.push(std::mem::take(&mut open));
    }
    if chars(section) <= limit {
      open.push_str(section);
    } else {
      parts.extend(fit(section, limit));
    }
  }
  parts.push(open);
  parts.retain(|part| !part.trim().is_empty());
  parts
}

/// What stood after a piece of the text, so its translation is joined to the next the same way: a blank
/// line, a line's end, or a space.
fn joiner(piece: &str) -> &'static str {
  let rest = piece.trim_end_matches([' ', '\t']);
  if rest.ends_with("\n\n") {
    "\n\n"
  } else if rest.ends_with('\n') {
    "\n"
  } else {
    " "
  }
}

/// Translated pieces joined as their originals stood, each translation's own edges trimmed.
fn join(pieces: Vec<(String, &'static str)>) -> String {
  let mut out = String::new();
  let last = pieces.len().saturating_sub(1);
  for (at, (text, joined)) in pieces.into_iter().enumerate() {
    out.push_str(text.trim());
    if at < last {
      out.push_str(joined);
    }
  }
  out
}

/// One part translated; a part the translator finds too long is halved and each half asked again, in order.
pub async fn translate_part(
  peers: &Peers,
  language: &str,
  part: &str,
  note: &str,
) -> Result<String, String> {
  let mut waiting = vec![part.to_string()];
  let mut done = Vec::new();
  while let Some(piece) = waiting.pop() {
    match peers.translate(language, &piece, note).await {
      Ok(text) => done.push((text, joiner(&piece))),
      Err(Untranslated::TooLarge) => match halve(&piece) {
        Some((first, second)) => {
          waiting.push(second.to_string());
          waiting.push(first.to_string());
        }
        None => return Err("the translator refused as too long a part that cannot be halved".into()),
      },
      Err(Untranslated::Failed(why)) => return Err(why),
    }
  }
  Ok(join(done))
}

/// The parts translated, at most `out` of them with the translator at once, joined in their order.
async fn translate_parts(
  peers: &Peers,
  language: &str,
  parts: Vec<String>,
  note: &str,
  out: usize,
) -> Result<String, String> {
  let joiners: Vec<&'static str> = parts.iter().map(|part| joiner(part)).collect();
  let mut done: Vec<Option<String>> = vec![None; parts.len()];
  let mut next = parts.into_iter().enumerate();
  let mut asked = JoinSet::new();
  loop {
    while asked.len() < out.max(1) {
      let Some((at, part)) = next.next() else {
        break;
      };
      let (peers, language, note) = (peers.clone(), language.to_string(), note.to_string());
      asked.spawn(async move { (at, translate_part(&peers, &language, &part, &note).await) });
    }
    match asked.join_next().await {
      None => break,
      Some(Ok((at, Ok(text)))) => done[at] = Some(text),
      Some(Ok((_, Err(why)))) => return Err(why),
      Some(Err(e)) => return Err(format!("a part's translation stopped: {e}")),
    }
  }
  let pieces = done.into_iter().zip(joiners).filter_map(|(text, joined)| text.map(|t| (t, joined)));
  Ok(join(pieces.collect()))
}

/// The English document rendered in Korean, filed as a document of its own beside it, and shared.
pub async fn render(app: &App, english: &English<'_>) -> Result<Rendering, String> {
  let words = words::translation();
  let note = note(english.name);
  let titled =
    translate_part(&app.peers, &words.language, english.title, &title_note(english.name)).await?;
  let title = titled
    .lines()
    .map(str::trim)
    .find(|line| !line.is_empty())
    .unwrap_or(english.title)
    .to_string();
  let description = match english.description.filter(|d| !d.trim().is_empty()) {
    Some(description) => Some(translate_part(&app.peers, &words.language, description, &note).await?),
    None => None,
  };
  let parts = cut(english.body, app.settings.translation.part_chars);
  let body =
    translate_parts(&app.peers, &words.language, parts, &note, app.settings.translation.parts_out).await?;
  let document_id = app
    .peers
    .file_rendering(&NewRendering {
      item_id: english.item_id,
      translation_of: english.document_id,
      title: &title,
      description: description.as_deref(),
      body: &body,
    })
    .await?;
  let share_url = app.peers.share(document_id).await?;
  Ok(Rendering {
    document_id,
    title,
    description,
    share_url,
  })
}
