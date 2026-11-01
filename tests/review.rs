//! A deep dive's page's review read and placed (`docs/runner.md`, 「The review」): `PASS`, the fixes it wrote
//! with what each names, an answer holding neither, and each rewritten section placed by its heading on the
//! English side and by that heading's place among the Korean side's headings.

use research_server::review::{Answer, Fix, Part, korean_place, place, read, replaced};

#[test]
fn a_pass_is_read_as_one_whatever_marks_stand_round_it() {
  assert_eq!(read("PASS"), Answer::Pass);
  assert_eq!(read("\n  **PASS**\n"), Answer::Pass);
  assert_eq!(read("The page looks fine to me."), Answer::Unread);
  assert_eq!(read(""), Answer::Unread);
}

#[test]
fn each_fix_block_is_read_with_its_part_its_reason_and_its_rewrite() {
  let answer = "FIX: en 3. What the maker states\nWHY: The figure differs from the source.\n## 3. What the \
                maker states\n\nIt decodes at 38.6 tokens/s.\n\nFIX: ko Verdict\nWHY: Machine word order.\n## \
                [ko] Verdict\n\n[ko] Use it.\nFIX: narration-en\nWHY: A wrong figure.\nIt decodes at 38.6 tokens \
                a second.\nFIX: narration-ko\n[ko] spoken words\nFIX: elsewhere Nothing\nignored\nFIX: en Empty\n";
  let Answer::Fixes(fixes) = read(answer) else {
    panic!("fixes are read");
  };
  assert_eq!(
    fixes,
    [
      Fix {
        part: Part::English("3. What the maker states".into()),
        why: Some("The figure differs from the source.".into()),
        text: "## 3. What the maker states\n\nIt decodes at 38.6 tokens/s.".into(),
      },
      Fix {
        part: Part::Korean("Verdict".into()),
        why: Some("Machine word order.".into()),
        text: "## [ko] Verdict\n\n[ko] Use it.".into(),
      },
      Fix {
        part: Part::Narration("en"),
        why: Some("A wrong figure.".into()),
        text: "It decodes at 38.6 tokens a second.".into(),
      },
      Fix {
        part: Part::Narration("ko"),
        why: None,
        text: "[ko] spoken words".into(),
      },
    ],
    "a block naming no part, or rewriting nothing, is left out"
  );
}

const ENGLISH: &str = "## What it is\n\nA model.\n\n## 3. What the maker states\n\nIt decodes at 40 tokens/s.\n\n\
                       ### Conditions\n\nOne card.\n\n## Sources\n\n1. https://example.org\n";

#[test]
fn a_section_is_found_by_its_heading_its_numbering_and_case_aside_and_replaced_with_its_subsections() {
  assert_eq!(place(ENGLISH, "what the maker states"), Some(1));
  assert_eq!(place(ENGLISH, "## 3. What the maker states"), Some(1));
  assert_eq!(place(ENGLISH, "Nowhere"), None);
  let fixed =
    replaced(ENGLISH, 1, "## 3. What the maker states\n\nIt decodes at 38.6 tokens/s.\n").unwrap();
  assert_eq!(
    fixed,
    "## What it is\n\nA model.\n\n## 3. What the maker states\n\nIt decodes at 38.6 tokens/s.\n\n## Sources\n\n1. \
     https://example.org\n"
  );
  let last = replaced(ENGLISH, 3, "## Sources\n\n1. https://example.org/model").unwrap();
  assert!(last.ends_with("## Sources\n\n1. https://example.org/model\n"), "{last}");
  assert_eq!(replaced(ENGLISH, 9, "## Nothing"), None);
}

#[test]
fn a_korean_section_stands_where_its_english_heading_stands_else_where_its_rewrite_s_heading_does() {
  let korean = "## [ko] What it is\n\n[ko] A model.\n\n## [ko] 3. What the maker states\n\n[ko] 40.\n\n### [ko] \
                Conditions\n\n[ko] One card.\n\n## [ko] Sources\n\n1. https://example.org\n";
  assert_eq!(korean_place(ENGLISH, korean, "What the maker states", None), Some(1));
  let merged = "## [ko] What it is\n\n[ko] A model.\n\n## [ko] Sources\n\n1. https://example.org\n";
  assert_eq!(korean_place(ENGLISH, merged, "What the maker states", None), None);
  assert_eq!(
    korean_place(ENGLISH, merged, "Sources", Some("## [ko] Sources\n\n1. https://example.org/model")),
    Some(1),
    "a side holding other headings is placed by the rewrite's own heading"
  );
}
