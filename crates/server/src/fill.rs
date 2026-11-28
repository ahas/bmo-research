//! What a search's result left unfilled (docs/runner.md, 「The fill」): read off a result that carries product
//! entries, its tables read by the markdown reader the document is drawn with.

use pulldown_cmark::{Event, Options, Parser, Tag, TagEnd};

/// The word a result writes in a cell it could not check.
const UNCHECKED: &str = "unchecked";

/// The line a product entry opens with.
const ITEM: &str = "===ITEM===";

/// The line a picture is given on.
const IMAGE: &str = "IMAGE:";

/// What a result left unfilled, one line each, in the order they stand; none for a result that carries no
/// product entries, which the fill does not read.
pub fn gaps(result: &str) -> Vec<String> {
  if !result.lines().any(|line| line.trim() == ITEM) {
    return Vec::new();
  }

  let mut gaps = Vec::new();

  for table in tables(result) {
    for row in &table.rows {
      let Some(first) = row.first() else {
        continue;
      };

      if !first.linked {
        gaps.push(format!("the row {:?} names no page it is sold on", first.text));
      }

      for (column, cell) in row.iter().enumerate().skip(1) {
        if cell.text.trim().eq_ignore_ascii_case(UNCHECKED) || cell.text.contains(UNCHECKED) {
          let head = table.head.get(column).map_or("?", String::as_str);
          gaps.push(format!("{} of {:?} is unchecked", head, first.text));
        }
      }
    }
  }

  if !result.lines().any(|line| line.trim_start().starts_with(IMAGE)) {
    gaps.push("no product has a picture".to_owned());
  }

  gaps
}

#[derive(Default)]
struct Cell {
  text: String,
  linked: bool,
}

#[derive(Default)]
struct Table {
  head: Vec<String>,
  rows: Vec<Vec<Cell>>,
}

// Every table of the result, its head's cells by text and each body row's cells with whether they link.
fn tables(result: &str) -> Vec<Table> {
  let mut tables = Vec::new();
  let mut table: Option<Table> = None;
  let mut row: Vec<Cell> = Vec::new();
  let mut cell: Option<Cell> = None;
  let mut in_head = false;

  for event in Parser::new_ext(result, Options::ENABLE_TABLES) {
    match event {
      Event::Start(Tag::Table(_)) => table = Some(Table::default()),
      Event::End(TagEnd::Table) => tables.extend(table.take()),
      Event::Start(Tag::TableHead) => in_head = true,
      Event::End(TagEnd::TableHead) => in_head = false,
      Event::Start(Tag::TableRow) => row = Vec::new(),
      Event::End(TagEnd::TableRow) => {
        if let Some(table) = table.as_mut() {
          table.rows.push(std::mem::take(&mut row));
        }
      }
      Event::Start(Tag::TableCell) => cell = Some(Cell::default()),
      Event::End(TagEnd::TableCell) => {
        if let (Some(done), Some(table)) = (cell.take(), table.as_mut()) {
          if in_head {
            table.head.push(done.text.trim().to_owned());
          } else {
            row.push(Cell {
              text: done.text.trim().to_owned(),
              linked: done.linked,
            });
          }
        }
      }
      Event::Start(Tag::Link { .. }) => {
        if let Some(cell) = cell.as_mut() {
          cell.linked = true;
        }
      }
      Event::Text(text) | Event::Code(text) => {
        if let Some(cell) = cell.as_mut() {
          cell.text.push_str(&text);
        }
      }
      _ => {}
    }
  }

  tables
}
