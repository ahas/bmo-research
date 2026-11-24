//! The store and the queue's rules (`docs/store.md`, `docs/api.md` 「queue」): the categories it starts with,
//! no cap on the queue, the queue's order, a candidate judged once, where each item came from, a stuck item's
//! moves, an error dated by its try, the count of each run's review asks, and a topic category.

use crate::support;

use bmo_research_client::{
  CategoryShape, ItemState, NewCategory, NewSource, OriginKind, RecipientKind, Running, Shape,
};
use research_server::sources::{Candidate, ItemKind};
use research_server::stack::AgeFloor;
use research_server::store::{Asked, Filed, NewResearchRun, Refusal, Store, now_ms, schema_version};

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
  assert_eq!(economy.gather_minutes, Some(1440));
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
fn the_queue_has_no_cap_and_takes_every_unqueued_row_oldest_first() {
  let store = store();
  let it = store.category("it").unwrap();
  for n in 0..40 {
    store
      .keep_seen(&candidate(&format!("model-{n:02}"), ItemKind::Model), "it", None, false)
      .unwrap();
  }
  let made = store.fill(&it).unwrap();
  assert_eq!(made.len(), 40, "every unqueued row is queued, however many");
  assert!(store.fill(&it).unwrap().is_empty(), "nothing is left unqueued");
  assert_eq!(store.item(made[0]).unwrap().name, "model-00");
  assert_eq!(store.item(made[0]).unwrap().shape, Shape::Manual);
  let waiting = store.queue(Some("it"), &[ItemState::Waiting], None, 100).unwrap().items;
  assert_eq!(waiting.len(), 40, "every one waits");
  assert_eq!(store.due_count("it", now_ms()).unwrap(), 40);
  let state = store.daemon_state().unwrap();
  let part = state.categories.iter().find(|c| c.category_name == "it").unwrap();
  assert_eq!((part.waiting_count, part.running_count), (40, 0));
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
fn the_age_floor_keeps_an_older_advisory_from_the_queue_and_takes_a_newer_one() {
  let store = store();
  let floor = AgeFloor {
    component: "Node.js".into(),
    version: "22.20.0".into(),
    at: 1_000,
  };
  let mut older = candidate("older", ItemKind::Entry);
  older.published_at = Some(999);
  let mut newer = candidate("newer", ItemKind::Entry);
  newer.published_at = Some(1_001);
  let undated = candidate("undated", ItemKind::Entry);
  for one in [&older, &newer, &undated] {
    store.keep_seen(one, "security", None, false).unwrap();
  }

  let (made, skipped) = store.fill_with(&store.category("security").unwrap(), Some(&floor)).unwrap();
  assert_eq!(skipped, ["older"]);
  assert_eq!(
    store.seen_state(&older.key()).unwrap().as_deref(),
    Some("stale"),
    "an advisory older than the floor is never queued"
  );
  let named: Vec<String> = made.iter().map(|id| store.item(*id).unwrap().name).collect();
  assert_eq!(named, ["newer", "undated"], "a newer or undated advisory is queued");
  assert_eq!(store.fill_with(&store.category("security").unwrap(), Some(&floor)).unwrap().0.len(), 0);
}

#[test]
fn without_a_floor_every_advisory_is_queued() {
  let store = store();
  let mut old = candidate("old", ItemKind::Entry);
  old.published_at = Some(1);
  store.keep_seen(&old, "security", None, false).unwrap();
  let made = store.fill(&store.category("security").unwrap()).unwrap();
  assert_eq!(made.len(), 1, "an empty or unknown inventory skips nothing");
}

#[test]
fn a_skipped_item_keeps_one_line_naming_the_floor_and_waits_for_no_retry() {
  let store = store();
  let item = store
    .ask("security", "example.org/sa", "SA", "https://example.org/sa", Shape::Advisory)
    .unwrap();
  let (run, _) = store.start_try("security", &[item.id]).unwrap().unwrap();
  let floor = "older than the age floor, Node.js 22.20.0 (recorded 2026-11-02T03:02:42Z)";
  store.skip(item.id, floor).unwrap();
  store.end_run(run, "skipped", Some("skipped"), None).unwrap();

  let skipped = store.item(item.id).unwrap();
  assert_eq!(skipped.state, ItemState::Skipped);
  assert_eq!(skipped.error.as_deref(), Some(floor), "the queue keeps the floor it used");
  assert!(skipped.retry_at.is_none(), "a skipped item waits for no retry");
  assert!(
    store.waiting("security", now_ms(), 1).unwrap().is_empty(),
    "a skipped item is never taken again"
  );
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
fn an_ask_of_a_key_still_waiting_is_a_conflict_and_stands_beside_the_scan_s_items() {
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
    "the ask stands beside the scan's items"
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
fn a_stuck_item_is_still_asked_goes_back_stuck_when_interrupted_and_is_asked_again_or_fronted() {
  let store = store();
  let item = store.ask("it", "example.org/y", "y", "https://example.org/y", Shape::Manual).unwrap();
  let (run, _) = store.start_try("it", &[item.id]).unwrap().unwrap();
  store.end_run(run, "failed", None, Some("short")).unwrap();
  assert_eq!(store.fail_try(run, "short", 0, true).unwrap(), [item.id]);
  let stuck = store.item(item.id).unwrap();
  assert_eq!((stuck.state, stuck.tries, stuck.retry_at), (ItemState::Stuck, 1, Some(0)));
  assert!(matches!(store.front(item.id), Ok(ref i) if i.retry_at.is_none() && i.state == ItemState::Stuck));
  assert_eq!(store.waiting("it", 1, 5).unwrap().len(), 1, "a stuck item is asked on its retry wait");
  let state = store.daemon_state().unwrap();
  assert_eq!(state.categories.iter().find(|c| c.category_name == "it").unwrap().stuck_count, 1);
  assert!(matches!(
    store.ask("it", "example.org/y", "y", "https://example.org/y", Shape::Manual),
    Err(Refusal::Conflict(_))
  ));

  // An interrupted try puts it back stuck, its try taken off the count.
  let (run, _) = store.start_try("it", &[item.id]).unwrap().unwrap();
  store
    .requeue(run, "cut", Some("cut"), "agency went away, so the try is not counted", 0)
    .unwrap();
  let back = store.item(item.id).unwrap();
  assert_eq!((back.state, back.tries, back.retry_at), (ItemState::Stuck, 1, None));

  store.keep_repair(&[item.id], 77).unwrap();
  assert!(store.unrepaired().unwrap().is_empty());
  let again = store.ask_again(item.id).unwrap();
  assert_eq!((again.state, again.tries, again.repair_item_id), (ItemState::Waiting, 0, None));
  assert!(matches!(store.ask_again(item.id), Err(Refusal::Conflict(_))));
}

#[test]
fn an_error_is_dated_by_its_try_and_cleared_as_the_next_starts() {
  let store = store();
  let item = store.ask("it", "example.org/d", "d", "https://example.org/d", Shape::Manual).unwrap();
  let (run, _) = store.start_try("it", &[item.id]).unwrap().unwrap();
  let before = now_ms();
  store
    .requeue(run, "cut", Some("cut"), "agency went away, so the try is not counted", 3)
    .unwrap();
  let back = store.item(item.id).unwrap();
  assert!(back.error_at.is_some_and(|at| at >= before), "{back:?}");
  let (run, _) = store.start_try("it", &[item.id]).unwrap().unwrap();
  assert_eq!(store.item(item.id).unwrap().error_at, None, "a try under way carries no outcome yet");
  store.fail_try(run, "the turn wrote nothing", now_ms() + 60_000, false).unwrap();
  let failed = store.item(item.id).unwrap();
  assert_eq!(failed.error.as_deref(), Some("the turn wrote nothing"));
  assert!(failed.error_at.is_some_and(|at| at >= before));
}

#[test]
fn a_category_with_sources_is_not_taken_away_and_stopping_one_leaves_the_others() {
  let store = store();
  store
    .add_category(&NewCategory {
      name: "health".into(),
      purpose: "p".into(),
      shape: CategoryShape::ManualOrInvestigation,
      cadence_minutes: None,
      gather_minutes: None,
      provider: None,
      model: None,
      instruction: None,
      calendar: None,
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
  assert!(store.file(parent.id, &filed).unwrap());
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

#[test]
fn each_review_ask_of_a_run_is_counted_anew_from_one() {
  let store = store();
  let open = || {
    store
      .open_research_run(&NewResearchRun {
        key: "search:exhaustive",
        line: "search",
        query: "R1",
        asker: None,
        options: None,
        room_id: None,
        provider: "registered",
        model: "flash-next",
        branches_allowed: 0,
        tells: false,
      })
      .unwrap()
  };
  let (first, other) = (open(), open());
  assert_eq!(store.count_review_ask(first).unwrap(), 1);
  assert_eq!(store.count_review_ask(first).unwrap(), 2);
  assert_eq!(store.count_review_ask(other).unwrap(), 1, "each run counts its own");
  assert!(store.count_review_ask(other + 1).is_err(), "no run, no count");
}

#[test]
fn a_category_takes_a_topic_beside_the_first_three() {
  let store = store();
  let before = store.categories().unwrap();
  assert_eq!(before.len(), 3);
  assert!(
    before
      .iter()
      .all(|category| category.instruction.is_none() && category.calendar.is_none())
  );

  let topic = store
    .add_category(&NewCategory {
      name: "ai-events".into(),
      purpose: "AI events".into(),
      shape: CategoryShape::Topic,
      cadence_minutes: None,
      gather_minutes: None,
      provider: None,
      model: None,
      instruction: Some("Find AI events.".into()),
      calendar: Some(bmo_research_client::CalendarTarget {
        realm: "organization".into(),
        calendar_id: "shared".into(),
      }),
    })
    .unwrap();
  assert_eq!(
    (topic.gather_minutes, topic.instruction.as_deref()),
    (Some(1440), Some("Find AI events."))
  );
  assert_eq!(
    topic.calendar.map(|calendar| (calendar.realm, calendar.calendar_id)),
    Some(("organization".into(), "shared".into()))
  );
  // A topic's items take its shape.
  assert_eq!(research_server::store::item_shape(CategoryShape::Topic, None), Shape::Topic);
}
