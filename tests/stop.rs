//! A stop (`docs/runner.md`, 「A stop」) against stand-in agency, artifact and receiver servers: the routes
//! accept no connection once it is asked, a run that ends within `stop_seconds` closes as it would, and a run
//! still under way at the bound is closed cut, its room told once and what it held given back by its owner
//! before the stop ends.

use crate::support;

use std::time::Duration;

use bmo_research_client::*;
use research_server::stop;
use tokio::net::{TcpListener, TcpStream};
use tokio::task::JoinHandle;

/// The routes served under the stop, with their address and the task that ends once the stop has.
async fn served(ground: &support::Ground) -> (std::net::SocketAddr, JoinHandle<Result<(), String>>) {
  let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
  let address = listener.local_addr().unwrap();
  let serving = tokio::spawn(stop::serve(ground.app.clone(), listener));
  (address, serving)
}

/// Waits until the condition holds, at most a few seconds.
async fn until(what: &str, mut holds: impl AsyncFnMut() -> bool) {
  for _ in 0..500 {
    if holds().await {
      return;
    }
    tokio::time::sleep(Duration::from_millis(10)).await;
  }
  panic!("{what} did not come");
}

/// A research run asked through the routes, its first turn held until the test lets it go.
async fn asked_and_held(ground: &support::Ground, address: std::net::SocketAddr, room: Option<i64>) {
  ground.world.lock().unwrap().holding = true;
  ResearchClient::new(format!("http://{address}"))
    .with_token("test-token")
    .ask_research(&ResearchRunAsk {
      key: "search:basic".into(),
      query: "Which engine serves Thing-7B fastest?".into(),
      room_id: room,
      ..ResearchRunAsk::default()
    })
    .await
    .unwrap();
  until("the run's first turn", async || !ground.turns_named("research-1").is_empty()).await;
}

#[tokio::test]
async fn a_run_still_under_way_at_the_bound_is_cut_its_room_told_and_its_holds_given_back() {
  let ground = support::ground(|s| s.stop_seconds = 0).await;
  let (address, serving) = served(&ground).await;
  asked_and_held(&ground, address, Some(5)).await;

  ground.app.stop.ask();
  tokio::time::timeout(Duration::from_secs(10), serving).await.unwrap().unwrap().unwrap();

  let run = ground.app.store.research_run(1).unwrap();
  assert_eq!((run.state, run.outcome.as_deref()), (ResearchRunState::Closed, Some("cut")));
  assert_eq!(run.error.as_deref(), Some("the research unit stopped while the run was under way"));
  let released = ground.world.lock().unwrap().owners_released.clone();
  assert_eq!(released, ["research:places", "research:1"], "every place, and everything the run held");
  let told = ground.asked_path("receiver", "POST", "/instructions");
  assert_eq!(told.len(), 1, "the room is told once, by the stop");
  assert_eq!(told[0].body["room"].as_i64(), Some(5));
  assert!(TcpStream::connect(address).await.is_err(), "the routes accept no connection");
}

#[tokio::test]
async fn a_run_that_ends_within_the_bound_closes_as_it_would_while_no_connection_is_accepted() {
  let ground = support::ground(|s| s.stop_seconds = 600).await;
  let (address, serving) = served(&ground).await;
  asked_and_held(&ground, address, None).await;

  ground.app.stop.ask();
  until("the routes refusing connections", async || TcpStream::connect(address).await.is_err()).await;
  assert!(!serving.is_finished(), "the stop waits for the run under way");
  ground.world.lock().unwrap().holding = false;
  tokio::time::timeout(Duration::from_secs(30), serving).await.unwrap().unwrap().unwrap();

  let run = ground.app.store.research_run(1).unwrap();
  assert_eq!(run.outcome.as_deref(), Some("answered"), "{:?}", run.error);
}
