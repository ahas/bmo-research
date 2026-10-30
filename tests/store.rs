//! The store and the queue's rules (`docs/store.md`, `docs/api.md` 「queue」): the categories it starts with,
//! the cap per category, the queue's order, and a candidate judged once.

mod support;

use bmo_research_client::{
  CategoryShape, ItemState, NewCategory, NewSource, RecipientKind, Running, Shape,
};
use research_server::sources::{Candidate, ItemKind};
use research_server::store::{Refusal, Store, schema_version};

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
  store.file(made[0], &Default::default()).unwrap();
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
  store.file(item.id, &Default::default()).unwrap();
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
