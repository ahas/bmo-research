//! A deep dive's page's one review (`docs/runner.md`, 「The review」): the finished page handed to the review,
//! what it answered read as `PASS` or as its fixes, and each fix's section placed on the side it names, by its
//! heading on the English side and by the place that heading stands among the Korean side's headings.

use crate::reading;
use crate::words;

/// The part of the page a fix rewrites.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Part {
  /// A section of the English side, by its heading as the English side writes it.
  English(String),
  /// A section of the Korean side, by the heading the English side writes it under.
  Korean(String),
  /// A narration, by its language's code.
  Narration(&'static str),
}

/// One fix the review wrote: the part, why, and the part rewritten whole.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Fix {
  pub part: Part,
  pub why: Option<String>,
  pub text: String,
}

/// What the review answered: `PASS`, its fixes, or neither, the review then counted as asking no fix.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Answer {
  Pass,
  Fixes(Vec<Fix>),
  Unread,
}

const FIX: &str = "FIX:";
const WHY: &str = "WHY:";

/// The part a `FIX:` line names, or none where it names no part a fix may rewrite.
fn part_of(named: &str) -> Option<Part> {
  let named = named.trim();
  let (side, heading) = match named.split_once(char::is_whitespace) {
    Some((side, heading)) => (side, heading.trim()),
    None => (named, ""),
  };
  match side.to_ascii_lowercase().as_str() {
    "en" if !heading.is_empty() => Some(Part::English(heading.to_string())),
    "ko" if !heading.is_empty() => Some(Part::Korean(heading.to_string())),
    "narration-en" => Some(Part::Narration("en")),
    "narration-ko" => Some(Part::Narration("ko")),
    _ => None,
  }
}

/// What the review wrote, read: `PASS` where it holds no fix and its first line says it; each block a `FIX:`
/// line opens otherwise, its `WHY:` line taken out and the rest the part rewritten; a block naming no part, or
/// rewriting nothing, is left out.
pub fn read(answer: &str) -> Answer {
  let mut fixes = Vec::new();
  let mut open: Option<(Option<Part>, Option<String>, Vec<&str>)> = None;
  let close = |open: Option<(Option<Part>, Option<String>, Vec<&str>)>, fixes: &mut Vec<Fix>| {
    if let Some((Some(part), why, lines)) = open {
      let text = lines.join("\n").trim().to_string();
      if !text.is_empty() {
        fixes.push(Fix { part, why, text });
      }
    }
  };
  let mut blocks = 0;
  for line in answer.lines() {
    if let Some(named) = line.trim_start().strip_prefix(FIX) {
      close(open.take(), &mut fixes);
      blocks += 1;
      open = Some((part_of(named), None, Vec::new()));
      continue;
    }
    if let Some((_, why, lines)) = open.as_mut() {
      match line.trim_start().strip_prefix(WHY) {
        Some(said) if why.is_none() && lines.iter().all(|l| l.trim().is_empty()) => {
          *why = Some(said.trim().to_string())
        }
        _ => lines.push(line),
      }
    }
  }
  close(open.take(), &mut fixes);
  if blocks == 0 {
    let first = answer.lines().map(str::trim).find(|line| !line.is_empty()).unwrap_or_default();
    let first = first.trim_matches(|c: char| c == '*' || c == '`' || c == '.');
    return if first.eq_ignore_ascii_case("PASS") { Answer::Pass } else { Answer::Unread };
  }
  Answer::Fixes(fixes)
}

/// A heading's words as they are matched: a numbering and the marks around them aside, regardless of case.
fn bare(heading: &str) -> String {
  let words = heading.trim().trim_start_matches('#').trim();
  let words = words.trim_start_matches(|c: char| c.is_ascii_digit() || matches!(c, '.' | ')' | ' '));
  words.trim_matches(|c: char| matches!(c, '*' | '_' | '`' | ' ')).to_lowercase()
}

/// Where the section under `heading` stands among the content's headings, or none where none says it.
pub fn place(content: &str, heading: &str) -> Option<usize> {
  let wanted = bare(heading);
  reading::sections(content).iter().position(|(said, _, _)| bare(said) == wanted)
}

/// How many headings the content holds.
pub fn headings(content: &str) -> usize {
  reading::sections(content).len()
}

/// The content with the section at `at` among its headings set to `with`, or none where it holds no section
/// there.
pub fn replaced(content: &str, at: usize, with: &str) -> Option<String> {
  let (_, start, end) = reading::sections(content).into_iter().nth(at)?;
  let mut out = content[..start].to_string();
  out.push_str(with.trim_end());
  let rest = &content[end..];
  if !rest.trim().is_empty() {
    out.push_str("\n\n");
    out.push_str(rest.trim_start());
  } else if content.ends_with('\n') {
    out.push('\n');
  }
  Some(out)
}

/// Where a Korean section stands on the Korean side: at the place its English heading stands on the English
/// side, where both sides hold as many headings; else at the Korean heading its rewrite opens with, where
/// the Korean side holds one saying it.
pub fn korean_place(english: &str, korean: &str, heading: &str, rewrite: Option<&str>) -> Option<usize> {
  if headings(english) == headings(korean)
    && let Some(at) = place(english, heading)
  {
    return Some(at);
  }
  let opening = reading::first_heading(rewrite?)?;
  place(korean, &opening)
}

/// One side of the page as the review is handed it: its title, its description and its body.
pub struct Shown<'a> {
  pub title: &'a str,
  pub description: Option<&'a str>,
  pub body: &'a str,
}

/// The finished page as the review reads it: each part under its name, a part the page has none of said so.
pub fn page(english: &Shown<'_>, korean: &Shown<'_>, narrations: [Option<&str>; 2]) -> String {
  let ask = &words::runner().review;
  let side = |shown: &Shown<'_>| {
    let mut text = format!("# {}", shown.title.trim());
    if let Some(description) = shown.description.filter(|d| !d.trim().is_empty()) {
      text.push_str(&format!("\n\n{}", description.trim()));
    }
    text.push_str(&format!("\n\n{}", shown.body.trim()));
    text
  };
  let said =
    |text: Option<&str>| text.filter(|t| !t.trim().is_empty()).unwrap_or(&ask.none).trim().to_string();
  [
    (&ask.english, side(english)),
    (&ask.korean, side(korean)),
    (&ask.english_narration, said(narrations[0])),
    (&ask.korean_narration, said(narrations[1])),
  ]
  .iter()
  .map(|(name, text)| format!("==== {name} ====\n\n{text}"))
  .collect::<Vec<_>>()
  .join("\n\n")
}
