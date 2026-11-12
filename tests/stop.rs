//! A stop (`docs/runner.md`, 「A stop」) against stand-in agency, artifact and receiver servers: the routes
//! accept no connection once it is asked; a run, a branch's ask and a room's tell under way within
//! `stop_seconds` end as they would; a run still under way at the bound is closed cut, its room told once and
//! what it held given back by its owner before the stop ends; and the daemon as its own process, stopped by
//! SIGTERM in the middle of a deep dive and started again, takes the cut try up again uncounted.

use crate::support;

use std::path::Path;
use std::process::Stdio;
use std::time::Duration;

use bmo_research_client::*;
use research_server::stop;
use research_server::words;
use tokio::net::{TcpListener, TcpStream};
use tokio::process::{Child, Command};
use tokio::task::JoinHandle;

/// The routes served under the stop, with their address and the task that ends once the stop has.
async fn served(ground: &support::Ground) -> (std::net::SocketAddr, JoinHandle<Result<(), String>>) {
  let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
  let address = listener.local_addr().unwrap();
  let serving = tokio::spawn(stop::serve(ground.app.clone(), listener));
  (address, serving)
}

/// Waits until the condition holds, at most `seconds`.
async fn until(what: &str, seconds: u64, mut holds: impl AsyncFnMut() -> bool) {
  for _ in 0..seconds * 100 {
    if holds().await {
      return;
    }
    tokio::time::sleep(Duration::from_millis(10)).await;
  }
  panic!("{what} did not come");
}

fn client(address: std::net::SocketAddr) -> ResearchClient {
  ResearchClient::new(format!("http://{address}")).with_token("test-token")
}

/// A research run asked through the routes, once its first turn is sent.
async fn asked(ground: &support::Ground, address: std::net::SocketAddr, room: Option<i64>) {
  client(address)
    .ask_research(&ResearchRunAsk {
      key: "search:basic".into(),
      query: "Which engine serves Thing-7B fastest?".into(),
      room_id: room,
      ..ResearchRunAsk::default()
    })
    .await
    .unwrap();
  until("the run's first turn", 5, async || !ground.turns_named("research-1").is_empty()).await;
}

async fn refusing(address: std::net::SocketAddr) {
  until("the routes refusing connections", 5, async || {
    TcpStream::connect(address).await.is_err()
  })
  .await;
}

#[tokio::test]
async fn a_run_still_under_way_at_the_bound_is_cut_its_room_told_and_its_holds_given_back() {
  let ground = support::ground(|s| s.stop_seconds = 0).await;
  let (address, serving) = served(&ground).await;
  ground.world.lock().unwrap().holding = true;
  asked(&ground, address, Some(5)).await;

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
  ground.world.lock().unwrap().holding = true;
  asked(&ground, address, None).await;

  ground.app.stop.ask();
  refusing(address).await;
  assert!(!serving.is_finished(), "the stop waits for the run under way");
  ground.world.lock().unwrap().holding = false;
  tokio::time::timeout(Duration::from_secs(30), serving).await.unwrap().unwrap().unwrap();

  let run = ground.app.store.research_run(1).unwrap();
  assert_eq!(run.outcome.as_deref(), Some("answered"), "{:?}", run.error);
}

#[tokio::test]
async fn a_branch_s_ask_under_way_at_the_stop_is_answered_on_its_open_connection() {
  let ground = support::ground(|s| s.stop_seconds = 600).await;
  let (address, serving) = served(&ground).await;
  let run = support::running(&ground, 1);
  ground.world.lock().unwrap().branch_holding = true;
  let asking = tokio::spawn(async move {
    client(address)
      .branch(&BranchAsk {
        run,
        angle: "the prices".into(),
      })
      .await
  });
  until("the branch's turn", 5, async || !ground.turns_named("research-1-branch-1").is_empty()).await;

  ground.app.stop.ask();
  refusing(address).await;
  assert!(!asking.is_finished(), "the branch's turn still holds");
  ground.world.lock().unwrap().branch_holding = false;
  let end = tokio::time::timeout(Duration::from_secs(10), asking).await.unwrap().unwrap().unwrap();
  assert_eq!(
    (end.state, end.notes.as_str()),
    (BranchState::Completed, "notes of research-1-branch-1")
  );
  assert!(!serving.is_finished(), "the stop still waits for the branch's run");
  ground.app.store.close_research_run(run, "answered", None, None).unwrap();
  tokio::time::timeout(Duration::from_secs(10), serving).await.unwrap().unwrap().unwrap();
  assert_eq!(ground.app.store.research_run(run).unwrap().outcome.as_deref(), Some("answered"));
}

#[tokio::test]
async fn a_room_s_tell_in_hand_at_the_stop_is_waited_on_to_its_answer() {
  let ground = support::ground(|s| s.stop_seconds = 0).await;
  {
    let mut world = ground.world.lock().unwrap();
    world.turns_lost.insert(1);
    world.receiver_holding = true;
  }
  let (address, serving) = served(&ground).await;
  asked(&ground, address, Some(5)).await;
  until("the cut run's tell", 5, async || {
    !ground.asked_path("receiver", "POST", "/instructions").is_empty()
  })
  .await;
  let run = ground.app.store.research_run(1).unwrap();
  assert_eq!(run.outcome.as_deref(), Some("cut"), "the run closed itself before its tell");

  ground.app.stop.ask();
  until("the hand-back", 5, async || {
    ground.world.lock().unwrap().owners_released.contains(&"research:places".to_string())
  })
  .await;
  tokio::pin!(serving);
  assert!(
    tokio::time::timeout(Duration::from_millis(200), &mut serving).await.is_err(),
    "the stop does not end while the room's tell is unanswered"
  );
  ground.world.lock().unwrap().receiver_holding = false;
  tokio::time::timeout(Duration::from_secs(10), serving).await.unwrap().unwrap().unwrap();

  let log = ground.app.store.research_log(1).unwrap();
  let told: Vec<&str> =
    log.iter().filter(|line| line.kind == "told").map(|line| line.text.as_str()).collect();
  assert_eq!(told, ["The room was told."], "told once, and answered before the stop ended");
  assert_eq!(ground.asked_path("receiver", "POST", "/instructions").len(), 1);
}

/// What tells the daemon's own process below that it is one.
const DAEMON: &str = "RESEARCH_TEST_DAEMON";

/// The daemon as `bmo-research serve` runs it, in a process of its own: this harness started again on this
/// test alone by the test below, over the data root it names. Run any other way it does nothing, so it never
/// opens the installation's own store.
#[tokio::test(flavor = "multi_thread")]
#[ignore = "the daemon's own process, started by the test that stops it"]
async fn the_daemon() {
  if std::env::var_os(DAEMON).is_some() {
    research_server::daemon::run().await.unwrap();
  }
}

/// The daemon started as its own process over the data root, its log written beside the store.
fn start(root: &Path, start: u32) -> Child {
  let log = std::fs::File::create(root.join(format!("daemon-{start}.log"))).unwrap();
  Command::new(std::env::current_exe().unwrap())
    .args(["--exact", "stop::the_daemon", "--ignored", "--test-threads=1", "--nocapture"])
    .env("BMO_DATA_DIR", root)
    .env_remove("BMO_RESEARCH_SETTINGS")
    .env(DAEMON, "1")
    .stdin(Stdio::null())
    .stdout(log.try_clone().unwrap())
    .stderr(log)
    .kill_on_drop(true)
    .spawn()
    .unwrap()
}

/// The daemon's process stopped by SIGTERM, and its exit once it has drained.
async fn terminate(mut daemon: Child) -> std::process::ExitStatus {
  let pid = i32::try_from(daemon.id().unwrap()).unwrap();
  // SAFETY: `kill` reads its two integers alone; the pid is the child's own, still unreaped.
  assert_eq!(unsafe { libc::kill(pid, libc::SIGTERM) }, 0);
  tokio::time::timeout(Duration::from_secs(30), daemon.wait()).await.unwrap().unwrap()
}

#[tokio::test]
async fn a_daemon_stopped_by_sigterm_in_a_deep_dive_and_started_again_takes_its_try_up_again() {
  let ground = support::ground(|_| {}).await;
  let item = ground
    .app
    .store
    .ask("it", "example.org/a", "A", "https://example.org/a", Shape::Investigation)
    .unwrap()
    .id;
  let heading = words::document().sections.investigation[0].clone();
  {
    let mut world = ground.world.lock().unwrap();
    world.holding = true;
    world.filed.insert(
      2,
      support::Filed {
        id: 702,
        title: "Report A".into(),
        description: "It is a thing.".into(),
        content: format!("# An investigation\n\n## {heading}\n\nIt is a thing.\n"),
      },
    );
  }
  let port = TcpListener::bind("127.0.0.1:0").await.unwrap().local_addr().unwrap().port();
  let settings = &ground.app.settings;
  std::fs::create_dir_all(ground.root.join("research")).unwrap();
  std::fs::write(
    ground.root.join("research/settings.toml"),
    format!(
      "listen = \"127.0.0.1:{port}\"\ntick_seconds = 1\npoll_seconds = 0\nstop_seconds = 0\n\n[artifact]\naddress = \"{}\"\n\n[engine]\naddress = \"{}\"\n\n[tool_host]\nsocket = \"{}\"\n",
      settings.artifact.address, settings.engine.address, settings.tool_host.socket
    ),
  )
  .unwrap();
  let address: std::net::SocketAddr = format!("127.0.0.1:{port}").parse().unwrap();
  let first = start(&ground.root, 1);
  until("the first process's routes", 30, async || TcpStream::connect(address).await.is_ok()).await;
  until("the deep dive's first turn", 30, async || !ground.turns_named("research-1").is_empty()).await;
  let stopped = terminate(first).await;
  assert!(stopped.success(), "the first process drained and ended on SIGTERM: {stopped}");

  let store = &ground.app.store;
  let cut = store.research_run(1).unwrap();
  assert_eq!(
    (cut.state, cut.outcome.as_deref(), cut.error.as_deref()),
    (
      ResearchRunState::Closed,
      Some("cut"),
      Some("the research unit stopped while the run was under way")
    )
  );
  let released = ground.world.lock().unwrap().owners_released.clone();
  assert_eq!(
    released.last().map(String::as_str),
    Some("research:1"),
    "the run's holds given back: {released:?}"
  );
  assert!(released.contains(&"research:places".to_string()));

  ground.world.lock().unwrap().holding = false;
  let second = start(&ground.root, 2);
  until("the cut try taken up again", 30, async || {
    store.research_run(2).is_ok_and(|run| run.state == ResearchRunState::Closed)
  })
  .await;
  let stopped = terminate(second).await;
  assert!(stopped.success(), "the second process drained and ended on SIGTERM: {stopped}");

  assert_eq!(store.research_run(2).unwrap().outcome.as_deref(), Some("answered"));
  let tries = store.runs(Some(item), None, None, 10).unwrap().items;
  let states: Vec<&str> = tries.iter().rev().map(|run| run.state.as_str()).collect();
  assert_eq!(states.first(), Some(&"cut"), "the try the stop cut: {states:?}");
  assert_eq!(tries.len(), 2, "taken up again once: {states:?}");
  assert_eq!(store.item(item).unwrap().tries, 1, "the cut try is not counted");
}
