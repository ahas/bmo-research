//! A part's structure kept by the daemon (`docs/api.md`, 「The page」; `docs/runner.md`, 「The page」): a table
//! written without the row under its head given it, the prose a part sends the translator, the translation
//! set back in the part's exact structure, the check every translated part passes, and the Korean that
//! stands in an English part's place.

use research_server::page::body_html;
use research_server::structure::{agrees, counterparts, marked_as, mended, skeleton};

/// A part's translation as a translator that marks every piece answers it.
fn marked(part: &str) -> (Vec<String>, String) {
  let skeleton = skeleton(part);
  let sent: Vec<String> = skeleton.pieces.iter().map(|piece| piece.text.clone()).collect();
  let said: Vec<String> = sent.iter().map(|text| format!("KO {text}")).collect();
  (sent, skeleton.filled(&said))
}

const TABLE: &str = "## Competitors and borrowed parts

| Route | How it removes the randomness | Public code |
| ERF (this paper) | keeps exponential clocks per token [2] | none found |
| The Diffusion Duality, arXiv:2506.10892 | solves a probability-flow ODE [10\u{2020}][11\u{2020}] | yes, github.com/s-sahoo/duo [12\u{2020}] |

## Sources

- [2] [the authors' own text](https://arxiv.org/pdf/2610.40618)
- [10\u{2020}] [a snippet of The Diffusion Duality](https://arxiv.org/abs/2506.10892v3)
- [11\u{2020}] [a snippet of its PF-ODE](https://arxiv.org/pdf/2506.10892v2)
- [12\u{2020}] [its code](https://github.com/s-sahoo/duo)
";

#[test]
fn a_table_written_without_the_row_under_its_head_is_given_it() {
  let table = "Some prose before it.\n| a | b |\n| c | d |\n\nAfter.\n";
  let html = body_html(table, "en");
  assert!(html.contains("<table>") && html.contains("<th>a</th>"), "{html}");
  assert!(html.contains("<p>Some prose before it.</p>"), "the prose stands apart: {html}");
  let whole = "| a | b |\n|---|---|\n| c | d |\n";
  assert_eq!(mended(whole), whole, "a table with its row is left as it stands");
  let fenced = "```\n| a | b |\n| c | d |\n```\n";
  assert_eq!(mended(fenced), fenced, "code is not a table");
  let listed = "- a list\n\n  | a | b |\n  | c | d |\n";
  assert!(body_html(listed, "en").contains("<table>"), "a table inside a list item too");
}

#[test]
fn a_table_part_with_citations_sends_its_cells_alone_and_is_set_back_as_a_table() {
  let (sent, korean) = marked(TABLE);
  assert!(sent.iter().all(|text| !text.contains('|')), "no pipe is sent: {sent:#?}");
  assert!(sent.iter().all(|text| !text.contains('\n')), "each piece on one line");
  assert!(
    sent.contains(&"keeps exponential clocks per token".to_string()),
    "a closing mark stays out"
  );
  assert!(sent.contains(&"solves a probability-flow ODE".to_string()));
  assert!(
    sent.contains(&"the authors' own text".to_string()),
    "a sources entry sends its words alone"
  );
  assert!(sent.iter().all(|text| !text.contains("https://")), "no address is sent");
  assert!(!sent.iter().any(|text| text.contains("[2]")), "an entry's mark stays out");
  assert!(agrees(TABLE, &korean), "{korean}");
  assert!(korean.contains("| --- | --- | --- |"), "the row under the head is rebuilt: {korean}");
  assert!(
    korean.contains("| KO solves a probability-flow ODE [10\u{2020}][11\u{2020}] |"),
    "{korean}"
  );
  assert!(
    korean.contains("- [12\u{2020}] [KO its code](https://github.com/s-sahoo/duo)"),
    "{korean}"
  );
  let html = body_html(&korean, "ko");
  assert!(html.contains("<table>") && html.contains("<th>KO Route</th>"), "{html}");
  for key in ["2", "10-snippet", "11-snippet", "12-snippet"] {
    assert!(
      html.contains(&format!("href=\"#ko-source-{key}\"")),
      "the mark {key} links its entry: {html}"
    );
  }
  let cell = &html[html.find("KO solves").unwrap()..];
  assert!(
    cell[..cell.find("</td>").unwrap()].contains("class=\"cite\""),
    "a mark in a cell is a footnote"
  );
}

#[test]
fn a_cell_s_pipe_is_escaped_and_a_translation_opening_a_block_is_escaped() {
  let part = "| a | b |\n|---|---|\n| one | two |\n\nA plain paragraph.\n";
  let skeleton = skeleton(part);
  let said: Vec<String> = ["x", "y", "left | right", "z", "# not a heading\nwrapped"]
    .iter()
    .map(|s| s.to_string())
    .collect();
  let korean = skeleton.filled(&said);
  assert!(korean.contains("left \\| right"), "{korean}");
  assert!(korean.contains("\\# not a heading wrapped"), "{korean}");
  assert!(agrees(part, &korean), "{korean}");
}

#[test]
fn a_list_keeps_its_markers_indentation_and_nesting() {
  let part = "Steps to run it:\n\n1. Fetch the weights.\n2. Run the server:\n   - on n3 with `--ctx 8192`\n   - on \
              n9\n3. Measure tokens/s.\n\n- [ ] check the license\n- [x] read the model card\n";
  let (sent, korean) = marked(part);
  assert_eq!(
    sent,
    [
      "Steps to run it:",
      "Fetch the weights.",
      "Run the server:",
      "on n3 with `--ctx 8192`",
      "on n9",
      "Measure tokens/s.",
      "check the license",
      "read the model card"
    ]
  );
  assert!(
    korean.contains("1. KO Fetch the weights.\n2. KO Run the server:\n   - KO on n3"),
    "{korean}"
  );
  assert!(korean.contains("- [ ] KO check the license\n- [x] KO read the model card"), "{korean}");
  assert!(agrees(part, &korean));
  let merged = "Steps to run it:\n\n1. Fetch the weights. Run the server.\n3. Measure tokens/s.\n";
  assert!(!agrees(part, merged), "a list that lost an item does not agree");
  let loose = "- one\n\n- two\n";
  assert!(
    agrees(loose, "- KO one\n- KO two\n"),
    "how loose a list is written is not its structure"
  );
}

#[test]
fn a_heading_set_keeps_every_heading_at_its_level_and_its_code_as_written() {
  let part = "# Thing-7B\n\n## What it is\n\nA small model.\n\n### Its card\n\n```toml\nname = \"keep me\"\n```\n\n\
              ## The answer ##\n\nIt fits.\n";
  let (sent, korean) = marked(part);
  assert_eq!(sent, ["Thing-7B", "What it is", "A small model.", "Its card", "The answer", "It fits."]);
  assert!(korean.starts_with("# KO Thing-7B\n\n## KO What it is\n\n"), "{korean}");
  assert!(
    korean.contains("### KO Its card\n\n```toml\nname = \"keep me\"\n```"),
    "code is kept: {korean}"
  );
  assert!(korean.contains("## KO The answer ##"), "{korean}");
  assert!(agrees(part, &korean));
  assert!(
    !agrees(part, &korean.replace("### KO Its card", "## KO Its card")),
    "a heading's level counts"
  );
  assert!(
    !agrees(part, &korean.replace("## KO What it is\n\n", "")),
    "a heading lost does not agree"
  );
}

#[test]
fn a_part_that_lost_a_row_or_a_mark_does_not_agree() {
  let (_, korean) = marked(TABLE);
  let row = korean.lines().find(|line| line.starts_with("| KO ERF")).unwrap();
  assert!(!agrees(TABLE, &korean.replace(&format!("{row}\n"), "")), "a row lost");
  assert!(!agrees(TABLE, &korean.replacen("[11\u{2020}]", "", 1)), "a mark lost");
  let flat = korean.replace("| --- | --- | --- |\n", "").replace("\n| ", " | ");
  assert!(!agrees(TABLE, &flat), "a table on one line");
}

#[test]
fn a_translation_keeps_its_english_piece_s_marks() {
  assert_eq!(
    marked_as("It fits [3] and runs [4].", "KO fits and runs [4][9]."),
    "KO fits and runs [4]. [3]"
  );
  assert_eq!(marked_as("No marks.", "KO no marks."), "KO no marks.");
}

#[test]
fn the_korean_in_an_english_part_s_place_is_found_where_its_section_agrees() {
  let english =
    "# Title\n\n## One\n\nFirst.\n\n## Two\n\n| a | b |\n|---|---|\n| c | d |\n\n## Three\n\nLast.\n";
  let korean = "# KO Title\n\n## KO One\n\nKO First.\n\n## KO Two\n\n| KO a | KO b |\n|---|---|\n\n## KO \
                Three\n\nKO Last.\n";
  let cut_at = |at: &str| english.find(at).unwrap();
  let parts = vec![
    english[..cut_at("## Two")].to_string(),
    english[cut_at("## Two")..cut_at("## Three")].to_string(),
    english[cut_at("## Three")..].to_string(),
  ];
  let places = counterparts(english, korean, &parts);
  assert_eq!(places[0].as_deref(), Some("# KO Title\n\n## KO One\n\nKO First."));
  assert_eq!(places[1], None, "the table lost its row");
  assert_eq!(places[2].as_deref(), Some("## KO Three\n\nKO Last."));
  let fewer = korean.replace("## KO One\n\n", "");
  assert!(counterparts(english, &fewer, &parts).iter().all(Option::is_none), "the headings differ");
}
