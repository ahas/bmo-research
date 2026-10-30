//! The readings of a filed document (`docs/api.md`, 「The fit」 and 「The brief's facts」). The documents are
//! composed here from the words `prompts/document.toml` holds, so this file holds none of them.

use research_server::reading;
use research_server::words;

fn nodes() -> Vec<String> {
  ["n3", "n4", "n5", "n6"].map(String::from).to_vec()
}

/// A manual whose fit section names `cards` on its label's line.
fn manual(cards: &str) -> String {
  let w = &words::document().manual;
  format!(
    "# Thing manual\n\n## 6. Limits\n\nNothing.\n\n## 7. {heading}\n\nThe arithmetic.\n\n- {label} {cards}\n- x{not} n9\n\n## 9. {plan}\n\n- step one\n",
    heading = w.heading,
    label = w.label,
    not = w.label,
    plan = w.plan_heading,
  )
}

#[test]
fn a_manual_naming_a_card_fits_and_keeps_the_model_nodes_it_names() {
  let read = reading::manual_fit(&manual("n3 (24 GB), n7"), &words::document().manual, &nodes());
  assert!(read.fit);
  assert_eq!(read.nodes, ["n3"]);
  assert_eq!(
    read.plan.as_deref(),
    Some(format!("9. {}", words::document().manual.plan_heading).as_str())
  );
}

#[test]
fn a_manual_naming_none_or_nothing_does_not_fit() {
  let w = &words::document().manual;
  assert!(!reading::manual_fit(&manual(&w.none), w, &nodes()).fit);
  assert!(!reading::manual_fit(&manual(""), w, &nodes()).fit);
  assert!(!reading::manual_fit("# A document with no such section\n", w, &nodes()).fit);
}

#[test]
fn a_label_inside_a_longer_label_is_not_the_line() {
  let w = &words::document().manual;
  let document = format!("## {}\n\n- x{} n3\n", w.heading, w.label);
  assert!(!reading::manual_fit(&document, w, &nodes()).fit);
}

/// An advisory's exposure table, its rows given as (component, repository, host, touched).
fn advisory(rows: &[(&str, &str, &str, bool)]) -> String {
  let w = &words::document().advisory;
  let mut text = format!(
    "## {}\n\n| {} | {} | {} | {} | {} |\n|---|---|---|---|---|\n",
    "exposure", w.component, w.version, w.repository, w.host, w.touched
  );
  for (component, repository, host, touched) in rows {
    let mark = if *touched { w.yes.clone() } else { "-".into() };
    text.push_str(&format!("| {component} | 1.0 | {repository} | {host} | {mark} |\n"));
  }
  text.push_str(&format!("\n## {}\n\n1. upgrade\n", w.plan_heading));
  text
}

#[test]
fn an_advisory_touches_the_rows_marked_and_names_their_repositories() {
  let document = advisory(&[
    ("libexample", "services/alpha", "node-a", true),
    ("libexample", "services/beta", "", true),
    ("otherlib", "services/gamma", "node-b", false),
  ]);
  let read = reading::exposure(&document, &words::document().advisory);
  assert_eq!(read.touches.len(), 2);
  assert_eq!(read.repositories(), ["services/alpha", "services/beta"]);
  assert_eq!(read.summary().as_deref(), Some("services/alpha, node-a, services/beta"));
  assert_eq!(read.plan.as_deref(), Some(words::document().advisory.plan_heading.as_str()));
  assert!(
    reading::exposure(&advisory(&[("x", "r", "h", false)]), &words::document().advisory)
      .touches
      .is_empty()
  );
}

#[test]
fn a_brief_keeps_each_figure_its_table_names_with_the_link_as_its_source() {
  let w = &words::document().brief;
  let document = format!(
    "# Brief\n\n| {} | {} | {} | {} | {} |\n|---|---|---|---|---|\n| index-a | 2,650.1 | pt | 2026-10-29 | [page](https://example.org/a) |\n| rate-b | 3.25 | % | 2026-10-30 | https://example.org/b |\n|  | 1 | x | | |\n",
    w.name, w.value, w.unit, w.as_of, w.source
  );
  let figures = reading::figures(&document, w);
  assert_eq!(figures.len(), 2, "a row naming no name is passed over");
  assert_eq!(figures[0].name, "index-a");
  assert_eq!(figures[0].value, "2,650.1");
  assert_eq!(figures[0].source.as_deref(), Some("https://example.org/a"));
  assert_eq!(figures[1].as_of.as_deref(), Some("2026-10-30"));
  assert!(reading::figures("no table here", w).is_empty());
}

#[test]
fn the_first_sentence_is_read_by_the_sentence_rules() {
  assert_eq!(reading::first_sentence("A model. It does things."), "A model.");
  assert_eq!(reading::first_sentence(""), "");
}

#[test]
fn every_template_reads_and_fills_what_it_names() {
  let q = words::question(
    bmo_research_client::Shape::Manual,
    &[("name", "Thing-7B"), ("address", "https://example.org/t")],
  );
  assert!(q.contains("Thing-7B") && q.contains("https://example.org/t") && !q.contains("{name}"));
  let b = words::question(bmo_research_client::Shape::Brief, &[("items", "- one — https://example.org/1")]);
  assert!(b.contains("https://example.org/1") && !b.contains("{items}"));
  let report = words::report();
  for template in [
    &report.line,
    &report.fit,
    &report.touches,
    &report.name,
    &report.plan,
    &report.bench,
    &report.worker,
    &report.failed,
  ] {
    assert!(!template.is_empty());
  }
  assert_eq!(words::fill("{a} {b} {c}", &[("a", "{b}"), ("b", "2")]), "{b} 2 {c}");
}
