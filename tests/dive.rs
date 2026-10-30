//! The deep dive against stand-in agency, artifact and receiver servers (`docs/api.md`, 「The deep dive」 and
//! 「The notices」): the bodies each unit is asked with, the retries, the failure told once, an advisory's
//! parts to the workers of its repositories, and a brief's gathering with its facts. The filed documents are
//! composed from the words `prompts/document.toml` holds, so this file holds none of them.

mod support;

use bmo_research_client::{ItemState, NewStackRow, RecipientKind, Running, Shape};
use research_server::dive::{give_owner_room, take_up, tick};
use research_server::words;
use serde_json::Value;
use support::{Filed, Ground};

async fn settle(ground: &Ground) {
  for handle in tick(&ground.app).await {
    handle.await.unwrap();
  }
}

fn manual_document() -> String {
  let w = &words::document().manual;
  format!(
    "# Thing-7B\n\n## 7. {}\n\n- {} n3, n9\n\n## 9. {}\n\n- re-measure\n",
    w.heading, w.label, w.plan_heading
  )
}

fn file(ground: &Ground, run: i64, id: i64, content: String) {
  ground.world.lock().unwrap().filed.insert(
    run,
    Filed {
      id,
      title: format!("Report {id}"),
      description: "It is a thing. It does more.".into(),
      content,
    },
  );
}

fn orders(ground: &Ground) -> Vec<Value> {
  ground
    .asked_path("receiver", "POST", "/instructions")
    .into_iter()
    .map(|a| a.body)
    .collect()
}

#[tokio::test]
async fn a_manual_is_asked_read_filed_and_told_with_what_the_owner_s_word_needs() {
  let ground = support::ground(|_| {}).await;
  give_owner_room(&ground.app).await;
  let item = ground
    .app
    .store
    .ask("it", "example.org/thing", "Thing-7B", "https://example.org/thing", Shape::Manual)
    .unwrap();
  file(&ground, 101, 501, manual_document());
  ground.world.lock().unwrap().open_polls = 2;

  settle(&ground).await;

  let asks = ground.asked_path("agency", "POST", "/research-runs");
  assert_eq!(asks.len(), 1);
  let body = &asks[0].body;
  assert_eq!(body["key"], "search:exhaustive");
  assert_eq!(body["provider"], "registered");
  assert_eq!(body["model"], "flash-next");
  assert!(body.get("roomId").is_none());
  let query = body["query"].as_str().unwrap();
  assert!(query.contains("Thing-7B") && query.contains("https://example.org/thing"), "{query}");
  assert_eq!(ground.asked_path("agency", "GET", "/runs/101").len(), 3, "read until it closes");

  assert_eq!(ground.asked_path("artifact", "GET", "/documents?label=research%3A101").len(), 1);
  assert_eq!(ground.asked_path("artifact", "GET", "/documents/501/content").len(), 1);
  assert_eq!(ground.asked_path("artifact", "GET", "/shares?documentId=501").len(), 1);
  let made = ground.asked_path("artifact", "POST", "/shares");
  assert_eq!(made[0].body["documentId"], 501);

  let told = orders(&ground);
  assert_eq!(told.len(), 1);
  assert_eq!(told[0]["room"], 7);
  assert_eq!(told[0]["sort"], "notice");
  assert_eq!(told[0]["kind"], format!("research-report:{}", item.id));
  let order = told[0]["said"]["order"].as_str().unwrap();
  let report = words::report();
  for part in [
    "Report 501",
    "It is a thing.",
    "https://share.example/d501",
    report.fit.as_str(),
    "Thing-7B",
  ] {
    assert!(order.contains(part), "{part} missing from {order}");
  }
  assert!(!order.contains("It does more."), "only the first sentence is told");
  assert!(order.contains(&words::fill(&report.bench, &[("nodes", "n3")])), "{order}");
  assert!(order.contains(&words::document().manual.plan_heading));
  assert!(told[0]["summary"].as_str().unwrap().contains("Thing-7B"));

  let filed = ground.app.store.item(item.id).unwrap();
  assert_eq!(filed.state, ItemState::Fit);
  assert_eq!(filed.fit_nodes, ["n3"]);
  assert_eq!(filed.run_id, Some(101));
  assert_eq!(filed.share_url.as_deref(), Some("https://share.example/d501"));
  let run = ground.app.store.runs(Some(item.id), None, None, 10).unwrap().items;
  assert_eq!((run[0].state.as_str(), run[0].outcome.as_deref()), ("closed", Some("answered")));
}

#[tokio::test]
async fn a_refused_ask_is_retried_and_counted() {
  let ground = support::ground(|s| s.retry_minutes = 0).await;
  give_owner_room(&ground.app).await;
  let item = ground
    .app
    .store
    .ask("it", "example.org/a", "A", "https://example.org/a", Shape::Investigation)
    .unwrap();
  ground.world.lock().unwrap().run_answers.push_back(503);
  file(&ground, 101, 9, "# An investigation\n".into());

  settle(&ground).await;
  let waiting = ground.app.store.item(item.id).unwrap();
  assert_eq!(waiting.state, ItemState::Waiting);
  assert!(waiting.retry_at.is_some() && waiting.error.unwrap().contains("503"));

  settle(&ground).await;
  let reported = ground.app.store.item(item.id).unwrap();
  assert_eq!((reported.state, reported.tries), (ItemState::Reported, 2));
  let runs = ground.app.store.runs(Some(item.id), None, None, 10).unwrap().items;
  let states: Vec<&str> = runs.iter().map(|r| r.state.as_str()).collect();
  assert_eq!(states, ["closed", "refused"]);
  assert_eq!(runs[1].run_id, None);
}

#[tokio::test]
async fn a_run_cut_past_its_retries_fails_and_is_told_once() {
  let ground = support::ground(|s| {
    s.retry_minutes = 0;
    s.retries = 3;
  })
  .await;
  give_owner_room(&ground.app).await;
  let item = ground
    .app
    .store
    .ask("it", "example.org/b", "B", "https://example.org/b", Shape::Manual)
    .unwrap();
  for run in 101..=104 {
    ground.world.lock().unwrap().outcomes.insert(run, "cut".into());
  }
  for _ in 0..6 {
    settle(&ground).await;
  }
  let failed = ground.app.store.item(item.id).unwrap();
  assert_eq!((failed.state, failed.tries), (ItemState::Failed, 4));
  assert_eq!(
    ground.asked_path("agency", "POST", "/research-runs").len(),
    4,
    "a failed item is asked no more"
  );
  let told = orders(&ground);
  assert_eq!(told.len(), 1);
  let expected = words::fill(&words::report().failed, &[("name", "B"), ("tries", "4")]);
  assert_eq!(told[0]["said"]["order"], expected.as_str());
}

#[tokio::test]
async fn an_advisory_tells_each_touched_repository_s_worker_its_part_and_the_recipients_the_whole() {
  let ground = support::ground(|_| {}).await;
  give_owner_room(&ground.app).await;
  let store = &ground.app.store;
  store
    .add_recipient("security", RecipientKind::Agent, None, Some("watcher"), "added")
    .unwrap();
  for (component, repository, worker) in [
    ("libexample", "services/alpha", Some("worker-a")),
    ("libexample", "services/beta", None),
  ] {
    store
      .add_stack_row(&NewStackRow {
        component: component.into(),
        version: Some("1.4.1".into()),
        place: None,
        repository: Some(repository.into()),
        worker: worker.map(String::from),
      })
      .unwrap();
  }
  let item = store
    .ask("security", "example.org/sa-1", "SA-1", "https://example.org/sa-1", Shape::Advisory)
    .unwrap();
  let w = &words::document().advisory;
  let document = format!(
    "# SA-1\n\n| {} | {} | {} | {} | {} |\n|---|---|---|---|---|\n| libexample | 1.4.1 | services/alpha | node-a | {} |\n| libexample | 1.4.1 | services/beta | | {} |\n\n## {}\n\n1. upgrade\n",
    w.component, w.version, w.repository, w.host, w.touched, w.yes, w.yes, w.plan_heading
  );
  file(&ground, 101, 77, document);

  settle(&ground).await;

  let query = ground.asked_path("agency", "POST", "/research-runs")[0].body["query"]
    .as_str()
    .unwrap()
    .to_string();
  assert!(
    query.contains("libexample 1.4.1 (services/alpha)"),
    "the stack is in the question: {query}"
  );
  let filed = store.item(item.id).unwrap();
  assert_eq!(filed.state, ItemState::Fit);
  assert_eq!(filed.touches.as_deref(), Some("services/alpha, node-a, services/beta"));

  let room = orders(&ground);
  assert_eq!(room.len(), 1);
  assert!(room[0]["said"]["order"].as_str().unwrap().contains("services/beta"));
  let part = ground.asked_path("agency", "POST", "/sessions/worker-a@worker/turns");
  assert_eq!(part.len(), 1);
  let prompt = part[0].body["prompt"].as_str().unwrap();
  assert!(prompt.contains("services/alpha") && !prompt.contains("services/beta"), "{prompt}");
  assert!(prompt.contains("https://share.example/d77"));
  let whole = ground.asked_path("agency", "POST", "/sessions/watcher@worker/turns");
  assert_eq!(whole.len(), 1);
  assert_eq!(whole[0].body["prompt"], room[0]["said"]["order"]);
  let reports = store.reports(item.id).unwrap();
  assert!(reports.contains(&("agent:worker-a".into(), "part".into(), "sent".into())));
}

#[tokio::test]
async fn a_brief_gathers_its_category_s_items_into_one_run_and_keeps_the_figures() {
  let ground = support::ground(|_| {}).await;
  give_owner_room(&ground.app).await;
  let store = &ground.app.store;
  let one = store
    .ask("economy", "example.org/e1", "E1", "https://example.org/e1", Shape::Brief)
    .unwrap();
  let two = store
    .ask("economy", "example.org/e2", "E2", "https://example.org/e2", Shape::Brief)
    .unwrap();
  let w = &words::document().brief;
  let document = format!(
    "# Brief\n\n| {} | {} | {} | {} | {} |\n|---|---|---|---|---|\n| index-a | 2,650.1 | pt | 2026-10-29 | [page](https://example.org/e1) |\n",
    w.name, w.value, w.unit, w.as_of, w.source
  );
  file(&ground, 101, 300, document);

  settle(&ground).await;

  let asks = ground.asked_path("agency", "POST", "/research-runs");
  assert_eq!(asks.len(), 1, "one run for the gathering");
  let query = asks[0].body["query"].as_str().unwrap();
  assert!(query.contains("https://example.org/e1") && query.contains("https://example.org/e2"));
  for id in [one.id, two.id] {
    assert_eq!(store.item(id).unwrap().state, ItemState::Reported);
  }
  assert_eq!(orders(&ground).len(), 1, "the brief is told once");
  let facts = store.facts(None, None, 10).unwrap().items;
  assert_eq!(facts.len(), 1);
  assert_eq!(
    (facts[0].name.as_str(), facts[0].value.as_str(), facts[0].run_id, facts[0].document_id),
    ("index-a", "2,650.1", Some(101), Some(300))
  );
  assert_eq!(facts[0].source_address.as_deref(), Some("https://example.org/e1"));

  // The next gathering waits its gathering time.
  store
    .ask("economy", "example.org/e3", "E3", "https://example.org/e3", Shape::Brief)
    .unwrap();
  settle(&ground).await;
  assert_eq!(ground.asked_path("agency", "POST", "/research-runs").len(), 1);
}

#[tokio::test]
async fn a_stopped_category_starts_no_run_and_the_least_recent_category_goes_first() {
  let ground = support::ground(|_| {}).await;
  let store = &ground.app.store;
  store.set_running(Some("it"), Running::Stopped).unwrap();
  let it = store.ask("it", "example.org/i", "I", "https://example.org/i", Shape::Manual).unwrap();
  settle(&ground).await;
  assert!(ground.asked_path("agency", "POST", "/research-runs").is_empty());
  store.set_running(None, Running::Running).unwrap();

  // `it` ran a try a moment ago; security has not run at all, so it goes first.
  let (run, _) = store.start_try("it", &[it.id]).unwrap().unwrap();
  store.end_run(run, "refused", None, Some("earlier")).unwrap();
  store.fail_try(run, "earlier", Some(0)).unwrap();
  let sec = store
    .ask("security", "example.org/s", "S", "https://example.org/s", Shape::Advisory)
    .unwrap();
  let handles = tick(&ground.app).await;
  assert_eq!(handles.len(), 1, "one run at a time");
  for handle in handles {
    handle.await.unwrap();
  }
  let first = &ground.asked_path("agency", "POST", "/research-runs")[0];
  assert!(first.body["query"].as_str().unwrap().contains("https://example.org/s"));
  assert_eq!(store.item(sec.id).unwrap().tries, 1);
  assert_eq!(store.item(it.id).unwrap().tries, 1, "it waits its turn");
}

#[tokio::test]
async fn a_restarted_daemon_takes_up_what_it_left_running() {
  let ground = support::ground(|_| {}).await;
  let store = &ground.app.store;
  let asked = store
    .ask("it", "example.org/r1", "R1", "https://example.org/r1", Shape::Investigation)
    .unwrap();
  let never = store
    .ask("it", "example.org/r2", "R2", "https://example.org/r2", Shape::Investigation)
    .unwrap();
  let (run, _) = store.start_try("it", &[asked.id]).unwrap().unwrap();
  store.keep_run_id(run, 150).unwrap();
  store.start_try("it", &[never.id]).unwrap().unwrap();
  file(&ground, 150, 15, "# Taken up\n".into());

  take_up(&ground.app);
  for _ in 0..100 {
    if store.item(asked.id).unwrap().state == ItemState::Reported {
      break;
    }
    tokio::time::sleep(std::time::Duration::from_millis(20)).await;
  }
  assert_eq!(store.item(asked.id).unwrap().state, ItemState::Reported);
  assert!(
    ground.asked_path("agency", "POST", "/research-runs").is_empty(),
    "an asked run is not asked again"
  );
  let waiting = store.item(never.id).unwrap();
  assert_eq!((waiting.state, waiting.tries), (ItemState::Waiting, 0));
}
