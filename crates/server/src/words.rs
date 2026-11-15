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
const TOPIC: &str = include_str!("../../../prompts/topic.md");
const REPORT: &str = include_str!("../../../prompts/report.toml");
const DOCUMENT: &str = include_str!("../../../prompts/document.toml");
const TRANSLATION: &str = include_str!("../../../prompts/translation.toml");
const PAGE: &str = include_str!("../../../prompts/page.toml");
pub const SEARCH_TASK: &str = include_str!("../../../prompts/search/task.md");
pub const SEARCH_BRANCH: &str = include_str!("../../../prompts/search/branch.md");
pub const SEARCH_REQUEST: &str = include_str!("../../../prompts/search/request.md");
pub const BROWSE_TASK: &str = include_str!("../../../prompts/browse/task.md");
pub const BROWSE_REQUEST: &str = include_str!("../../../prompts/browse/request.md");
pub const REFERENCE_TASK: &str = include_str!("../../../prompts/references/task.md");
pub const REFERENCE_REQUEST: &str = include_str!("../../../prompts/references/request.md");
const SEARCH_DOCUMENT: &str = include_str!("../../../prompts/search/document.toml");
const RUNNER: &str = include_str!("../../../prompts/runner.toml");
const REPAIR: &str = include_str!("../../../prompts/repair.toml");

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
  ("references", include_str!("../../../prompts/rules/references.md")),
];

/// The words of the notices.
#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ReportWords {
  pub line: String,
  pub follows: String,
  pub follows_document: String,
  pub fit: String,
  pub touches: String,
  pub name: String,
  pub plan: String,
  pub worker: String,
  pub stuck: String,
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
  pub events: Vec<EventWords>,
  pub topic: TopicWords,
  pub references: Vec<ReferenceWords>,
  pub sections: Sections,
  pub opening: Sections,
}

impl DocumentWords {
  /// The words a reference dossier is written in, in the language the person who asked for the draw writes.
  pub fn reference(&self, language: &str) -> &ReferenceWords {
    self
      .references
      .iter()
      .find(|words| words.language == language)
      .or_else(|| self.references.iter().find(|words| words.language == "en"))
      .expect("prompts/document.toml holds an en reference dossier")
  }
}

/// The headings, columns and reasons a reference dossier is written with, in one language.
#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ReferenceWords {
  pub language: String,
  /// The document's title, with `{name}` set to the subject's.
  pub title: String,
  pub what: String,
  pub appearance: String,
  pub aliases: String,
  pub constraints: String,
  pub candidates: String,
  pub sources: String,
  pub unresolved: String,
  /// The heading over the candidate JSON, which is the body's last fenced block.
  pub json: String,
  pub none: String,
  pub unnamed: String,
  pub no_url: String,
  pub no_page: String,
  pub no_dimensions: String,
  pub no_media: String,
  /// `{count}` where more candidates came than the ask allows.
  pub past_limit: String,
  pub title_column: String,
  pub page_column: String,
  pub address_column: String,
  pub creator_column: String,
  pub publisher_column: String,
  pub dimensions_column: String,
  pub media_column: String,
  pub licence_column: String,
}

/// Headings a deep dive's result holds, by its item's shape, in every language it may be written in: its
/// sections, or its opening's.
#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Sections {
  pub manual: Vec<String>,
  pub investigation: Vec<String>,
  pub advisory: Vec<String>,
  pub brief: Vec<String>,
  pub topic: Vec<String>,
}

impl Sections {
  pub fn of(&self, shape: Shape) -> &[String] {
    match shape {
      Shape::Manual => &self.manual,
      Shape::Investigation => &self.investigation,
      Shape::Advisory => &self.advisory,
      Shape::Brief => &self.brief,
      Shape::Topic => &self.topic,
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

/// A topic's events table's column words, in one language.
#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct EventWords {
  pub language: String,
  pub title: String,
  pub start: String,
  pub end: String,
  pub place: String,
  pub source: String,
}

/// What a topic's question says of the items its sources turned up and of its events table.
#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TopicWords {
  pub items: String,
  pub events: String,
}

pub fn report() -> &'static ReportWords {
  static WORDS: LazyLock<ReportWords> =
    LazyLock::new(|| toml::from_str(REPORT).expect("prompts/report.toml reads"));
  &WORDS
}

/// The words of a stuck item's repair, filed for the repair worker (`docs/api.md`, 「The repair」).
#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RepairWords {
  pub title: String,
  pub brief: String,
  pub item: String,
  #[serde(rename = "try")]
  pub attempt: String,
  pub line: String,
  pub no_log: String,
}

pub fn repair() -> &'static RepairWords {
  static WORDS: LazyLock<RepairWords> =
    LazyLock::new(|| toml::from_str(REPAIR).expect("prompts/repair.toml reads"));
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

/// The words one language of a deep dive's page is drawn with: its code, its name on the switch, what the
/// switch is called, the listen band's heading, and the line under the title saying where the research came
/// from: its label, its words for a source's item, an ask and a follow-up, and for the words an ask was made
/// with; and the title of a source's mark leading back to where the text cites it.
#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PageLanguage {
  pub code: String,
  pub name: String,
  pub switch: String,
  pub listen: String,
  pub origin: String,
  pub source: String,
  pub asked: String,
  pub follow_up: String,
  pub follow_up_untitled: String,
  pub because: String,
  pub back: String,
}

/// The title a page falls back on, by shape, in one language.
#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Titles {
  pub manual: String,
  pub investigation: String,
  pub advisory: String,
  pub brief: String,
  pub topic: String,
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
      Shape::Topic => &titles.topic,
    };
    fill(template, &[("name", name)])
  }
}

pub fn page() -> &'static PageWords {
  static WORDS: LazyLock<PageWords> =
    LazyLock::new(|| toml::from_str(PAGE).expect("prompts/page.toml reads"));
  &WORDS
}

/// The question an item of this shape is asked, its values set in: `name`, `address` and `origin` for one
/// thing, `stack` for an advisory, `items` for a brief, and for a topic its `instruction` with what it says
/// of its `items` and its `events`.
pub fn question(shape: Shape, values: &[(&str, &str)]) -> String {
  let template = match shape {
    Shape::Manual => MANUAL,
    Shape::Investigation => INVESTIGATION,
    Shape::Advisory => ADVISORY,
    Shape::Brief => BRIEF,
    Shape::Topic => TOPIC,
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
  pub picture_source: String,
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

/// One research key's rounds, asked in turn to the last.
#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Rounds {
  pub rounds: Vec<String>,
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
  pub scope: ScopeAsk,
  pub narration: NarrationAsk,
  pub whole: WholeAsk,
  pub references: ReferencesAsk,
  pub review: ReviewAsk,
}

/// What a references run's session is asked where its result is not the dossier JSON: the question, with
/// `{problems}` set to what the block lacks.
#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ReferencesAsk {
  pub whole: String,
}

/// What an advisory's run is asked before its research (`docs/runner.md`, 「The deep dive's page」): the
/// question, and the markers its answer opens with where the advisory is older than the age floor or not.
#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ScopeAsk {
  pub question: String,
  pub outside: String,
  pub inside: String,
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

/// What a deep dive's session is asked where its result is short of its shape: the question, with `{missing}`
/// set to what the result lacks.
#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct WholeAsk {
  pub question: String,
}

/// What a deep dive's whole page is reviewed with: the question, and the rulebook and its section the review is
/// held to.
#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ReviewAsk {
  pub question: String,
  pub rulebook: String,
  pub section: String,
  /// The names the page's parts are handed under, and the word standing for a part the page has none of.
  pub english: String,
  pub korean: String,
  pub english_narration: String,
  pub korean_narration: String,
  pub none: String,
}

/// The name of the round whose answer is a run's result.
pub const FINAL: &str = "final";

pub fn runner() -> &'static RunnerWords {
  static WORDS: LazyLock<RunnerWords> =
    LazyLock::new(|| toml::from_str(RUNNER).expect("prompts/runner.toml reads"));
  &WORDS
}
