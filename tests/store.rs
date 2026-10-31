//! The store and the queue's rules (`docs/store.md`, `docs/api.md` 「queue」): the categories it starts with,
//! the cap per category, the queue's order, a candidate judged once, and where each item came from.

mod support;

use bmo_research_client::{
  CategoryShape, ItemState, NewCategory, NewSource, OriginKind, RecipientKind, Running, Shape,
};
use research_server::sources::{Candidate, ItemKind};
use research_server::store::{Asked, Filed, Refusal, Store, schema_version};

fn store() -> Store {
  Store::open(&support::scratch().join("research")).unwrap()
}

fn candidate(name: &str, kind: ItemKind) -> Candidate {
  Candidate {
    name: name.into(),
    maker: Some("maker".into()),
    address: Some(format!("https://example.org/{name}")),
    published_at: None,
    kind,
    words: name.into(),
  }
}

#[test]
fn a_new_store_holds_the_three_first_categories_at_its_version() {
  let store = store();
  assert_eq!(store.version().unwrap(), schema_version());
  let categories = store.categories().unwrap();
  let named: Vec<(&str, CategoryShape)> = categories.iter().map(|c| (c.name.as_str(), c.shape)).collect();
  assert_eq!(
    named,
    [
      ("economy", CategoryShape::Brief),
      ("it", CategoryShape::ManualOrInvestigation),
      ("security", CategoryShape::Advisory)
    ]
  );
  let economy = &categories[0];
  assert_eq!((economy.cap, economy.gather_minutes), (30, Some(1440)));
  assert!(categories.iter().all(|c| c.state == Running::Running && c.provider.is_none()));
}

#[test]
fn a_store_opened_again_keeps_a_category_taken_away() {
  let dir = support::scratch().join("research");
  let store = Store::open(&dir).unwrap();
  store.remove_category("economy").unwrap();
  drop(store);
  let again = Store::open(&dir).unwrap();
  assert!(again.category("economy").is_err());
}

#[test]
fn the_cap_holds_a_category_s_part_and_the_oldest_unqueued_fill_a_freed_slot() {
  let store = store();
  let it = store.category("it").unwrap();
  for n in 0..7 {
    store
      .keep_seen(&candidate(&format!("model-{n}"), ItemKind::Model), "it", None, false)
      .unwrap();
    std::thread::sleep(std::time::Duration::from_millis(2));
  }
  let made = store.fill(&it).unwrap();
  assert_eq!(made.len(), 5, "the cap is 5");
  assert!(store.fill(&it).unwrap().is_empty(), "a full part takes nothing more");

  // A slot freed by a try that ends takes the oldest that waits unqueued.
  let (run, _) = store.start_try("it", &[made[0]]).unwrap().unwrap();
  store.end_run(run, "closed", Some("answered"), None).unwrap();
  store.file(made[0], &Default::default(), None).unwrap();
  let next = store.fill(&it).unwrap();
  assert_eq!(next.len(), 1);
  assert_eq!(store.item(next[0]).unwrap().name, "model-5");
  assert_eq!(store.item(next[0]).unwrap().shape, Shape::Manual);
}

#[test]
fn a_category_s_shape_sets_its_items_shape() {
  let store = store();
  for (category, kind, shape) in [
    ("it", ItemKind::Paper, Shape::Investigation),
    ("security", ItemKind::Entry, Shape::Advisory),
    ("economy", ItemKind::Entry, Shape::Brief),
  ] {
    store
      .keep_seen(&candidate(&format!("{category}-thing"), kind), category, None, false)
      .unwrap();
    let made = store.fill(&store.category(category).unwrap()).unwrap();
    assert_eq!(store.item(made[0]).unwrap().shape, shape, "{category}");
  }
}

#[test]
fn a_candidate_is_kept_once_and_an_archived_one_is_never_queued() {
  let store = store();
  let one = candidate("seen-once", ItemKind::Model);
  store.keep_seen(&one, "it", None, false).unwrap();
  assert!(store.is_seen(&one.key()).unwrap());
  store.keep_seen(&one, "it", None, true).unwrap();
  assert_eq!(
    store.seen_state(&one.key()).unwrap().as_deref(),
    Some("unqueued"),
    "the first judgement stands"
  );
  let held = candidate("held-already", ItemKind::Model);
  store.keep_seen(&held, "it", None, true).unwrap();
  let made = store.fill(&store.category("it").unwrap()).unwrap();
  assert_eq!(made.len(), 1);
  assert_eq!(store.item(made[0]).unwrap().name, "seen-once");
}

#[test]
fn an_ask_stands_first_and_a_front_move_goes_above_it() {
  let store = store();
  for n in 0..3 {
    store
      .keep_seen(&candidate(&format!("found-{n}"), ItemKind::Model), "it", None, false)
      .unwrap();
    std::thread::sleep(std::time::Duration::from_millis(2));
  }
  let found = store.fill(&store.category("it").unwrap()).unwrap();
  let asked = store
    .ask("it", "example.org/asked", "asked", "https://example.org/asked", Shape::Manual)
    .unwrap();
  assert!(asked.asked);
  let waiting = store.waiting("it", i64::MAX, 10).unwrap();
  let order: Vec<i64> = waiting.iter().map(|i| i.id).collect();
  assert_eq!(order, [asked.id, found[0], found[1], found[2]]);

  let moved = store.front(found[2]).unwrap();
  assert!(moved.priority > asked.priority);
  assert_eq!(store.waiting("it", i64::MAX, 1).unwrap()[0].id, found[2]);
}

#[test]
fn an_ask_of_a_key_still_waiting_is_a_conflict_and_never_counts_against_the_cap() {
  let store = store();
  let first = store
    .ask("security", "example.org/a", "a", "https://example.org/a", Shape::Advisory)
    .unwrap();
  let again = store.ask("security", "example.org/a", "a", "https://example.org/a", Shape::Advisory);
  assert!(matches!(again, Err(Refusal::Conflict(_))));
  for n in 0..3 {
    store
      .keep_seen(&candidate(&format!("adv-{n}"), ItemKind::Entry), "security", None, false)
      .unwrap();
  }
  assert_eq!(
    store.fill(&store.category("security").unwrap()).unwrap().len(),
    3,
    "the ask stands beside the cap"
  );
  assert_eq!(store.item(first.id).unwrap().state, ItemState::Waiting);
}

#[test]
fn dismissal_and_taking_follow_the_states() {
  let store = store();
  let item = store.ask("it", "example.org/x", "x", "https://example.org/x", Shape::Manual).unwrap();
  assert!(
    matches!(store.take(item.id, 5), Err(Refusal::Conflict(_))),
    "a waiting item is not taken"
  );
  let (run, _) = store.start_try("it", &[item.id]).unwrap().unwrap();
  store.end_run(run, "closed", None, None).unwrap();
  store.file(item.id, &Default::default(), None).unwrap();
  assert_eq!(store.item(item.id).unwrap().state, ItemState::Finishing);
  assert!(
    matches!(store.take(item.id, 5), Err(Refusal::Conflict(_))),
    "an item nobody was told of is not taken"
  );
  assert!(store.finished(item.id, 0).unwrap(), "the whole page's report is told once");
  assert!(!store.finished(item.id, 0).unwrap());
  assert_eq!(store.item(item.id).unwrap().state, ItemState::Reported);
  let taken = store.take(item.id, 5).unwrap();

  assert_eq!((taken.state, taken.job_item_id), (ItemState::Taken, Some(5)));
  assert!(matches!(store.dismiss(item.id), Err(Refusal::Conflict(_))));
}

#[test]
fn a_category_with_sources_is_not_taken_away_and_stopping_one_leaves_the_others() {
  let store = store();
  store
    .add_category(&NewCategory {
      name: "health".into(),
      purpose: "p".into(),
      shape: CategoryShape::ManualOrInvestigation,
      cap: None,
      cadence_minutes: None,
      gather_minutes: None,
      provider: None,
      model: None,
    })
    .unwrap();
  store.add_recipient("health", RecipientKind::Room, Some(3), None, "added").unwrap();
  let source = store
    .add_source(
      &NewSource {
        category_name: "health".into(),
        kind: "rss".into(),
        ..NewSource::default()
      },
      "https://example.org/feed.xml",
      30,
    )
    .unwrap();
  assert!(matches!(store.remove_category("health"), Err(Refusal::Conflict(_))));
  store.remove_source(source.id).unwrap();
  assert!(matches!(
    store.add_recipient("health", RecipientKind::Room, Some(3), None, "added"),
    Err(Refusal::Conflict(_))
  ));
  store.set_running(Some("health"), Running::Stopped).unwrap();
  let state = store.daemon_state().unwrap();
  assert_eq!(state.state, Running::Running);
  let stopped: Vec<&str> = state
    .categories
    .iter()
    .filter(|c| c.state == Running::Stopped)
    .map(|c| c.category_name.as_str())
    .collect();
  assert_eq!(stopped, ["health"]);
  store.set_running(None, Running::Stopped).unwrap();
  assert_eq!(store.daemon_state().unwrap().state, Running::Stopped);
  store.remove_category("health").unwrap();
  assert!(store.recipients("health").unwrap().is_empty());
}

#[test]
fn an_item_keeps_where_it_came_from_a_source_an_ask_or_the_document_it_follows() {
  let store = store();
  let source = store
    .add_source(
      &NewSource {
        category_name: "it".into(),
        kind: "huggingface-trending".into(),
        ..NewSource::default()
      },
      "https://example.org/api/models",
      5,
    )
    .unwrap();
  store
    .keep_seen(&candidate("found", ItemKind::Model), "it", Some(source.id), false)
    .unwrap();
  let found = store.item(store.fill(&store.category("it").unwrap()).unwrap()[0]).unwrap();
  assert_eq!(found.origin.kind, OriginKind::Source);
  assert_eq!(
    found.origin.source.as_deref(),
    Some("huggingface-trending (https://example.org/api/models)")
  );
  assert_eq!((found.origin.because.as_deref(), found.origin.document_id), (None, None));

  let asked = Asked {
    because: Some("Which small model answers on one card?".into()),
    parent: None,
  };
  let parent = store
    .ask_from(
      "it",
      "example.org/parent",
      "Parent-7B",
      "https://example.org/parent",
      Shape::Manual,
      &asked,
    )
    .unwrap();
  assert_eq!(parent.origin.kind, OriginKind::Asked);
  assert_eq!(parent.origin.because.as_deref(), Some("Which small model answers on one card?"));
  assert!(
    matches!(store.parent(Some(parent.id), None), Err(Refusal::Conflict(_))),
    "nothing filed yet"
  );
  assert!(matches!(store.parent(Some(9999), None), Err(Refusal::NotFound(_))));

  store.start_try("it", &[parent.id]).unwrap();
  let filed = Filed {
    document_id: 812,
    title: "Parent-7B manual".into(),
    share_url: "https://share.example/d812".into(),
    ..Filed::default()
  };
  assert!(store.file(parent.id, &filed, None).unwrap());
  let from_document = store.parent(None, Some(812)).unwrap();
  assert_eq!(from_document, store.parent(Some(parent.id), None).unwrap());
  assert_eq!(
    (
      from_document.item_id,
      from_document.title.as_deref(),
      from_document.share_url.as_deref()
    ),
    (Some(parent.id), Some("Parent-7B manual"), Some("https://share.example/d812"))
  );
  let unknown = store.parent(None, Some(900)).unwrap();
  assert_eq!((unknown.item_id, unknown.document_id, unknown.title), (None, Some(900), None));

  let follow = Asked {
    because: Some("The manual names a lighter variant; what is it?".into()),
    parent: Some(from_document),
  };
  let child = store
    .ask_from(
      "it",
      "example.org/child",
      "Child-2B",
      "https://example.org/child",
      Shape::Manual,
      &follow,
    )
    .unwrap();
  let read = store.item(child.id).unwrap();
  assert_eq!(read.origin.kind, OriginKind::FollowUp);
  assert_eq!(
    (read.origin.item_id, read.origin.document_id, read.origin.title.as_deref()),
    (Some(parent.id), Some(812), Some("Parent-7B manual"))
  );
  assert_eq!(read.origin.share_url.as_deref(), Some("https://share.example/d812"));
  assert_eq!(read.origin.because.as_deref(), Some("The manual names a lighter variant; what is it?"));
  assert_eq!(read.origin.source, None);
  let listed = store.queue(None, &[ItemState::Waiting], None, 50).unwrap().items;
  assert!(listed.iter().any(|item| item.id == child.id && item.origin == read.origin));
}
