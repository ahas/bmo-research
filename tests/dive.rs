//! The deep dive against stand-in agency, artifact, receiver and tool host servers (`docs/api.md`, 「The deep dive」,
//! 「The Korean rendering」 and 「The notices」): the bodies each unit is asked with, the retries, the failure
//! told once, the Korean rendering and its follow-up, an advisory's parts to the workers of its
//! repositories, and a brief's gathering with its facts. The filed documents are composed from the English
//! words `prompts/document.toml` holds, so this file holds none of them.

mod support;

use bmo_research_client::{ItemState, NewStackRow, RecipientKind, ResearchRunState, Running, Shape};
use research_server::dive::{give_owner_room, take_up, tick};
use research_server::store::{NewResearchRun, now_ms};
use research_server::translation::{cut, note, title_note};
use research_server::words;
use serde_json::{Value, json};
use support::{Filed, Ground, translated};

async fn settle(ground: &Ground) {
  for handle in tick(&ground.app).await {
    handle.await.unwrap();
  }
}

fn manual_document() -> String {
  let w = &words::document().manual[0];
  format!(
    "# Thing-7B\n\n## 7. {}\n\n- {} n3, n9\n\n## 9. {}\n\n- re-measure\n",
    w.heading, w.label, w.plan_heading
  )
}

/// Prose of at least `size` characters, in sentences.
fn prose(size: usize) -> String {
  let mut text = String::new();
  while text.chars().count() < size {
    text.push_str("The maker measured it on its own cards. ");
  }
  text.trim_end().to_string()
}

/// A manual whose two sections each hold `size` characters of prose beside the fit and the plan.
fn long_manual(size: usize) -> String {
  let w = &words::document().manual[0];
  format!(
    "# Thing-7B\n\n## 7. {}\n\n- {} n3, n9\n\n{}\n\n## 9. {}\n\n- re-measure\n\n{}\n",
    w.heading,
    w.label,
    prose(size),
    w.plan_heading,
    prose(size)
  )
}

/// The translation asks agency was handed, in the order they were asked.
fn translations(ground: &Ground) -> Vec<Value> {
  ground
    .asked_path("agency", "POST", "/translation-runs")
    .into_iter()
    .map(|a| a.body)
    .collect()
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
  file(&ground, 1, 501, manual_document());

  settle(&ground).await;

  let seats = ground.run_seats();
  assert_eq!(seats.len(), 1);
  assert_eq!(
    (seats[0]["workKey"].as_str(), seats[0]["provider"].as_str()),
    (Some("search:exhaustive"), Some("registered"))
  );
  let started = ground.asked_path("agency", "POST", "/sessions");
  assert_eq!(started[0].body["model"], "flash-next");
  assert_eq!(started[0].body["labels"]["research"]["chat"], 0, "the run names no room");
  let query = ground.turns_named("research-1")[0]["prompt"].as_str().unwrap().to_string();
  assert!(query.contains("Thing-7B") && query.contains("https://example.org/thing"), "{query}");
  let run = ground.app.store.research_run(1).unwrap();
  assert_eq!(
    (run.outcome.as_deref(), run.document_id, run.tells),
    (Some("answered"), Some(501), false)
  );

  assert_eq!(ground.asked_path("artifact", "GET", "/documents/501/content").len(), 1);
  assert_eq!(ground.asked_path("artifact", "GET", "/shares?documentId=501").len(), 1);
  let made = ground.asked_path("artifact", "POST", "/shares");
  let english: Vec<_> = made.iter().filter(|a| a.body["documentId"] == 501).collect();
  assert_eq!(english.len(), 1, "the run's own link stands");
  assert!(english[0].body["expiresAt"].is_i64());

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
  assert!(order.contains(&words::document().manual[0].plan_heading));
  assert!(told[0]["summary"].as_str().unwrap().contains("Thing-7B"));

  let filed = ground.app.store.item(item.id).unwrap();
  assert_eq!(filed.state, ItemState::Fit);
  assert_eq!(filed.fit_nodes, ["n3"]);
  assert_eq!(filed.run_id, Some(1));
  assert_eq!(filed.share_url.as_deref(), Some("https://share.example/d501"));
  let run = ground.app.store.runs(Some(item.id), None, None, 10).unwrap().items;
  assert_eq!((run[0].state.as_str(), run[0].outcome.as_deref()), ("closed", Some("answered")));
}

#[tokio::test]
async fn a_report_is_rendered_in_korean_part_by_part_filed_and_told_with_both_links() {
  let ground = support::ground(|s| {
    s.translation.part_chars = 400;
    s.translation.parts_out = 2;
  })
  .await;
  give_owner_room(&ground.app).await;
  let item = ground
    .app
    .store
    .ask("it", "example.org/thing", "Thing-7B", "https://example.org/thing", Shape::Manual)
    .unwrap();
  let content = long_manual(300);
  file(&ground, 1, 501, content.clone());

  settle(&ground).await;

  assert_eq!(ground.asked_path("artifact", "GET", "/documents/501/content").len(), 1);
  let parts = cut(&content, 400);
  assert_eq!(parts.len(), 2, "{parts:#?}");
  let asks = translations(&ground);
  assert_eq!(asks.len(), 4, "the title, the description and two parts");
  for ask in &asks {
    assert_eq!((ask["purpose"].as_str(), ask["language"].as_str()), (Some("document"), Some("Korean")));
  }
  assert_eq!(asks[0]["text"], "Report 501");
  assert_eq!(asks[0]["note"], title_note("Thing-7B").as_str());
  assert_eq!(asks[1]["text"], "It is a thing. It does more.");
  let mut sent: Vec<&str> = asks[2..].iter().map(|a| a["text"].as_str().unwrap()).collect();
  sent.sort();
  let mut expected: Vec<&str> = parts.iter().map(String::as_str).collect();
  expected.sort();
  assert_eq!(sent, expected);
  for ask in &asks[1..] {
    assert_eq!(ask["note"], note("Thing-7B").as_str());
  }
  assert!(ground.world.lock().unwrap().translating_most <= 2);

  let filed = ground.renderings();
  assert_eq!(filed.len(), 1);
  let record = &filed[0].body;
  assert_eq!(record["kind"], "record");
  assert_eq!(record["title"], translated("Report 501").as_str());
  assert_eq!(record["description"], translated("It is a thing. It does more.").as_str());
  assert_eq!(record["actorName"], "bmo-research");
  let body = format!("{}\n\n{}", translated(parts[0].trim()), translated(parts[1].trim()));
  assert_eq!(record["body"], body.as_str());
  assert_eq!(
    record["labels"],
    json!({"research": item.id.to_string(), "translationOf": "501", "language": "ko"})
  );
  let shared = ground.asked_path("artifact", "POST", "/shares");
  assert_eq!(shared.iter().map(|a| a.body["documentId"].clone()).collect::<Vec<_>>(), [501, 901]);

  let told = orders(&ground);
  assert_eq!(told.len(), 1);
  let order = told[0]["said"]["order"].as_str().unwrap();
  let report = words::report();
  let korean = order.find("https://share.example/d901").expect(order);
  let english = order
    .find(&words::fill(&report.english, &[("link", "https://share.example/d501")]))
    .expect(order);
  assert!(korean < english, "the Korean link first: {order}");
  assert!(order.contains(&translated("Report 501")) && order.contains(&translated("It is a thing.")));
  assert!(!order.contains(&report.pending));
  assert!(order.contains(&report.fit));

  let rendered = ground.app.store.item(item.id).unwrap();
  assert_eq!(rendered.ko_document_id, Some(901));
  assert_eq!(rendered.ko_share_url.as_deref(), Some("https://share.example/d901"));
  assert_eq!(rendered.document_id, Some(501));
  assert_eq!(ground.app.store.translation_state(item.id).unwrap(), (1, None));
}

#[tokio::test]
async fn a_part_the_translator_finds_too_large_is_halved_and_asked_again() {
  let ground = support::ground(|s| s.translation.part_chars = 400).await;
  give_owner_room(&ground.app).await;
  let item = ground
    .app
    .store
    .ask("it", "example.org/h", "H", "https://example.org/h", Shape::Investigation)
    .unwrap();
  let content = format!("## Findings\n\n{}\n", prose(300));
  file(&ground, 1, 60, content.clone());
  ground.world.lock().unwrap().too_large_over = Some(200);

  settle(&ground).await;

  let asks = translations(&ground);
  let texts: Vec<&str> = asks.iter().map(|a| a["text"].as_str().unwrap()).collect();
  let whole = texts.iter().position(|t| *t == content).expect("the part was asked whole first");
  let halves = &texts[whole + 1..];
  assert_eq!(halves.len(), 2, "{halves:#?}");
  assert_eq!(halves.concat(), content, "the halves are the part, in order");
  let record = &ground.renderings()[0].body;
  let body = record["body"].as_str().unwrap();
  assert!(body.starts_with(&translated(halves[0].trim())) && body.ends_with(&translated(halves[1].trim())));
  assert!(ground.app.store.item(item.id).unwrap().ko_share_url.is_some());
}

#[tokio::test]
async fn a_rendering_no_road_carries_is_pending_and_its_retry_sends_the_follow_up() {
  let ground = support::ground(|s| s.retry_minutes = 0).await;
  give_owner_room(&ground.app).await;
  let item = ground
    .app
    .store
    .ask("it", "example.org/thing", "Thing-7B", "https://example.org/thing", Shape::Manual)
    .unwrap();
  file(&ground, 1, 501, manual_document());
  ground.world.lock().unwrap().translation_answers.push_back(503);
  let report = words::report();

  settle(&ground).await;

  let told = orders(&ground);
  assert_eq!(told.len(), 1);
  let order = told[0]["said"]["order"].as_str().unwrap();
  assert!(order.contains("Report 501 — It is a thing. https://share.example/d501"), "{order}");
  assert!(order.contains(&report.pending) && order.contains(&report.fit), "{order}");
  assert!(!order.contains("[ko]"));
  let pending = ground.app.store.item(item.id).unwrap();
  assert_eq!((pending.state, pending.ko_share_url), (ItemState::Fit, None));
  let (tries, retry_at) = ground.app.store.translation_state(item.id).unwrap();
  assert_eq!(tries, 1);
  assert!(retry_at.is_some());
  assert!(ground.renderings().is_empty());

  settle(&ground).await;

  assert_eq!(ground.run_seats().len(), 1, "the run is not asked again");
  let told = orders(&ground);
  assert_eq!(told.len(), 2);
  assert_eq!(told[1]["kind"], format!("research-report:{}", item.id), "the same kind replaces it");
  let follow = told[1]["said"]["order"].as_str().unwrap();
  let expected = words::fill(
    &report.translated,
    &[("title", &translated("Report 501")), ("link", "https://share.example/d901")],
  );
  assert!(follow.starts_with(&expected), "{follow}");
  assert!(follow.contains(&words::fill(&report.english, &[("link", "https://share.example/d501")])));
  assert!(follow.contains(&report.fit), "a fit item's follow-up keeps what acts on it: {follow}");
  let rendered = ground.app.store.item(item.id).unwrap();
  assert_eq!(rendered.ko_share_url.as_deref(), Some("https://share.example/d901"));
  assert_eq!(ground.app.store.translation_state(item.id).unwrap(), (2, None));
  let reports = ground.app.store.reports(item.id).unwrap();
  assert!(reports.contains(&("room:7".into(), "translated".into(), "sent".into())));

  settle(&ground).await;
  assert_eq!(orders(&ground).len(), 2, "a filed rendering is told once");
}

#[tokio::test]
async fn a_rendering_still_failing_after_its_retries_is_no_longer_asked() {
  let ground = support::ground(|s| {
    s.retry_minutes = 0;
    s.retries = 1;
  })
  .await;
  give_owner_room(&ground.app).await;
  let item = ground
    .app
    .store
    .ask("it", "example.org/n", "N", "https://example.org/n", Shape::Investigation)
    .unwrap();
  file(&ground, 1, 70, "# N\n\nIt is a thing.\n".into());
  ground.world.lock().unwrap().translation_answers.extend([503, 503]);

  for _ in 0..4 {
    settle(&ground).await;
  }

  assert_eq!(translations(&ground).len(), 2, "the first try and one retry");
  assert_eq!(ground.app.store.translation_state(item.id).unwrap(), (2, None));
  assert_eq!(orders(&ground).len(), 1, "no follow-up");
  assert_eq!(ground.app.store.item(item.id).unwrap().ko_share_url, None);
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
  ground.world.lock().unwrap().session_answers.push_back(503);
  // The refused ask opened run 1 and closed it; the second try is run 2.
  file(&ground, 2, 9, "# An investigation\n".into());

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
  ground.world.lock().unwrap().failing.extend(1..=4);
  for _ in 0..6 {
    settle(&ground).await;
  }
  let failed = ground.app.store.item(item.id).unwrap();
  assert_eq!((failed.state, failed.tries), (ItemState::Failed, 4));
  assert_eq!(ground.run_seats().len(), 4, "a failed item is asked no more");
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
  let w = &words::document().advisory[0];
  let document = format!(
    "# SA-1\n\n| {} | {} | {} | {} | {} |\n|---|---|---|---|---|\n| libexample | 1.4.1 | services/alpha | node-a | {} |\n| libexample | 1.4.1 | services/beta | | {} |\n\n## {}\n\n1. upgrade\n",
    w.component, w.version, w.repository, w.host, w.touched, w.yes, w.yes, w.plan_heading
  );
  file(&ground, 1, 77, document);

  settle(&ground).await;

  let query = ground.turns_named("research-1")[0]["prompt"].as_str().unwrap().to_string();
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
  let w = &words::document().brief[0];
  let document = format!(
    "# Brief\n\n| {} | {} | {} | {} | {} |\n|---|---|---|---|---|\n| index-a | 2,650.1 | pt | 2026-10-29 | [page](https://example.org/e1) |\n",
    w.name, w.value, w.unit, w.as_of, w.source
  );
  file(&ground, 1, 300, document);

  settle(&ground).await;

  assert_eq!(ground.run_seats().len(), 1, "one run for the gathering");
  let first = ground.turns_named("research-1");
  let query = first[0]["prompt"].as_str().unwrap();
  assert!(query.contains("https://example.org/e1") && query.contains("https://example.org/e2"));
  for id in [one.id, two.id] {
    assert_eq!(store.item(id).unwrap().state, ItemState::Reported);
  }
  assert_eq!(orders(&ground).len(), 1, "the brief is told once");
  let facts = store.facts(None, None, 10).unwrap().items;
  assert_eq!(facts.len(), 1);
  assert_eq!(
    (facts[0].name.as_str(), facts[0].value.as_str(), facts[0].run_id, facts[0].document_id),
    ("index-a", "2,650.1", Some(1), Some(300))
  );
  assert_eq!(facts[0].source_address.as_deref(), Some("https://example.org/e1"));

  // The next gathering waits its gathering time.
  store
    .ask("economy", "example.org/e3", "E3", "https://example.org/e3", Shape::Brief)
    .unwrap();
  settle(&ground).await;
  assert_eq!(ground.run_seats().len(), 1);
}

#[tokio::test]
async fn a_stopped_category_starts_no_run_and_the_least_recent_category_goes_first() {
  let ground = support::ground(|_| {}).await;
  let store = &ground.app.store;
  store.set_running(Some("it"), Running::Stopped).unwrap();
  let it = store.ask("it", "example.org/i", "I", "https://example.org/i", Shape::Manual).unwrap();
  settle(&ground).await;
  assert!(ground.run_seats().is_empty());
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
  let first = &ground.turns_named("research-1")[0];
  assert!(first["prompt"].as_str().unwrap().contains("https://example.org/s"));
  assert_eq!(store.item(sec.id).unwrap().tries, 1);
  assert_eq!(store.item(it.id).unwrap().tries, 1, "it waits its turn");
}

#[tokio::test]
async fn a_restarted_daemon_cuts_the_runs_it_left_open_and_takes_up_its_tries() {
  let ground = support::ground(|s| s.retry_minutes = 0).await;
  let store = &ground.app.store;
  let asked = store
    .ask("it", "example.org/r1", "R1", "https://example.org/r1", Shape::Investigation)
    .unwrap();
  let never = store
    .ask("it", "example.org/r2", "R2", "https://example.org/r2", Shape::Investigation)
    .unwrap();
  let open = |tells: bool, room: Option<i64>| {
    store
      .open_research_run(&NewResearchRun {
        key: "search:exhaustive",
        line: "search",
        query: "R1",
        asker: None,
        options: None,
        room_id: room,
        provider: "registered",
        model: "flash-next",
        closes_at: now_ms(),
        branches_allowed: 0,
        tells,
      })
      .unwrap()
  };
  let research = open(false, None);
  store.hold(research, "seat", "seat-left").unwrap();
  store.hold(research, "session", "search@apart:9").unwrap();
  let asked_by_room = open(true, Some(5));
  store.keep_research_turn(asked_by_room, "research-2", Some("Half of it.")).unwrap();
  let (run, _) = store.start_try("it", &[asked.id]).unwrap().unwrap();
  store.keep_run_id(run, research).unwrap();
  store.start_try("it", &[never.id]).unwrap().unwrap();

  assert_eq!(research_server::runner::sweep(&ground.app).await, 2);
  take_up(&ground.app);
  for _ in 0..100 {
    if store.item(asked.id).unwrap().state == ItemState::Waiting {
      break;
    }
    tokio::time::sleep(std::time::Duration::from_millis(20)).await;
  }
  tokio::time::sleep(std::time::Duration::from_millis(50)).await;

  let cut = store.research_run(research).unwrap();
  assert_eq!((cut.state, cut.outcome.as_deref()), (ResearchRunState::Closed, Some("cut")));
  let gave_back = ground.asked_path("agency", "PATCH", "/seat-requests/seat-left");
  assert_eq!(gave_back[0].body["state"], "finished");
  assert_eq!(ground.asked_path("agency", "DELETE", "/sessions/search%40apart%3A9").len(), 1);
  let told = orders(&ground);
  assert_eq!(told.len(), 1, "a run the deep dive asked tells no room");
  assert_eq!(
    (told[0]["room"].as_i64(), told[0]["kind"].as_str()),
    (Some(5), Some("research-ended:2"))
  );
  assert!(told[0]["said"]["order"].as_str().unwrap().contains("Half of it."));
  assert!(told[0].get("run").is_none());

  let retried = store.item(asked.id).unwrap();
  assert_eq!((retried.state, retried.tries), (ItemState::Waiting, 1));
  let tries = store.runs(Some(asked.id), None, None, 10).unwrap().items;
  assert_eq!(tries[0].state.as_str(), "cut");
  assert!(ground.run_seats().is_empty(), "an asked run is not asked again");
  let waiting = store.item(never.id).unwrap();
  assert_eq!((waiting.state, waiting.tries), (ItemState::Waiting, 0));
}
