//! The store and the queue's rules (`docs/store.md`, `docs/api.md` 「queue」): the categories it starts with,
//! the cap per category, the queue's order, a candidate judged once, where each item came from, a stuck item's
//! moves, an error dated by its try, the step that stands every item a store held given up stuck, the step
//! that dates every error the store held, the count of each run's review asks, and the step that takes the
//! run's clock off its runs and its kept settings.

mod support;

use bmo_research_client::{
  CategoryShape, ItemState, NewCategory, NewSource, OriginKind, RecipientKind, Running, Shape,
};
use research_server::sources::{Candidate, ItemKind};
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

/// What undoes the step that keeps a page by its document: its table dropped and the item's columns back.
const UNDO_PAGES: &str = "drop table pages;
  alter table queue add column languages text not null default '[]';
  alter table queue add column narrations text not null default '[]';
  alter table queue add column pending text not null default '[]';
  alter table queue add column pending_retry_at integer;
  alter table queue add column translation_tries integer not null default 0;
  alter table queue add column narration_tries integer not null default 0;";

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
fn the_step_that_keeps_repairs_stands_every_given_up_item_stuck_with_its_retry_due() {
  let dir = support::scratch().join("research");
  let store = Store::open(&dir).unwrap();
  let item = store.ask("it", "example.org/z", "z", "https://example.org/z", Shape::Manual).unwrap();
  let kept = store.ask("it", "example.org/w", "w", "https://example.org/w", Shape::Manual).unwrap();
  drop(store);
  // The store as it stood one step before: an item given up after its retries, and its notice.
  let conn = rusqlite::Connection::open(dir.join("research.db")).unwrap();
  conn
    .execute_batch(&format!(
      "{UNDO_PAGES}
       update queue set state = 'failed', tries = 4, retry_at = null, error = 'the turn wrote nothing'
         where id = {};
       insert into reports (item_id, recipient, kind, state, at) values ({}, 'room:7', 'failed', 'sent', 1);
       alter table queue drop column repair_item_id;
       alter table queue drop column error_at;
       alter table research_runs drop column continues;
       alter table research_runs drop column answered;
       alter table research_runs drop column review_asks;
       alter table research_runs add column closes_at integer;
       drop table translated_parts;
       drop table page_texts;
       drop table page_reviews;
       drop table narration_asks;
       drop table syntheses;
       pragma user_version = 9;",
      item.id, item.id
    ))
    .unwrap();
  drop(conn);

  let store = Store::open(&dir).unwrap();
  assert_eq!(store.version().unwrap(), schema_version());
  let stuck = store.item(item.id).unwrap();
  assert_eq!(
    (stuck.state, stuck.tries, stuck.retry_at, stuck.repair_item_id),
    (ItemState::Stuck, 4, Some(0), None)
  );
  assert_eq!(stuck.error.as_deref(), Some("the turn wrote nothing"));
  assert_eq!(store.item(kept.id).unwrap().state, ItemState::Waiting);
  assert_eq!(store.unrepaired().unwrap().iter().map(|i| i.id).collect::<Vec<_>>(), [item.id]);
  let conn = rusqlite::Connection::open(dir.join("research.db")).unwrap();
  let kinds: String = conn.query_row("select group_concat(kind) from reports", [], |r| r.get(0)).unwrap();
  assert_eq!(kinds, "stuck");
}

#[test]
fn the_step_that_dates_errors_dates_each_by_the_latest_try_that_carried_its_item() {
  let dir = support::scratch().join("research");
  let store = Store::open(&dir).unwrap();
  let lead = store
    .ask("economy", "example.org/e1", "E1", "https://example.org/e1", Shape::Brief)
    .unwrap();
  let other = store
    .ask("economy", "example.org/e2", "E2", "https://example.org/e2", Shape::Brief)
    .unwrap();
  let quiet = store.ask("it", "example.org/q", "q", "https://example.org/q", Shape::Manual).unwrap();
  let (run, _) = store.start_try("economy", &[lead.id, other.id]).unwrap().unwrap();
  store.keep_run_id(run, 116).unwrap();
  store
    .requeue(run, "cut", Some("cut"), "agency went away, so the try is not counted", 3)
    .unwrap();
  drop(store);
  // The store as it stood before that step: the try ended at a known time, and no item dated.
  let conn = rusqlite::Connection::open(dir.join("research.db")).unwrap();
  conn
    .execute_batch(&format!(
      "{UNDO_PAGES}
       update runs set ended_at = 5000 where id = {run};
       alter table queue drop column error_at;
       alter table research_runs drop column review_asks;
       alter table research_runs add column closes_at integer;
       pragma user_version = 12;"
    ))
    .unwrap();
  drop(conn);

  let store = Store::open(&dir).unwrap();
  assert_eq!(store.version().unwrap(), schema_version());
  for id in [lead.id, other.id] {
    let item = store.item(id).unwrap();
    assert_eq!((item.error.is_some(), item.error_at), (true, Some(5000)), "item {id}");
  }
  assert_eq!(store.item(quiet.id).unwrap().error_at, None, "an item with no error stays undated");
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
fn each_review_ask_of_a_run_is_counted_anew_from_one_and_a_store_from_before_counts_from_one() {
  let dir = support::scratch().join("research");
  let store = Store::open(&dir).unwrap();
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
  let (before, other) = (open(), open());
  drop(store);
  // The store as it stood before the count's step, holding runs with no count and a closing time.
  let conn = rusqlite::Connection::open(dir.join("research.db")).unwrap();
  conn
    .execute_batch(&format!(
      "{UNDO_PAGES} alter table research_runs drop column review_asks; alter table research_runs add column \
       closes_at integer; pragma user_version = 13;",
    ))
    .unwrap();
  drop(conn);

  let store = Store::open(&dir).unwrap();
  assert_eq!(store.version().unwrap(), schema_version());
  assert_eq!(store.count_review_ask(before).unwrap(), 1);
  assert_eq!(store.count_review_ask(before).unwrap(), 2);
  assert_eq!(store.count_review_ask(other).unwrap(), 1, "each run counts its own");
  assert!(store.count_review_ask(other + 1).is_err(), "no run, no count");
}

#[test]
fn the_step_that_drops_the_run_s_clock_takes_it_off_the_runs_and_the_kept_settings() {
  let dir = support::scratch().join("research");
  let store = Store::open(&dir).unwrap();
  store
    .keep_research_settings(
      r#"{"branchRole":"researcher","strategy":"direct-then-browser","hostIntervalMs":null,
"engineIntervalMs":null,"keys":{"browse":{"branches":0,"minutes":15},
"search:basic":{"branches":0,"minutes":5,"pages":3}}}"#,
    )
    .unwrap();
  drop(store);
  // The store as it stood one step before, its runs and its kept settings holding a clock.
  let conn = rusqlite::Connection::open(dir.join("research.db")).unwrap();
  conn
    .execute_batch(&format!(
      "{UNDO_PAGES} alter table research_runs add column closes_at integer; pragma user_version = 14;"
    ))
    .unwrap();
  drop(conn);

  let store = Store::open(&dir).unwrap();
  assert_eq!(store.version().unwrap(), schema_version());
  let kept: serde_json::Value =
    serde_json::from_str(&store.kept_research_settings().unwrap().unwrap()).unwrap();
  assert_eq!(kept["keys"]["browse"], serde_json::json!({"branches": 0}));
  assert_eq!(kept["keys"]["search:basic"], serde_json::json!({"branches": 0, "pages": 3}));
  drop(store);
  let conn = rusqlite::Connection::open(dir.join("research.db")).unwrap();
  let columns: Vec<String> = conn
    .prepare("select name from pragma_table_info('research_runs')")
    .unwrap()
    .query_map([], |row| row.get(0))
    .unwrap()
    .map(Result::unwrap)
    .collect();
  assert!(!columns.iter().any(|column| column == "closes_at"), "{columns:?}");
}

#[test]
fn the_step_that_keeps_pages_by_document_takes_each_page_from_the_item_its_run_was_asked_for() {
  let dir = support::scratch().join("research");
  let store = Store::open(&dir).unwrap();
  let lead = store
    .ask("economy", "example.org/p1", "P1", "https://example.org/p1", Shape::Brief)
    .unwrap();
  let other = store
    .ask("economy", "example.org/p2", "P2", "https://example.org/p2", Shape::Brief)
    .unwrap();
  let (run, _) = store.start_try("economy", &[lead.id, other.id]).unwrap().unwrap();
  store.keep_run_id(run, 300).unwrap();
  let filed = Filed {
    document_id: 600,
    ..Filed::default()
  };
  for id in [lead.id, other.id] {
    assert!(store.file(id, &filed).unwrap());
  }
  drop(store);
  // The store as it stood one step before: each item kept its document's page, the lead its retry wait.
  let conn = rusqlite::Connection::open(dir.join("research.db")).unwrap();
  conn
    .execute_batch(&format!(
      "{UNDO_PAGES}
       update queue set languages = '[\"ko\",\"en\"]', narrations = '[\"ko\"]', pending = '[\"narration-en\"]',
         pending_retry_at = 42, translation_tries = 2, narration_tries = 3 where id = {};
       update queue set languages = '[\"en\"]' where id = {};
       pragma user_version = 15;",
      lead.id, other.id
    ))
    .unwrap();
  drop(conn);

  let store = Store::open(&dir).unwrap();
  assert_eq!(store.version().unwrap(), schema_version());
  let page = store.page_state(600).unwrap();
  assert_eq!(
    (page.languages, page.narrations, page.pending, page.retry_at),
    (
      vec!["ko".to_string(), "en".to_string()],
      vec!["ko".to_string()],
      vec!["narration-en".to_string()],
      Some(42)
    )
  );
  assert_eq!((page.translation_tries, page.narration_tries), (2, 3));
  // The page is the run's lead item's, the first its try carried, and every item of it answers it.
  let due: Vec<i64> = store.pages_due(42).unwrap().iter().map(|item| item.id).collect();
  assert_eq!(due, [lead.id]);
  assert_eq!(store.item(other.id).unwrap().languages, ["ko", "en"]);
  assert_eq!(store.item(other.id).unwrap().pending_retry_at, Some(42));
  drop(store);
  let conn = rusqlite::Connection::open(dir.join("research.db")).unwrap();
  let columns: Vec<String> = conn
    .prepare("select name from pragma_table_info('queue')")
    .unwrap()
    .query_map([], |row| row.get(0))
    .unwrap()
    .map(Result::unwrap)
    .collect();
  for gone in [
    "languages",
    "narrations",
    "pending",
    "pending_retry_at",
    "translation_tries",
    "narration_tries",
  ] {
    assert!(!columns.iter().any(|column| column == gone), "{gone} in {columns:?}");
  }
}

#[test]
fn the_step_that_names_slots_takes_every_hold_of_a_seat_request_as_a_slot() {
  let dir = support::scratch().join("research");
  let store = Store::open(&dir).unwrap();
  let run = store
    .open_research_run(&NewResearchRun {
      key: "search:basic",
      line: "search",
      query: "Q",
      asker: None,
      options: None,
      room_id: None,
      provider: "",
      model: "",
      branches_allowed: 0,
      tells: false,
    })
    .unwrap();
  store.hold(run, "session", "search@apart:1").unwrap();
  drop(store);
  // The store as it stood one step before, its run holding a slot request under the older word.
  let conn = rusqlite::Connection::open(dir.join("research.db")).unwrap();
  conn
    .execute_batch(&format!(
      "insert into research_holds (run_id, kind, value) values ({run}, 'seat', 'token-1');
       pragma user_version = {};",
      schema_version() - 1
    ))
    .unwrap();
  drop(conn);

  let store = Store::open(&dir).unwrap();
  assert_eq!(store.version().unwrap(), schema_version());
  assert_eq!(
    store.holds(run).unwrap(),
    [
      ("session".to_string(), "search@apart:1".to_string()),
      ("slot".to_string(), "token-1".to_string())
    ]
  );
}
