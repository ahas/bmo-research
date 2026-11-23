//! The `references` research key (`docs/runner.md`, 「The references dossier」): the options the drawing
//! service asks it with, the candidate list it must bring back with each licence line as metadata, and the
//! dossier it files in the archive for the drawing service to read. The run itself is a search run,
//! composed and sent by the runner; this module holds what is the key's own: what it takes, what it
//! accepts, and the document it files.

use std::collections::BTreeSet;

use bmo_research_client::ResearchRunAsk;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use url::Url;

use crate::App;
use crate::document;
use crate::reading;
use crate::store::{RunDocument, now_ms};
use crate::words::{self, ReferenceWords};

/// The key this module answers.
pub const KEY: &str = "references";

/// The label the drawing service reads the dossier's shape by.
pub const SCHEMA: &str = "references-v1";

/// The subject kinds the design's subject record names, and the key takes.
pub const KINDS: [&str; 7] = ["character", "person", "place", "object", "style", "work", "other"];

/// The licence-carrying image engines the design names, and the only ones a run asks.
pub const ENGINES: [&str; 3] = ["openverse", "wikicommons.images", "unsplash"];

/// How many candidates a dossier lists where the ask names none.
pub const CANDIDATES: usize = 12;

/// The most candidates one ask may name.
pub const MOST_CANDIDATES: usize = 24;

/// What a run of the key is asked with, read off the ask's options.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReferenceAsk {
  /// The subject's key, `<slug>-<hash8>`, which the drawing service builds and the dossier carries.
  pub subject_key: String,
  /// `character`, `person`, `place`, `object`, `style`, `work` or `other`.
  pub kind: String,
  /// The dossier's language, `ko` or `en`, the language of the person who asked for the draw.
  pub language: String,
  /// The most candidates the dossier lists.
  pub candidate_limit: usize,
  /// The image engines the run may ask, each one `ENGINES` names.
  pub image_engines: Vec<String>,
}

/// The options as the drawing service writes them, read before anything is looked up.
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ReferenceOptions {
  subject_key: String,
  kind: String,
  #[serde(default)]
  language: Option<String>,
  #[serde(default)]
  candidate_limit: Option<i64>,
  #[serde(default)]
  image_engines: Option<Vec<String>>,
}

/// Why an ask is not a references ask, or the references ask it is.
pub fn ask(ask: &ResearchRunAsk) -> Result<ReferenceAsk, String> {
  let value = match ask.options.as_ref() {
    Some(Value::String(text)) => serde_json::from_str::<Value>(text)
      .map_err(|e| format!("the options of a references run are not JSON: {e}"))?,
    Some(value) if value.is_object() => value.clone(),
    Some(_) => return Err("a references run names its options as an object".into()),
    None => {
      return Err(
        "a references run names its options, with its subjectKey, its kind and its language".into(),
      );
    }
  };
  let options: ReferenceOptions = serde_json::from_value(value)
    .map_err(|e| format!("the options of a references run do not read: {e}"))?;

  let subject_key = options.subject_key.trim().to_owned();
  if subject_key.is_empty() {
    return Err("a references run names its subjectKey: the subject's key in the brain".into());
  }
  let kind = options.kind.trim().to_ascii_lowercase();
  if !KINDS.contains(&kind.as_str()) {
    return Err(format!("{kind} is no subject kind for a reference dossier: {}", KINDS.join(", ")));
  }
  let language = match options.language.as_deref().map(str::trim).filter(|code| !code.is_empty()) {
    Some(code) if code == "ko" || code == "en" => code.to_owned(),
    Some(code) => return Err(format!("{code} is no dossier language: ko or en")),
    None => document::language_of(&ask.query).to_owned(),
  };
  let candidate_limit = match options.candidate_limit {
    None => CANDIDATES,
    Some(limit) if limit > 0 && limit <= MOST_CANDIDATES as i64 => limit as usize,
    Some(limit) => {
      return Err(format!("a candidate limit of {limit} does not stand: 1 to {MOST_CANDIDATES}"));
    }
  };
  let image_engines = match options.image_engines {
    None => ENGINES.iter().map(|engine| (*engine).to_owned()).collect(),
    Some(engines) => {
      let mut named: Vec<String> = Vec::new();
      for engine in engines {
        let engine = engine.trim().to_ascii_lowercase();
        if !ENGINES.contains(&engine.as_str()) {
          return Err(format!(
            "{engine} is no image engine for a reference dossier: {}",
            ENGINES.join(", ")
          ));
        }
        if !named.contains(&engine) {
          named.push(engine);
        }
      }
      if named.is_empty() {
        return Err("a references run names at least one image engine".into());
      }
      named
    }
  };
  Ok(ReferenceAsk {
    subject_key,
    kind,
    language,
    candidate_limit,
    image_engines,
  })
}

/// One subject of a dossier and the names it answers to.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Subject {
  pub name: String,
  pub aliases: Vec<String>,
  pub kind: String,
}

/// One candidate picture, with every field the drawing service reads before it fetches the bytes itself.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Candidate {
  /// The direct address of the picture itself, never the page.
  pub url: String,
  /// The page the picture came from, null where it stands alone.
  pub page_url: Option<String>,
  pub title: String,
  pub creator: String,
  pub publisher: String,
  /// Epoch milliseconds of the read; the run's own moment where the answer names none.
  pub retrieved_at: i64,
  pub licence: Licence,
  pub media_type: String,
  pub width: u64,
  pub height: u64,
  pub note: String,
}

/// The licence a candidate carries, as its source states it: metadata reported with the candidate, which
/// decides nothing. A source that states none leaves it empty.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Licence {
  /// The licence's own identifier, such as `CC-BY-4.0`, `CC0-1.0`, `PDM-1.0` or `Unsplash`.
  pub id: String,
  /// The licence's own page, where the source names one.
  pub url: Option<String>,
  /// The attribution line the licence requires.
  pub attribution: String,
}

/// A page the written description rests on.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Source {
  pub url: String,
  pub what: String,
}

/// The whole dossier: what the drawing service parses out of the document's last fenced block.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Dossier {
  pub subject: Subject,
  pub description: String,
  pub features: Vec<String>,
  pub constraints: Vec<String>,
  pub candidates: Vec<Candidate>,
  pub sources: Vec<Source>,
  pub unresolved: Vec<String>,
}

/// The dossier a run's result holds, or why it is not one: its last fenced block read as JSON against the
/// key's shape, its candidates held to the fields the drawing service needs. A candidate it cannot support
/// is dropped, with why said under `unresolved`; a licence never drops one.
pub fn dossier(text: &str, ask: &ReferenceAsk, now: i64) -> Result<Dossier, String> {
  let (_, block) = reading::last_fenced(text).ok_or("its answer holds no fenced block")?;
  let value: Value =
    serde_json::from_str(&block).map_err(|e| format!("its last fenced block is not JSON: {e}"))?;
  let object = value.as_object().ok_or("its fenced block is not a JSON object")?;

  let subject = object.get("subject").and_then(Value::as_object).ok_or("its subject does not stand")?;
  let name = subject
    .get("name")
    .and_then(Value::as_str)
    .map(str::trim)
    .filter(|name| !name.is_empty())
    .ok_or("its subject names no name")?;
  let aliases = strings(subject.get("aliases"));

  let description = object
    .get("description")
    .and_then(Value::as_str)
    .map(str::trim)
    .filter(|text| !text.is_empty())
    .ok_or("its description is empty")?
    .to_owned();
  let features = strings(object.get("features"));
  let constraints = strings(object.get("constraints"));

  if ask.language == "ko"
    && !document::has_hangul(&format!("{description}\n{}\n{}", features.join("\n"), constraints.join("\n")))
  {
    return Err("its description, features and constraints are not written in Korean".into());
  }

  let words = words::document().reference(&ask.language);
  let mut unresolved = strings(object.get("unresolved"));
  let listed = object.get("candidates").and_then(Value::as_array).ok_or("it holds no candidate list")?;
  let mut candidates: Vec<Candidate> = Vec::new();
  let mut seen: BTreeSet<String> = BTreeSet::new();
  let mut past = 0usize;
  for entry in listed {
    match candidate(entry, words, now) {
      Ok(one) => {
        if !seen.insert(one.url.clone()) {
          continue;
        }
        if candidates.len() >= ask.candidate_limit {
          past += 1;
          continue;
        }
        candidates.push(one);
      }
      Err(reason) => unresolved.push(format!("{}: {reason}", label(entry, words))),
    }
  }
  if past > 0 {
    unresolved.push(words::fill(&words.past_limit, &[("count", &past.to_string())]));
  }

  let sources = object
    .get("sources")
    .and_then(Value::as_array)
    .map(|items| items.iter().filter_map(source).collect())
    .unwrap_or_default();

  Ok(Dossier {
    subject: Subject {
      name: name.to_owned(),
      aliases,
      kind: ask.kind.clone(),
    },
    description,
    features,
    constraints,
    candidates,
    sources,
    unresolved,
  })
}

// One candidate read from the answer's JSON, or why it is not one the drawing service can fetch.
fn candidate(entry: &Value, words: &ReferenceWords, now: i64) -> Result<Candidate, String> {
  let object = entry.as_object().ok_or_else(|| words.no_url.clone())?;
  let url = object
    .get("url")
    .and_then(Value::as_str)
    .map(str::trim)
    .filter(|url| web_url(url))
    .ok_or_else(|| words.no_url.clone())?
    .to_owned();
  let page_url = match object.get("pageUrl").and_then(Value::as_str).map(str::trim) {
    None | Some("") => None,
    Some(page) if web_url(page) => Some(page.to_owned()),
    Some(_) => return Err(words.no_page.clone()),
  };
  let media_type = object
    .get("mediaType")
    .and_then(Value::as_str)
    .map(str::trim)
    .filter(|media| media.to_ascii_lowercase().starts_with("image/"))
    .ok_or_else(|| words.no_media.clone())?
    .to_owned();
  let width = object
    .get("width")
    .and_then(Value::as_u64)
    .filter(|width| *width > 0)
    .ok_or_else(|| words.no_dimensions.clone())?;
  let height = object
    .get("height")
    .and_then(Value::as_u64)
    .filter(|height| *height > 0)
    .ok_or_else(|| words.no_dimensions.clone())?;
  let retrieved_at = object.get("retrievedAt").and_then(Value::as_i64).filter(|at| *at > 0).unwrap_or(now);
  let licence = licence(object.get("licence"));
  let title = string(object.get("title"));
  Ok(Candidate {
    url,
    page_url,
    title,
    creator: string(object.get("creator")),
    publisher: string(object.get("publisher")),
    retrieved_at,
    licence,
    media_type,
    width,
    height,
    note: string(object.get("note")),
  })
}

// The licence line a candidate's JSON names, as metadata: whatever it names, it decides nothing, and where it
// names none its id and attribution stand empty and its page null.
fn licence(value: Option<&Value>) -> Licence {
  match value {
    Some(Value::String(id)) => Licence {
      id: id.trim().to_owned(),
      ..Licence::default()
    },
    Some(Value::Object(object)) => Licence {
      id: string(object.get("id")),
      url: object
        .get("url")
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|url| web_url(url))
        .map(str::to_owned),
      attribution: string(object.get("attribution")),
    },
    _ => Licence::default(),
  }
}

// A source page read from the answer's JSON, none where it names no address that reads.
fn source(value: &Value) -> Option<Source> {
  let object = value.as_object()?;
  let url = object.get("url").and_then(Value::as_str).map(str::trim).filter(|url| web_url(url))?;
  let what = string(object.get("what"));
  Some(Source {
    url: url.to_owned(),
    what: if what.is_empty() { url.to_owned() } else { what },
  })
}

// A JSON array of strings, the strings it holds alone.
fn strings(value: Option<&Value>) -> Vec<String> {
  value
    .and_then(Value::as_array)
    .map(|items| {
      items
        .iter()
        .filter_map(Value::as_str)
        .map(str::trim)
        .filter(|t| !t.is_empty())
        .map(str::to_owned)
        .collect()
    })
    .unwrap_or_default()
}

fn string(value: Option<&Value>) -> String {
  value.and_then(Value::as_str).map(str::trim).unwrap_or_default().to_owned()
}

// What a candidate is called where the answer does not support it: its title, else its address.
fn label(entry: &Value, words: &ReferenceWords) -> String {
  let object = entry.as_object();
  let title = object.and_then(|object| object.get("title")).and_then(Value::as_str).map(str::trim);
  let url = object.and_then(|object| object.get("url")).and_then(Value::as_str).map(str::trim);
  title
    .filter(|title| !title.is_empty())
    .or(url.filter(|url| !url.is_empty()))
    .unwrap_or(&words.unnamed)
    .to_owned()
}

// Whether a text is an http or https address.
fn web_url(text: &str) -> bool {
  Url::parse(text).is_ok_and(|url| matches!(url.scheme(), "http" | "https"))
}

/// The dossier as its document is read: the sections the design gives, the candidate table, and the JSON
/// the drawing service parses as the body's last fenced block, in the language of the person who asked for
/// the draw.
pub fn render(dossier: &Dossier, words: &ReferenceWords) -> String {
  let mut out = String::new();
  heading(&mut out, &words.what);
  out.push_str(&dossier.description);
  out.push_str("\n\n");
  heading(&mut out, &words.appearance);
  out.push_str(&bullets(&dossier.features, words));
  heading(&mut out, &words.aliases);
  out.push_str(&bullets(&dossier.subject.aliases, words));
  heading(&mut out, &words.constraints);
  out.push_str(&bullets(&dossier.constraints, words));
  heading(&mut out, &words.candidates);
  out.push_str(&table(&dossier.candidates, words));
  out.push_str("\n\n");
  heading(&mut out, &words.sources);
  let sources: Vec<String> = dossier
    .sources
    .iter()
    .map(|source| format!("- {}: {}", cell(&source.what), source.url))
    .collect();
  out.push_str(&if sources.is_empty() { words.none.clone() } else { sources.join("\n") });
  out.push_str("\n\n");
  heading(&mut out, &words.unresolved);
  let unresolved: Vec<String> = dossier
    .unresolved
    .iter()
    .enumerate()
    .map(|(at, line)| format!("{}. {}", at + 1, line))
    .collect();
  out.push_str(&if unresolved.is_empty() { words.none.clone() } else { unresolved.join("\n") });
  out.push_str("\n\n");
  heading(&mut out, &words.json);
  out.push_str("```json\n");
  out.push_str(&serde_json::to_string_pretty(dossier).expect("a dossier always renders"));
  out.push_str("\n```\n");
  out
}

fn heading(out: &mut String, name: &str) {
  out.push_str(&format!("## {name}\n\n"));
}

fn bullets(items: &[String], words: &ReferenceWords) -> String {
  let lines: Vec<String> = items.iter().map(|item| format!("- {}", cell(item))).collect();
  let mut out = if lines.is_empty() { words.none.clone() } else { lines.join("\n") };
  out.push_str("\n\n");
  out
}

fn table(candidates: &[Candidate], words: &ReferenceWords) -> String {
  if candidates.is_empty() {
    return words.none.clone();
  }
  let mut out = format!(
    "| {} | {} | {} | {} | {} | {} | {} | {} |\n| --- | --- | --- | --- | --- | --- | --- | --- |\n",
    words.title_column,
    words.page_column,
    words.address_column,
    words.creator_column,
    words.publisher_column,
    words.dimensions_column,
    words.media_column,
    words.licence_column,
  );
  for candidate in candidates {
    let Licence {
      id, attribution, ..
    } = &candidate.licence;
    let licence = match (id.is_empty(), attribution.is_empty()) {
      (true, true) => words.none.clone(),
      (true, false) => attribution.clone(),
      (false, true) => id.clone(),
      (false, false) => format!("{id} ({attribution})"),
    };
    out.push_str(&format!(
      "| {} | {} | {} | {} | {} | {}×{} | {} | {} |\n",
      cell(&candidate.title),
      cell(candidate.page_url.as_deref().unwrap_or(&words.none)),
      cell(&candidate.url),
      cell(&candidate.creator),
      cell(&candidate.publisher),
      candidate.width,
      candidate.height,
      cell(&candidate.media_type),
      cell(&licence),
    ));
  }
  out.trim_end().to_owned()
}

// A markdown table cell: its pipes escaped and its lines joined.
fn cell(text: &str) -> String {
  text.replace('|', "\\|").replace('\n', " ")
}

/// A dossier filed in the artifact service for the drawing service: kind `search`, the labels the design
/// names and the research labels, the body the design's sections, and its last fenced block the candidate
/// JSON. No room is told, no picture is attached, and the share link is opened but not required.
pub async fn file(
  app: &App,
  run: i64,
  ask: &ResearchRunAsk,
  reference: &ReferenceAsk,
  text: &str,
) -> Result<RunDocument, String> {
  let dossier = dossier(text, reference, now_ms())?;
  let words = words::document().reference(&reference.language);
  let body = render(&dossier, words);
  let title = words::fill(&words.title, &[("name", &dossier.subject.name)]);
  let description = dossier.description.clone();
  let filing = json!({
    "kind": "search",
    "title": title,
    "description": description,
    "body": body,
    "actorName": document::ACTOR,
    "labels": {
      "research": run.to_string(),
      "key": KEY,
      "query": ask.query.trim(),
      "category": KEY,
      "languages": reference.language,
      "paint.subject-key": reference.subject_key,
      "paint.schema": SCHEMA,
    },
  });
  let filed = app.peers.artifact_send(reqwest::Method::POST, "/documents", &filing).await?;
  let id = filed["id"].as_i64().ok_or("the artifact service named no document for the dossier")?;
  let link = app.peers.share(id).await.unwrap_or_default();
  Ok(RunDocument {
    id,
    title,
    description: Some(description),
    link,
  })
}
