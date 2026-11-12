//! A node pulls (`docs/design.md`, 「A node pulls」) against a stand-in agency whose line lets in no more of a
//! role's slots than its cap: three queued items under a cap of two run two and leave one waiting, a slot
//! given back pulls the third, a stopped category's items are taken by no place, a daemon asked to stop asks
//! no place, and no cap is answered anywhere.

use crate::support;

use std::time::Duration;

use bmo_research_client::{ItemState, Running, Shape};
use research_server::dive::tick;
use research_server::pull;
use research_server::words;
use serde_json::Value;
use support::{Filed, Ground};

/// Waits until the condition holds, at most a few seconds.
async fn until(what: &str, mut holds: impl FnMut() -> bool) {
  for _ in 0..1000 {
    if holds() {
      return;
    }
    tokio::time::sleep(Duration::from_millis(10)).await;
  }
  panic!("{what} did not come");
}

fn ask(ground: &Ground, category: &str, name: &str, shape: Shape) -> i64 {
  let key = format!("example.org/{name}");
  let address = format!("https://{key}");
  ground.app.store.ask(category, &key, name, &address, shape).unwrap().id
}

/// Each research run's result filed whole as an investigation.
fn file_all(ground: &Ground, runs: std::ops::RangeInclusive<i64>) {
  let heading = words::document().sections.investigation[0].clone();
  let mut world = ground.world.lock().unwrap();
  for run in runs {
    world.filed.insert(
      run,
      Filed {
        id: 700 + run,
        title: format!("Report {run}"),
        description: "It is a thing.".into(),
        content: format!("# An investigation\n\n## {heading}\n\nIt is a thing.\n"),
      },
    );
  }
}

fn state_of(ground: &Ground, id: i64) -> ItemState {
  ground.app.store.item(id).unwrap().state
}

fn count(ground: &Ground, state: ItemState) -> usize {
  ground.app.store.queue(None, &[state], None, 100).unwrap().items.len()
}

/// The places asked of agency, by their request keys.
fn places(ground: &Ground) -> Vec<Value> {
  ground
    .asked_path("agency", "POST", "/slot-requests")
    .into_iter()
    .map(|a| a.body)
    .filter(|b| b["requestKey"].as_str().is_some_and(|key| key.starts_with("research:place:")))
    .collect()
}

#[tokio::test]
async fn a_daemon_asked_to_stop_asks_no_place_and_leaves_its_items_waiting() {
  let ground = support::ground(|_| {}).await;
  let item = ask(&ground, "it", "a", Shape::Investigation);
  ground.app.stop.ask();
  assert!(tick(&ground.app).await.is_empty());
  assert!(places(&ground).is_empty(), "no place is asked once a stop is");
  assert_eq!(ground.app.store.item(item).unwrap().tries, 0);
  assert_eq!(state_of(&ground, item), ItemState::Waiting);
}

#[tokio::test]
async fn a_watched_road_asks_no_more_places_than_its_pool_may_use() {
  let ground = support::ground(|_| {}).await;
  {
    let mut world = ground.world.lock().unwrap();
    world.role_cap = 4;
    world.lining = true;
    world.held_runs.extend([1, 2]);
  }
  file_all(&ground, 1..=2);
  for name in ["a", "b"] {
    ask(&ground, "it", name, Shape::Investigation);
  }
  let started = tick(&ground.app).await;
  assert_eq!(started.len(), 1, "one card of the engine, one place, whatever the role's cap");
  assert_eq!(tick(&ground.app).await.len(), 0, "the place out counts until its try ends");
  ground.world.lock().unwrap().held_runs.clear();
}

#[tokio::test]
async fn two_places_run_two_items_the_third_waits_and_a_slot_given_back_pulls_it() {
  let ground = support::ground(|s| s.engine.cards = 2).await;
  {
    let mut world = ground.world.lock().unwrap();
    world.role_cap = 2;
    world.lining = true;
    world.held_runs.extend([1, 2]);
  }
  file_all(&ground, 1..=3);
  let items: Vec<i64> = ["a", "b", "c"]
    .into_iter()
    .map(|name| ask(&ground, "it", name, Shape::Investigation))
    .collect();
  tokio::spawn(pull::wakes(ground.app.clone()));

  let started = tick(&ground.app).await;
  assert_eq!(started.len(), 2, "one place for each slot the cap leaves");
  until("two items running", || count(&ground, ItemState::Running) == 2).await;
  assert_eq!(count(&ground, ItemState::Waiting), 1, "the third waits in the queue");
  assert_eq!(state_of(&ground, items[2]), ItemState::Waiting);
  assert_eq!(ground.world.lock().unwrap().slots_held(), 2, "each running item holds a slot");
  assert_eq!(ground.app.store.running_count().unwrap(), 2);

  // A pull while both slots are held asks nothing more.
  assert!(tick(&ground.app).await.is_empty());
  assert_eq!(places(&ground).len(), 2);
  let base = support::serve(research_server::routes::router(ground.app.clone())).await;
  let state: Value = reqwest::Client::new()
    .get(format!("{base}/state"))
    .bearer_auth("test-token")
    .send()
    .await
    .unwrap()
    .json()
    .await
    .unwrap();
  let it = state["categories"]
    .as_array()
    .unwrap()
    .iter()
    .find(|c| c["categoryName"] == "it")
    .unwrap();
  assert_eq!((it["waitingCount"].as_i64(), it["runningCount"].as_i64()), (Some(1), Some(2)));

  // The first run ends and gives its slot back: the pull it wakes takes the third.
  ground.world.lock().unwrap().held_runs.remove(&1);
  until("the third pulled and reported", || state_of(&ground, items[2]) == ItemState::Reported).await;
  until("the first reported", || state_of(&ground, items[0]) == ItemState::Reported).await;
  assert_eq!(state_of(&ground, items[1]), ItemState::Running, "the second still holds its slot");
  assert_eq!(places(&ground).len(), 3);

  ground.world.lock().unwrap().held_runs.clear();
  for handle in started {
    handle.await.unwrap();
  }
  until("every item reported", || count(&ground, ItemState::Reported) == 3).await;
}

#[tokio::test]
async fn a_stopped_category_s_items_are_taken_by_no_place_until_it_runs_again() {
  let ground = support::ground(|_| {}).await;
  {
    let mut world = ground.world.lock().unwrap();
    world.role_cap = 2;
    world.lining = true;
  }
  file_all(&ground, 1..=2);
  let stopped = ask(&ground, "security", "a", Shape::Advisory);
  let running = ask(&ground, "it", "b", Shape::Investigation);
  ground.app.store.set_running(Some("security"), Running::Stopped).unwrap();

  let started = tick(&ground.app).await;
  assert_eq!(started.len(), 1, "a place for the running category's item alone");
  for handle in started {
    handle.await.unwrap();
  }
  assert_eq!(state_of(&ground, running), ItemState::Reported);
  let waiting = ground.app.store.item(stopped).unwrap();
  assert_eq!((waiting.state, waiting.tries), (ItemState::Waiting, 0), "no place took it");
  assert_eq!(places(&ground).len(), 1);

  ground.app.store.set_running(Some("security"), Running::Running).unwrap();
  for handle in tick(&ground.app).await {
    handle.await.unwrap();
  }
  assert_eq!(places(&ground).len(), 2);
  assert_eq!(ground.app.store.item(stopped).unwrap().tries, 1, "a place took it once it ran again");
}

#[tokio::test]
async fn no_cap_is_answered_or_described_anywhere() {
  let ground = support::ground(|_| {}).await;
  let base = support::serve(research_server::routes::router(ground.app.clone())).await;
  let client = reqwest::Client::new();
  for path in ["/state", "/categories", "/categories/it"] {
    let body: String = client
      .get(format!("{base}{path}"))
      .bearer_auth("test-token")
      .send()
      .await
      .unwrap()
      .text()
      .await
      .unwrap();
    assert!(!body.contains("\"cap\""), "{path} answers a cap: {body}");
  }
  let document = research_server::openapi::openapi_json();
  assert!(!document.contains("\"cap\""), "the OpenAPI document names a cap");
}
