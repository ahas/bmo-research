//! A page's Korean side, its parts, its notes and a note the translator answered as text (`docs/api.md`, 「The
//! page」).

use research_server::translation::{cut, halve, narration_note, note, without_leaked_note};
use research_server::words;

fn chars(text: &str) -> usize {
  text.chars().count()
}

/// A body of sections, each its heading and `size` characters of prose in sentences.
fn body(sections: usize, size: usize) -> String {
  (1..=sections)
    .map(|n| {
      let mut prose = String::new();
      while chars(&prose) < size {
        prose.push_str(&format!("Sentence {n} of the section. "));
      }
      format!("## {n}. Section\n\n{}\n\n", prose.trim_end())
    })
    .collect()
}

#[test]
fn a_body_is_cut_at_its_headings_into_parts_that_fit() {
  let text = body(4, 200);
  let parts = cut(&text, 520);
  assert_eq!(parts.len(), 2, "two whole sections to a part: {parts:#?}");
  for part in &parts {
    assert!(chars(part) <= 520);
    assert!(part.starts_with("## "), "a part opens at a heading: {part}");
  }
  assert_eq!(parts.concat(), text, "nothing is lost or moved");
  assert_eq!(cut(&text, 100_000), std::slice::from_ref(&text), "a body that fits is one part");
}

#[test]
fn a_section_longer_than_a_part_is_halved_until_each_piece_fits() {
  let text = body(1, 2000);
  let parts = cut(&text, 600);
  assert!(parts.len() >= 4, "{}", parts.len());
  for part in &parts {
    assert!(chars(part) <= 600, "{}", chars(part));
  }
  assert_eq!(parts.concat(), text);
}

#[test]
fn a_text_is_halved_nearest_its_middle_at_the_best_boundary_it_holds() {
  let sections = body(2, 100);
  let (first, second) = halve(&sections).unwrap();
  assert!(second.starts_with("## 2."), "at a heading: {second}");
  assert_eq!(format!("{first}{second}"), sections);
  let (first, second) = halve("one two three four").unwrap();
  assert_eq!((first, second), ("one two ", "three four"), "at a space");
  let (first, second) = halve("abcdef").unwrap();
  assert_eq!((first, second), ("abc", "def"), "at the middle character");
  assert_eq!(halve("é"), None, "one character is not halved");
}

#[test]
fn the_notes_name_the_thing_and_the_language_is_korean() {
  let words = words::translation();
  assert_eq!(words.language, "Korean");
  let part = note("Thing-7B");
  assert!(part.contains("Thing-7B") && !part.contains("{name}"), "{part}");
  assert!(part.contains("research report"));
  let spoken = narration_note("Thing-7B");
  assert!(spoken.contains("Thing-7B") && spoken.contains("narration"), "{spoken}");
  assert_ne!(part, spoken);
}

#[test]
fn a_note_the_translator_answered_as_text_is_taken_out_and_nothing_else_is() {
  let leak = "A note about the report on Thing-7B, which keeps its names.";
  let part = "# Thing-7B manual\n\n## Running it\n\nIt runs.\n";
  let answer = format!("{leak}\n\n# Thing-7B manual (ko)\n\n## Running it (ko)\n\nIt runs (ko).\n");
  assert_eq!(without_leaked_note(part, &answer, "Thing-7B"), &answer[leak.len() + 2..]);
  let prose = "It runs on one card.\n\nIt needs 24 GB.";
  let leaked = format!("{leak}\n\nIt runs on one card (ko).\n\nIt needs 24 GB (ko).");
  assert!(without_leaked_note(prose, &leaked, "Thing-7B").starts_with("It runs on one card (ko)."));
  let kept = "Thing-7B runs on one card (ko).\n\nIt needs 24 GB (ko).";
  assert_eq!(
    without_leaked_note("Thing-7B runs on one card.\n\nIt needs 24 GB.", kept, "Thing-7B"),
    kept,
    "a translation holding the part's paragraphs keeps them"
  );
  let split = "Thing-7B runs (ko).\n\nOn one card (ko).\n\nIt needs 24 GB (ko).";
  assert_eq!(
    without_leaked_note("Thing-7B runs on one card.\n\nIt needs 24 GB.", split, "Thing-7B"),
    split,
    "a part opening with the name keeps a first paragraph saying it"
  );
}
