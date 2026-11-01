//! A deep dive's page in Korean (`docs/api.md`, 「The page」): its body, the title set over it as its heading,
//! cut at its headings into parts the translator takes, each part's prose alone sent and its translation set
//! back in the part's own structure, checked against the part and kept as it is had so a part that fails has
//! only itself asked again, pieces the translator answers in another number or finds too long halved and
//! asked again, a piece translated short asked again alone, a note the translator answered as text taken out,
//! and a description that is a name kept as written; a section the review rewrote translated alone; a Korean
//! side kept that does not agree with its English set aside, the parts of it that still do kept; and its
//! narration's script translated to be spoken.

use tokio::task::JoinSet;

use crate::App;
use crate::peers::{Peers, Untranslated};
use crate::reading;
use crate::structure::{self, Piece};
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

/// The most terms one part carries.
const TERMS_MOST: usize = 40;

/// Whether a word of prose is a name a translation keeps: one holding both a letter and a digit, or a capital
/// after its first letter.
fn names_a_thing(word: &str) -> bool {
  let letters = word.chars().any(char::is_alphabetic);
  let digits = word.chars().any(|c| c.is_ascii_digit());
  (letters && digits) || word.chars().skip(1).any(|c| c.is_uppercase())
}

/// Whether a word after a number is its unit: one holding a slash, or an abbreviation of at most four letters
/// holding a capital.
fn unit(word: &str) -> bool {
  let letters = word.chars().all(|c| c.is_alphabetic() || c == '/');
  letters && (word.contains('/') || (word.chars().count() <= 4 && word.chars().any(char::is_uppercase)))
}

/// Whether a word is a number, a decimal mark or a thousands separator in it aside.
fn number(word: &str) -> bool {
  !word.is_empty()
    && word.chars().any(|c| c.is_ascii_digit())
    && word.chars().all(|c| c.is_ascii_digit() || c == '.' || c == ',')
}

/// What a part about `name` keeps exactly as written (`docs/api.md`, 「The page」), each once, in the order the
/// part holds them, at most `TERMS_MOST`: the name; every inline code span, link address and bare web address;
/// every fixed section name a heading holds, as the heading writes it; every word of prose naming a thing; and
/// every figure with its unit.
pub fn terms(part: &str, name: &str) -> Vec<String> {
  use pulldown_cmark::{Event, Options, Parser, Tag, TagEnd};
  let fixed: Vec<&String> = {
    let words = words::document();
    [&words.sections, &words.opening]
      .into_iter()
      .flat_map(|s| [&s.manual, &s.investigation, &s.advisory, &s.brief])
      .flatten()
      .filter(|word| word.is_ascii())
      .collect()
  };
  let mut found: Vec<String> = Vec::new();
  let keep = |term: &str, found: &mut Vec<String>| {
    let term = term.trim();
    if !term.is_empty() && !found.iter().any(|had| had == term) {
      found.push(term.to_string());
    }
  };
  keep(name, &mut found);
  let mut heading: Option<String> = None;
  for event in Parser::new_ext(part, Options::empty()) {
    match event {
      Event::Start(Tag::Heading { .. }) => heading = Some(String::new()),
      Event::End(TagEnd::Heading(_)) => {
        if let Some(said) = heading.take() {
          let said = said.trim();
          if fixed.iter().any(|word| said.to_lowercase().contains(&word.to_lowercase())) {
            keep(said, &mut found);
          }
        }
      }
      Event::Code(code) => {
        keep(&format!("`{code}`"), &mut found);
        if let Some(said) = heading.as_mut() {
          said.push_str(&code);
        }
      }
      Event::Start(Tag::Link { dest_url, .. }) => keep(&dest_url, &mut found),
      Event::Text(text) => {
        if let Some(said) = heading.as_mut() {
          said.push_str(&text);
        }
        let words: Vec<&str> = text
          .split_whitespace()
          .map(|word| word.trim_matches(|c: char| "()[]{},.;:!?\"'*_".contains(c)))
          .collect();
        let mut at = 0;
        while let Some(word) = words.get(at) {
          if word.starts_with("http://") || word.starts_with("https://") {
            keep(word, &mut found);
          } else if number(word) && words.get(at + 1).is_some_and(|next| unit(next)) {
            // The unit stands with its figure, never as a term of its own.
            keep(&format!("{word} {}", words[at + 1]), &mut found);
            at += 1;
          } else if names_a_thing(word) && !number(word) {
            keep(word, &mut found);
          }
          at += 1;
        }
      }
      _ => {}
    }
  }
  found.truncate(TERMS_MOST);
  found
}

/// One text about `name` translated as it stands; a text the translator finds too long, or translates short,
/// is halved and each half asked again, in order, and a note the translator answered as text is taken out.
pub async fn translate_text(
  peers: &Peers,
  language: &str,
  part: &str,
  note: &str,
  name: &str,
) -> Result<String, String> {
  let mut waiting = vec![part.to_string()];
  let mut done = Vec::new();
  while let Some(piece) = waiting.pop() {
    match peers.translate(language, &piece, (note, &terms(&piece, name))).await {
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

/// The pieces of a part's prose as they are sent, a blank line between two, a heading's words marked as a
/// heading's, so the terms and a note answered as text are read from them as from the part.
fn shown(pieces: &[Piece]) -> String {
  let lines: Vec<String> = pieces
    .iter()
    .map(|piece| match piece.heading {
      true => format!("# {}", piece.text),
      false => piece.text.clone(),
    })
    .collect();
  lines.join("\n\n")
}

/// A text's paragraphs, each trimmed.
fn paragraphs_of(text: &str) -> Vec<&str> {
  paragraphs(text).into_iter().map(|(start, end)| text[start..end].trim()).collect()
}

/// The pieces of a part's prose about `name` translated, each in its place (`docs/api.md`, 「The page」): sent
/// as one text, a blank line between two; where the answer holds another number of pieces, or was cut short,
/// halved and each half asked again, down to a piece asked alone, whose answer is taken whole; a piece
/// translated short asked again alone; and each translation's citation marks its English piece's.
async fn translate_pieces(
  peers: &Peers,
  language: &str,
  pieces: &[Piece],
  (note, name): (&str, &str),
) -> Result<Vec<String>, String> {
  let mut said: Vec<Option<String>> = vec![None; pieces.len()];
  let mut waiting: Vec<std::ops::Range<usize>> = Vec::new();
  waiting.push(0..pieces.len());
  let halved = |batch: std::ops::Range<usize>, waiting: &mut Vec<std::ops::Range<usize>>| {
    let middle = batch.start + batch.len() / 2;
    waiting.push(middle..batch.end);
    waiting.push(batch.start..middle);
  };
  while let Some(batch) = waiting.pop() {
    if batch.is_empty() {
      continue;
    }
    if batch.len() == 1 {
      let text = translate_text(peers, language, &pieces[batch.start].text, note, name).await?;
      said[batch.start] = Some(text);
      continue;
    }
    let sent: Vec<&str> = pieces[batch.clone()].iter().map(|piece| piece.text.as_str()).collect();
    let text = sent.join("\n\n");
    let marked = shown(&pieces[batch.clone()]);
    match peers.translate(language, &text, (note, &terms(&marked, name))).await {
      Ok(answer) => {
        let answer = without_leaked_note(&marked, &answer, name);
        let read = paragraphs_of(answer);
        if read.len() != batch.len() || cut_short(&text, answer) {
          halved(batch, &mut waiting);
          continue;
        }
        for (at, translated) in batch.zip(read) {
          match cut_short(&pieces[at].text, translated) {
            true => waiting.push(at..at + 1),
            false => said[at] = Some(translated.to_string()),
          }
        }
      }
      Err(Untranslated::TooLarge) => halved(batch, &mut waiting),
      Err(Untranslated::Failed(why)) => return Err(why),
    }
  }
  Ok(
    pieces
      .iter()
      .zip(said)
      .map(|(piece, translated)| structure::marked_as(&piece.text, &translated.unwrap_or_default()))
      .collect(),
  )
}

/// One part of a text about `name` translated in its own structure (`docs/api.md`, 「The page」): its prose
/// alone sent, and each piece's translation set back in the part's place for it, the part's structure kept as
/// it writes it. A translation that does not agree with the part is refused.
pub async fn translate_part(
  peers: &Peers,
  language: &str,
  part: &str,
  note: &str,
  name: &str,
) -> Result<String, String> {
  let skeleton = structure::skeleton(part);
  let said = translate_pieces(peers, language, &skeleton.pieces, (note, name)).await?;
  let korean = skeleton.filled(&said);
  if !structure::agrees(part, &korean) {
    return Err("a part's translation does not agree with its English part's structure".into());
  }
  Ok(korean)
}

/// One part of a page's text translated, the translation kept as the page's part held already where the
/// store holds one for its English text that agrees with it, else asked and kept as it is had.
async fn kept_part(
  app: &App,
  document_id: i64,
  language: &str,
  part: &str,
  (note, name): (&str, &str),
) -> Result<String, String> {
  if let Ok(Some(korean)) = app.store.translated_part(document_id, part)
    && structure::agrees(part, &korean)
  {
    return Ok(korean);
  }
  let korean = translate_part(&app.peers, language, part, note, name).await?;
  if let Err(why) = app.store.keep_translated_part(document_id, part, &korean) {
    eprintln!("bmo-research: a part of document {document_id}'s Korean side was not kept: {why}");
  }
  Ok(korean)
}

/// The parts of a page's body translated, at most `out` of them with the translator at once, joined in their
/// order: a part the store holds already taken as it was had while it agrees with its English part, and each
/// part asked kept as it is had, so a part that fails has only itself asked again.
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
      Ok(Some(korean)) if structure::agrees(&part, &korean) => done[at] = Some(korean),
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

/// A body with its title set over it as its heading, as a side is cut into parts and checked.
fn headed(title: &str, body: &str) -> String {
  format!("# {}\n\n{}", title.trim(), body)
}

/// The description a Korean side translates: the English one where it has one that is not a name.
fn translated_description<'a>(english: &English<'a>) -> Option<&'a str> {
  english.description.filter(|d| !d.trim().is_empty() && !is_name(d, english.name))
}

/// Whether a Korean side kept agrees with the English side it was made of (`docs/api.md`, 「The page」): its
/// body with its title set over it as the English body with its, and its description as the English one
/// where that is translated.
pub fn agrees(english: &English<'_>, korean: &Rendering) -> bool {
  let description = match translated_description(english) {
    Some(said) => korean.description.as_deref().is_some_and(|kept| structure::agrees(said, kept)),
    None => true,
  };
  description
    && structure::agrees(&headed(english.title, english.body), &headed(&korean.title, &korean.body))
}

/// The parts of a Korean side that does not agree with its English kept where they still do (`docs/api.md`,
/// 「The page」): each English part the body is cut into now takes as its translation the Korean that stands
/// in its place where that agrees with the part and none is kept for it already, and the description
/// likewise. Answers how many were kept.
pub fn keep_agreeing(app: &App, document_id: i64, english: &English<'_>, korean: &Rendering) -> usize {
  let text = headed(english.title, english.body);
  let parts = cut(&text, app.settings.translation.part_chars);
  let places = structure::counterparts(&text, &headed(&korean.title, &korean.body), &parts);
  let mut pairs: Vec<(&str, String)> = parts
    .iter()
    .zip(places)
    .filter_map(|(part, place)| place.map(|place| (part.as_str(), place)))
    .collect();
  if let (Some(said), Some(kept)) = (translated_description(english), korean.description.as_deref()) {
    pairs.push((said, kept.to_string()));
  }
  let mut kept = 0;
  for (part, korean) in pairs {
    let held = app.store.translated_part(document_id, part).ok().flatten();
    if held.is_some_and(|held| structure::agrees(part, &held)) || !structure::agrees(part, &korean) {
      continue;
    }
    match app.store.keep_translated_part(document_id, part, &korean) {
      Ok(()) => kept += 1,
      Err(why) => {
        eprintln!("bmo-research: a part of document {document_id}'s Korean side was not kept: {why}")
      }
    }
  }
  kept
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
  let text = headed(english.title, english.body);
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
  let spoken = translate_text(&app.peers, &words.language, script, &narration_note(name), name).await?;
  Ok(spoken.trim().to_string())
}
