//! The bench watch against stand-in agency, artifact, receiver and arbiter servers (`docs/api.md`,
//! 「The bench」): the queue held while a bench's ticket stands on a watched node and said so in the state,
//! the wait on the model's health before it goes on, a try the bench cut put back without counting it, and
//! the owner's room told once where the model stays down.

mod support;

use bmo_research_client::{HoldReason, ItemState, ResearchClient, Shape};
use research_server::bench::holds;
use research_server::dive::tick;
use research_server::words;
use serde_json::Value;
use support::{Filed, Ground};

async fn settle(ground: &Ground) {
  for handle in tick(&ground.app).await {
    handle.await.unwrap();
  }
}

fn stand(ground: &Ground, node: &str, services: &[&str]) {
  ground
    .world
    .lock()
    .unwrap()
    .tickets
    .insert(node.into(), services.iter().map(|s| s.to_string()).collect());
}

fn file(ground: &Ground, run: i64, id: i64) {
  ground.world.lock().unwrap().filed.insert(
    run,
    Filed {
      id,
      title: format!("Report {id}"),
      description: "It is a thing.".into(),
      content: "# An investigation\n".into(),
    },
  );
}

fn asks(ground: &Ground) -> usize {
  ground.asked_path("agency", "POST", "/research-runs").len()
}

fn restore_notices(ground: &Ground) -> Vec<Value> {
  ground
    .asked_path("receiver", "POST", "/instructions")
    .into_iter()
    .map(|a| a.body)
    .filter(|b| b["kind"] == "research-restore:flash-next")
    .collect()
}

#[tokio::test]
async fn a_bench_on_the_model_s_node_holds_the_queue_and_the_state_says_why() {
  let ground = support::ground(|_| {}).await;
  let item = ground
    .app
    .store
    .ask("it", "example.org/a", "A", "https://example.org/a", Shape::Investigation)
    .unwrap();
  file(&ground, 101, 11);
  stand(&ground, "n4", &["lab-arena", "bench"]);

  settle(&ground).await;
  assert!(
    holds(&ground.app).is_empty(),
    "a bench on a node the model does not stand on holds nothing"
  );
  assert_eq!(asks(&ground), 1);
  assert_eq!(ground.asked_path("arbiter", "GET", "/cluster").len(), 1);

  ground
    .app
    .store
    .ask("it", "example.org/b", "B", "https://example.org/b", Shape::Investigation)
    .unwrap();
  stand(&ground, "n3", &["lab-arena", "bench"]);
  settle(&ground).await;
  assert_eq!(asks(&ground), 1, "a waiting bench ticket holds the queue");
  stand(&ground, "n3", &["bench"]);
  settle(&ground).await;
  assert_eq!(asks(&ground), 1, "so does a granted one");
  assert_eq!(ground.app.store.item(item.id).unwrap().state, ItemState::Reported);

  let base = support::serve(research_server::routes::router(ground.app.clone())).await;
  let state = ResearchClient::new(&base).with_token("test-token").state().await.unwrap();
  assert_eq!(state.holds.len(), 1);
  let hold = &state.holds[0];
  assert_eq!(
    (hold.registration.as_str(), hold.reason, hold.node_name.as_deref(), hold.reported),
    ("flash-next", HoldReason::Bench, Some("n3"), false)
  );
  let mut held = hold.category_names.clone();
  held.sort();
  assert_eq!(held, ["economy", "it", "security"]);
  assert!(hold.line().contains("n3"), "{}", hold.line());
}

#[tokio::test]
async fn the_queue_waits_on_the_model_s_health_after_the_bench_and_goes_on_once_it_answers() {
  let ground = support::ground(|_| {}).await;
  let item = ground
    .app
    .store
    .ask("it", "example.org/a", "A", "https://example.org/a", Shape::Investigation)
    .unwrap();
  file(&ground, 101, 11);
  stand(&ground, "n3", &["bench"]);
  settle(&ground).await;
  assert_eq!(asks(&ground), 0);
  assert!(
    ground.asked_path("agency", "GET", "/models/").is_empty(),
    "health is not asked under a bench"
  );

  stand(&ground, "n3", &[]);
  ground.world.lock().unwrap().healthy = false;
  settle(&ground).await;
  assert_eq!(asks(&ground), 0, "the model is not back yet");
  let health = ground.asked_path("agency", "GET", "/models/flash-next");
  assert_eq!(health.len(), 1);
  assert!(health[0].path.contains("provider=registered"), "{}", health[0].path);
  let hold = holds(&ground.app);
  assert_eq!((hold[0].reason, hold[0].node_name.as_deref()), (HoldReason::Restore, None));
  assert!(restore_notices(&ground).is_empty(), "the restore wait has not passed");

  ground.world.lock().unwrap().healthy = true;
  settle(&ground).await;
  assert!(holds(&ground.app).is_empty());
  assert_eq!(asks(&ground), 1);
  let reported = ground.app.store.item(item.id).unwrap();
  assert_eq!((reported.state, reported.tries), (ItemState::Reported, 1));
}

#[tokio::test]
async fn a_try_the_bench_cut_waits_again_uncounted_and_runs_once_the_model_is_back() {
  let ground = support::ground(|_| {}).await;
  let item = ground
    .app
    .store
    .ask("it", "example.org/a", "A", "https://example.org/a", Shape::Investigation)
    .unwrap();
  {
    let mut world = ground.world.lock().unwrap();
    world.outcomes.insert(101, "cut".into());
    world.open_polls = u32::MAX;
  }
  file(&ground, 102, 12);

  let carried = tick(&ground.app).await;
  for _ in 0..100 {
    if !ground.asked_path("agency", "GET", "/runs/101").is_empty() {
      break;
    }
    tokio::time::sleep(std::time::Duration::from_millis(10)).await;
  }
  stand(&ground, "n3", &["bench"]);
  ground.world.lock().unwrap().open_polls = 0;
  for handle in carried {
    handle.await.unwrap();
  }

  let waiting = ground.app.store.item(item.id).unwrap();
  assert_eq!((waiting.state, waiting.tries, waiting.retry_at), (ItemState::Waiting, 0, None));
  assert!(waiting.error.unwrap().contains("not counted"));
  let runs = ground.app.store.runs(Some(item.id), None, None, 10).unwrap().items;
  assert_eq!((runs[0].state.as_str(), runs[0].outcome.as_deref()), ("cut", Some("cut")));

  settle(&ground).await;
  assert_eq!(asks(&ground), 1, "nothing runs while the bench stands");

  stand(&ground, "n3", &[]);
  settle(&ground).await;
  assert_eq!(asks(&ground), 2);
  let reported = ground.app.store.item(item.id).unwrap();
  assert_eq!((reported.state, reported.tries), (ItemState::Reported, 1), "the cut try is not counted");
}

#[tokio::test]
async fn a_try_failing_while_the_model_does_not_answer_is_not_counted_and_waits_on_its_health() {
  let ground = support::ground(|s| s.retry_minutes = 0).await;
  let item = ground
    .app
    .store
    .ask("it", "example.org/a", "A", "https://example.org/a", Shape::Investigation)
    .unwrap();
  {
    let mut world = ground.world.lock().unwrap();
    world.run_answers.push_back(503);
    world.healthy = false;
  }
  settle(&ground).await;
  let waiting = ground.app.store.item(item.id).unwrap();
  assert_eq!((waiting.state, waiting.tries), (ItemState::Waiting, 0));
  assert_eq!(holds(&ground.app)[0].reason, HoldReason::Restore);
  settle(&ground).await;
  assert_eq!(asks(&ground), 1, "the queue waits on the health");

  // With the model answering, a refusal is counted as any other failure.
  ground.world.lock().unwrap().healthy = true;
  ground.world.lock().unwrap().run_answers.push_back(503);
  settle(&ground).await;
  assert_eq!(asks(&ground), 2);
  let counted = ground.app.store.item(item.id).unwrap();
  assert_eq!((counted.state, counted.tries), (ItemState::Waiting, 1));
}

#[tokio::test]
async fn a_model_that_stays_down_past_the_restore_wait_is_told_to_the_owner_s_room_once() {
  let ground = support::ground(|s| s.restore_wait_minutes = 0).await;
  stand(&ground, "n3", &["bench"]);
  settle(&ground).await;
  stand(&ground, "n3", &[]);
  ground.world.lock().unwrap().healthy = false;
  for _ in 0..3 {
    settle(&ground).await;
  }

  let told = restore_notices(&ground);
  assert_eq!(told.len(), 1, "told once");
  assert_eq!(told[0]["room"], 7);
  assert_eq!(told[0]["sort"], "notice");
  let expected =
    words::fill(&words::report().restore, &[("model", "flash-next"), ("nodes", "n3"), ("minutes", "0")]);
  assert_eq!(told[0]["said"]["order"], expected.as_str());
  assert!(holds(&ground.app)[0].reported);

  ground.world.lock().unwrap().healthy = true;
  settle(&ground).await;
  assert!(holds(&ground.app).is_empty());
  assert_eq!(restore_notices(&ground).len(), 1);
}
