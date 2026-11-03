//! The scan against stand-in sources and a stand-in archive (`docs/sources.md`): each kind asked as it
//! states, the filter and the archive applied, every candidate kept queued at once with no cap, a source
//! read no more often than its cadence, a failing source counted, what each reading leaves on its source,
//! and a listing page read through a stand-in explorer.

use crate::support;

use bmo_research_client::{Filter, ItemState, NewSource, Running, Shape};
use research_server::scan::{read_source, scan};

fn source(category: &str, kind: &str, address: String, query: Option<&str>) -> NewSource {
  NewSource {
    category_name: category.into(),
    kind: kind.into(),
    address: Some(address),
    query: query.map(String::from),
    ..NewSource::default()
  }
}

#[tokio::test]
async fn a_scan_reads_every_kind_and_queues_what_is_new() {
  let ground = support::ground(|_| {}).await;
  ground.world.lock().unwrap().archived.push("A Survey of Things".into());
  let store = &ground.app.store;
  let mut hub = source("it", "huggingface-trending", format!("{}/api/models", ground.sources), None);
  hub.filter = Some(Filter {
    require: vec![],
    exclude: vec!["roleplay".into()],
  });
  hub.limit = Some(10);
  store.add_source(&hub, hub.address.as_deref().unwrap(), 10).unwrap();
  let arxiv = source("it", "arxiv", format!("{}/api/query", ground.sources), Some("all:quantization"));
  store.add_source(&arxiv, arxiv.address.as_deref().unwrap(), 30).unwrap();
  let releases =
    source("it", "github-releases", ground.sources.clone(), Some("example-org/example-engine"));
  store.add_source(&releases, &ground.sources, 30).unwrap();
  let mut feed = source("security", "rss", format!("{}/feed.xml", ground.sources), None);
  feed.filter = Some(Filter {
    require: vec!["libexample".into()],
    exclude: vec![],
  });
  store.add_source(&feed, feed.address.as_deref().unwrap(), 30).unwrap();

  scan(&ground.app).await.unwrap();

  let asked = ground.asked("source");
  let hub_asked = asked.iter().find(|a| a.path.starts_with("/api/models")).expect("the Hub is read");
  assert!(
    hub_asked.path.contains("sort=trendingScore") && hub_asked.path.contains("limit=10"),
    "{}",
    hub_asked.path
  );
  let arxiv_asked = asked.iter().find(|a| a.path.starts_with("/api/query")).expect("arXiv is read");
  for part in [
    "search_query=all%3Aquantization",
    "sortBy=submittedDate",
    "sortOrder=descending",
    "max_results=30",
  ] {
    assert!(arxiv_asked.path.contains(part), "{}", arxiv_asked.path);
  }
  assert!(
    asked
      .iter()
      .any(|a| a.path.starts_with("/repos/example-org/example-engine/releases?per_page=30"))
  );
  assert!(asked.iter().any(|a| a.path == "/feed.xml"));

  let archive: Vec<String> = ground
    .asked_path("artifact", "GET", "/documents")
    .iter()
    .map(|a| a.path.clone())
    .collect();
  assert!(archive.iter().all(|p| p.contains("limit=1") && p.contains("q=")), "{archive:?}");

  let it = store.queue(Some("it"), &[ItemState::Waiting], None, 50).unwrap().items;
  let mut names: Vec<&str> = it.iter().map(|i| i.name.as_str()).collect();
  names.sort();
  assert_eq!(
    names,
    [
      "Two-Bit Weights Without Loss",
      "example-engine v2.1.0",
      "example-lab/Example-Model-7B",
      "other-lab/Tiny-Vision-2B"
    ]
  );
  let shapes: Vec<(String, Shape)> = it.iter().map(|i| (i.kind.clone(), i.shape)).collect();
  assert!(
    shapes.iter().all(|(kind, shape)| (kind == "model") == (*shape == Shape::Manual)),
    "{shapes:?}"
  );
  assert_eq!(store.seen_state("arxiv.org/abs/2610.00002").unwrap().as_deref(), Some("archived"));
  assert!(
    !store.is_seen("huggingface.co/someone/example-model-7b-chat-roleplay").unwrap(),
    "a filtered item stays unseen"
  );

  let security = store.queue(Some("security"), &[], None, 50).unwrap().items;
  assert_eq!(security.len(), 1);
  assert_eq!(security[0].shape, Shape::Advisory);
  assert_eq!(security[0].maker.as_deref(), Some("Example Security List"));

  // Read again at once, no source is due: nothing is asked.
  let before = ground.asked("source").len();
  scan(&ground.app).await.unwrap();
  assert_eq!(ground.asked("source").len(), before);
}

#[tokio::test]
async fn a_scan_queues_every_candidate_it_keeps_with_no_cap() {
  let ground = support::ground(|_| {}).await;
  let store = &ground.app.store;
  let mut hub = source("it", "huggingface-trending", format!("{}/api/models", ground.sources), None);
  hub.limit = Some(10);
  let id = store.add_source(&hub, hub.address.as_deref().unwrap(), 10).unwrap().id;
  scan(&ground.app).await.unwrap();
  let items = store.queue(Some("it"), &[], None, 50).unwrap().items;
  let kept = store.source(id).unwrap().last_taken.unwrap();
  assert!(kept > 2, "the reading kept more than a handful: {kept}");
  assert_eq!(items.len() as i64, kept, "every candidate kept is queued");
  assert!(items.iter().all(|item| item.state == ItemState::Waiting));
  assert_eq!(
    store.seen_state("huggingface.co/other-lab/tiny-vision-2b").unwrap().as_deref(),
    Some("queued")
  );
}

#[tokio::test]
async fn a_failing_source_is_counted_and_a_stopped_category_is_not_read() {
  let ground = support::ground(|_| {}).await;
  let store = &ground.app.store;
  let broken = source("security", "rss", format!("{}/missing.xml", ground.sources), None);
  let id = store.add_source(&broken, broken.address.as_deref().unwrap(), 30).unwrap().id;
  let feed = source("economy", "rss", format!("{}/feed.xml", ground.sources), None);
  store.add_source(&feed, feed.address.as_deref().unwrap(), 30).unwrap();
  store.set_running(Some("economy"), Running::Stopped).unwrap();

  scan(&ground.app).await.unwrap();

  let failed = store.source(id).unwrap();
  assert_eq!(failed.failure_count, 1);
  assert!(failed.last_error.unwrap().contains("404"));
  assert!(
    ground.asked("source").iter().all(|a| a.path != "/feed.xml"),
    "a stopped category's source is read"
  );
  assert!(store.queue(Some("economy"), &[], None, 50).unwrap().items.is_empty());
}

#[tokio::test]
async fn a_reading_writes_what_it_turned_up_and_a_failed_one_keeps_the_earlier_yield() {
  let ground = support::ground(|_| {}).await;
  ground.world.lock().unwrap().archived.push("A Survey of Things".into());
  let store = &ground.app.store;
  let arxiv = source("it", "arxiv", format!("{}/api/query", ground.sources), Some("all:quantization"));
  let id = store.add_source(&arxiv, arxiv.address.as_deref().unwrap(), 30).unwrap().id;
  let unread = store.source(id).unwrap();
  assert_eq!(
    (unread.read_at, unread.scan_count, unread.last_yield, unread.last_taken),
    (None, 0, None, None)
  );

  scan(&ground.app).await.unwrap();

  // Two entries pass the filter; the archive holds one, so one is kept as new.
  let first = store.source(id).unwrap();
  assert!(first.read_at.is_some());
  assert_eq!(
    (first.scan_count, first.last_yield, first.last_taken, first.last_error.as_deref()),
    (1, Some(2), Some(1), None)
  );

  // Read again, both are seen: the yield stands and nothing is taken.
  let category = store.category("it").unwrap();
  read_source(&ground.app, &category, &first).await;
  let again = store.source(id).unwrap();
  assert_eq!((again.scan_count, again.last_yield, again.last_taken), (2, Some(2), Some(0)));

  let mut broken = again.clone();
  broken.address = format!("{}/missing", ground.sources);
  read_source(&ground.app, &category, &broken).await;
  let failed = store.source(id).unwrap();
  assert_eq!((failed.scan_count, failed.failure_count), (3, 1));
  assert!(failed.last_error.as_deref().is_some_and(|why| why.contains("404")), "{failed:?}");
  assert!(failed.read_at >= again.read_at);
  assert_eq!((failed.last_yield, failed.last_taken), (Some(2), Some(0)), "the earlier yield is kept");
}

/// A `page` source of the listing at `address`, matching `pattern`.
fn listing(address: &str, pattern: &str, strategy: Option<&str>) -> NewSource {
  NewSource {
    category_name: "it".into(),
    kind: "page".into(),
    address: Some(address.into()),
    pattern: Some(pattern.into()),
    strategy: strategy.map(String::from),
    ..NewSource::default()
  }
}

#[tokio::test]
async fn a_listing_is_read_through_explorer_slice_by_slice_and_its_posts_queued() {
  let ground = support::ground(|_| {}).await;
  let address = "https://example-lab.org/news";
  {
    let mut world = ground.world.lock().unwrap();
    world.pages.insert(address.into(), support::fixture("listing.html"));
    world.slice_chars = Some(300);
    world.archived.push("Faster serving for long contexts".into());
  }
  let store = &ground.app.store;
  let mut page = listing(address, "/news/", Some("browser"));
  page.filter = Some(Filter {
    require: vec![],
    exclude: vec!["roleplay".into()],
  });
  let id = store.add_source(&page, address, 30).unwrap().id;

  scan(&ground.app).await.unwrap();

  let sessions = ground.asked_path("tool-host", "POST", "/tool-sessions");
  assert_eq!(sessions.len(), 1, "one record holds the whole reading");
  let record = &sessions[0].body;
  assert_eq!(record["namespaces"], serde_json::json!(["web"]));
  assert_eq!(record["capabilities"], serde_json::json!(["web.read"]));
  assert_eq!(record["labels"]["research"]["strategy"], "browser-first");
  let calls = ground.asked_path("tool-host", "POST", "/tool-calls");
  let total = support::fixture("listing.html").chars().count();
  assert_eq!(calls.len(), total.div_ceil(300), "the page is read on to its end");
  assert!(calls.iter().all(|c| c.body["name"] == "web_fetch" && c.body["input"]["raw"] == true));
  let offsets: Vec<u64> = calls.iter().map(|c| c.body["input"]["offset"].as_u64().unwrap()).collect();
  assert_eq!(offsets, (0..calls.len() as u64).map(|n| n * 300).collect::<Vec<_>>());
  assert_eq!(
    ground.asked_path("tool-host", "DELETE", "/tool-sessions/").len(),
    1,
    "the record is taken back"
  );
  assert!(ground.asked("source").is_empty(), "the daemon's own client never reads the page");

  let mut names: Vec<String> = store
    .queue(Some("it"), &[], None, 50)
    .unwrap()
    .items
    .into_iter()
    .map(|i| i.name)
    .collect();
  names.sort();
  assert_eq!(names, ["A link relative to the listing", "Introducing Example-Model 8"]);
  let read = store.source(id).unwrap();
  assert_eq!(
    (read.scan_count, read.last_yield, read.last_taken, read.last_error.as_deref()),
    (1, Some(3), Some(2), None)
  );
  assert_eq!(read.strategy.as_deref(), Some("browser"));
}

#[tokio::test]
async fn a_listing_that_fails_or_matches_nothing_says_which() {
  let ground = support::ground(|_| {}).await;
  let address = "https://example-lab.org/news";
  ground
    .world
    .lock()
    .unwrap()
    .pages
    .insert(address.into(), support::fixture("listing.html"));
  let store = &ground.app.store;
  let missing = listing("https://example-lab.org/gone", "/news/", None);
  let gone = store.add_source(&missing, "https://example-lab.org/gone", 30).unwrap().id;
  let unmatched = listing(address, "/blog/", None);
  let nothing = store.add_source(&unmatched, address, 30).unwrap().id;

  scan(&ground.app).await.unwrap();

  let failed = store.source(gone).unwrap();
  let why = failed.last_error.unwrap_or_default();
  assert!(
    why.contains("explorer could not read https://example-lab.org/gone") && why.contains("404"),
    "{why}"
  );
  assert_eq!((failed.failure_count, failed.scan_count, failed.last_yield), (1, 1, None));
  let empty = store.source(nothing).unwrap();
  let why = empty.last_error.unwrap_or_default();
  assert!(why.contains("no link on the page matches the pattern /blog/"), "{why}");
  assert_eq!(empty.failure_count, 1);
  let records = ground.asked_path("tool-host", "POST", "/tool-sessions");
  assert!(
    records
      .iter()
      .all(|r| r.body["labels"]["research"]["strategy"] == "direct-then-browser"),
    "a source naming no strategy is read directly"
  );
  assert!(store.queue(Some("it"), &[], None, 50).unwrap().items.is_empty());
}
