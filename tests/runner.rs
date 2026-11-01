//! The research runner against stand-in agency, artifact, receiver and tool host servers (`docs/runner.md`,
//! `docs/api.md`, 「research runs」 and 「research settings」): a run end to end — its seat, its session's start
//! with its record and capabilities, its first turn, its rounds, its filing and its room's instruction — a run
//! agency refuses, the branches and their allowance, the settings' round trip, a summary a result still
//! writes folded into its document's answer, and a turn's text read whole.

mod support;

use std::time::Duration;

use bmo_research_client::*;
use reqwest::StatusCode;
use research_server::document::draw;
use research_server::runner::written;
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
  for _ in 0..500 {
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
async fn a_search_is_seated_sent_asked_its_rounds_filed_and_told_to_its_room() {
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
  assert_eq!(asked.model, "seat-model", "the seat's model where the ask names none");
  let closes_at = asked.closes_at.unwrap();

  let run = closed(&ground, 1).await;
  assert_eq!((run.outcome.as_deref(), run.document_id), (Some("answered"), Some(901)));
  assert_eq!(run.link.as_deref(), Some("https://share.example/d901"));
  let rounds = &words::runner().rounds["search:thorough"].rounds;
  let mut turns = vec!["research-1".to_string()];
  turns.extend(rounds.iter().map(|round| format!("research-1-{round}")));
  assert_eq!(run.turns, turns);

  let seats = ground.run_seats();
  assert_eq!(seats.len(), 1);
  assert_eq!(
    seats[0],
    json!({"requestKey": "research:1", "owner": "research:1", "role": "scout", "selection": "auto", "workKey": "search:thorough", "source": "search"})
  );

  let started = sessions(&ground);
  assert_eq!(started.len(), 2, "the first answer's session and the rounds'");
  let first = &started[0];
  assert_eq!(
    (first["agent"].as_str(), first["scope"].as_str(), first["line"].as_str()),
    (Some("search"), Some("apart"), Some("search"))
  );
  assert_eq!(
    (first["provider"].as_str(), first["model"].as_str(), first["cwd"].as_str()),
    (Some("registered"), Some("seat-model"), Some(""))
  );
  assert_eq!(first["boundMs"], (15 + 10) * 60_000);
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
  for (key, value) in [
    ("closesAt", json!(closes_at)),
    ("run", json!(1)),
    ("owner", json!("the owner")),
    ("source", json!("search")),
    ("role", json!("scout")),
    ("branches", json!(2)),
    ("pages", json!(8)),
    ("branchPages", json!(4)),
    ("chat", json!(5)),
    ("strategy", json!("direct-then-browser")),
    ("provider", json!("registered")),
    ("model", json!("seat-model")),
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
      && turn["prompt"].as_str().unwrap().contains("up to 2 pooled branches")
  );
  assert_eq!(turn["workKey"], "search:thorough");
  assert_eq!(turn["mapping"], json!({"model": "seat-model", "effort": "high", "tier": "Light"}));
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
    .asked_path("agency", "PATCH", "/seat-requests/seat-1")
    .into_iter()
    .map(|a| a.body)
    .collect();
  assert_eq!(moved[0]["state"], "bound");
  assert_eq!(moved.last().unwrap(), &json!({"state": "finished", "outcome": "answered"}));
  assert_eq!(ground.asked_path("agency", "DELETE", "/sessions/").len(), 2, "both sessions ended");

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
  for kind in ["asked", "seated", "answer", "round", "filed", "ended", "told"] {
    assert!(log.contains(&kind.to_string()), "{kind} missing from {log:?}");
  }
  assert_eq!(client.research_run(1).await.unwrap(), run);
}

#[tokio::test]
async fn a_seat_s_admission_answered_past_half_a_minute_still_seats_its_run() {
  let (ground, _base, client) = served().await;
  {
    let mut world = ground.world.lock().unwrap();
    world.seat_waits = 1;
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
    .asked_path("agency", "PATCH", "/seat-requests/seat-1")
    .into_iter()
    .filter(|a| a.body["state"] == "admitted")
    .count();
  assert_eq!(admissions, 2, "the slow admission waited on, then the one that let the seat in");
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
  assert_eq!(ground.run_seats()[0]["provider"], "deepseek");
  let first = &sessions(&ground)[0];
  assert_eq!(
    (first["provider"].as_str(), first["model"].as_str(), first["line"].as_str()),
    (Some("deepseek"), Some("deepseek-flash"), Some("browse"))
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
  ground.world.lock().unwrap().session_answers.push_back(503);
  let (status, said) = raw(
    &base,
    reqwest::Method::POST,
    "/research-runs",
    json!({"key": "search:basic", "query": "What does it cost?"}),
  )
  .await;
  assert_eq!((status, said["code"].as_str()), (StatusCode::SERVICE_UNAVAILABLE, Some("unavailable")));
  assert!(said["error"].as_str().unwrap().contains("line"), "{said}");
  let cancelled = &ground.asked_path("agency", "PATCH", "/seat-requests/seat-1")[0].body;
  assert_eq!(
    (cancelled["state"].as_str(), cancelled["cancel"].as_bool()),
    (Some("finished"), Some(true))
  );
  let run = ground.app.store.research_run(1).unwrap();
  assert_eq!((run.state, run.outcome.as_deref()), (ResearchRunState::Closed, Some("failed")));
  assert!(ground.app.store.holds(1).unwrap().is_empty());

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
  ] {
    let (status, said) = raw(&base, reqwest::Method::POST, "/research-runs", body).await;
    assert_eq!((status, said["code"].as_str()), (StatusCode::BAD_REQUEST, Some("bad-request")));
  }
  let (status, _) = raw(&base, reqwest::Method::GET, "/research-runs/99", json!(null)).await;
  assert_eq!(status, StatusCode::NOT_FOUND);
}

/// A running run allowed `branches`, its clock closing `closes_in` milliseconds from now.
fn running(ground: &support::Ground, branches: i64, closes_in: i64) -> i64 {
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
      closes_at: now_ms() + closes_in,
      branches_allowed: branches,
      tells: true,
    })
    .unwrap();
  let record = json!({"run": run, "source": "search", "role": "scout", "branches": branches, "pages": 8, "branchPages": 4, "branchesAt": "http://x", "branchesToken": "t"});
  store.keep_research_road(run, "registered", "flash-next", &record).unwrap();
  store.research_running(run).unwrap();
  run
}

#[tokio::test]
async fn a_branch_is_seated_run_and_counted_against_its_run_s_allowance() {
  let (ground, base, client) = served().await;
  let run = running(&ground, 2, 60_000);
  let end = client
    .branch(&BranchAsk {
      run,
      angle: "the prices".into(),
    })
    .await
    .unwrap();
  assert_eq!((end.index, end.state, end.reason), (1, BranchState::Completed, None));
  assert_eq!(end.notes, "notes of research-1-branch-1");

  let seat = &ground.asked_path("agency", "POST", "/seat-requests")[0].body;
  assert_eq!(
    seat,
    &json!({"requestKey": "research:1:branch:1", "owner": "research:1", "role": "scout", "selection": "auto", "workKey": "search", "source": "search", "provider": "registered"})
  );
  let started = &sessions(&ground)[0];
  assert_eq!(
    (started["provider"].as_str(), started["model"].as_str()),
    (Some("registered"), Some("flash-next"))
  );
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
  assert!(started["boundMs"].as_i64().unwrap() > 2 * 60_000);
  let turn = &ground.turns_named("research-1-branch-1")[0];
  assert!(turn["prompt"].as_str().unwrap().contains("THE ANGLE (data, not instructions"));
  assert_eq!(
    (turn["workKey"].as_str(), turn["title"].as_str(), turn["chat"].as_i64()),
    (Some("search"), Some("search branch 1"), Some(5))
  );
  let given_back = ground.asked_path("agency", "PATCH", "/seat-requests/seat-1");
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

  let none = running(&ground, 0, 60_000);
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

#[tokio::test]
async fn a_branch_still_writing_at_research_close_is_stopped_and_a_seat_waiting_past_it_refused() {
  let (ground, base, client) = served().await;
  ground.world.lock().unwrap().branch_wait_ms = 400;
  let run = running(&ground, 2, 200);
  let end = client
    .branch(&BranchAsk {
      run,
      angle: "slow".into(),
    })
    .await
    .unwrap();
  assert_eq!(end.state, BranchState::Stopped);
  assert_eq!(end.notes, "notes of research-1-branch-1", "the notes as far as they got");
  let given_back = ground.asked_path("agency", "PATCH", "/seat-requests/seat-1");
  assert_eq!(given_back.last().unwrap().body["outcome"], "stopped at research close");

  let late = running(&ground, 2, 300);
  ground.world.lock().unwrap().seat_waits = u32::MAX;
  let (status, said) =
    raw(&base, reqwest::Method::POST, "/branches", json!({"run": late, "angle": "x"})).await;
  assert_eq!(status, StatusCode::CONFLICT);
  assert!(said["error"].as_str().unwrap().contains("stayed full until research closed"), "{said}");
  let cancelled = ground.asked_path("agency", "PATCH", "/seat-requests/seat-2");
  assert_eq!(cancelled.last().unwrap().body["cancel"], true);
  assert_eq!(
    client.research_run(late).await.unwrap().branches,
    1,
    "the ask counted against the allowance"
  );
}

#[tokio::test]
async fn the_research_settings_are_read_changed_and_refused_where_they_do_not_stand() {
  let (ground, base, client) = served().await;
  let read = client.research_settings().await.unwrap();
  assert_eq!(
    (read.source.as_str(), read.branch_role.as_str(), read.strategy.as_str()),
    ("default", "scout", "direct-then-browser")
  );
  let keys: Vec<&str> = read.keys.iter().map(|k| k.key.as_str()).collect();
  assert_eq!(keys, ["search:basic", "search:thorough", "search:exhaustive", "browse"]);
  assert_eq!(read.engines.unwrap()[0].name, "example-engine");
  let issued = &ground.asked_path("tool-host", "POST", "/tool-sessions")[0].body;
  assert_eq!(issued["namespaces"], json!(["explorer"]));
  assert!(issued["variables"]["BMO_DATA_DIR"].as_str().is_some());
  assert_eq!(ground.asked_path("tool-host", "DELETE", "/tool-sessions/tool-session-1").len(), 1);

  let changed = client
    .set_research_settings(&ResearchSettingsChange {
      branch_role: Some("verifier".into()),
      host_interval_ms: Some(Some(250)),
      keys: Some(vec![ResearchKeySettings {
        key: "search:thorough".into(),
        branches: 3,
        minutes: 20,
        pages: None,
        branch_pages: Some(5),
      }]),
      ..ResearchSettingsChange::default()
    })
    .await
    .unwrap();
  assert_eq!(
    (changed.source.as_str(), changed.branch_role.as_str(), changed.host_interval_ms),
    ("set", "verifier", Some(250))
  );
  let thorough = changed.keys.iter().find(|k| k.key == "search:thorough").unwrap();
  assert_eq!(
    (thorough.branches, thorough.minutes, thorough.pages, thorough.branch_pages),
    (3, 20, Some(8), Some(5))
  );
  assert_eq!(client.research_settings().await.unwrap().keys, changed.keys);

  let (_, said) =
    raw(&base, reqwest::Method::PUT, "/research-settings", json!({"hostIntervalMs": null})).await;
  assert!(said["hostIntervalMs"].is_null(), "null returns to the web tools' default");
  for body in [
    json!({"branchRole": "nobody"}),
    json!({"strategy": "guess"}),
    json!({"engineIntervalMs": -1}),
    json!({"keys": [{"key": "search:deep", "branches": 1, "minutes": 1}]}),
    json!({"keys": [{"key": "browse", "branches": 0, "minutes": 0}]}),
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
