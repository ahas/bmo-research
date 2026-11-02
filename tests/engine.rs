//! The engine a run needs against stand-in agency, artifact, receiver, tool host and arbiter servers
//! (`docs/api.md`, 「The engine」): a try asks its card before its seat, its seat and sessions stand on the
//! grant and the demand is given back at its end; a try waiting for its card asks no seat and the state says
//! why; what a stopped daemon left held is given back; a branch asks its own card within its run's; a try
//! failing after its card was taken, or after the master stopped holding its demand, waits again uncounted; a card that does not come is told to the owner's
//! room once; a stopped category takes its waiting try's demand back; and no master named asks no demand.

mod support;

use std::time::Duration;

use bmo_research_client::{BranchAsk, BranchState, HoldReason, ItemState, ResearchClient, Running, Shape};
use research_server::dive::tick;
use research_server::engine::{give_back_all, holds};
use research_server::store::{NewResearchRun, now_ms};
use research_server::words;
use serde_json::{Value, json};
use support::{Filed, Ground};

async fn settle(ground: &Ground) {
  for handle in tick(&ground.app).await {
    handle.await.unwrap();
  }
}

// A heading an investigation holds.
fn sections() -> String {
  words::document().sections.investigation[0].clone()
}

fn file(ground: &Ground, run: i64, id: i64) {
  ground.world.lock().unwrap().filed.insert(
    run,
    Filed {
      id,
      title: format!("Report {id}"),
      description: "It is a thing.".into(),
      content: format!("# An investigation\n\n## {}\n\nIt is a thing.\n", sections()),
    },
  );
}

fn ask(ground: &Ground, name: &str) -> i64 {
  let key = format!("example.org/{name}");
  let address = format!("https://{key}");
  ground.app.store.ask("it", &key, name, &address, Shape::Investigation).unwrap().id
}

fn demands_asked(ground: &Ground) -> Vec<Value> {
  ground.asked_path("arbiter", "POST", "/demands").into_iter().map(|a| a.body).collect()
}

fn given_back(ground: &Ground) -> Vec<String> {
  ground
    .asked_path("arbiter", "DELETE", "/demands/")
    .into_iter()
    .map(|a| a.path)
    .collect()
}

fn sessions(ground: &Ground) -> Vec<Value> {
  ground
    .asked_path("agency", "POST", "/sessions")
    .into_iter()
    .filter(|a| a.path == "/sessions")
    .map(|a| a.body)
    .collect()
}

/// Waits until the condition holds, at most a few seconds.
async fn until(what: &str, mut holds: impl FnMut() -> bool) {
  for _ in 0..500 {
    if holds() {
      return;
    }
    tokio::time::sleep(Duration::from_millis(10)).await;
  }
  panic!("{what} did not come");
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
async fn a_try_asks_its_card_first_stands_on_the_grant_and_gives_it_back_at_its_end() {
  let ground = support::ground(|_| {}).await;
  ground.world.lock().unwrap().seat_waits = 1;
  let item = ask(&ground, "a");
  file(&ground, 1, 11);
  settle(&ground).await;

  let asked = demands_asked(&ground);
  assert_eq!(asked.len(), 1, "one demand for the try");
  assert_eq!(
    (
      asked[0]["serviceName"].as_str(),
      asked[0]["count"].as_i64(),
      asked[0]["workKey"].as_str()
    ),
    (Some("lab-arena"), Some(1), Some("research:run:1"))
  );
  assert!(asked[0].get("within").is_none_or(Value::is_null));
  let demand = 4001;
  let all = ground.world.lock().unwrap().asked.clone();
  let first = |unit: &str, method: &str, path: &str| {
    all
      .iter()
      .position(|a| a.unit == unit && a.method == method && a.path.starts_with(path))
      .unwrap()
  };
  assert!(
    first("arbiter", "POST", "/demands") < first("agency", "POST", "/seat-requests"),
    "the card first"
  );

  // The page's review, asked after the run has ended and its card given back, stands on agency's own card.
  let started: Vec<Value> = sessions(&ground)
    .into_iter()
    .filter(|s| s["toolsAllowed"] != serde_json::json!([]))
    .collect();
  assert!(!started.is_empty());
  for session in &started {
    assert_eq!(session["demandId"], demand, "{session}");
  }
  let admissions: Vec<Value> = ground
    .asked_path("agency", "PATCH", "/seat-requests/seat-1")
    .into_iter()
    .map(|a| a.body)
    .filter(|b| b["state"] == "admitted")
    .collect();
  assert!(!admissions.is_empty());
  assert!(
    admissions.iter().all(|a| a == &json!({"state": "admitted", "demandId": demand})),
    "{admissions:?}"
  );

  assert_eq!(given_back(&ground), [format!("/demands/{demand}")]);
  assert!(ground.app.store.demands().unwrap().is_empty());
  let reported = ground.app.store.item(item).unwrap();
  assert_eq!((reported.state, reported.tries), (ItemState::Reported, 1));
}

#[tokio::test]
async fn a_try_waiting_for_its_card_asks_no_seat_and_the_state_says_why() {
  let ground = support::ground(|_| {}).await;
  ground.world.lock().unwrap().cards_busy = true;
  let item = ask(&ground, "a");
  file(&ground, 1, 11);
  let carried = tick(&ground.app).await;
  until("the hold", || holds(&ground.app).first().is_some_and(|h| h.demand_id.is_some())).await;

  assert!(ground.run_seats().is_empty(), "no seat while the card waits");
  let base = support::serve(research_server::routes::router(ground.app.clone())).await;
  let state = ResearchClient::new(&base).with_token("test-token").state().await.unwrap();
  assert_eq!(state.holds.len(), 1);
  let hold = &state.holds[0];
  assert_eq!(
    (hold.reason, hold.run_id, hold.category_name.as_str(), hold.registration.as_str()),
    (HoldReason::WaitingCard, 1, "it", "flash-next")
  );
  assert_eq!(
    (hold.service_name.as_str(), hold.demand_id, hold.cause.as_deref(), hold.reported),
    ("lab-arena", Some(4001), Some("in-use"), false)
  );
  assert!(
    hold.line().contains("lab-arena") && hold.line().contains("Every card is in use"),
    "{}",
    hold.line()
  );
  let it = state.categories.iter().find(|c| c.category_name == "it").unwrap();
  assert_eq!(it.running_count, 1, "a try waiting for its card counts as running");
  assert_eq!(ground.app.store.item(item).unwrap().state, ItemState::Running);

  ground.world.lock().unwrap().grant_cards();
  for handle in carried {
    handle.await.unwrap();
  }
  assert!(holds(&ground.app).is_empty());
  assert_eq!(ground.run_seats().len(), 1);
  assert_eq!(demands_asked(&ground).len(), 1, "the same demand waited on");
  assert_eq!(ground.app.store.item(item).unwrap().state, ItemState::Reported);
  assert_eq!(given_back(&ground), ["/demands/4001"]);
}

#[tokio::test]
async fn demands_a_stopped_daemon_left_held_are_given_back() {
  let ground = support::ground(|_| {}).await;
  let store = &ground.app.store;
  store.keep_demand(77, Some(3), Some(9), None, "lab-arena").unwrap();
  store.keep_demand(78, None, Some(9), Some(77), "lab-arena").unwrap();
  assert_eq!(give_back_all(&ground.app).await, 2);
  assert_eq!(given_back(&ground), ["/demands/77", "/demands/78"]);
  assert!(
    store.demands().unwrap().is_empty(),
    "a demand the master no longer holds is given back too"
  );

  // A pass of the loop gives back what an ended try left held.
  store.keep_demand(79, Some(42), None, None, "lab-arena").unwrap();
  settle(&ground).await;
  assert_eq!(given_back(&ground).last().map(String::as_str), Some("/demands/79"));
  assert!(store.demands().unwrap().is_empty());
}

#[tokio::test]
async fn a_branch_of_a_run_holding_a_card_asks_its_own_within_the_run_s() {
  let ground = support::ground(|_| {}).await;
  let base = support::serve(research_server::routes::router(ground.app.clone())).await;
  let client = ResearchClient::new(&base).with_token("test-token");
  let store = &ground.app.store;
  let run = store
    .open_research_run(&NewResearchRun {
      key: "search:thorough",
      line: "search",
      query: "Q",
      asker: None,
      options: None,
      room_id: None,
      provider: "registered",
      model: "flash-next",
      closes_at: now_ms() + 60_000,
      branches_allowed: 2,
      tells: false,
    })
    .unwrap();
  let record =
    json!({"run": run, "source": "search", "role": "scout", "branches": 2, "pages": 8, "branchPages": 4});
  store.keep_research_road(run, "registered", "flash-next", &record).unwrap();
  store.research_running(run).unwrap();
  store.keep_demand(3000, Some(5), Some(run), None, "lab-arena").unwrap();
  ground.world.lock().unwrap().seat_waits = 1;

  let end = client
    .branch(&BranchAsk {
      run,
      angle: "the prices".into(),
    })
    .await
    .unwrap();
  assert_eq!(end.state, BranchState::Completed);

  let asked = demands_asked(&ground);
  assert_eq!(asked.len(), 1);
  assert_eq!(
    (
      asked[0]["serviceName"].as_str(),
      asked[0]["within"].as_i64(),
      asked[0]["count"].as_i64()
    ),
    (Some("lab-arena"), Some(3000), Some(1))
  );
  assert_eq!(asked[0]["workKey"], format!("research:{run}:branch:1"));
  let branch = 4001;
  assert_eq!(sessions(&ground)[0]["demandId"], branch);
  let admitted: Vec<Value> = ground
    .asked_path("agency", "PATCH", "/seat-requests/seat-1")
    .into_iter()
    .map(|a| a.body)
    .filter(|b| b["state"] == "admitted")
    .collect();
  assert!(!admitted.is_empty());
  assert!(
    admitted.iter().all(|a| a == &json!({"state": "admitted", "demandId": branch})),
    "{admitted:?}"
  );
  assert_eq!(given_back(&ground), [format!("/demands/{branch}")], "the branch's own, not the run's");
  assert_eq!(store.demands().unwrap(), [3000]);
  assert_eq!(store.run_demand(run).unwrap(), Some(3000));
}

/// A branch's card still not granted at research's close is waited for however long the arbiter takes, while
/// the run is running.
#[tokio::test]
async fn a_branch_s_card_granted_after_research_closed_still_opens_the_branch() {
  let ground = support::ground(|_| {}).await;
  let base = support::serve(research_server::routes::router(ground.app.clone())).await;
  let client = ResearchClient::new(&base).with_token("test-token");
  let store = &ground.app.store;
  let closes_at = now_ms() + 200;
  let run = store
    .open_research_run(&NewResearchRun {
      key: "search:thorough",
      line: "search",
      query: "Q",
      asker: None,
      options: None,
      room_id: None,
      provider: "registered",
      model: "flash-next",
      closes_at,
      branches_allowed: 2,
      tells: false,
    })
    .unwrap();
  let record =
    json!({"run": run, "source": "search", "role": "scout", "branches": 2, "pages": 8, "branchPages": 4});
  store.keep_research_road(run, "registered", "flash-next", &record).unwrap();
  store.research_running(run).unwrap();
  store.keep_demand(3000, Some(5), Some(run), None, "lab-arena").unwrap();
  ground.world.lock().unwrap().cards_busy = true;

  let asked = tokio::spawn(async move {
    client
      .branch(&BranchAsk {
        run,
        angle: "the prices".into(),
      })
      .await
  });
  until("the branch's demand", || !demands_asked(&ground).is_empty()).await;
  while now_ms() <= closes_at + 300 {
    tokio::time::sleep(Duration::from_millis(20)).await;
  }
  assert!(!asked.is_finished(), "still waiting for its card past the close");
  ground.world.lock().unwrap().grant_cards();

  let end = asked.await.unwrap().unwrap();
  assert_eq!(end.state, BranchState::Stopped, "{:?}", end.reason);
  assert_eq!(sessions(&ground)[0]["demandId"], 4001);
  assert_eq!(given_back(&ground), ["/demands/4001"]);
}

#[tokio::test]
async fn a_try_failing_after_its_card_was_taken_waits_again_uncounted() {
  let ground = support::ground(|s| s.retry_minutes = 60).await;
  let item = ask(&ground, "a");
  {
    let mut world = ground.world.lock().unwrap();
    world.failing.insert(1);
    world.cards_taken = true;
  }
  settle(&ground).await;
  let waiting = ground.app.store.item(item).unwrap();
  assert_eq!((waiting.state, waiting.tries, waiting.retry_at), (ItemState::Waiting, 0, None));
  assert!(waiting.error.unwrap().contains("not counted"));
  assert_eq!(given_back(&ground), ["/demands/4001"]);

  // With its card standing, a failure is counted as any other.
  ground.world.lock().unwrap().cards_taken = false;
  ground.world.lock().unwrap().failing.insert(2);
  settle(&ground).await;
  let counted = ground.app.store.item(item).unwrap();
  assert_eq!((counted.state, counted.tries), (ItemState::Waiting, 1));
  assert!(counted.retry_at.is_some());
}

#[tokio::test]
async fn a_try_failing_after_the_master_stopped_holding_its_demand_waits_again_uncounted() {
  let ground = support::ground(|s| s.retry_minutes = 60).await;
  let item = ask(&ground, "a");
  {
    let mut world = ground.world.lock().unwrap();
    world.failing.insert(1);
    world.holding = true;
  }
  let handles = tick(&ground.app).await;
  for _ in 0..500 {
    if !ground.turns_named("research-1").is_empty() {
      break;
    }
    tokio::time::sleep(Duration::from_millis(10)).await;
  }
  {
    // The master restarted under the run: it holds no demand of it any longer.
    let mut world = ground.world.lock().unwrap();
    world.demands.clear();
    world.holding = false;
  }
  for handle in handles {
    handle.await.unwrap();
  }
  let waiting = ground.app.store.item(item).unwrap();
  assert_eq!((waiting.state, waiting.tries, waiting.retry_at), (ItemState::Waiting, 0, None));
  let error = waiting.error.unwrap();
  assert!(error.contains("no longer holds") && error.contains("not counted"), "{error}");
}

#[tokio::test]
async fn a_card_that_does_not_come_is_told_to_the_owner_s_room_once() {
  let ground = support::ground(|s| s.restore_wait_minutes = 0).await;
  ground.world.lock().unwrap().cards_busy = true;
  ask(&ground, "a");
  file(&ground, 1, 11);
  let carried = tick(&ground.app).await;
  until("the notice", || !restore_notices(&ground).is_empty()).await;
  until("another reading", || ground.asked_path("arbiter", "GET", "/demands/4001").len() >= 2).await;

  let told = restore_notices(&ground);
  assert_eq!(told.len(), 1, "told once");
  assert_eq!((told[0]["room"].as_i64(), told[0]["sort"].as_str()), (Some(7), Some("notice")));
  let expected = words::fill(
    &words::report().restore,
    &[("model", "flash-next"), ("service", "lab-arena"), ("minutes", "0")],
  );
  assert_eq!(told[0]["said"]["order"], expected.as_str());
  assert!(holds(&ground.app)[0].reported);
  assert!(ground.run_seats().is_empty(), "the demand goes on waiting");

  ground.world.lock().unwrap().grant_cards();
  for handle in carried {
    handle.await.unwrap();
  }
  assert_eq!(restore_notices(&ground).len(), 1);
  assert_eq!(ground.run_seats().len(), 1);
}

#[tokio::test]
async fn a_stopped_category_gives_back_the_card_its_waiting_try_asked() {
  let ground = support::ground(|_| {}).await;
  ground.world.lock().unwrap().cards_busy = true;
  let item = ask(&ground, "a");
  let carried = tick(&ground.app).await;
  until("the hold", || holds(&ground.app).first().is_some_and(|h| h.demand_id.is_some())).await;

  ground.app.store.set_running(Some("it"), Running::Stopped).unwrap();
  for handle in carried {
    handle.await.unwrap();
  }
  assert!(holds(&ground.app).is_empty());
  assert_eq!(given_back(&ground), ["/demands/4001"]);
  assert!(ground.app.store.demands().unwrap().is_empty());
  assert!(ground.run_seats().is_empty());
  let waiting = ground.app.store.item(item).unwrap();
  assert_eq!((waiting.state, waiting.tries), (ItemState::Waiting, 0));
  assert_eq!(ground.app.store.running_count().unwrap(), 0);
}

#[tokio::test]
async fn with_no_master_named_a_try_asks_no_card() {
  let ground = support::ground(|s| s.engine.address = String::new()).await;
  let item = ask(&ground, "a");
  file(&ground, 1, 11);
  settle(&ground).await;
  assert!(ground.asked("arbiter").is_empty());
  assert!(sessions(&ground).iter().all(|s| s.get("demandId").is_none()));
  assert_eq!(ground.app.store.item(item).unwrap().state, ItemState::Reported);
}
