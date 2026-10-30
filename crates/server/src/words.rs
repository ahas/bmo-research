//! The words under `prompts/`, built into the binary: the question templates, one per shape, the notices'
//! words, the words a filed document is read by, and the notes the translator is handed (`docs/api.md`,
//! 「The questions」 and 「The Korean rendering」).

use std::sync::LazyLock;

use bmo_research_client::Shape;
use serde::Deserialize;

const MANUAL: &str = include_str!("../../../prompts/manual.md");
const INVESTIGATION: &str = include_str!("../../../prompts/investigation.md");
const ADVISORY: &str = include_str!("../../../prompts/advisory.md");
const BRIEF: &str = include_str!("../../../prompts/brief.md");
const REPORT: &str = include_str!("../../../prompts/report.toml");
const DOCUMENT: &str = include_str!("../../../prompts/document.toml");
const TRANSLATION: &str = include_str!("../../../prompts/translation.toml");

/// The words of the notices.
#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ReportWords {
  pub line: String,
  pub english: String,
  pub pending: String,
  pub translated: String,
  pub fit: String,
  pub touches: String,
  pub name: String,
  pub plan: String,
  pub bench: String,
  pub worker: String,
  pub failed: String,
  pub restore: String,
}

/// The words a filed document is read by, by shape, one set per language in the order they are tried.
#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DocumentWords {
  pub manual: Vec<ManualWords>,
  pub advisory: Vec<AdvisoryWords>,
  pub brief: Vec<BriefWords>,
}

/// A manual's: the section on running the thing in the lab, its line of cards, and the plan's heading.
#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ManualWords {
  pub language: String,
  pub heading: String,
  pub label: String,
  pub none: String,
  pub plan_heading: String,
}

/// An advisory's: the exposure table's column words, the touched cell's word, and the plan's heading.
#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AdvisoryWords {
  pub language: String,
  pub component: String,
  pub version: String,
  pub repository: String,
  pub host: String,
  pub touched: String,
  pub yes: String,
  pub plan_heading: String,
}

/// A brief's figures table's column words.
#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct BriefWords {
  pub language: String,
  pub name: String,
  pub value: String,
  pub unit: String,
  pub as_of: String,
  pub source: String,
}

pub fn report() -> &'static ReportWords {
  static WORDS: LazyLock<ReportWords> =
    LazyLock::new(|| toml::from_str(REPORT).expect("prompts/report.toml reads"));
  &WORDS
}

pub fn document() -> &'static DocumentWords {
  static WORDS: LazyLock<DocumentWords> =
    LazyLock::new(|| toml::from_str(DOCUMENT).expect("prompts/document.toml reads"));
  &WORDS
}

/// The translator's words: the language the owner's copy is rendered in, and the notes its parts carry.
#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TranslationWords {
  pub language: String,
  pub note: String,
  pub title_note: String,
}

pub fn translation() -> &'static TranslationWords {
  static WORDS: LazyLock<TranslationWords> =
    LazyLock::new(|| toml::from_str(TRANSLATION).expect("prompts/translation.toml reads"));
  &WORDS
}

/// The question an item of this shape is asked, its values set in: `name` and `address` for one thing,
/// `stack` for an advisory, `items` for a brief.
pub fn question(shape: Shape, values: &[(&str, &str)]) -> String {
  let template = match shape {
    Shape::Manual => MANUAL,
    Shape::Investigation => INVESTIGATION,
    Shape::Advisory => ADVISORY,
    Shape::Brief => BRIEF,
  };
  fill(template, values).trim().to_string()
}

/// A template with each `{key}` it names replaced by its value, in one pass, so a value is never read
/// again as a template; a brace naming no key stands as written.
pub fn fill(template: &str, values: &[(&str, &str)]) -> String {
  let mut out = String::with_capacity(template.len());
  let mut rest = template;
  while let Some(open) = rest.find('{') {
    out.push_str(&rest[..open]);
    let after = &rest[open + 1..];
    let named = after
      .find('}')
      .and_then(|close| values.iter().find(|(key, _)| *key == &after[..close]).map(|(_, v)| (close, *v)));
    match named {
      Some((close, value)) => {
        out.push_str(value);
        rest = &after[close + 1..];
      }
      None => {
        out.push('{');
        rest = after;
      }
    }
  }
  out.push_str(rest);
  out
}
