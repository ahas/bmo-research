//! A deep dive's page in Korean (`docs/api.md`, 「The page」): its body, the title set over it as its heading,
//! cut at its headings into parts the translator takes, each part translated with the note that gives it its
//! context and kept as it is had so a part that fails has only itself asked again, a part the translator
//! finds too long or translates short halved and asked again, a note the translator answered as text taken
//! out, and a description that is a name kept as written; a section the review rewrote translated alone;
//! and its narration's script translated to be spoken.

use tokio::task::JoinSet;

use crate::App;
use crate::peers::{Peers, Untranslated};
use crate::reading;
use crate::words::{self, fill};

/// A page's Korean side: its title, its description and its markdown body.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Rendering {
  pub title: String,
  pub description: Option<String>,
  pub body: String,
}

/// The English side a rendering is made of: the researched thing's name, and the page's title, description
/// and body, the body without the heading saying the title.
#[derive(Debug, Clone)]
pub struct English<'a> {
  pub name: &'a str,
  pub title: &'a str,
  pub description: Option<&'a str>,
  pub body: &'a str,
}

fn chars(text: &str) -> usize {
  text.chars().count()
}

/// The most characters a part may hold and still be taken as the translator answers it, its end unchecked.
const SHORT_UNCHECKED: usize = 200;

/// Whether a text ends a sentence: its last character, closing quotes, brackets and emphasis marks aside, a
/// full stop, a question mark, an exclamation mark or an ellipsis.
pub fn ends_sentence(text: &str) -> bool {
  let rest = text
    .trim_end()
    .trim_end_matches(['"', '\'', '\u{201d}', '\u{2019}', ')', ']', '*', '_']);
  rest.ends_with(['.', '!', '?', '\u{2026}', '\u{3002}', '\u{ff01}', '\u{ff1f}'])
}

/// Whether the translator cut a part's translation short: the part, longer than a few sentences, ends a
/// sentence and its translation does not.
pub fn cut_short(part: &str, translated: &str) -> bool {
  chars(part) > SHORT_UNCHECKED && ends_sentence(part) && !ends_sentence(translated)
}

/// Whether a description is a name rather than prose: the researched thing's `name`, case aside, or one word
/// with no whitespace in it. A name is never translated.
pub fn is_name(description: &str, name: &str) -> bool {
  let said = description.trim();
  said.eq_ignore_ascii_case(name.trim()) || !said.contains(char::is_whitespace)
}

/// The note every part of a report about `name` carries.
pub fn note(name: &str) -> String {
  fill(&words::translation().note, &[("name", name)])
}

/// The note a narration's script about `name` carries.
pub fn narration_note(name: &str) -> String {
  fill(&words::translation().narration_note, &[("name", name)])
}

// Where each paragraph of a text starts and ends, a paragraph being lines up to a blank one.
fn paragraphs(text: &str) -> Vec<(usize, usize)> {
  let mut found = Vec::new();
  let mut open: Option<usize> = None;
  let mut at = 0;
  for line in text.split_inclusive('\n') {
    if line.trim().is_empty() {
      if let Some(start) = open.take() {
        found.push((start, at));
      }
    } else if open.is_none() {
      open = Some(at);
    }
    at += line.len();
  }
  if let Some(start) = open {
    found.push((start, text.len()));
  }
  found
}

/// A translation with the note it was asked with taken out, where the translator answered the note as text:
/// while the translation holds more paragraphs than its part, its first paragraph is prose saying the thing's
/// `name`, and the part opens with a heading or with a paragraph not saying it, that paragraph is dropped.
pub fn without_leaked_note<'a>(part: &str, translated: &'a str, name: &str) -> &'a str {
  let source = paragraphs(part);
  let opens_apart = source.first().is_none_or(|(start, end)| {
    let first = &part[*start..*end];
    first.trim_start().starts_with('#') || !first.contains(name)
  });
  if name.trim().is_empty() || !opens_apart {
    return translated;
  }
  let said = paragraphs(translated);
  let mut from = 0;
  for (start, end) in &said[..said.len().saturating_sub(source.len())] {
    let paragraph = &translated[*start..*end];
    if paragraph.trim_start().starts_with('#') || !paragraph.contains(name) {
      break;
    }
    from += 1;
  }
  match said.get(from) {
    Some((start, _)) if from > 0 => &translated[*start..],
    _ => translated,
  }
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

/// One part of a text about `name` translated; a part the translator finds too long, or translates short, is
/// halved and each half asked again, in order, and a note the translator answered as text is taken out.
pub async fn translate_part(
  peers: &Peers,
  language: &str,
  part: &str,
  note: &str,
  name: &str,
) -> Result<String, String> {
  let mut waiting = vec![part.to_string()];
  let mut done = Vec::new();
  while let Some(piece) = waiting.pop() {
    match peers.translate(language, &piece, note).await {
      Ok(text) => {
        let text = without_leaked_note(&piece, &text, name);
        match halve(&piece).filter(|_| cut_short(&piece, text)) {
          Some((first, second)) => {
            waiting.push(second.to_string());
            waiting.push(first.to_string());
          }
          None => done.push((text.to_string(), joiner(&piece))),
        }
      }
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

/// One part of a page's text translated, the translation kept as the page's part held already where the
/// store holds one for its English text, else asked and kept as it is had.
async fn kept_part(
  app: &App,
  document_id: i64,
  language: &str,
  part: &str,
  (note, name): (&str, &str),
) -> Result<String, String> {
  if let Ok(Some(korean)) = app.store.translated_part(document_id, part) {
    return Ok(korean);
  }
  let korean = translate_part(&app.peers, language, part, note, name).await?;
  if let Err(why) = app.store.keep_translated_part(document_id, part, &korean) {
    eprintln!("bmo-research: a part of document {document_id}'s Korean side was not kept: {why}");
  }
  Ok(korean)
}

/// The parts of a page's body translated, at most `out` of them with the translator at once, joined in their
/// order: a part the store holds already taken as it was had, and each part asked kept as it is had, so a
/// part that fails has only itself asked again.
async fn translate_parts(
  app: &App,
  document_id: i64,
  language: &str,
  parts: Vec<String>,
  (note, name): (&str, &str),
  out: usize,
) -> Result<String, String> {
  let joiners: Vec<&'static str> = parts.iter().map(|part| joiner(part)).collect();
  let mut done: Vec<Option<String>> = vec![None; parts.len()];
  let mut waiting = Vec::new();
  for (at, part) in parts.into_iter().enumerate() {
    match app.store.translated_part(document_id, &part) {
      Ok(Some(korean)) => done[at] = Some(korean),
      _ => waiting.push((at, part)),
    }
  }
  let mut next = waiting.into_iter();
  let mut asked = JoinSet::new();
  let mut failed = None;
  loop {
    while failed.is_none() && asked.len() < out.max(1) {
      let Some((at, part)) = next.next() else {
        break;
      };
      let (peers, language, note, name) =
        (app.peers.clone(), language.to_string(), note.to_string(), name.to_string());
      asked.spawn(async move {
        let text = translate_part(&peers, &language, &part, &note, &name).await;
        (at, part, text)
      });
    }
    match asked.join_next().await {
      None => break,
      Some(Ok((at, part, Ok(text)))) => {
        if let Err(why) = app.store.keep_translated_part(document_id, &part, &text) {
          eprintln!("bmo-research: a part of document {document_id}'s Korean side was not kept: {why}");
        }
        done[at] = Some(text);
      }
      // The parts out beside a failed one are waited on and kept; no other is asked.
      Some(Ok((_, _, Err(why)))) => failed = failed.or(Some(why)),
      Some(Err(e)) => failed = failed.or(Some(format!("a part's translation stopped: {e}"))),
    }
  }
  if let Some(why) = failed {
    return Err(why);
  }
  let pieces = done.into_iter().zip(joiners).filter_map(|(text, joined)| text.map(|t| (t, joined)));
  Ok(join(pieces.collect()))
}

/// The English side rendered in Korean: the description, a name kept as written, then the body with the
/// title set over it as its heading, so the Korean title is the translated heading, else `title` where the
/// translation opens with none.
pub async fn render(
  app: &App,
  document_id: i64,
  english: &English<'_>,
  title: &str,
) -> Result<Rendering, String> {
  let words = words::translation();
  let note = note(english.name);
  let description = match english.description.filter(|d| !d.trim().is_empty()) {
    Some(description) if is_name(description, english.name) => Some(description.trim().to_string()),
    Some(description) => Some(
      kept_part(app, document_id, &words.language, description, (&note, english.name))
        .await?
        .trim()
        .to_string(),
    ),
    None => None,
  };
  let text = format!("# {}\n\n{}", english.title.trim(), english.body);
  let parts = cut(&text, app.settings.translation.part_chars);
  let body = translate_parts(
    app,
    document_id,
    &words.language,
    parts,
    (&note, english.name),
    app.settings.translation.parts_out,
  )
  .await?;
  let (title, body) = match reading::opening_heading(&body) {
    Some((said, rest)) if !said.is_empty() => (said, rest.to_string()),
    _ => (title.to_string(), body),
  };
  Ok(Rendering {
    title,
    description,
    body,
  })
}

/// One section of a page about `name` that its review rewrote in English, translated alone with the page's
/// note.
pub async fn section(app: &App, name: &str, text: &str) -> Result<String, String> {
  let words = words::translation();
  let korean = translate_part(&app.peers, &words.language, text, &note(name), name).await?;
  Ok(korean.trim().to_string())
}

/// A narration's script about `name` translated to be spoken, as one text.
pub async fn narration(app: &App, name: &str, script: &str) -> Result<String, String> {
  let words = words::translation();
  let spoken = translate_part(&app.peers, &words.language, script, &narration_note(name), name).await?;
  Ok(spoken.trim().to_string())
}
