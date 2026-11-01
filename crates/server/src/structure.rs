//! A markdown text's structure (`docs/api.md`, 「The page」; `docs/runner.md`, 「The page」): a table written
//! without the row under its head given that row; the pieces of prose a part sends the translator and the
//! structure their translations are set back in; the citation marks a text holds; whether two texts agree in
//! their blocks and their marks; and the Korean that stands in an English part's place on a Korean side
//! made of it. The markdown is read by `pulldown-cmark`.

use std::collections::HashMap;
use std::ops::Range;

use pulldown_cmark::{Event, Options, Parser, Tag, TagEnd};

/// The markdown a page's body is read with: tables, strikethrough and task lists.
pub fn options() -> Options {
  Options::ENABLE_TABLES | Options::ENABLE_STRIKETHROUGH | Options::ENABLE_TASKLISTS
}

/// Every citation mark in a text, `[n]` or `[n†]`, by where it stands and its key, the mark without its
/// brackets.
pub fn marks(text: &str) -> Vec<(Range<usize>, &str)> {
  let mut found = Vec::new();
  let mut from = 0;
  while let Some(at) = text[from..].find('[').map(|at| from + at) {
    let key_from = at + 1;
    let digits = text[key_from..].bytes().take_while(u8::is_ascii_digit).count();
    let mut key_to = key_from + digits;
    if digits > 0 && text[key_to..].starts_with('\u{2020}') {
      key_to += '\u{2020}'.len_utf8();
    }
    if digits > 0 && text[key_to..].starts_with(']') {
      found.push((at..key_to + 1, &text[key_from..key_to]));
      from = key_to + 1;
    } else {
      from = key_from;
    }
  }
  found
}

// Tables.

/// Whether a line is a table's row: it opens and closes with a pipe, the closing one not escaped.
fn row_line(line: &str) -> bool {
  let line = line.trim();
  line.len() > 1 && line.starts_with('|') && line.ends_with('|') && !line.ends_with("\\|")
}

/// The cells of a table's row, split at its pipes outside an escape, the outer pipes' empty edges dropped.
fn cells_of(line: &str) -> Vec<&str> {
  let line = line.trim();
  let mut cells = Vec::new();
  let (mut from, mut escaped) = (0, false);
  for (at, c) in line.char_indices() {
    if escaped {
      escaped = false;
    } else if c == '\\' {
      escaped = true;
    } else if c == '|' {
      cells.push(&line[from..at]);
      from = at + 1;
    }
  }
  cells.push(&line[from..]);
  if cells.first().is_some_and(|cell| cell.trim().is_empty()) {
    cells.remove(0);
  }
  if cells.last().is_some_and(|cell| cell.trim().is_empty()) {
    cells.pop();
  }
  cells
}

/// Whether a row is the one under a table's head: every cell dashes, a colon at either end aside.
fn head_row(line: &str) -> bool {
  let cells = cells_of(line);
  !cells.is_empty()
    && cells.iter().all(|cell| {
      let dashes = cell.trim().trim_start_matches(':').trim_end_matches(':');
      !dashes.is_empty() && dashes.chars().all(|c| c == '-')
    })
}

/// What a paragraph's source lacks to stand as a table, as text to set in at offsets of it: the row under
/// the head of the rows it runs to its end with, two or more and none of them that row already, and a blank
/// line setting them apart from the prose it opens with. None where it holds no such rows.
fn table_lacks(paragraph: &str) -> Option<Vec<(usize, String)>> {
  let mut lines = Vec::new();
  let mut at = 0;
  for line in paragraph.split_inclusive('\n') {
    lines.push((at, line));
    at += line.len();
  }
  let from = lines.iter().rposition(|(_, line)| !row_line(line)).map_or(0, |at| at + 1);
  let rows = &lines[from..];
  if rows.len() < 2 || rows.iter().any(|(_, line)| head_row(line)) {
    return None;
  }
  let (start, head) = rows[0];
  let columns = cells_of(head).len();
  if columns == 0 || !head.ends_with('\n') {
    return None;
  }
  let next = rows[1].1;
  let indent = &next[..next.len() - next.trim_start().len()];
  let mut set = Vec::new();
  if from > 0 {
    set.push((start, "\n".to_string()));
  }
  set.push((start + head.len(), format!("{indent}|{}\n", " --- |".repeat(columns))));
  Some(set)
}

/// A text with every table written without the row under its head given that row (`docs/runner.md`, 「The
/// page」): rows that run to the end of a paragraph the library reads, set apart from the prose it opens
/// with.
pub fn mended(markdown: &str) -> String {
  let mut set: Vec<(usize, String)> = Vec::new();
  for (event, range) in Parser::new_ext(markdown, options()).into_offset_iter() {
    if let Event::Start(Tag::Paragraph) = event
      && let Some(lacks) = table_lacks(&markdown[range.clone()])
    {
      set.extend(lacks.into_iter().map(|(at, text)| (range.start + at, text)));
    }
  }
  if set.is_empty() {
    return markdown.to_string();
  }
  let mut out = String::with_capacity(markdown.len() + set.iter().map(|(_, t)| t.len()).sum::<usize>());
  let mut from = 0;
  for (at, text) in set {
    out.push_str(&markdown[from..at]);
    out.push_str(&text);
    from = at;
  }
  out.push_str(&markdown[from..]);
  out
}

// Blocks.

/// The name a block's start is told by in a text's shape, or none for a span.
fn block_name(tag: &Tag<'_>) -> Option<String> {
  Some(match tag {
    Tag::Paragraph => "p".into(),
    Tag::Heading { level, .. } => format!("{level:?}"),
    Tag::BlockQuote(_) => "quote".into(),
    Tag::CodeBlock(_) => "code".into(),
    Tag::HtmlBlock => "html".into(),
    Tag::List(Some(_)) => "ol".into(),
    Tag::List(None) => "ul".into(),
    Tag::Item => "li".into(),
    Tag::FootnoteDefinition(_) => "footnote".into(),
    Tag::DefinitionList => "dl".into(),
    Tag::DefinitionListTitle => "dt".into(),
    Tag::DefinitionListDefinition => "dd".into(),
    Tag::Table(columns) => format!("table{}", columns.len()),
    Tag::TableHead => "head".into(),
    Tag::TableRow => "row".into(),
    Tag::TableCell => "cell".into(),
    Tag::MetadataBlock(_) => "meta".into(),
    Tag::Emphasis
    | Tag::Strong
    | Tag::Strikethrough
    | Tag::Superscript
    | Tag::Subscript
    | Tag::Link { .. }
    | Tag::Image { .. } => return None,
  })
}

/// Whether an end closes a block rather than a span.
fn block_end(end: &TagEnd) -> bool {
  !matches!(
    end,
    TagEnd::Emphasis
      | TagEnd::Strong
      | TagEnd::Strikethrough
      | TagEnd::Superscript
      | TagEnd::Subscript
      | TagEnd::Link
      | TagEnd::Image
  )
}

/// A text's shape: its blocks in order and nesting, a list item's paragraphs aside, and the keys of the
/// citation marks its text holds, sorted, a mark in code aside.
fn shape(markdown: &str) -> (Vec<String>, Vec<String>) {
  let text = mended(markdown);
  let (mut blocks, mut cited) = (Vec::new(), Vec::new());
  let mut run = String::new();
  let mut code = false;
  // The blocks open, each by its name, or none for one the shape does not tell.
  let mut open: Vec<Option<String>> = Vec::new();
  for event in Parser::new_ext(&text, options()) {
    if let Event::Text(said) = &event {
      if !code {
        run.push_str(said);
      }
      continue;
    }
    cited.extend(marks(&run).into_iter().map(|(_, key)| key.to_string()));
    run.clear();
    match &event {
      Event::Start(tag) => {
        code |= matches!(tag, Tag::CodeBlock(_));
        if let Some(name) = block_name(tag) {
          // A list item's paragraph is how loose its list is written, not a block of its own.
          let loose =
            matches!(tag, Tag::Paragraph) && open.last().is_some_and(|had| had.as_deref() == Some("li"));
          if !loose {
            blocks.push(name.clone());
          }
          open.push((!loose).then_some(name));
        }
      }
      Event::End(end) if block_end(end) => {
        code &= !matches!(end, TagEnd::CodeBlock);
        if open.pop().flatten().is_some() {
          blocks.push("/".into());
        }
      }
      Event::Rule => blocks.push("rule".into()),
      _ => {}
    }
  }
  cited.extend(marks(&run).into_iter().map(|(_, key)| key.to_string()));
  cited.sort();
  (blocks, cited)
}

/// Whether a translation agrees with its English text (`docs/api.md`, 「The page」): both read with a table
/// without the row under its head given it, the same blocks in the same order and nesting, each heading at
/// its level and each table with its columns, and the same citation marks, each as often.
pub fn agrees(english: &str, translated: &str) -> bool {
  shape(english) == shape(translated)
}

/// A text's top-level blocks, each where it stands in the text and whether it is a heading.
fn top_blocks(text: &str) -> Vec<(Range<usize>, bool)> {
  let mut found = Vec::new();
  let mut depth = 0usize;
  for (event, range) in Parser::new_ext(text, options()).into_offset_iter() {
    match &event {
      Event::Start(tag) if block_name(tag).is_some() => {
        if depth == 0 {
          found.push((range, matches!(tag, Tag::Heading { .. })));
        }
        depth += 1;
      }
      Event::End(end) if block_end(end) => depth = depth.saturating_sub(1),
      Event::Rule if depth == 0 => found.push((range, false)),
      _ => {}
    }
  }
  found
}

/// A text's top-level blocks gathered into its sections, a section opening at each heading.
fn sections_of(blocks: &[(Range<usize>, bool)]) -> Vec<Range<usize>> {
  let mut found: Vec<Range<usize>> = Vec::new();
  for (at, (_, heading)) in blocks.iter().enumerate() {
    match found.last_mut() {
      Some(open) if !heading => open.end = at + 1,
      _ => found.push(at..at + 1),
    }
  }
  found
}

/// For each of `parts`, pieces of `english` in its order, the text of `translated`, a translation of it,
/// that stands in its place: where both texts hold as many sections, the same top-level blocks of the same
/// section, each agreeing with its English block, the part beginning and ending between blocks. None for a
/// part that has no such place.
pub fn counterparts(english: &str, translated: &str, parts: &[String]) -> Vec<Option<String>> {
  let (eb, tb) = (top_blocks(english), top_blocks(translated));
  let (es, ts) = (sections_of(&eb), sections_of(&tb));
  let mut paired: HashMap<usize, usize> = HashMap::new();
  if es.len() == ts.len() {
    for (e, t) in es.iter().zip(&ts) {
      if e.len() != t.len() {
        continue;
      }
      for (ei, ti) in e.clone().zip(t.clone()) {
        if agrees(&english[eb[ei].0.clone()], &translated[tb[ti].0.clone()]) {
          paired.insert(ei, ti);
        }
      }
    }
  }
  let mut at = 0;
  parts
    .iter()
    .map(|part| {
      let start = english[at..].find(part.as_str()).map(|found| at + found)?;
      let end = start + part.len();
      at = end;
      let mut inside = Vec::new();
      for (index, (range, _)) in eb.iter().enumerate() {
        let within = range.start >= start && range.end <= end;
        if within {
          inside.push(index);
        } else if range.start < end && range.end > start {
          return None;
        }
      }
      let mapped: Option<Vec<usize>> = inside.iter().map(|index| paired.get(index).copied()).collect();
      let mapped = mapped?;
      let (first, last) = (*mapped.first()?, *mapped.last()?);
      Some(translated[tb[first].0.start..tb[last].0.end].trim_end().to_string())
    })
    .collect()
}

// Pieces of prose.

/// One piece of a text's prose: its words as the text writes them on one line, and whether they are a
/// heading's words or a table cell's.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Piece {
  pub text: String,
  pub heading: bool,
  pub cell: bool,
}

/// A text's prose apart from its structure: its pieces of prose in order, and the text around them, kept as
/// the text writes it, one more than the pieces.
#[derive(Debug, Clone)]
pub struct Skeleton {
  pub pieces: Vec<Piece>,
  around: Vec<String>,
}

/// Whether an event is a span's, part of the prose a block holds.
fn spans(event: &Event<'_>) -> bool {
  match event {
    Event::Text(_)
    | Event::Code(_)
    | Event::InlineMath(_)
    | Event::InlineHtml(_)
    | Event::FootnoteReference(_)
    | Event::SoftBreak
    | Event::HardBreak => true,
    Event::Start(tag) => block_name(tag).is_none(),
    Event::End(end) => !block_end(end),
    _ => false,
  }
}

/// The prose a block holds as it is gathered: where it stands, and each link at its top with where its words
/// stand.
struct Gathered {
  range: Range<usize>,
  links: Vec<(Range<usize>, Option<Range<usize>>)>,
}

fn widened(range: &mut Range<usize>, by: &Range<usize>) {
  range.start = range.start.min(by.start);
  range.end = range.end.max(by.end);
}

/// Where a piece of prose stands once its citation marks at either edge are set outside it, and the words
/// of a link that is the whole of it in its place; none where it holds no letter.
fn refined(text: &str, gathered: &Gathered) -> Option<Range<usize>> {
  let mut range = gathered.range.clone();
  loop {
    let said = &text[range.clone()];
    let Some((place, _)) = marks(said).into_iter().next().filter(|(place, _)| place.start == 0) else {
      break;
    };
    let rest = &said[place.end..];
    if !(rest.is_empty() || rest.starts_with(char::is_whitespace) || rest.starts_with('[')) {
      break;
    }
    range.start += place.end + (rest.len() - rest.trim_start().len());
  }
  loop {
    let said = &text[range.clone()];
    let trimmed = said.trim_end();
    let Some((place, _)) =
      marks(trimmed).into_iter().last().filter(|(place, _)| place.end == trimmed.len())
    else {
      break;
    };
    let before = &trimmed[..place.start];
    range.end = range.start + before.trim_end().len();
  }
  if let [(whole, Some(words))] = gathered.links.as_slice()
    && *whole == range
  {
    range = words.clone();
  }
  text[range.clone()].chars().any(char::is_alphabetic).then_some(range)
}

/// A piece of prose on one line: each line's end, and the indentation and quote marks after it, one space.
fn one_line(text: &str, quoted: bool) -> String {
  text
    .lines()
    .enumerate()
    .map(|(at, line)| {
      let line = if at == 0 { line } else { line.trim_start() };
      let line = if at > 0 && quoted { line.trim_start_matches(['>', ' ']) } else { line };
      line.trim_end()
    })
    .filter(|line| !line.is_empty())
    .collect::<Vec<_>>()
    .join(" ")
}

/// A text's prose apart from its structure (`docs/api.md`, 「The page」), read once a table without the row
/// under its head is given it: each heading's words, each paragraph, each list item's words and each table
/// cell a piece, a citation mark at either edge of it and a link that is the whole of it but its words kept
/// around it, and a piece holding no letter, every code block, diagram and raw HTML kept as written.
pub fn skeleton(markdown: &str) -> Skeleton {
  let text = mended(markdown);
  let mut found: Vec<(Range<usize>, Piece)> = Vec::new();
  let mut open: Option<Gathered> = None;
  let (mut heading, mut cell, mut code, mut quotes) = (false, false, false, 0usize);
  // How deep in spans the reading is, and whether it is inside a link at the top of its block's prose.
  let (mut depth, mut linked) = (0usize, false);
  let mut close = |open: &mut Option<Gathered>, heading: bool, cell: bool, quoted: bool| {
    if let Some(gathered) = open.take()
      && let Some(range) = refined(&text, &gathered)
    {
      let said = one_line(&text[range.clone()], quoted);
      found.push((
        range,
        Piece {
          text: said,
          heading,
          cell,
        },
      ));
    }
  };
  for (event, range) in Parser::new_ext(&text, options()).into_offset_iter() {
    if !code && spans(&event) {
      let gathered = open.get_or_insert_with(|| Gathered {
        range: range.clone(),
        links: Vec::new(),
      });
      widened(&mut gathered.range, &range);
      match &event {
        Event::Start(Tag::Link { .. }) if depth == 0 => {
          gathered.links.push((range.clone(), None));
          linked = true;
        }
        Event::End(_) if depth == 1 => linked = false,
        _ if depth > 0 && linked => {
          if let Some((_, words)) = gathered.links.last_mut() {
            match words {
              Some(words) => widened(words, &range),
              None => *words = Some(range.clone()),
            }
          }
        }
        _ => {}
      }
      match &event {
        Event::Start(_) => depth += 1,
        Event::End(_) => depth = depth.saturating_sub(1),
        _ => {}
      }
      continue;
    }
    close(&mut open, heading, cell, quotes > 0);
    (depth, linked) = (0, false);
    match &event {
      Event::Start(Tag::Heading { .. }) => heading = true,
      Event::End(TagEnd::Heading(_)) => heading = false,
      Event::Start(Tag::TableCell) => cell = true,
      Event::End(TagEnd::TableCell) => cell = false,
      Event::Start(Tag::CodeBlock(_)) => code = true,
      Event::End(TagEnd::CodeBlock) => code = false,
      Event::Start(Tag::BlockQuote(_)) => quotes += 1,
      Event::End(TagEnd::BlockQuote(_)) => quotes = quotes.saturating_sub(1),
      _ => {}
    }
  }
  close(&mut open, heading, cell, quotes > 0);
  let mut around = Vec::with_capacity(found.len() + 1);
  let mut pieces = Vec::with_capacity(found.len());
  let mut from = 0;
  for (range, piece) in found {
    around.push(text[from..range.start].to_string());
    pieces.push(piece);
    from = range.end;
  }
  around.push(text[from..].to_string());
  Skeleton { pieces, around }
}

/// The mark a line of prose opens a block with, a heading's, a quote's, a list item's or a rule's: where
/// the mark stands to be escaped. None where it opens none.
fn block_mark(line: &str) -> Option<usize> {
  let first = line.chars().next()?;
  let rest = &line[first.len_utf8()..];
  match first {
    '#' | '>' => Some(0),
    '-' | '+' | '*' if rest.is_empty() || rest.starts_with(' ') => Some(0),
    '-' | '*' | '_' if line.starts_with("---") || line.starts_with("***") || line.starts_with("___") => {
      Some(0)
    }
    '0'..='9' => {
      let digits = line.bytes().take_while(u8::is_ascii_digit).count();
      let after = &line[digits..];
      (digits <= 9 && (after.starts_with(". ") || after.starts_with(") ") || after == "." || after == ")"))
        .then_some(digits)
    }
    _ => None,
  }
}

/// A translation with its citation marks its English piece's: a mark it added taken out, then each mark it
/// lost set at its end.
pub fn marked_as(english: &str, translated: &str) -> String {
  let mut owed: HashMap<&str, usize> = HashMap::new();
  for (_, key) in marks(english) {
    *owed.entry(key).or_default() += 1;
  }
  let mut out = String::with_capacity(translated.len());
  let mut from = 0;
  for (place, key) in marks(translated) {
    match owed.get_mut(key) {
      Some(left) if *left > 0 => *left -= 1,
      _ => {
        out.push_str(translated[from..place.start].trim_end_matches(' '));
        from = place.end;
      }
    }
  }
  out.push_str(&translated[from..]);
  let lost: Vec<&str> = marks(english)
    .into_iter()
    .filter_map(|(place, key)| {
      let left = owed.get_mut(key)?;
      (*left > 0).then(|| {
        *left -= 1;
        &english[place]
      })
    })
    .collect();
  let out = out.trim().to_string();
  match lost.is_empty() {
    true => out,
    false => format!("{out} {}", lost.concat()),
  }
}

impl Skeleton {
  /// The text with each piece's translation in its place, on one line, a pipe in a cell's escaped and a
  /// block's mark it opens with and its English piece does not escaped; a piece with no translation stands
  /// as written.
  pub fn filled(&self, said: &[String]) -> String {
    let mut out = String::new();
    for (at, piece) in self.pieces.iter().enumerate() {
      out.push_str(&self.around[at]);
      let mut text = match said.get(at) {
        Some(translated) => one_line(translated, false),
        None => piece.text.clone(),
      };
      if piece.cell {
        text = escaped_pipes(&text);
      }
      if let Some(mark) = block_mark(&text).filter(|_| block_mark(&piece.text).is_none()) {
        text.insert(mark, '\\');
      }
      out.push_str(&text);
    }
    out.push_str(self.around.last().map(String::as_str).unwrap_or_default());
    out
  }
}

/// A cell's text with every pipe not escaped yet escaped.
fn escaped_pipes(text: &str) -> String {
  let mut out = String::with_capacity(text.len());
  let mut escaped = false;
  for c in text.chars() {
    if c == '|' && !escaped {
      out.push('\\');
    }
    escaped = c == '\\' && !escaped;
    out.push(c);
  }
  out
}
