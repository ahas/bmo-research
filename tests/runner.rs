//! The research runner against stand-in agency, artifact, receiver and tool host servers (`docs/runner.md`,
//! `docs/api.md`, 「research runs」 and 「research settings」): a run end to end — its slot, its session's
//! start with its record and capabilities, its first turn, its rounds, its filing and its room's
//! instruction — a run agency refuses, a turn waited on however long it holds and through a session still
//! in a turn, the branches and their allowance, the settings' round trip, a summary a result still writes
//! folded into its document's answer, a turn's text read whole, and a room's document given its narration's
//! sound at its head before its room is told.

use crate::support;

use std::time::Duration;

use bmo_research_client::*;
use reqwest::StatusCode;
use research_server::document::draw;
use research_server::runner::{finish_told, written};
use research_server::store::{NewResearchRun, now_ms};
use research_server::words;
use serde_json::{Value, json};

async fn served() -> (support::Ground, String, ResearchClient) {
  let ground = support::ground(|_| {}).await;
  let base = support::serve(research_server::routes::router(ground.app.clone())).await;
  let client = ResearchClient::new(&base).with_token("test-token");
  (ground, base, client)
}

async fn raw(base: &str, method: reqwest::Method, path: &str, body: Value) -> (StatusCode, Value) {
  let answer = reqwest::Client::new()
    .request(method, format!("{base}{path}"))
    .bearer_auth("test-token")
    .json(&body)
    .send()
    .await
    .unwrap();
  let status = answer.status();
  (status, answer.json().await.unwrap_or_default())
}

/// The run once it has closed and, where it tells its room, told it.
async fn closed(ground: &support::Ground, run: i64) -> ResearchRunView {
  for _ in 0..1500 {
    let row = ground.app.store.research_run(run).unwrap();
    let told = || ground.app.store.research_log(run).unwrap().iter().any(|line| line.kind == "told");
    if row.state == ResearchRunState::Closed && (!row.tells || told()) {
      return row.view();
    }
    tokio::time::sleep(Duration::from_millis(10)).await;
  }
  panic!("research run {run} did not close");
}

fn sessions(ground: &support::Ground) -> Vec<Value> {
  ground
    .asked_path("agency", "POST", "/sessions")
    .into_iter()
    .filter(|a| a.path == "/sessions")
    .map(|a| a.body)
    .collect()
}

#[tokio::test]
async fn a_search_is_let_in_sent_asked_its_rounds_filed_and_told_to_its_room() {
  let (ground, _base, client) = served().await;
  let asked = client
    .ask_research(&ResearchRunAsk {
      key: "search:thorough".into(),
      query: "Which engine serves Thing-7B fastest?".into(),
      asker: Some("the owner".into()),
      room_id: Some(5),
      ..ResearchRunAsk::default()
    })
    .await
    .unwrap();
  assert_eq!(
    (asked.run_id, asked.state, asked.provider.as_str()),
    (1, ResearchRunState::Running, "registered")
  );
  assert_eq!(asked.model, "slot-model", "the slot's model where the ask names none");

  let run = closed(&ground, 1).await;
  assert_eq!((run.outcome.as_deref(), run.document_id), (Some("answered"), Some(901)));
  assert_eq!(run.link.as_deref(), Some("https://share.example/d901"));
  let rounds = &words::runner().rounds["search:thorough"].rounds;
  let mut turns = vec!["research-1".to_string()];
  turns.extend(rounds.iter().map(|round| format!("research-1-{round}")));
  turns.push("research-1-narration".to_string());
  assert_eq!(run.turns, turns, "a result with no script is asked for its narration");
  assert!(
    run.pending.is_empty() && run.narrations.is_empty(),
    "a turn that wrote none leaves no sound"
  );

  let slots = ground.run_slots();
  assert_eq!(slots.len(), 1);
  assert_eq!(
    slots[0],
    json!({"requestKey": "research:1", "owner": "research:1", "role": "voyager", "source": "search"}),
    "a slot of the search line's role on its role's own road"
  );

  let started = sessions(&ground);
  assert_eq!(started.len(), 2, "the first answer's session and the rounds'");
  let first = &started[0];
  assert_eq!(
    (first["scope"].as_str(), first["slot"].as_str(), first["line"].as_str()),
    (Some("role"), Some("slot-1"), Some("search"))
  );
  for field in ["agent", "role", "provider", "model", "cwd"] {
    assert!(first.get(field).is_none(), "a role's session runs on its slot and names no {field}");
  }
  assert_eq!(started[1]["slot"], "slot-1", "the rounds' session runs on the run's slot too");
  assert!(first.get("boundMs").is_none(), "no turn of a run is cut");
  let system = first["systemPrompt"].as_str().unwrap();
  assert!(
    system.find("- web_search").unwrap() < system.find("- site_prices").unwrap(),
    "the listing by its order"
  );
  assert!(!system.contains("{tools}"));
  assert!(system.contains("RULEBOOK prompts/rules/search-kinds.md:\n"));
  assert!(system.contains(
    "RULEBOOK prompts/rules/product-search.md (sections: Whether this rulebook applies at all):"
  ));
  assert!(system.contains("RULEBOOK prompts/rules/self-preservation.md:\n"));
  assert!(system.contains("research/rulebook"));
  let record = &first["labels"]["research"];
  assert!(record.get("closesAt").is_none(), "the web tools hold the run to no closing time");
  for (key, value) in [
    ("run", json!(1)),
    ("owner", json!("the owner")),
    ("source", json!("search")),
    ("role", json!("voyager")),
    ("branches", json!(2)),
    ("pages", json!(8)),
    ("branchPages", json!(4)),
    ("chat", json!(5)),
    ("strategy", json!("direct-then-browser")),
    ("provider", json!("registered")),
    ("model", json!("slot-model")),
    ("branchesToken", json!("test-token")),
  ] {
    assert_eq!(record[key], value, "{key}");
  }
  assert!(record["branchesAt"].as_str().unwrap().starts_with("http://"));
  assert_eq!(record["rulebooks"], json!(words::runner().search.on_request));
  let allowed: Vec<&str> =
    first["toolsAllowed"].as_array().unwrap().iter().map(|c| c.as_str().unwrap()).collect();
  assert!(allowed.contains(&"web.branches") && allowed.contains(&"web.rulebooks"), "{allowed:?}");

  let rounds_session = &started[1];
  assert_eq!(rounds_session["labels"]["research"]["branches"], 0);
  assert!(rounds_session["labels"]["research"].get("branchesAt").is_none());
  assert!(rounds_session["labels"]["research"].get("branchesToken").is_none());
  let allowed: Vec<&str> = rounds_session["toolsAllowed"]
    .as_array()
    .unwrap()
    .iter()
    .map(|c| c.as_str().unwrap())
    .collect();
  assert!(!allowed.contains(&"web.branches") && !allowed.contains(&"web.rulebooks"), "{allowed:?}");

  let turn = &ground.turns_named("research-1")[0];
  assert!(turn["prompt"].as_str().unwrap().contains("Which engine serves Thing-7B fastest?"));
  assert!(
    turn["prompt"].as_str().unwrap().contains("thorough")
      && turn["prompt"].as_str().unwrap().contains("up to 2 branches")
  );
  for part in ["provider", "model", "mapping", "workKey", "cwd"] {
    assert!(turn.get(part).is_none(), "a turn's road is its session's, so it names no {part}");
  }
  assert_eq!(
    ground.asked_path("agency", "POST", "/sessions/voyager@run:1/turns").len(),
    1,
    "the turn goes to the session by the id agency named"
  );
  assert_eq!(
    (turn["resume"]["kind"].as_str(), turn["chat"].as_i64(), turn["hearsWords"].as_bool()),
    (Some("fresh"), Some(5), Some(false))
  );
  let last = &ground.turns_named("research-1-final")[0];
  let prompt = last["prompt"].as_str().unwrap();
  assert!(prompt.starts_with(&words::runner().questions["final"]));
  assert!(
    prompt.contains("## The task")
      && prompt.contains("## What you wrote last")
      && prompt.contains("Run 1 answered.")
  );

  let moved: Vec<Value> = ground
    .asked_path("agency", "PATCH", "/slot-requests/slot-1")
    .into_iter()
    .map(|a| a.body)
    .collect();
  assert!(moved.iter().all(|m| m["state"] != "bound"), "this unit binds no slot: {moved:?}");
  assert_eq!(moved.last().unwrap(), &json!({"state": "finished", "outcome": "answered"}));
  let ended: Vec<String> = ground
    .asked_path("agency", "DELETE", "/sessions/")
    .into_iter()
    .map(|a| a.path)
    .collect();
  assert_eq!(ended.len(), 2, "both sessions ended");
  let started: Vec<Value> = ground
    .asked_path("agency", "POST", "/sessions")
    .into_iter()
    .filter(|a| a.path == "/sessions")
    .map(|a| a.body)
    .collect();
  assert_eq!(
    started.iter().map(|s| s["slot"].clone()).collect::<Vec<_>>(),
    [json!("slot-1"), json!("slot-1")],
    "the rounds' session starts on the run's slot, which its start moves onto it"
  );
  let order: Vec<String> = ground
    .asked("agency")
    .into_iter()
    .filter(|a| {
      (a.method == "DELETE" && a.path.starts_with("/sessions/"))
        || (a.method == "POST" && a.path == "/sessions")
        || (a.path == "/slot-requests/slot-1" && a.body["state"] == "finished")
    })
    .map(|a| format!("{} {}", a.method, a.path))
    .collect();
  assert_eq!(
    order,
    [
      "POST /sessions",
      "POST /sessions",
      "DELETE /sessions/voyager@run:1",
      "PATCH /slot-requests/slot-1",
      "DELETE /sessions/voyager@run:2",
    ],
    "the first session is ended once the rounds' session stands on the slot, and the slot is given back \
     before the session it is bound to then is ended"
  );

  let filed = ground.run_documents();
  assert_eq!(filed.len(), 1);
  assert_eq!(
    (filed[0]["title"].as_str(), filed[0]["kind"].as_str()),
    (Some("Research 1"), Some("search"))
  );
  assert_eq!(filed[0]["labels"]["research"], "1");
  assert_eq!(filed[0]["labels"]["key"], "search:thorough");
  assert_eq!(filed[0]["scope"], "5");
  let body = filed[0]["body"].as_str().unwrap();
  assert!(
    body.contains("## The answer")
      && body.contains("[the answer](https://example.org/1)")
      && !body.contains("ANSWERED:"),
    "{body}"
  );
  let shared = ground.asked_path("artifact", "POST", "/shares");
  assert!(shared[0].body["expiresAt"].as_i64().unwrap() > now_ms() + 23 * 3_600_000);

  let told: Vec<Value> = ground
    .asked_path("receiver", "POST", "/instructions")
    .into_iter()
    .map(|a| a.body)
    .collect();
  assert_eq!(told.len(), 1);
  assert_eq!(
    (told[0]["room"].as_i64(), told[0]["kind"].as_str(), told[0]["sort"].as_str()),
    (Some(5), Some("research-ended:1"), Some("notice"))
  );
  let order = told[0]["said"]["order"].as_str().unwrap();
  assert!(
    order.contains("https://share.example/d901") && order.contains("Run 1 answered."),
    "{order}"
  );
  assert!(told[0].get("run").is_none(), "no ledger run field is sent");

  let log: Vec<String> = client.research_log(1).await.unwrap().items.into_iter().map(|l| l.kind).collect();
  for kind in ["asked", "admitted", "answer", "round", "filed", "ended", "told"] {
    assert!(log.contains(&kind.to_string()), "{kind} missing from {log:?}");
  }
  assert_eq!(client.research_run(1).await.unwrap(), run);
}

#[tokio::test]
async fn a_slot_s_admission_answered_past_half_a_minute_still_lets_its_run_in() {
  let (ground, _base, client) = served().await;
  {
    let mut world = ground.world.lock().unwrap();
    world.slot_waits = 1;
    world.admission_wait_ms = 31_000;
  }
  let asked = client
    .ask_research(&ResearchRunAsk {
      key: "search:thorough".into(),
      query: "Which engine serves Thing-7B fastest?".into(),
      ..ResearchRunAsk::default()
    })
    .await
    .unwrap();
  assert_eq!(asked.state, ResearchRunState::Waiting);

  let mut row = ground.app.store.research_run(asked.run_id).unwrap();
  for _ in 0..1_200 {
    if row.state == ResearchRunState::Closed {
      break;
    }
    tokio::time::sleep(Duration::from_millis(50)).await;
    row = ground.app.store.research_run(asked.run_id).unwrap();
  }
  let run = row.view();
  assert_eq!(run.outcome.as_deref(), Some("answered"), "{:?}", run.error);
  let admissions = ground
    .asked_path("agency", "PATCH", "/slot-requests/slot-1")
    .into_iter()
    .filter(|a| a.body == json!({}))
    .count();
  assert_eq!(admissions, 2, "the slow admission waited on, then the one that let the slot in");
}

#[tokio::test]
async fn a_run_naming_its_road_and_no_room_is_told_to_none_and_a_browse_carries_no_rulebook() {
  let (ground, _base, client) = served().await;
  let asked = client
    .ask_research(&ResearchRunAsk {
      key: "browse".into(),
      query: "Open the shop's page.".into(),
      provider: Some("deepseek".into()),
      model: Some("deepseek-flash".into()),
      ..ResearchRunAsk::default()
    })
    .await
    .unwrap();
  closed(&ground, asked.run_id).await;
  assert_eq!(
    ground.run_slots()[0],
    json!({"requestKey": "research:1", "owner": "research:1", "role": "seeker", "source": "browse", "provider": "deepseek", "model": "deepseek-flash"}),
    "a browse's slot of the browse line's role, on the road its ask names"
  );
  let first = &sessions(&ground)[0];
  assert_eq!((first["slot"].as_str(), first["line"].as_str()), (Some("slot-1"), Some("browse")));
  assert_eq!(
    (
      first["labels"]["research"]["provider"].as_str(),
      first["labels"]["research"]["model"].as_str()
    ),
    (Some("deepseek"), Some("deepseek-flash")),
    "the run's road is its slot's, the one its ask named"
  );
  assert!(
    first["systemPrompt"]
      .as_str()
      .unwrap()
      .contains("RULEBOOK prompts/rules/self-preservation.md")
  );
  assert_eq!(first["labels"]["research"]["rulebooks"], json!([]));
  let allowed: Vec<&str> =
    first["toolsAllowed"].as_array().unwrap().iter().map(|c| c.as_str().unwrap()).collect();
  assert!(!allowed.contains(&"web.branches") && !allowed.contains(&"web.rulebooks"), "{allowed:?}");
  let told = ground.asked_path("receiver", "POST", "/instructions");
  assert!(told[0].body["room"].is_null(), "the receiver hands it the owner's private room");
}

#[tokio::test]
async fn a_run_agency_refuses_is_answered_503_and_leaves_nothing_standing() {
  let (ground, base, _client) = served().await;
  ground.world.lock().unwrap().slot_answers.push_back(503);
  let (status, said) = raw(
    &base,
    reqwest::Method::POST,
    "/research-runs",
    json!({"key": "search:basic", "query": "What does it cost?"}),
  )
  .await;
  assert_eq!((status, said["code"].as_str()), (StatusCode::SERVICE_UNAVAILABLE, Some("unavailable")));
  assert!(said["error"].as_str().unwrap().contains("line"), "{said}");
  let run = ground.app.store.research_run(1).unwrap();
  assert_eq!((run.state, run.outcome.as_deref()), (ResearchRunState::Closed, Some("failed")));
  assert!(ground.app.store.holds(1).unwrap().is_empty());
  assert!(sessions(&ground).is_empty(), "no session without a slot");

  // A session agency cannot start once the slot is let in cuts the run, its slot given back.
  ground.world.lock().unwrap().session_answers.push_back(503);
  let (status, said) = raw(
    &base,
    reqwest::Method::POST,
    "/research-runs",
    json!({"key": "search:basic", "query": "What does it cost?"}),
  )
  .await;
  assert_eq!(status, StatusCode::ACCEPTED, "{said}");
  let run = closed(&ground, 2).await;
  assert_eq!(run.outcome.as_deref(), Some("cut"), "{:?}", run.error);
  assert!(run.error.as_deref().unwrap_or_default().contains("no session"), "{:?}", run.error);
  let given_back = ground.asked_path("agency", "PATCH", "/slot-requests/slot-1");
  assert_eq!(given_back.last().unwrap().body["state"], "finished");
  assert!(ground.app.store.holds(2).unwrap().is_empty());

  ground.world.lock().unwrap().no_listing = true;
  let (status, _) = raw(
    &base,
    reqwest::Method::POST,
    "/research-runs",
    json!({"key": "search:basic", "query": "Q"}),
  )
  .await;
  assert_eq!(status, StatusCode::SERVICE_UNAVAILABLE, "a search with no web listing is not asked");

  for body in [
    json!({"key": "search:deep", "query": "Q"}),
    json!({"key": "search:basic", "query": " "}),
    json!({"key": "search:basic", "query": "Q", "provider": "deepseek"}),
  ] {
    let (status, said) = raw(&base, reqwest::Method::POST, "/research-runs", body).await;
    assert_eq!((status, said["code"].as_str()), (StatusCode::BAD_REQUEST, Some("bad-request")));
  }
  let (status, _) = raw(&base, reqwest::Method::GET, "/research-runs/99", json!(null)).await;
  assert_eq!(status, StatusCode::NOT_FOUND);
}

/// A turn that holds is waited on until it answers and its answer kept (`docs/runner.md`, 「Nothing of a run
/// is clocked」): its send carries no clock, no session names a bound, and the record names no closing time.
#[tokio::test]
async fn a_turn_that_holds_is_waited_on_until_it_answers() {
  let (ground, _base, client) = served().await;
  ground.world.lock().unwrap().holding = true;
  client
    .ask_research(&ResearchRunAsk {
      key: "search:basic".into(),
      query: "Which engine serves Thing-7B fastest?".into(),
      ..ResearchRunAsk::default()
    })
    .await
    .unwrap();
  while ground.turns_named("research-1").is_empty() {
    tokio::time::sleep(Duration::from_millis(10)).await;
  }
  tokio::time::sleep(Duration::from_millis(300)).await;
  assert_eq!(ground.app.store.research_run(1).unwrap().state, ResearchRunState::Running);
  ground.world.lock().unwrap().holding = false;

  let run = closed(&ground, 1).await;
  assert_eq!(run.outcome.as_deref(), Some("answered"), "{:?}", run.error);
  assert_eq!(run.turns, ["research-1", "research-1-final", "research-1-narration"]);
  let started = sessions(&ground);
  assert!(!started.is_empty() && started.iter().all(|s| s.get("boundMs").is_none()), "{started:?}");
  assert!(started.iter().all(|s| s["labels"]["research"].get("closesAt").is_none()), "{started:?}");
}

/// A turn sent into a session still in a turn an earlier send lost is answered busy, waits for that turn's
/// end, reading the session's state until it names no run, and is sent again (`docs/runner.md`, 「A turn's
/// send」).
#[tokio::test]
async fn a_turn_sent_into_a_session_still_in_a_turn_waits_for_its_end_and_is_sent_again() {
  let (ground, _base, client) = served().await;
  {
    let mut world = ground.world.lock().unwrap();
    world.busy_turns = 1;
    world.turn_running_reads = 1;
  }
  client
    .ask_research(&ResearchRunAsk {
      key: "search:basic".into(),
      query: "Which engine serves Thing-7B fastest?".into(),
      ..ResearchRunAsk::default()
    })
    .await
    .unwrap();

  let run = closed(&ground, 1).await;
  assert_eq!(run.outcome.as_deref(), Some("answered"), "{:?}", run.error);
  assert_eq!(ground.turns_named("research-1").len(), 2, "answered busy, then sent again once free");
  let reads: Vec<String> =
    ground.asked_path("agency", "GET", "/sessions/").into_iter().map(|a| a.path).collect();
  assert_eq!(
    reads,
    ["/sessions/voyager@run:1", "/sessions/voyager@run:1"],
    "read until it named no run"
  );
  let log = ground.app.store.research_log(1).unwrap();
  assert!(log.iter().all(|line| !line.text.contains("session-busy")), "no turn failed busy");
}

/// A running run allowed `branches`.
fn running(ground: &support::Ground, branches: i64) -> i64 {
  let store = &ground.app.store;
  let run = store
    .open_research_run(&NewResearchRun {
      key: "search:thorough",
      line: "search",
      query: "Q",
      asker: None,
      options: None,
      room_id: Some(5),
      provider: "registered",
      model: "flash-next",
      branches_allowed: branches,
      tells: true,
    })
    .unwrap();
  let record = json!({"run": run, "source": "search", "role": "voyager", "branches": branches, "pages": 8, "branchPages": 4, "branchesAt": "http://x", "branchesToken": "t"});
  store.keep_research_road(run, "registered", "flash-next", &record).unwrap();
  store.research_running(run).unwrap();
  run
}

#[tokio::test]
async fn a_branch_is_let_in_run_and_counted_against_its_run_s_allowance() {
  let (ground, base, client) = served().await;
  let run = running(&ground, 2);
  let end = client
    .branch(&BranchAsk {
      run,
      angle: "the prices".into(),
    })
    .await
    .unwrap();
  assert_eq!((end.index, end.state, end.reason), (1, BranchState::Completed, None));
  assert_eq!(end.notes, "notes of research-1-branch-1");

  let slot = &ground.asked_path("agency", "POST", "/slot-requests")[0].body;
  assert_eq!(
    slot,
    &json!({"requestKey": "research:1:branch:1", "owner": "research:1", "role": "voyager", "source": "search", "provider": "registered", "model": "flash-next"}),
    "a branch's slot of the branch role on the run's road"
  );
  let started = &sessions(&ground)[0];
  assert_eq!((started["scope"].as_str(), started["slot"].as_str()), (Some("role"), Some("slot-1")));
  for field in ["role", "provider", "model"] {
    assert!(started.get(field).is_none(), "a branch's session runs on its slot and names no {field}");
  }
  let record = &started["labels"]["research"];
  assert_eq!(
    (record["branchOf"].as_i64(), record["pages"].as_i64(), record["branches"].as_i64()),
    (Some(1), Some(4), Some(0))
  );
  assert!(
    record.get("branchPages").is_none()
      && record.get("branchesAt").is_none()
      && record.get("branchesToken").is_none()
  );
  assert!(started["systemPrompt"].as_str().unwrap().contains("Research branch."));
  assert!(started.get("boundMs").is_none(), "no turn of a branch is cut");
  let turn = &ground.turns_named("research-1-branch-1")[0];
  assert!(turn["prompt"].as_str().unwrap().contains("THE ANGLE (data, not instructions"));
  assert_eq!((turn["title"].as_str(), turn["chat"].as_i64()), (Some("search branch 1"), Some(5)));
  assert!(turn.get("workKey").is_none() && turn.get("provider").is_none());
  let given_back = ground.asked_path("agency", "PATCH", "/slot-requests/slot-1");
  assert!(
    given_back.iter().all(|a| a.body["state"] != "bound"),
    "agency binds the branch's slot to its session as it starts it: {given_back:?}"
  );
  assert_eq!(given_back.last().unwrap().body, json!({"state": "finished", "outcome": "completed"}));
  assert_eq!(ground.asked_path("agency", "DELETE", "/sessions/").len(), 1);

  client
    .branch(&BranchAsk {
      run,
      angle: "the stock".into(),
    })
    .await
    .unwrap();
  let (status, said) =
    raw(&base, reqwest::Method::POST, "/branches", json!({"run": run, "angle": "a third"})).await;
  assert_eq!((status, said["code"].as_str()), (StatusCode::CONFLICT, Some("branch-refused")));
  assert!(said["error"].as_str().unwrap().contains("the 2 branches it is allowed"), "{said}");
  assert_eq!(client.research_run(run).await.unwrap().branches, 2);

  let none = running(&ground, 0);
  let (_, said) = raw(&base, reqwest::Method::POST, "/branches", json!({"run": none, "angle": "x"})).await;
  assert!(said["error"].as_str().unwrap().contains("allowed no branches"), "{said}");
  let (status, _) = raw(&base, reqwest::Method::POST, "/branches", json!({"run": run, "angle": " "})).await;
  assert_eq!(status, StatusCode::BAD_REQUEST);
  let (status, _) = raw(&base, reqwest::Method::POST, "/branches", json!({"run": 99, "angle": "x"})).await;
  assert_eq!(status, StatusCode::NOT_FOUND);
  ground.app.store.close_research_run(run, "answered", None, None).unwrap();
  let (status, said) =
    raw(&base, reqwest::Method::POST, "/branches", json!({"run": run, "angle": "x"})).await;
  assert_eq!(status, StatusCode::CONFLICT);
  assert!(said["error"].as_str().unwrap().contains("no longer running"), "{said}");
}

/// A branch's turn is waited on until it answers, and its slot for its place however long its role's line
/// takes (`docs/runner.md`, 「Branches」).
#[tokio::test]
async fn a_slow_branch_is_waited_on_and_a_waiting_slot_still_let_in() {
  let (ground, base, client) = served().await;
  ground.world.lock().unwrap().branch_wait_ms = 400;
  let run = running(&ground, 2);
  let end = client
    .branch(&BranchAsk {
      run,
      angle: "slow".into(),
    })
    .await
    .unwrap();
  assert_eq!(end.state, BranchState::Completed);
  assert_eq!(end.notes, "notes of research-1-branch-1");
  let given_back = ground.asked_path("agency", "PATCH", "/slot-requests/slot-1");
  assert_eq!(given_back.last().unwrap().body["outcome"], "completed");

  let late = running(&ground, 2);
  {
    let mut world = ground.world.lock().unwrap();
    world.branch_wait_ms = 0;
    world.slot_waits = 2;
  }
  let (status, said) =
    raw(&base, reqwest::Method::POST, "/branches", json!({"run": late, "angle": "x"})).await;
  assert_eq!(status, StatusCode::OK, "{said}");
  assert_eq!(said["notes"], format!("notes of research-{late}-branch-1"));
  let moved = ground.asked_path("agency", "PATCH", "/slot-requests/slot-2");
  assert!(moved.iter().all(|a| a.body.get("cancel").is_none()), "the slot was never cancelled");
  assert_eq!(client.research_run(late).await.unwrap().branches, 1);
}

/// A run's slot that agency gives back while it waits is asked for no more: the admission's `state` says so,
/// and the run is cut naming who gave it back and why (`docs/runner.md`, 「One run, step by step」).
#[tokio::test]
async fn a_run_s_slot_given_back_while_it_waits_is_asked_no_more_and_the_run_is_cut() {
  let (ground, _base, client) = served().await;
  {
    let mut world = ground.world.lock().unwrap();
    world.slot_waits = 1_000;
    world.given_back.insert(
      "slot-1".into(),
      ("cancelled".into(), "owner".into(), "its owner gave back everything it held".into()),
    );
  }
  client
    .ask_research(&ResearchRunAsk {
      key: "search:basic".into(),
      query: "Which engine serves Thing-7B fastest?".into(),
      ..ResearchRunAsk::default()
    })
    .await
    .unwrap();

  let run = closed(&ground, 1).await;
  assert_eq!(run.outcome.as_deref(), Some("cut"));
  let error = run.error.unwrap_or_default();
  assert!(
    error.contains("cancelled by owner") && error.contains("its owner gave back everything it held"),
    "{error}"
  );
  let admissions = ground
    .asked_path("agency", "PATCH", "/slot-requests/slot-1")
    .into_iter()
    .filter(|a| a.body == json!({}))
    .count();
  assert_eq!(admissions, 1, "a request given back is asked for no more");
  assert!(ground.asked_path("agency", "POST", "/sessions").is_empty(), "no session is started");
}

/// A run's slot still waiting is asked for its place until it is let in, however long its role's line
/// takes, its session started only then, and the run goes on to its answer (`docs/runner.md`, 「One run,
/// step by step」).
#[tokio::test]
async fn a_run_s_waiting_slot_is_asked_for_its_place_until_let_in_and_answered() {
  let (ground, _base, client) = served().await;
  ground.world.lock().unwrap().slot_waits = 3;
  let asked = client
    .ask_research(&ResearchRunAsk {
      key: "search:basic".into(),
      query: "Which engine serves Thing-7B fastest?".into(),
      ..ResearchRunAsk::default()
    })
    .await
    .unwrap();
  assert_eq!(asked.state, ResearchRunState::Waiting);
  assert_eq!(
    (asked.provider.as_str(), asked.model.as_str()),
    ("", ""),
    "no road is known while it waits"
  );

  let run = closed(&ground, 1).await;
  assert_eq!(run.outcome.as_deref(), Some("answered"), "{:?}", run.error);
  assert_eq!((run.provider.as_str(), run.model.as_str()), ("registered", "slot-model"));
  let asked_agency = ground.asked("agency");
  let admissions: Vec<usize> = asked_agency
    .iter()
    .enumerate()
    .filter(|(_, a)| a.path == "/slot-requests/slot-1" && a.body == json!({}))
    .map(|(at, _)| at)
    .collect();
  assert!(admissions.len() >= 3, "asked for its place until let in: {admissions:?}");
  let started = asked_agency.iter().position(|a| a.method == "POST" && a.path == "/sessions").unwrap();
  assert!(started > *admissions.last().unwrap(), "the session is started once the slot is let in");
  let log = ground.app.store.research_log(1).unwrap();
  assert!(log.iter().any(|line| line.kind == "admitted"), "{log:?}");
}

#[tokio::test]
async fn the_research_settings_are_read_changed_and_refused_where_they_do_not_stand() {
  let (ground, base, client) = served().await;
  let read = client.research_settings().await.unwrap();
  assert_eq!(
    (read.source.as_str(), read.branch_role.as_str(), read.strategy.as_str()),
    ("default", "voyager", "direct-then-browser")
  );
  let keys: Vec<&str> = read.keys.iter().map(|k| k.key.as_str()).collect();
  assert_eq!(keys, ["search:basic", "search:thorough", "search:exhaustive", "browse"]);
  assert_eq!(read.engines.unwrap()[0].name, "example-engine");
  let issued = &ground.asked_path("tool-host", "POST", "/tool-sessions")[0].body;
  assert_eq!(issued["namespaces"], json!(["explorer"]));
  assert!(issued.get("ttlMs").is_none(), "the record stands until its call ends: {issued}");
  assert!(issued["variables"]["BMO_DATA_DIR"].as_str().is_some());
  assert_eq!(ground.asked_path("tool-host", "DELETE", "/tool-sessions/tool-session-1").len(), 1);

  let changed = client
    .set_research_settings(&ResearchSettingsChange {
      branch_role: Some("scout".into()),
      host_interval_ms: Some(Some(250)),
      keys: Some(vec![ResearchKeySettings {
        key: "search:thorough".into(),
        branches: 3,
        pages: None,
        branch_pages: Some(5),
      }]),
      ..ResearchSettingsChange::default()
    })
    .await
    .unwrap();
  assert_eq!(
    (changed.source.as_str(), changed.branch_role.as_str(), changed.host_interval_ms),
    ("set", "scout", Some(250))
  );
  let thorough = changed.keys.iter().find(|k| k.key == "search:thorough").unwrap();
  assert_eq!((thorough.branches, thorough.pages, thorough.branch_pages), (3, Some(8), Some(5)));
  assert_eq!(client.research_settings().await.unwrap().keys, changed.keys);

  let (_, said) =
    raw(&base, reqwest::Method::PUT, "/research-settings", json!({"hostIntervalMs": null})).await;
  assert!(said["hostIntervalMs"].is_null(), "null returns to the web tools' default");
  for body in [
    json!({"branchRole": "nobody"}),
    json!({"strategy": "guess"}),
    json!({"engineIntervalMs": -1}),
    json!({"keys": [{"key": "search:deep", "branches": 1}]}),
    json!({"keys": [{"key": "browse", "branches": -1}]}),
  ] {
    let (status, said) = raw(&base, reqwest::Method::PUT, "/research-settings", body.clone()).await;
    assert_eq!(
      (status, said["code"].as_str()),
      (StatusCode::BAD_REQUEST, Some("bad-request")),
      "{body}"
    );
  }
}

#[test]
fn a_summary_a_result_still_writes_is_folded_into_the_answer() {
  let words = words::search_document("en");
  let answer = format!("## {}", words.answer);

  let drawn = draw(
    "TITLE: Pick\nTOPIC: What to buy\n\nVERDICT:\nBuy the B.\n\nSUMMARY:\n1. It is the cheapest.\n2. It ships \
     today.\n\nSECTION: Prices\nThe B costs less.\n\nUNCONFIRMED:\n1. Nothing.\n",
    words,
  );
  let body = &drawn.body;
  let (opened, prices) = (body.find(&answer).unwrap(), body.find("## Prices").unwrap());
  let held = &body[opened..prices];
  assert!(
    held.contains("Buy the B.")
      && held.contains("1. It is the cheapest.")
      && held.contains("2. It ships today."),
    "the summary stands in the answer's text: {body}"
  );
  assert_eq!(body.matches("## ").count(), 3, "no heading of its own: {body}");
  assert!(!body.contains("SUMMARY"), "{body}");

  let drawn = draw(
    "TITLE: Rate\nTOPIC: The rate\n\nSUMMARY:\n1. 1.25 as of today.\n\nSECTION: Grounds\nRead twice.\n",
    words,
  );
  assert!(
    drawn.body.starts_with(&format!("{answer}\n\n1. 1.25 as of today.")),
    "with no verdict the summary is the answer: {}",
    drawn.body
  );
}

#[test]
fn a_turn_s_text_is_every_block_from_the_head_its_answer_where_the_blocks_hold_no_more() {
  let head = "ANSWERED: yes :: it\nTITLE: A thing\n\nVERDICT:\nIt works.\n\nSECTION: Plan\nStage 1.";
  let tail = "SECTION: Plan (continued)\nStage 2.";
  let turn = json!({"answer": tail, "text": ["I will read the card first.", head, tail]});
  assert_eq!(written(&turn), format!("{head}\n{tail}"), "the head on, the chatter before it left out");
  let turn = json!({"answer": tail, "text": ["The notes so far.", tail]});
  assert_eq!(
    written(&turn),
    format!("The notes so far.\n{tail}"),
    "every block where none opens a head"
  );
  let turn = json!({"answer": head, "text": [head]});
  assert_eq!(written(&turn), head);
  let turn = json!({"answer": "", "text": [head, tail]});
  assert_eq!(written(&turn), format!("{head}\n{tail}"));
  let turn = json!({"answer": "The answer.", "text": []});
  assert_eq!(written(&turn), "The answer.");
}

/// A room's result whose script for the ear is `script`, written in the question's language.
fn narrated_result(run: i64, script: &str) -> String {
  format!(
    "ANSWERED: yes :: the run\nTITLE: Research {run}\nTOPIC: What run {run} found\n\nLISTEN:\n{script}\n\n\
     VERDICT:\nRun {run} answered.\n\nUNCONFIRMED:\n1. Nothing.\nSOURCE [1]: https://example.org/{run} :: the \
     answer\n"
  )
}

/// The run once it has closed and its log says `kind` with `words` in its text.
async fn closed_saying(ground: &support::Ground, run: i64, kind: &str, words: &str) -> ResearchRunView {
  for _ in 0..1500 {
    let row = ground.app.store.research_run(run).unwrap();
    let said = ground
      .app
      .store
      .research_log(run)
      .unwrap()
      .iter()
      .any(|l| l.kind == kind && l.text.contains(words));
    if row.state == ResearchRunState::Closed && said {
      return row.view();
    }
    tokio::time::sleep(Duration::from_millis(10)).await;
  }
  panic!("research run {run} did not close saying {words}");
}

#[tokio::test]
async fn a_room_s_document_carries_its_narration_at_its_head_and_its_room_is_told_once_it_is_attached() {
  let (ground, _base, client) = served().await;
  let script = "Thing-7B serves fastest on the second engine, and that is the one thing to know.";
  ground.world.lock().unwrap().raw.insert(1, narrated_result(1, script));
  client
    .ask_research(&ResearchRunAsk {
      key: "search:basic".into(),
      query: "Which engine serves Thing-7B fastest?".into(),
      room_id: Some(5),
      ..ResearchRunAsk::default()
    })
    .await
    .unwrap();

  let run = closed(&ground, 1).await;
  assert_eq!((run.outcome.as_deref(), run.document_id), (Some("answered"), Some(901)));
  assert!(
    ground.turns_named("research-1-narration").is_empty(),
    "a result with its script is asked none"
  );
  let filed = ground.run_documents();
  assert_eq!(
    (filed[0]["labels"]["languages"].as_str(), filed[0]["labels"]["pending"].as_str()),
    (Some("en"), Some("narration-en"))
  );
  assert!(
    !filed[0]["body"].as_str().unwrap().contains(script),
    "the script stands apart from the body"
  );

  let attached: Vec<(i64, String, Option<String>)> = ground
    .world
    .lock()
    .unwrap()
    .attachments
    .iter()
    .map(|a| (a.document, a.name.clone(), a.role.clone()))
    .collect();
  assert_eq!(attached, [(901, "narration-en.ogg".to_string(), Some("head".to_string()))]);
  assert_eq!(ground.spoken(), [script], "spoken as written");
  assert_eq!(ground.synthesis_keys(), ["research:901:en"]);
  let asked = ground.asked_path("voice", "POST", "/syntheses");
  assert_eq!(asked[0].body["language"], "en");
  assert!(
    ground.asked_path("agency", "POST", "/translation-runs").is_empty(),
    "nothing is translated"
  );

  let labels = ground.label_sets(901);
  let last = labels.last().expect("the labels are set again once the sound is attached");
  assert!(last.get("pending").is_none(), "{last}");
  assert_eq!(last["languages"], "en");
  let told = ground.asked_path("receiver", "POST", "/instructions");
  assert_eq!(told.len(), 1);
  let order = told[0].body["said"]["order"].as_str().unwrap();
  assert_eq!(order.matches("https://").count(), 1, "one link: {order}");
  assert!(
    order.contains("https://share.example/d901") && order.contains("Run 1 answered."),
    "{order}"
  );
  assert!(!order.contains(script), "the room hears nothing of the sound: {order}");
  assert_eq!((run.narrations, run.pending), (vec!["en".to_string()], Vec::<String>::new()));
  let log = ground.app.store.research_log(1).unwrap();
  let attached_at = log.iter().position(|l| l.kind == "narration" && l.text.contains("attached")).unwrap();
  let told_at = log.iter().position(|l| l.kind == "told").unwrap();
  assert!(attached_at < told_at, "the room is told after the sound is attached");
  assert!(
    ground.app.store.run_pages_due(now_ms() + 3_600_000_000).unwrap().is_empty(),
    "nothing is pending"
  );
}

#[tokio::test]
async fn a_room_s_document_whose_sound_fails_is_told_only_once_its_narration_is_attached_on_its_retry_wait()
{
  let (ground, _base, client) = served().await;
  // A Korean question: its result and its script are Korean, and the sound is spoken as written. The
  // syllables are made from their code points, so the source holds no Hangul.
  let syllables = |points: &[u32]| points.iter().filter_map(|p| char::from_u32(*p)).collect::<String>();
  let query = format!("Thing-7B {}?", syllables(&[0xc5d4, 0xc9c4]));
  let script = format!(
    "Thing-7B{} {}.",
    syllables(&[0xb294]),
    syllables(&[0xbaa8, 0xb378, 0xc785, 0xb2c8, 0xb2e4])
  );
  let script = script.as_str();
  {
    let mut world = ground.world.lock().unwrap();
    world.raw.insert(1, narrated_result(1, script));
    world.voice_failing = true;
  }
  client
    .ask_research(&ResearchRunAsk {
      key: "search:basic".into(),
      query: query.clone(),
      room_id: Some(5),
      ..ResearchRunAsk::default()
    })
    .await
    .unwrap();

  let run = closed_saying(&ground, 1, "narration", "was not made").await;
  assert_eq!((run.outcome.as_deref(), run.document_id), (Some("answered"), Some(901)));
  assert_eq!(run.pending, ["narration-ko"]);
  assert!(
    ground.asked_path("receiver", "POST", "/instructions").is_empty(),
    "the room hears nothing yet"
  );
  assert_eq!(ground.world.lock().unwrap().kept_labels[&901]["pending"], "narration-ko");
  assert!(ground.world.lock().unwrap().attachments.is_empty());
  assert!(
    ground.app.store.run_pages_due(now_ms()).unwrap().is_empty(),
    "asked again on its retry wait"
  );
  let due = ground.app.store.run_pages_due(now_ms() + 3_600_000).unwrap();
  assert_eq!(due.iter().map(|row| row.id).collect::<Vec<_>>(), [1]);

  ground.world.lock().unwrap().voice_failing = false;
  finish_told(&ground.app, &due[0]).await;
  assert_eq!(
    ground.synthesis_keys(),
    ["research:901:ko", "research:901:ko:1"],
    "a failed one asked anew"
  );
  assert_eq!(ground.spoken(), [script, script], "spoken as written, untranslated");
  let asked = ground.asked_path("voice", "POST", "/syntheses");
  assert_eq!(asked[1].body["language"], "ko");
  assert!(ground.asked_path("agency", "POST", "/translation-runs").is_empty());
  let attached: Vec<(String, Option<String>)> = ground
    .world
    .lock()
    .unwrap()
    .attachments
    .iter()
    .map(|a| (a.name.clone(), a.role.clone()))
    .collect();
  assert_eq!(attached, [("narration-ko.ogg".to_string(), Some("head".to_string()))]);
  assert!(ground.world.lock().unwrap().kept_labels[&901].get("pending").is_none());
  let told = ground.asked_path("receiver", "POST", "/instructions");
  assert_eq!(told.len(), 1, "told once the sound is attached");
  assert!(told[0].body["said"]["order"].as_str().unwrap().contains("https://share.example/d901"));
  let run = ground.app.store.research_run(1).unwrap().view();
  assert_eq!((run.narrations, run.pending), (vec!["ko".to_string()], Vec::<String>::new()));

  let row = ground.app.store.research_run(1).unwrap();
  finish_told(&ground.app, &row).await;
  assert_eq!(ground.asked_path("receiver", "POST", "/instructions").len(), 1, "told once");
  assert_eq!(ground.synthesis_keys().len(), 2, "a sound attached is not made again");
}

#[tokio::test]
async fn a_room_s_document_gone_before_its_room_was_told_is_told_whole_once_and_not_asked_again() {
  let (ground, _base, client) = served().await;
  {
    let mut world = ground.world.lock().unwrap();
    world.raw.insert(1, narrated_result(1, "Thing-7B is a model."));
    world.voice_failing = true;
  }
  client
    .ask_research(&ResearchRunAsk {
      key: "search:basic".into(),
      query: "Which engine serves Thing-7B?".into(),
      room_id: Some(5),
      ..ResearchRunAsk::default()
    })
    .await
    .unwrap();
  let run = closed_saying(&ground, 1, "narration", "was not made").await;
  assert_eq!(run.pending, ["narration-en"]);
  ground.world.lock().unwrap().gone_documents.insert(901);

  let due = ground.app.store.run_pages_due(now_ms() + 3_600_000).unwrap();
  finish_told(&ground.app, &due[0]).await;

  let told = ground.asked_path("receiver", "POST", "/instructions");
  assert_eq!(told.len(), 1, "the room is told once");
  let order = told[0].body["said"]["order"].as_str().unwrap();
  assert!(
    order.contains("Its result, as the run wrote it") && order.contains("Run 1 answered."),
    "{order}"
  );
  assert!(ground.synthesis_keys().len() == 1, "no sound is made for a document that is gone");
  assert!(
    ground.app.store.run_pages_due(i64::MAX).unwrap().is_empty(),
    "the page is not asked again"
  );
  let run = ground.app.store.research_run(1).unwrap().view();
  assert!(run.pending.is_empty());
}

/// Every search is told its title is a noun phrase naming the subject, never a sentence or a verdict, and
/// short enough for the archive to show it whole on a phone (`docs/design.md`, 「Reports and rooms」).
#[test]
fn a_search_is_told_its_title_names_its_subject_in_a_bounded_noun_phrase() {
  let system = research_server::runner::search_instructions("- web_search").unwrap();
  let rule = system
    .lines()
    .find(|line| line.starts_with("- \"TITLE: "))
    .expect("the TITLE line's rule");
  for words in [
    "a noun phrase",
    "never a sentence",
    "no verdict",
    "At most 15 Korean syllables, spaces not counted, or 30 characters in English",
  ] {
    assert!(rule.contains(words), "{words} missing from {rule}");
  }
  assert!(!rule.contains("headline"), "{rule}");
  let research =
    std::fs::read_to_string(concat!(env!("CARGO_MANIFEST_DIR"), "/prompts/rules/research.md")).unwrap();
  assert!(research.contains("**A title is a noun phrase, never a sentence:**"));
  let dives =
    std::fs::read_to_string(concat!(env!("CARGO_MANIFEST_DIR"), "/prompts/rules/deep-dives.md")).unwrap();
  assert!(dives.contains("**The title names the thing, never the verdict:**"));
}

#[tokio::test]
async fn a_run_naming_a_model_alone_asks_its_slot_on_the_provider_its_role_s_order_holds_it_under() {
  let (ground, _base, client) = served().await;
  let asked = client
    .ask_research(&ResearchRunAsk {
      key: "browse".into(),
      query: "Open the shop's page.".into(),
      model: Some("deepseek-flash".into()),
      ..ResearchRunAsk::default()
    })
    .await
    .unwrap();
  assert_eq!((asked.provider.as_str(), asked.model.as_str()), ("deepseek", "deepseek-flash"));
  assert_eq!(
    ground.asked_path("agency", "GET", "/roles/seeker").len(),
    1,
    "the role's order is read"
  );
  let slot = &ground.run_slots()[0];
  assert_eq!(
    (slot["provider"].as_str(), slot["model"].as_str()),
    (Some("deepseek"), Some("deepseek-flash")),
    "the slot is asked on the order's member naming that model"
  );
  closed(&ground, asked.run_id).await;

  // A model no member of the order names goes out on the provider the role stands on now.
  let other = client
    .ask_research(&ResearchRunAsk {
      key: "browse".into(),
      query: "Open the shop's other page.".into(),
      model: Some("other-model".into()),
      ..ResearchRunAsk::default()
    })
    .await
    .unwrap();
  assert_eq!((other.provider.as_str(), other.model.as_str()), ("registered", "other-model"));
  closed(&ground, other.run_id).await;
}

#[tokio::test]
async fn a_run_whose_role_agency_does_not_hold_is_refused_before_any_slot_is_asked() {
  let ground = support::ground(|s| s.runner.browse_role = "nobody".into()).await;
  let base = support::serve(research_server::routes::router(ground.app.clone())).await;
  let (status, said) = raw(
    &base,
    reqwest::Method::POST,
    "/research-runs",
    json!({"key": "browse", "query": "Open the page.", "model": "deepseek-flash"}),
  )
  .await;
  assert_eq!(status, StatusCode::BAD_GATEWAY, "{said}");
  assert!(ground.asked_path("agency", "POST", "/slot-requests").is_empty());
}

#[tokio::test]
async fn a_request_key_is_never_asked_again_once_its_slot_was_given_back() {
  let (ground, _base, client) = served().await;
  for query in ["One.", "Two."] {
    let asked = client
      .ask_research(&ResearchRunAsk {
        key: "search:basic".into(),
        query: query.into(),
        ..ResearchRunAsk::default()
      })
      .await
      .unwrap();
    closed(&ground, asked.run_id).await;
  }
  let keys: Vec<String> = ground
    .asked_path("agency", "POST", "/slot-requests")
    .into_iter()
    .map(|a| a.body["requestKey"].as_str().unwrap_or_default().to_string())
    .collect();
  let mut distinct = keys.clone();
  distinct.sort();
  distinct.dedup();
  assert_eq!(distinct.len(), keys.len(), "every slot goes under a key of its own: {keys:?}");
}
