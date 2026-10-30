//! The source kinds and their readers (`docs/sources.md`): how each kind is asked, how its answer becomes
//! candidates, and the filter a candidate passes.

use bmo_research_client::{Filter, Shape, Source};
use serde::Deserialize;

use crate::App;
use crate::tool_host::ToolHost;

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
  Page,
}

impl SourceKind {
  pub const ALL: [SourceKind; 6] = [
    SourceKind::HuggingfaceTrending,
    SourceKind::HuggingfaceSearch,
    SourceKind::Arxiv,
    SourceKind::GithubReleases,
    SourceKind::Rss,
    SourceKind::Page,
  ];

  pub fn as_str(self) -> &'static str {
    match self {
      SourceKind::HuggingfaceTrending => "huggingface-trending",
      SourceKind::HuggingfaceSearch => "huggingface-search",
      SourceKind::Arxiv => "arxiv",
      SourceKind::GithubReleases => "github-releases",
      SourceKind::Rss => "rss",
      SourceKind::Page => "page",
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
      SourceKind::Rss | SourceKind::Page => None,
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
      SourceKind::Rss | SourceKind::Page => ItemKind::Entry,
    }
  }

  /// The request a reading of the source makes with the daemon's own client; none for a `page`, whose
  /// listing is read through explorer.
  pub fn request(self, http: &reqwest::Client, source: &Source) -> Option<reqwest::RequestBuilder> {
    let address = source.address.trim_end_matches('/');
    let query = source.query.clone().unwrap_or_default();
    let limit = source.limit.to_string();
    Some(match self {
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
      SourceKind::Page => return None,
    })
  }

  /// The candidates an answer's body holds.
  pub fn read(self, body: &[u8], source: &Source) -> Result<Vec<Candidate>, String> {
    match self {
      SourceKind::HuggingfaceTrending | SourceKind::HuggingfaceSearch => read_huggingface(body),
      SourceKind::Arxiv => read_arxiv(body),
      SourceKind::GithubReleases => read_github_releases(body, source.query.as_deref().unwrap_or_default()),
      SourceKind::Rss => read_feed(body),
      SourceKind::Page => {
        let pattern = LinkPattern::parse(source.pattern.as_deref().unwrap_or_default())?;
        read_listing(&String::from_utf8_lossy(body), &source.address, &pattern, source.limit)
      }
    }
  }
}

/// Which links of a `page` source's listing are posts: a path prefix on the listing's own host, or a regular
/// expression searched in the whole address.
#[derive(Debug, Clone)]
pub enum LinkPattern {
  Prefix(String),
  Expression(regex::Regex),
}

impl LinkPattern {
  /// A pattern beginning with `/` is a path prefix; any other is a regular expression.
  pub fn parse(pattern: &str) -> Result<LinkPattern, String> {
    let pattern = pattern.trim();
    if pattern.is_empty() {
      return Err("a page source needs a pattern".into());
    }
    if pattern.starts_with('/') {
      return Ok(LinkPattern::Prefix(pattern.to_string()));
    }
    regex::Regex::new(pattern).map(LinkPattern::Expression).map_err(|e| {
      format!("the pattern {pattern} is no path prefix and does not read as a regular expression: {e}")
    })
  }

  pub fn as_str(&self) -> &str {
    match self {
      LinkPattern::Prefix(prefix) => prefix,
      LinkPattern::Expression(expression) => expression.as_str(),
    }
  }

  /// Whether a link of the listing at `listing` is a post: on its host, under the prefix and past it; or
  /// where the expression is found in it.
  pub fn matches(&self, link: &url::Url, listing: &url::Url) -> bool {
    match self {
      LinkPattern::Prefix(prefix) => {
        link.host_str() == listing.host_str()
          && link.path().len() > prefix.len()
          && link.path().starts_with(prefix)
      }
      LinkPattern::Expression(expression) => expression.is_match(link.as_str()),
    }
  }
}

/// How explorer reads a `page` source's listing.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Strategy {
  /// The direct client, and the real Chrome only after a block or an empty shell.
  Direct,
  /// The real Chrome from the start, for a listing whose links a script draws.
  Browser,
}

impl Strategy {
  pub fn as_str(self) -> &'static str {
    match self {
      Strategy::Direct => "direct",
      Strategy::Browser => "browser",
    }
  }

  pub fn parse(word: &str) -> Option<Strategy> {
    [Strategy::Direct, Strategy::Browser].into_iter().find(|s| s.as_str() == word)
  }

  /// The strategy as explorer's session record names it.
  pub fn explorer(self) -> &'static str {
    match self {
      Strategy::Direct => "direct-then-browser",
      Strategy::Browser => "browser-first",
    }
  }
}

/// The most characters of a link's text a candidate's name keeps.
const NAME_CHARS: usize = 200;

/// A listing's markup, read at the address `at`: the first `limit` posts its links name, in the page's order,
/// each an entry candidate made by the page's title. A listing with no link the pattern matches is a failure.
pub fn read_listing(
  markup: &str,
  at: &str,
  pattern: &LinkPattern,
  limit: i64,
) -> Result<Vec<Candidate>, String> {
  let base = url::Url::parse(at.trim()).map_err(|e| format!("{at} is no address: {e}"))?;
  let document = scraper::Html::parse_document(markup);
  let anchors = scraper::Selector::parse("a[href]").expect("the selector reads");
  let title = scraper::Selector::parse("title").expect("the selector reads");
  let maker = document
    .select(&title)
    .next()
    .map(|t| folded(&t.text().collect::<Vec<_>>().join(" ")))
    .filter(|t| !t.is_empty())
    .or_else(|| base.host_str().map(str::to_string));
  let listing = address_key(base.as_str());
  let mut posts: Vec<(url::Url, Vec<String>)> = Vec::new();
  for anchor in document.select(&anchors) {
    let Ok(mut link) = base.join(anchor.value().attr("href").unwrap_or_default().trim()) else {
      continue;
    };
    if !matches!(link.scheme(), "http" | "https") {
      continue;
    }
    link.set_fragment(None);
    if address_key(link.as_str()) == listing || !pattern.matches(&link, &base) {
      continue;
    }
    let mut text = folded(&anchor.text().collect::<Vec<_>>().join(" "));
    if text.is_empty() {
      text = ["aria-label", "title"]
        .into_iter()
        .find_map(|name| anchor.value().attr(name).map(folded).filter(|t| !t.is_empty()))
        .unwrap_or_default();
    }
    let key = address_key(link.as_str());
    if let Some((_, texts)) = posts.iter_mut().find(|(known, _)| address_key(known.as_str()) == key) {
      texts.push(text);
    } else if (posts.len() as i64) < limit.max(1) {
      posts.push((link, vec![text]));
    }
  }
  if posts.is_empty() {
    return Err(format!("{at}: no link on the page matches the pattern {}", pattern.as_str()));
  }
  Ok(
    posts
      .into_iter()
      .map(|(link, texts)| {
        let address = link.to_string();
        let name = texts
          .iter()
          .find(|t| !t.is_empty())
          .map(|t| t.chars().take(NAME_CHARS).collect::<String>().trim_end().to_string())
          .unwrap_or_else(|| address.clone());
        Candidate {
          words: format!("{} {address}", texts.join(" ")),
          name,
          maker: maker.clone(),
          address: Some(address),
          published_at: None,
          kind: ItemKind::Entry,
        }
      })
      .collect(),
  )
}

/// A reading of one source: its candidates, every one the filter lets through.
pub async fn fetch(app: &App, source: &Source) -> Result<Vec<Candidate>, String> {
  let kind = SourceKind::parse(&source.kind).ok_or_else(|| format!("{} is no source kind", source.kind))?;
  let candidates = match kind.request(&app.http, source) {
    Some(request) => kind.read(&ask(request, source).await?, source)?,
    None => read_page(app, source).await?,
  };
  Ok(candidates.into_iter().filter(|c| c.passes(&source.filter)).collect())
}

/// A `page` source's listing read through explorer, its links read against the address the reading ended at.
async fn read_page(app: &App, source: &Source) -> Result<Vec<Candidate>, String> {
  let pattern = LinkPattern::parse(source.pattern.as_deref().unwrap_or_default())?;
  let strategy = source.strategy.as_deref().and_then(Strategy::parse).unwrap_or(Strategy::Direct);
  let (at, markup) = ToolHost::new(&app.settings)
    .read_page(&source.address, strategy.explorer())
    .await
    .map_err(|why| format!("explorer could not read {}: {why}", source.address))?;
  read_listing(&markup, &at, &pattern, source.limit)
}

/// One request of the daemon's own client answered with a success: its body.
async fn ask(request: reqwest::RequestBuilder, source: &Source) -> Result<Vec<u8>, String> {
  let answer = request
    .timeout(std::time::Duration::from_secs(60))
    .send()
    .await
    .map_err(|e| format!("{}: {e}", source.address))?;
  let status = answer.status();
  if !status.is_success() {
    return Err(format!("{} answered {}", source.address, status.as_u16()));
  }
  let body = answer.bytes().await.map_err(|e| format!("{}: {e}", source.address))?;
  Ok(body.to_vec())
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
