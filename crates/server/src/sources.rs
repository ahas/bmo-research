//! The source kinds and their readers (`docs/sources.md`): how each kind is asked, how its answer becomes
//! candidates, and the filter a candidate passes.

use bmo_research_client::{Filter, Shape, Source};
use serde::Deserialize;

/// What a source's items are, which says the shape a `manual-or-investigation` category asks them in.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ItemKind {
  Model,
  Paper,
  Release,
  Entry,
}

impl ItemKind {
  pub fn as_str(self) -> &'static str {
    match self {
      ItemKind::Model => "model",
      ItemKind::Paper => "paper",
      ItemKind::Release => "release",
      ItemKind::Entry => "entry",
    }
  }

  pub fn parse(word: &str) -> Option<ItemKind> {
    [ItemKind::Model, ItemKind::Paper, ItemKind::Release, ItemKind::Entry]
      .into_iter()
      .find(|kind| kind.as_str() == word)
  }

  /// A made thing is asked for its manual; a claim or an event for an investigation.
  pub fn shape(self) -> Shape {
    match self {
      ItemKind::Model => Shape::Manual,
      ItemKind::Paper | ItemKind::Release | ItemKind::Entry => Shape::Investigation,
    }
  }
}

/// One item a source turned up.
#[derive(Debug, Clone, PartialEq)]
pub struct Candidate {
  pub name: String,
  pub maker: Option<String>,
  pub address: Option<String>,
  /// epoch milliseconds
  pub published_at: Option<i64>,
  pub kind: ItemKind,
  /// The text the filter reads.
  pub words: String,
}

impl Candidate {
  /// Its address with the scheme and a trailing slash dropped, lower-cased; else its name lower-cased.
  pub fn key(&self) -> String {
    match &self.address {
      Some(address) if !address.trim().is_empty() => address_key(address),
      _ => self.name.trim().to_lowercase(),
    }
  }

  /// Whether the filter lets the candidate through: every required word and no excluded one, without
  /// regard to case.
  pub fn passes(&self, filter: &Filter) -> bool {
    let words = self.words.to_lowercase();
    let holds = |word: &String| words.contains(&word.trim().to_lowercase());
    filter.require.iter().filter(|w| !w.trim().is_empty()).all(holds)
      && !filter.exclude.iter().filter(|w| !w.trim().is_empty()).any(holds)
  }
}

/// An address as a key: its scheme and a trailing slash dropped, lower-cased.
pub fn address_key(address: &str) -> String {
  let trimmed = address.trim();
  let bare = trimmed.split_once("://").map(|(_, rest)| rest).unwrap_or(trimmed);
  bare.trim_end_matches('/').to_lowercase()
}

/// The kinds a source may be.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SourceKind {
  HuggingfaceTrending,
  HuggingfaceSearch,
  Arxiv,
  GithubReleases,
  Rss,
}

impl SourceKind {
  pub const ALL: [SourceKind; 5] = [
    SourceKind::HuggingfaceTrending,
    SourceKind::HuggingfaceSearch,
    SourceKind::Arxiv,
    SourceKind::GithubReleases,
    SourceKind::Rss,
  ];

  pub fn as_str(self) -> &'static str {
    match self {
      SourceKind::HuggingfaceTrending => "huggingface-trending",
      SourceKind::HuggingfaceSearch => "huggingface-search",
      SourceKind::Arxiv => "arxiv",
      SourceKind::GithubReleases => "github-releases",
      SourceKind::Rss => "rss",
    }
  }

  pub fn parse(word: &str) -> Option<SourceKind> {
    SourceKind::ALL.into_iter().find(|kind| kind.as_str() == word)
  }

  /// The address a source of this kind is read at where it names none.
  pub fn default_address(self) -> Option<&'static str> {
    match self {
      SourceKind::HuggingfaceTrending | SourceKind::HuggingfaceSearch => {
        Some("https://huggingface.co/api/models")
      }
      SourceKind::Arxiv => Some("https://export.arxiv.org/api/query"),
      SourceKind::GithubReleases => Some("https://api.github.com"),
      SourceKind::Rss => None,
    }
  }

  pub fn takes_query(self) -> bool {
    matches!(self, SourceKind::HuggingfaceSearch | SourceKind::Arxiv | SourceKind::GithubReleases)
  }

  pub fn item_kind(self) -> ItemKind {
    match self {
      SourceKind::HuggingfaceTrending | SourceKind::HuggingfaceSearch => ItemKind::Model,
      SourceKind::Arxiv => ItemKind::Paper,
      SourceKind::GithubReleases => ItemKind::Release,
      SourceKind::Rss => ItemKind::Entry,
    }
  }

  /// The request a reading of the source makes.
  pub fn request(self, http: &reqwest::Client, source: &Source) -> reqwest::RequestBuilder {
    let address = source.address.trim_end_matches('/');
    let query = source.query.clone().unwrap_or_default();
    let limit = source.limit.to_string();
    match self {
      SourceKind::HuggingfaceTrending => {
        http.get(address).query(&[("sort", "trendingScore"), ("limit", limit.as_str())])
      }
      SourceKind::HuggingfaceSearch => http.get(address).query(&[
        ("search", query.as_str()),
        ("sort", "lastModified"),
        ("direction", "-1"),
        ("limit", limit.as_str()),
      ]),
      SourceKind::Arxiv => http.get(address).query(&[
        ("search_query", query.as_str()),
        ("sortBy", "submittedDate"),
        ("sortOrder", "descending"),
        ("max_results", limit.as_str()),
      ]),
      SourceKind::GithubReleases => http
        .get(format!("{address}/repos/{}/releases", query.trim_matches('/')))
        .header("accept", "application/vnd.github+json")
        .query(&[("per_page", limit.as_str())]),
      SourceKind::Rss => http.get(address),
    }
  }

  /// The candidates an answer's body holds.
  pub fn read(self, body: &[u8], source: &Source) -> Result<Vec<Candidate>, String> {
    match self {
      SourceKind::HuggingfaceTrending | SourceKind::HuggingfaceSearch => read_huggingface(body),
      SourceKind::Arxiv => read_arxiv(body),
      SourceKind::GithubReleases => read_github_releases(body, source.query.as_deref().unwrap_or_default()),
      SourceKind::Rss => read_feed(body),
    }
  }
}

/// A reading of one source: its candidates, every one the filter lets through.
pub async fn fetch(http: &reqwest::Client, source: &Source) -> Result<Vec<Candidate>, String> {
  let kind = SourceKind::parse(&source.kind).ok_or_else(|| format!("{} is no source kind", source.kind))?;
  let answer = kind
    .request(http, source)
    .timeout(std::time::Duration::from_secs(60))
    .send()
    .await
    .map_err(|e| format!("{}: {e}", source.address))?;
  let status = answer.status();
  if !status.is_success() {
    return Err(format!("{} answered {}", source.address, status.as_u16()));
  }
  let body = answer.bytes().await.map_err(|e| format!("{}: {e}", source.address))?;
  let candidates = kind.read(&body, source)?;
  Ok(candidates.into_iter().filter(|c| c.passes(&source.filter)).collect())
}

fn epoch_ms(text: &str) -> Option<i64> {
  chrono::DateTime::parse_from_rfc3339(text.trim()).ok().map(|at| at.timestamp_millis())
}

fn folded(text: &str) -> String {
  text.split_whitespace().collect::<Vec<_>>().join(" ")
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct HubModel {
  id: String,
  #[serde(default)]
  author: Option<String>,
  #[serde(default)]
  tags: Vec<String>,
  #[serde(default)]
  pipeline_tag: Option<String>,
  #[serde(default)]
  created_at: Option<String>,
  #[serde(default)]
  last_modified: Option<String>,
}

/// The Hub's model list: each repository a model candidate.
pub fn read_huggingface(body: &[u8]) -> Result<Vec<Candidate>, String> {
  let models: Vec<HubModel> =
    serde_json::from_slice(body).map_err(|e| format!("the Hub's answer did not read: {e}"))?;
  Ok(
    models
      .into_iter()
      .map(|m| {
        let maker = m.author.clone().or_else(|| m.id.split_once('/').map(|(owner, _)| owner.to_string()));
        let mut words = vec![m.id.clone()];
        words.extend(m.tags.iter().cloned());
        words.extend(m.pipeline_tag.iter().cloned());
        Candidate {
          address: Some(format!("https://huggingface.co/{}", m.id)),
          published_at: m.created_at.as_deref().or(m.last_modified.as_deref()).and_then(epoch_ms),
          name: m.id,
          maker,
          kind: ItemKind::Model,
          words: words.join(" "),
        }
      })
      .collect(),
  )
}

/// An arXiv `abs` address without its version suffix, so every version of a paper is one key.
pub fn arxiv_address(address: &str) -> String {
  let trimmed = address.trim().trim_end_matches('/');
  let (head, last) = trimmed.rsplit_once('/').unwrap_or(("", trimmed));
  let bare = match last.rfind('v') {
    Some(at) if at > 0 && last[at + 1..].chars().all(|c| c.is_ascii_digit()) && at + 1 < last.len() => {
      &last[..at]
    }
    _ => last,
  };
  let head = head.replace("http://", "https://");
  if head.is_empty() { bare.to_string() } else { format!("{head}/{bare}") }
}

/// The arXiv API's Atom feed: each entry a paper candidate.
pub fn read_arxiv(body: &[u8]) -> Result<Vec<Candidate>, String> {
  let feed = feed_rs::parser::parse(body).map_err(|e| format!("the arXiv answer did not read: {e}"))?;
  Ok(
    feed
      .entries
      .into_iter()
      .map(|entry| {
        let page = entry
          .links
          .iter()
          .find(|l| l.rel.as_deref() == Some("alternate"))
          .map(|l| l.href.clone())
          .unwrap_or_else(|| entry.id.clone());
        let title = folded(&entry.title.map(|t| t.content).unwrap_or_default());
        let summary = entry.summary.map(|t| t.content).unwrap_or_default();
        let categories: Vec<String> = entry.categories.into_iter().map(|c| c.term).collect();
        Candidate {
          words: format!("{title} {summary} {}", categories.join(" ")),
          name: title,
          maker: entry.authors.into_iter().find_map(|a| a.name.filter(|n| !n.is_empty())),
          address: Some(arxiv_address(&page)),
          published_at: entry.published.or(entry.updated).map(|at| at.timestamp_millis()),
          kind: ItemKind::Paper,
        }
      })
      .collect(),
  )
}

#[derive(Deserialize)]
struct Release {
  tag_name: String,
  #[serde(default)]
  name: Option<String>,
  #[serde(default)]
  html_url: Option<String>,
  #[serde(default)]
  published_at: Option<String>,
  #[serde(default)]
  body: Option<String>,
  #[serde(default)]
  draft: bool,
}

/// A repository's releases: each one that is no draft a release candidate.
pub fn read_github_releases(body: &[u8], repository: &str) -> Result<Vec<Candidate>, String> {
  let releases: Vec<Release> =
    serde_json::from_slice(body).map_err(|e| format!("the releases answer did not read: {e}"))?;
  let repository = repository.trim_matches('/');
  let (owner, project) = repository.split_once('/').unwrap_or(("", repository));
  Ok(
    releases
      .into_iter()
      .filter(|r| !r.draft)
      .map(|r| {
        let title = r.name.clone().filter(|n| !n.trim().is_empty()).unwrap_or_else(|| r.tag_name.clone());
        Candidate {
          name: format!("{project} {}", r.tag_name),
          maker: Some(owner.to_string()).filter(|o| !o.is_empty()),
          address: r.html_url.clone(),
          published_at: r.published_at.as_deref().and_then(epoch_ms),
          kind: ItemKind::Release,
          words: format!("{project} {title} {} {}", r.tag_name, r.body.unwrap_or_default()),
        }
      })
      .collect(),
  )
}

/// An RSS or an Atom feed: each entry an entry candidate, its maker the feed's title.
pub fn read_feed(body: &[u8]) -> Result<Vec<Candidate>, String> {
  let feed = feed_rs::parser::parse(body).map_err(|e| format!("the feed did not read: {e}"))?;
  let maker = feed.title.map(|t| folded(&t.content)).filter(|t| !t.is_empty());
  Ok(
    feed
      .entries
      .into_iter()
      .map(|entry| {
        let title = folded(&entry.title.map(|t| t.content).unwrap_or_default());
        let summary = entry.summary.map(|t| t.content).unwrap_or_default();
        Candidate {
          words: format!("{title} {summary}"),
          name: title,
          maker: maker.clone(),
          address: entry.links.first().map(|l| l.href.clone()),
          published_at: entry.published.or(entry.updated).map(|at| at.timestamp_millis()),
          kind: ItemKind::Entry,
        }
      })
      .filter(|c| !c.name.is_empty())
      .collect(),
  )
}
