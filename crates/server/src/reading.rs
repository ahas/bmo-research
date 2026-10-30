//! The readings of a filed document (`docs/api.md`, 「The fit」 and 「The brief's facts」): its sections by
//! heading, its tables, and the words `prompts/document.toml` holds. The markdown is read by
//! `pulldown-cmark`; a reading reads words and judges nothing.

use pulldown_cmark::{Event, HeadingLevel, Options, Parser, Tag, TagEnd};
use unicode_segmentation::UnicodeSegmentation;

use crate::words::{AdvisoryWords, BriefWords, ManualWords};

/// One heading of the document: its level, its text, and where its section starts and ends in the text.
#[derive(Debug, Clone)]
struct Heading {
  level: HeadingLevel,
  text: String,
  start: usize,
  end: usize,
}

fn headings(content: &str) -> Vec<Heading> {
  let mut found: Vec<Heading> = Vec::new();
  let mut open: Option<(HeadingLevel, usize, String)> = None;
  for (event, range) in Parser::new_ext(content, Options::empty()).into_offset_iter() {
    match event {
      Event::Start(Tag::Heading { level, .. }) => open = Some((level, range.start, String::new())),
      Event::Text(text) | Event::Code(text) => {
        if let Some((_, _, words)) = open.as_mut() {
          words.push_str(&text);
        }
      }
      Event::End(TagEnd::Heading(_)) => {
        if let Some((level, start, text)) = open.take() {
          found.push(Heading {
            level,
            text: text.trim().to_string(),
            start,
            end: content.len(),
          });
        }
      }
      _ => {}
    }
  }
  // A section runs to the next heading at its level or above.
  for at in 0..found.len() {
    let level = found[at].level;
    if let Some(next) = found[at + 1..].iter().find(|h| h.level <= level) {
      found[at].end = next.start;
    }
  }
  found
}

/// The first section whose heading holds `word`: the heading as the document holds it, and the section's
/// text, the heading's line among it.
pub fn section<'a>(content: &'a str, word: &str) -> Option<(String, &'a str)> {
  headings(content)
    .into_iter()
    .find(|h| h.text.contains(word))
    .map(|h| (h.text, &content[h.start..h.end]))
}

/// A line with the marks a list item or emphasis opens it with taken off.
fn bare_line(line: &str) -> &str {
  let mut rest = line.trim();
  loop {
    let before = rest;
    rest = rest.trim_start_matches(['-', '*', '+', '>', '`', '_', ' ', '\t']);
    let digits = rest.chars().take_while(|c| c.is_ascii_digit()).count();
    if digits > 0 && rest[digits..].starts_with(['.', ')']) {
      rest = &rest[digits + 1..];
    }
    if rest == before {
      return rest;
    }
  }
}

/// What a manual's fit section says.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ManualFit {
  pub fit: bool,
  /// The nodes of `model_nodes` its line names as words of their own.
  pub nodes: Vec<String>,
  /// The heading of the reproduction plan, as the document holds it.
  pub plan: Option<String>,
}

/// A manual read for its fit: the line opening with the label under the fit heading, naming anything but
/// the none word.
pub fn manual_fit(content: &str, words: &ManualWords, model_nodes: &[String]) -> ManualFit {
  let plan = section(content, &words.plan_heading).map(|(heading, _)| heading);
  let Some((_, text)) = section(content, &words.heading) else {
    return ManualFit {
      plan,
      ..ManualFit::default()
    };
  };
  let named = text.lines().skip(1).find_map(|line| {
    bare_line(line)
      .strip_prefix(words.label.as_str())
      .map(|value| value.trim_matches(|c: char| c.is_whitespace() || c == '*' || c == '`' || c == '_'))
  });
  let Some(value) = named else {
    return ManualFit {
      plan,
      ..ManualFit::default()
    };
  };
  let fit = !value.is_empty() && !value.starts_with(words.none.as_str());
  let tokens: Vec<String> = value
    .split(|c: char| !c.is_alphanumeric())
    .map(|t| t.to_lowercase())
    .filter(|t| !t.is_empty())
    .collect();
  let nodes = if fit {
    model_nodes
      .iter()
      .filter(|node| tokens.contains(&node.to_lowercase()))
      .cloned()
      .collect()
  } else {
    Vec::new()
  };
  ManualFit { fit, nodes, plan }
}

/// One cell of a table: its text, and the address of the first link it holds.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Cell {
  pub text: String,
  pub link: Option<String>,
}

/// A table of the document: its head and its rows.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Table {
  pub head: Vec<Cell>,
  pub rows: Vec<Vec<Cell>>,
}

impl Table {
  /// The column whose head holds the word.
  pub fn column(&self, word: &str) -> Option<usize> {
    self.head.iter().position(|cell| cell.text.contains(word))
  }

  fn cell(row: &[Cell], column: Option<usize>) -> Option<&Cell> {
    column.and_then(|at| row.get(at))
  }
}

/// Every table of the document, in order.
pub fn tables(content: &str) -> Vec<Table> {
  let mut found = Vec::new();
  let mut table: Option<Table> = None;
  let mut row: Vec<Cell> = Vec::new();
  let mut cell: Option<Cell> = None;
  for event in Parser::new_ext(content, Options::ENABLE_TABLES) {
    match event {
      Event::Start(Tag::Table(_)) => table = Some(Table::default()),
      Event::End(TagEnd::Table) => found.extend(table.take()),
      Event::Start(Tag::TableHead) | Event::Start(Tag::TableRow) => row = Vec::new(),
      Event::End(TagEnd::TableHead) => {
        if let Some(t) = table.as_mut() {
          t.head = std::mem::take(&mut row);
        }
      }
      Event::End(TagEnd::TableRow) => {
        if let Some(t) = table.as_mut() {
          t.rows.push(std::mem::take(&mut row));
        }
      }
      Event::Start(Tag::TableCell) => cell = Some(Cell::default()),
      Event::End(TagEnd::TableCell) => {
        if let Some(mut c) = cell.take() {
          c.text = c.text.trim().to_string();
          row.push(c);
        }
      }
      Event::Start(Tag::Link { dest_url, .. }) => {
        if let Some(c) = cell.as_mut()
          && c.link.is_none()
        {
          c.link = Some(dest_url.to_string());
        }
      }
      Event::Text(text) | Event::Code(text) => {
        if let Some(c) = cell.as_mut() {
          c.text.push_str(&text);
        }
      }
      _ => {}
    }
  }
  found
}

/// One touched row of an advisory's exposure table.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Touch {
  pub component: String,
  pub version: String,
  pub repository: Option<String>,
  pub host: Option<String>,
}

/// What an advisory says it touches.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Exposure {
  pub touches: Vec<Touch>,
  /// The heading of the patch steps, as the document holds it.
  pub plan: Option<String>,
}

impl Exposure {
  /// The repositories and hosts touched, each once, joined.
  pub fn summary(&self) -> Option<String> {
    let mut named: Vec<String> = Vec::new();
    for touch in &self.touches {
      for one in [&touch.repository, &touch.host].into_iter().flatten() {
        if !one.is_empty() && !named.contains(one) {
          named.push(one.clone());
        }
      }
    }
    if named.is_empty() { None } else { Some(named.join(", ")) }
  }

  /// The repositories touched, each once, in the table's order.
  pub fn repositories(&self) -> Vec<String> {
    let mut named: Vec<String> = Vec::new();
    for repository in self.touches.iter().filter_map(|t| t.repository.clone()) {
      if !repository.is_empty() && !named.contains(&repository) {
        named.push(repository);
      }
    }
    named
  }
}

/// An advisory read for its exposure table: every row whose touched cell opens with the yes word.
pub fn exposure(content: &str, words: &AdvisoryWords) -> Exposure {
  let plan = section(content, &words.plan_heading).map(|(heading, _)| heading);
  let table = tables(content)
    .into_iter()
    .find(|t| t.column(&words.touched).is_some() && t.column(&words.component).is_some());
  let Some(table) = table else {
    return Exposure {
      plan,
      ..Exposure::default()
    };
  };
  let (component, version, repository, host, touched) = (
    table.column(&words.component),
    table.column(&words.version),
    table.column(&words.repository),
    table.column(&words.host),
    table.column(&words.touched),
  );
  let text = |row: &[Cell], column| Table::cell(row, column).map(|c| c.text.clone()).unwrap_or_default();
  let named = |row: &[Cell], column| Some(text(row, column)).filter(|t| !t.is_empty());
  let touches = table
    .rows
    .iter()
    .filter(|row| text(row, touched).starts_with(words.yes.as_str()))
    .map(|row| Touch {
      component: text(row, component),
      version: text(row, version),
      repository: named(row, repository),
      host: named(row, host),
    })
    .collect();
  Exposure { touches, plan }
}

/// One figure of a brief's figures table.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Figure {
  pub name: String,
  pub value: String,
  pub unit: Option<String>,
  pub as_of: Option<String>,
  pub source: Option<String>,
}

/// A brief read for its figures: every row of the figures table naming a name and a value.
pub fn figures(content: &str, words: &BriefWords) -> Vec<Figure> {
  let Some(table) = tables(content)
    .into_iter()
    .find(|t| t.column(&words.name).is_some() && t.column(&words.value).is_some())
  else {
    return Vec::new();
  };
  let (name, value, unit, as_of, source) = (
    table.column(&words.name),
    table.column(&words.value),
    table.column(&words.unit),
    table.column(&words.as_of),
    table.column(&words.source),
  );
  let text =
    |row: &[Cell], column| Table::cell(row, column).map(|c| c.text.clone()).filter(|t| !t.is_empty());
  table
    .rows
    .iter()
    .filter_map(|row| {
      Some(Figure {
        name: text(row, name)?,
        value: text(row, value)?,
        unit: text(row, unit),
        as_of: text(row, as_of),
        source: Table::cell(row, source).and_then(|c| c.link.clone()).or_else(|| text(row, source)),
      })
    })
    .collect()
}

/// The first sentence of a text, as Unicode's sentence rules find it.
pub fn first_sentence(text: &str) -> String {
  text.unicode_sentences().next().unwrap_or_default().trim().to_string()
}
