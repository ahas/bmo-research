//! The words under `prompts/`, built into the binary: the question templates, one per shape, the notices'
//! words and the columns a source's readings are listed with, the words a filed document is read by and the
//! sections it holds, the notes the translator is handed
//! and the words a deep dive's page is drawn with (`docs/api.md`, 「The questions」 and 「The page」); and the runner's: the search and browse lines' tasks,
//! requests and branch instructions, the rulebooks, the words a run's document is written in, and the
//! rulebooks' order, the rounds and the narration's question (`docs/runner.md`).

use std::collections::BTreeMap;
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
const PAGE: &str = include_str!("../../../prompts/page.toml");
pub const SEARCH_TASK: &str = include_str!("../../../prompts/search/task.md");
pub const SEARCH_BRANCH: &str = include_str!("../../../prompts/search/branch.md");
pub const SEARCH_REQUEST: &str = include_str!("../../../prompts/search/request.md");
pub const BROWSE_TASK: &str = include_str!("../../../prompts/browse/task.md");
pub const BROWSE_REQUEST: &str = include_str!("../../../prompts/browse/request.md");
const SEARCH_DOCUMENT: &str = include_str!("../../../prompts/search/document.toml");
const RUNNER: &str = include_str!("../../../prompts/runner.toml");

/// The rulebooks under `prompts/rules/`, by name.
const RULEBOOKS: &[(&str, &str)] = &[
  ("search-kinds", include_str!("../../../prompts/rules/search-kinds.md")),
  ("product-search", include_str!("../../../prompts/rules/product-search.md")),
  ("restaurant-search", include_str!("../../../prompts/rules/restaurant-search.md")),
  ("investment-search", include_str!("../../../prompts/rules/investment-search.md")),
  ("study-material", include_str!("../../../prompts/rules/study-material.md")),
  ("deep-dives", include_str!("../../../prompts/rules/deep-dives.md")),
  ("research", include_str!("../../../prompts/rules/research.md")),
  ("security-advisory", include_str!("../../../prompts/rules/security-advisory.md")),
  ("economy-brief", include_str!("../../../prompts/rules/economy-brief.md")),
  ("plain-writing", include_str!("../../../prompts/rules/plain-writing.md")),
  ("self-preservation", include_str!("../../../prompts/rules/self-preservation.md")),
];

/// The words of the notices.
#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ReportWords {
  pub line: String,
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
  pub sources: SourceColumns,
}

/// The columns a source's readings are listed with: when it was last read and what its last reading that
/// succeeded kept, each with its word for a source that has none.
#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SourceColumns {
  pub read: String,
  pub unread: String,
  pub taken: String,
  pub untaken: String,
}

/// The words a filed document is read by, by shape, one set per language in the order they are tried.
#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DocumentWords {
  pub manual: Vec<ManualWords>,
  pub advisory: Vec<AdvisoryWords>,
  pub brief: Vec<BriefWords>,
  pub sections: Sections,
}

/// The section headings a deep dive's result holds, by its item's shape, in every language it may be written
/// in.
#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Sections {
  pub manual: Vec<String>,
  pub investigation: Vec<String>,
  pub advisory: Vec<String>,
  pub brief: Vec<String>,
}

impl Sections {
  pub fn of(&self, shape: Shape) -> &[String] {
    match shape {
      Shape::Manual => &self.manual,
      Shape::Investigation => &self.investigation,
      Shape::Advisory => &self.advisory,
      Shape::Brief => &self.brief,
    }
  }
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

/// The translator's words: the language the owner's copy is rendered in, the note a page's parts carry and
/// the one its narration's script carries.
#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TranslationWords {
  pub language: String,
  pub note: String,
  pub narration_note: String,
}

pub fn translation() -> &'static TranslationWords {
  static WORDS: LazyLock<TranslationWords> =
    LazyLock::new(|| toml::from_str(TRANSLATION).expect("prompts/translation.toml reads"));
  &WORDS
}

/// The words one language of a deep dive's page is drawn with: its code, its name on the switch and what the
/// switch is called.
#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PageLanguage {
  pub code: String,
  pub name: String,
  pub switch: String,
}

/// The title a page falls back on, by shape, in one language.
#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Titles {
  pub manual: String,
  pub investigation: String,
  pub advisory: String,
  pub brief: String,
}

/// A deep dive's page's words: each language it may hold, in the switch's order, and the titles it falls back
/// on by language.
#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PageWords {
  pub language: Vec<PageLanguage>,
  pub titles: BTreeMap<String, Titles>,
}

impl PageWords {
  /// A language's words, by its code.
  pub fn language(&self, code: &str) -> Option<&PageLanguage> {
    self.language.iter().find(|language| language.code == code)
  }

  /// The title an item of this shape falls back on in a language, its name set in.
  pub fn title(&self, code: &str, shape: Shape, name: &str) -> String {
    let titles = self
      .titles
      .get(code)
      .or_else(|| self.titles.get("en"))
      .expect("prompts/page.toml holds en");
    let template = match shape {
      Shape::Manual => &titles.manual,
      Shape::Investigation => &titles.investigation,
      Shape::Advisory => &titles.advisory,
      Shape::Brief => &titles.brief,
    };
    fill(template, &[("name", name)])
  }
}

pub fn page() -> &'static PageWords {
  static WORDS: LazyLock<PageWords> =
    LazyLock::new(|| toml::from_str(PAGE).expect("prompts/page.toml reads"));
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

/// A rulebook's text, by its name under `prompts/rules/`.
pub fn rulebook(name: &str) -> Result<&'static str, String> {
  RULEBOOKS
    .iter()
    .find(|(named, _)| *named == name)
    .map(|(_, text)| *text)
    .ok_or_else(|| format!("prompts/rules/{name}.md is not built in"))
}

/// The headings and labels a run's document stands under, in one language.
#[derive(Debug, Clone, Default, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct SearchDocumentWords {
  pub answer: String,
  pub grounds: String,
  pub unconfirmed: String,
  pub sources: String,
  pub address: String,
  pub map: String,
  pub roadview: String,
  pub menu: String,
  pub released: String,
  pub newer: String,
  pub stock: String,
  pub deal: String,
  pub review: String,
  pub news: String,
}

/// The words a run's document is written in, by language: `ko`, and `en` for every other.
pub fn search_document(language: &str) -> &'static SearchDocumentWords {
  static WORDS: LazyLock<BTreeMap<String, SearchDocumentWords>> =
    LazyLock::new(|| toml::from_str(SEARCH_DOCUMENT).expect("prompts/search/document.toml reads"));
  WORDS
    .get(language)
    .or_else(|| WORDS.get("en"))
    .expect("prompts/search/document.toml holds en")
}

/// What a past ceiling does to a run's rounds.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum PastCeiling {
  Final,
  Last,
}

/// One research key's rounds.
#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Rounds {
  pub rounds: Vec<String>,
  pub ceiling_seconds: i64,
  pub past_ceiling: PastCeiling,
}

/// The search's rulebooks: their order, those given by their deciding section alone, that section's heading,
/// and the two rulebooks every search ends with.
#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SearchRulebooks {
  pub rulebooks: Vec<String>,
  pub on_request: Vec<String>,
  pub applies_section: String,
  pub plain_writing: String,
  pub self_preservation: String,
}

/// What the runner composes a run of.
#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RunnerWords {
  pub search: SearchRulebooks,
  pub questions: BTreeMap<String, String>,
  pub rounds: BTreeMap<String, Rounds>,
  pub narration: NarrationAsk,
}

/// What a deep dive's session is asked for a narration its result lacks: the question, and the rulebook and
/// its section the narration is written by.
#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct NarrationAsk {
  pub question: String,
  pub rulebook: String,
  pub section: String,
}

/// The name of the round whose answer is a run's result.
pub const FINAL: &str = "final";

pub fn runner() -> &'static RunnerWords {
  static WORDS: LazyLock<RunnerWords> =
    LazyLock::new(|| toml::from_str(RUNNER).expect("prompts/runner.toml reads"));
  &WORDS
}
