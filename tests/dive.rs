//! The deep dive against stand-in agency, artifact, receiver, voice and tool host servers (`docs/api.md`, 「The
//! deep dive」, 「The page」 and 「The notices」): the bodies each unit is asked with, an interrupted try asked
//! again at once uncounted and a failed one counted and waited, an item stuck past its retries told once,
//! handed to the helper and still asked until it is filed or dismissed, a result short of its shape asked again whole and never filed short, a final answer in two
//! text blocks filed whole, the one page with both languages and its switch, the narration's sounds, a
//! missing one logged and asked again on every retry wait, a synthesis waited on however long with its
//! reasons logged, and a long script cut to what the voice speaks, the pending Korean side drawn into the
//! same page, the report held until the page is whole and told once, across a restart too, an advisory's
//! parts to the workers of its repositories, a brief's gathering with its facts, the page opening with what
//! its subject is and saying under its title where the research came from, and a result with no opening
//! asked again whole. The filed documents are
//! composed from the English words `prompts/document.toml` holds, so this file holds none of them.

mod support;

use bmo_research_client::{ItemState, NewStackRow, RecipientKind, ResearchRunState, Running, Shape};
use research_server::App;
use research_server::dive::{VOICE_MOST_CHARACTERS, give_owner_room, speakable, take_up, tick};
use research_server::document::{draw, page_body};
use research_server::runner::narration_script;
use research_server::store::{Asked, NewResearchRun, Parent, Store, now_ms};
use research_server::translation::{Rendering, cut, narration_note, note, translate_part};
use research_server::words;
use serde_json::Value;
use support::{Filed, Ground, KO, result_text, spoken, translated};

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

/// The script a result's narration says.
const SCRIPT: &str = "Thing-7B is a small model. It fits one card, and its maker's figures hold.";

/// A document with the narration's script after it.
fn narrated(content: String) -> String {
  format!("{content}\nLISTEN:\n{SCRIPT}\n")
}

/// A heading of the investigation's sections.
fn answer_heading() -> String {
  words::document().sections.investigation[0].clone()
}

/// The page's English body as the run drew it, for a document filed by `file`.
fn english_body(id: i64, content: &str) -> String {
  let result = result_text(&Filed {
    id,
    title: format!("Report {id}"),
    description: "It is a thing. It does more.".into(),
    content: content.to_string(),
  });
  let drawn = draw(&result, words::search_document("en"));
  page_body(&drawn, &format!("Report {id}"), &words::document().opening.manual)
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

  let filed = ground.run_documents();
  assert_eq!(filed.len(), 1, "one document");
  assert_eq!(
    (filed[0]["kind"].as_str(), filed[0]["title"].as_str()),
    (Some("page"), Some("Report 501"))
  );
  assert_eq!(filed[0]["labels"]["languages"], "en");
  assert!(filed[0]["html"].as_str().unwrap().contains(&words::document().manual[0].plan_heading));
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
  assert!(order.contains(&words::document().manual[0].plan_heading));
  assert!(told[0]["summary"].as_str().unwrap().contains("Thing-7B"));

  let filed = ground.app.store.item(item.id).unwrap();
  assert_eq!(filed.state, ItemState::Fit);
  assert_eq!(filed.run_id, Some(1));
  assert_eq!(filed.share_url.as_deref(), Some("https://share.example/d501"));
  let run = ground.app.store.runs(Some(item.id), None, None, 10).unwrap().items;
  assert_eq!((run[0].state.as_str(), run[0].outcome.as_deref()), ("closed", Some("answered")));
}

#[tokio::test]
async fn a_report_s_korean_side_is_drawn_into_the_same_page_with_a_switch_and_told_with_one_link() {
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

  assert_eq!(ground.run_documents().len(), 1, "one document, not two");
  let text = format!("# Report 501\n\n{}", english_body(501, &content));
  let parts = cut(&text, 400);
  assert_eq!(parts.len(), 3, "the title's heading and two sections: {parts:#?}");
  let asks = translations(&ground);
  assert_eq!(asks.len(), 4, "the description and three parts");
  for ask in &asks {
    assert_eq!((ask["purpose"].as_str(), ask["language"].as_str()), (Some("document"), Some("Korean")));
    assert_eq!(ask["note"], note("Thing-7B").as_str());
  }
  assert_eq!(asks[0]["text"], "It is a thing. It does more.");
  let mut sent: Vec<&str> = asks[1..].iter().map(|a| a["text"].as_str().unwrap()).collect();
  sent.sort();
  let mut expected: Vec<&str> = parts.iter().map(String::as_str).collect();
  expected.sort();
  assert_eq!(sent, expected);
  assert!(parts[0].starts_with("# Report 501"), "the title is the body's heading");
  assert!(ground.world.lock().unwrap().translating_most <= 2);

  let rewrites = ground.rewrites(501);
  assert_eq!(rewrites.len(), 1);
  let korean_title = format!("{KO}Report 501");
  assert_eq!(rewrites[0]["title"], korean_title.as_str());
  assert_eq!(rewrites[0]["labels"]["languages"], "ko,en");
  assert_eq!(rewrites[0]["labels"]["research"], "1");
  let html = rewrites[0]["html"].as_str().unwrap();
  let page = words::page();
  for part in [
    "<html lang=\"ko\" data-shown=\"ko\">",
    "<style data-archive>",
    "data-show=\"ko\" lang=\"ko\" aria-pressed=\"true\"",
    "data-show=\"en\" lang=\"en\" aria-pressed=\"false\"",
    page.language("ko").unwrap().name.as_str(),
    page.language("en").unwrap().name.as_str(),
    &format!("<h1 data-language=\"ko\" lang=\"ko\">{korean_title}</h1>"),
    "<h1 data-language=\"en\" lang=\"en\">Report 501</h1>",
    "<article data-language=\"ko\" lang=\"ko\">",
    "<article data-language=\"en\" lang=\"en\">",
    "<script>",
  ] {
    assert!(html.contains(part), "{part} missing from the page");
  }
  assert!(html.find("data-show=\"ko\"") < html.find("data-show=\"en\""), "Korean first");
  assert!(!html.contains("<audio"), "the page draws no player");
  let shared = ground.asked_path("artifact", "POST", "/shares");
  assert_eq!(shared.iter().map(|a| a.body["documentId"].clone()).collect::<Vec<_>>(), [501]);

  let told = orders(&ground);
  assert_eq!(told.len(), 1);
  let order = told[0]["said"]["order"].as_str().unwrap();
  assert_eq!(order.matches("https://").count(), 1, "one link: {order}");
  assert!(
    order.starts_with(&format!("{korean_title} — {KO}It is a thing. https://share.example/d501")),
    "{order}"
  );
  assert!(order.contains(&words::report().fit));

  let rendered = ground.app.store.item(item.id).unwrap();
  assert_eq!(rendered.languages, ["ko", "en"]);
  assert!(rendered.narrations.is_empty());
  assert_eq!(rendered.document_id, Some(501));
  let state = ground.app.store.page_state(item.id).unwrap();
  assert_eq!((state.translation_tries, state.retry_at), (1, None));
}

#[tokio::test]
async fn a_narration_is_spoken_in_both_languages_and_attached_the_korean_at_the_head() {
  let ground = support::ground(|_| {}).await;
  give_owner_room(&ground.app).await;
  let item = ground
    .app
    .store
    .ask("it", "example.org/thing", "Thing-7B", "https://example.org/thing", Shape::Manual)
    .unwrap();
  file(&ground, 1, 501, narrated(manual_document()));

  settle(&ground).await;

  let filed = ground.run_documents();
  assert!(
    !filed[0]["html"].as_str().unwrap().contains(SCRIPT),
    "the script is not the page's text"
  );
  let asks = translations(&ground);
  let script = asks.iter().find(|a| a["text"] == SCRIPT).expect("the script is translated");
  assert_eq!(script["note"], narration_note("Thing-7B").as_str());
  let korean = translated(SCRIPT);
  assert_eq!(ground.spoken(), [korean.clone(), SCRIPT.to_string()]);
  let kept = ground.world.lock().unwrap().attachments.clone();
  let sounds: Vec<_> = kept
    .iter()
    .filter(|a| a.document == 501)
    .map(|a| (a.name.as_str(), a.media_type.as_str(), a.role.as_deref(), a.bytes.clone()))
    .collect();
  assert_eq!(
    sounds,
    [
      ("narration-ko.ogg", "audio/ogg", Some("head"), spoken(&korean)),
      ("narration-en.ogg", "audio/ogg", None, spoken(SCRIPT)),
    ]
  );

  let rewrites = ground.rewrites(501);
  assert_eq!(rewrites.len(), 1, "the page is drawn again once, with both sides and both sounds");
  let html = rewrites[0]["html"].as_str().unwrap();
  assert!(html.contains("class=\"listen\""), "{html}");
  for code in ["ko", "en"] {
    let audio = format!("<audio src=\"attachment:narration-{code}.ogg\" data-language=\"{code}\"></audio>");
    assert!(html.contains(&audio), "{audio} missing from {html}");
  }
  assert!(!html.contains("data:audio"), "the page refers to its sounds by name");
  let rendered = ground.app.store.item(item.id).unwrap();
  assert_eq!(rendered.narrations, ["ko", "en"]);
  assert_eq!(ground.app.store.page_state(item.id).unwrap().retry_at, None);
  let order = orders(&ground)[0]["said"]["order"].as_str().unwrap().to_string();
  assert!(
    !order.contains("narration-") && !order.contains(SCRIPT),
    "the notice says nothing of it"
  );
}

#[tokio::test]
async fn a_result_with_no_narration_asks_its_session_for_one_before_it_is_filed() {
  let ground = support::ground(|_| {}).await;
  give_owner_room(&ground.app).await;
  ground
    .app
    .store
    .ask("it", "example.org/thing", "Thing-7B", "https://example.org/thing", Shape::Manual)
    .unwrap();
  file(&ground, 1, 501, manual_document());
  ground.world.lock().unwrap().narration = Some(format!("LISTEN:\n{SCRIPT}\n"));

  settle(&ground).await;

  let turns = ground.asked("agency");
  let asked: Vec<&Value> = turns
    .iter()
    .filter(|a| a.path.ends_with("/turns") && a.body["name"] == "research-1-narration")
    .map(|a| &a.body)
    .collect();
  assert_eq!(asked.len(), 1, "the narration is asked once");
  let prompt = asked[0]["prompt"].as_str().unwrap();
  assert!(prompt.starts_with(&words::runner().narration.question), "{prompt}");
  assert!(prompt.contains("## The narration"), "the rule is handed: {prompt}");
  let last = turns.iter().rposition(|a| a.path.ends_with("/turns")).unwrap();
  assert_eq!(turns[last].body["name"], "research-1-narration", "it is the session's last turn");
  assert!(
    turns[last + 1..]
      .iter()
      .any(|a| a.method == "DELETE" && a.path.starts_with("/sessions/")),
    "the session is ended after it"
  );
  let written = ground.app.store.research_run(1).unwrap().written.unwrap();
  assert!(written.trim_end().ends_with(&format!("LISTEN:\n{SCRIPT}")), "{written}");
  assert_eq!(ground.spoken(), [translated(SCRIPT), SCRIPT.to_string()]);
  assert_eq!(narration_script(&format!("LISTEN:\n\n{SCRIPT}\n")), SCRIPT);
}

#[tokio::test]
async fn a_result_carrying_its_narration_asks_for_none() {
  let ground = support::ground(|_| {}).await;
  give_owner_room(&ground.app).await;
  ground
    .app
    .store
    .ask("it", "example.org/thing", "Thing-7B", "https://example.org/thing", Shape::Manual)
    .unwrap();
  file(&ground, 1, 501, narrated(manual_document()));
  ground.world.lock().unwrap().narration = Some("Another script.".into());

  settle(&ground).await;

  assert!(
    ground.asked("agency").iter().all(|a| a.body["name"] != "research-1-narration"),
    "no narration is asked"
  );
  assert_eq!(ground.spoken(), [translated(SCRIPT), SCRIPT.to_string()]);
}

/// The lines the first run's log holds under `kind`.
fn logged(ground: &Ground, kind: &str) -> Vec<String> {
  ground
    .app
    .store
    .research_log(1)
    .unwrap()
    .into_iter()
    .filter(|line| line.kind == kind)
    .map(|line| line.text)
    .collect()
}

/// How many of the `it` category's items are finishing, as the state answers it.
fn finishing(ground: &Ground) -> i64 {
  let state = ground.app.store.daemon_state().unwrap();
  state.categories.iter().find(|c| c.category_name == "it").unwrap().finishing_count
}

#[tokio::test]
async fn a_report_waits_while_a_narration_is_pending_and_is_told_once_it_is_attached() {
  let ground = support::ground(|s| s.retry_minutes = 0).await;
  give_owner_room(&ground.app).await;
  let item = ground
    .app
    .store
    .ask("it", "example.org/thing", "Thing-7B", "https://example.org/thing", Shape::Manual)
    .unwrap();
  file(&ground, 1, 501, narrated(manual_document()));
  ground.world.lock().unwrap().voice_failing = true;

  settle(&ground).await;

  assert_eq!(ground.run_documents().len(), 1, "the page is filed");
  let rewrites = ground.rewrites(501);
  assert_eq!(rewrites.len(), 1, "drawn again with its Korean side");
  assert!(!rewrites[0]["html"].as_str().unwrap().contains("<audio"));
  assert!(ground.world.lock().unwrap().attachments.is_empty());
  let waiting = ground.app.store.item(item.id).unwrap();
  assert_eq!(
    (waiting.state, waiting.languages.clone()),
    (ItemState::Finishing, vec!["ko".to_string(), "en".to_string()])
  );
  assert!(waiting.narrations.is_empty());
  assert_eq!(waiting.pending, ["narration-ko", "narration-en"]);
  assert!(waiting.pending_retry_at.is_some(), "the narrations are asked again");
  assert_eq!(finishing(&ground), 1);
  assert!(orders(&ground).is_empty(), "nobody is told while a narration is pending");
  assert!(ground.app.store.reports(item.id).unwrap().is_empty());
  let waits = logged(&ground, "filed");
  assert!(
    waits
      .iter()
      .any(|text| text.contains("waits") && text.contains("the ko narration and the en narration")),
    "the run's log says what the report waits on: {waits:?}"
  );
  assert!(logged(&ground, "told").is_empty());

  ground.world.lock().unwrap().voice_failing = false;
  settle(&ground).await;

  let rewrites = ground.rewrites(501);
  assert_eq!(rewrites.len(), 2, "a sound attached draws the page again with its band");
  let html = rewrites[1]["html"].as_str().unwrap();
  assert!(html.contains("attachment:narration-ko.ogg") && html.contains("attachment:narration-en.ogg"));
  assert!(html.contains("data-language=\"ko\" lang=\"ko\""), "the Korean side stays: {html}");
  let attached: Vec<String> =
    ground.world.lock().unwrap().attachments.iter().map(|a| a.name.clone()).collect();
  assert_eq!(attached, ["narration-ko.ogg", "narration-en.ogg"]);
  let told = ground.app.store.item(item.id).unwrap();
  assert_eq!(told.narrations, ["ko", "en"]);
  assert_eq!(
    (told.state, told.pending.is_empty(), told.pending_retry_at),
    (ItemState::Fit, true, None)
  );
  assert_eq!(finishing(&ground), 0);
  let state = ground.app.store.page_state(item.id).unwrap();
  assert_eq!((state.narration_tries, state.translation_tries, state.retry_at), (2, 1, None));
  let said = orders(&ground);
  assert_eq!(said.len(), 1, "the report is told once the page is whole");
  assert_eq!(said[0]["kind"], format!("research-report:{}", item.id));
  let order = said[0]["said"]["order"].as_str().unwrap();
  assert!(
    order.starts_with(&format!("{KO}Report 501 — {KO}It is a thing. https://share.example/d501")),
    "{order}"
  );
  assert!(
    !order.contains("narration-") && !order.contains(SCRIPT),
    "the notice says nothing of it"
  );
  assert_eq!(logged(&ground, "told").len(), 1, "the run's log says when it was told");

  settle(&ground).await;
  assert_eq!(ground.rewrites(501).len(), 2, "nothing more is asked");
  assert_eq!(ground.world.lock().unwrap().attachments.len(), 2, "no sound is made again");
  assert_eq!(orders(&ground).len(), 1, "a report is told once");
}

#[tokio::test]
async fn a_failing_narration_is_asked_again_on_every_retry_wait_until_it_is_made() {
  let ground = support::ground(|s| s.retry_minutes = 0).await;
  give_owner_room(&ground.app).await;
  let item = ground
    .app
    .store
    .ask("it", "example.org/thing", "Thing-7B", "https://example.org/thing", Shape::Manual)
    .unwrap();
  file(&ground, 1, 501, narrated(manual_document()));
  ground.world.lock().unwrap().voice_failing = true;

  for _ in 0..3 {
    settle(&ground).await;
  }

  assert_eq!(ground.spoken().len(), 6, "each language on every try, the third among them");
  let state = ground.app.store.page_state(item.id).unwrap();
  assert_eq!(state.narration_tries, 3);
  assert!(state.retry_at.is_some(), "no count of tries gives it up");
  assert_eq!(ground.app.store.item(item.id).unwrap().state, ItemState::Finishing);
  assert_eq!(ground.rewrites(501).len(), 1, "the Korean side alone");
  assert!(orders(&ground).is_empty());
  let waits: Vec<String> = logged(&ground, "filed").into_iter().filter(|t| t.contains("waits")).collect();
  assert_eq!(waits.len(), 1, "what it waits on is said once while it stands: {waits:?}");

  ground.world.lock().unwrap().voice_failing = false;
  settle(&ground).await;

  assert_eq!(ground.spoken().len(), 8);
  let told = ground.app.store.item(item.id).unwrap();
  assert_eq!(
    (told.state, told.narrations.clone()),
    (ItemState::Fit, vec!["ko".to_string(), "en".to_string()])
  );
  assert_eq!(ground.app.store.page_state(item.id).unwrap().narration_tries, 4);
  assert_eq!(orders(&ground).len(), 1);
}

#[tokio::test]
async fn a_narration_goes_to_the_voice_as_its_written_words_and_a_refusal_leaves_it_pending() {
  let ground = support::ground(|s| s.retry_minutes = 0).await;
  give_owner_room(&ground.app).await;
  let item = ground
    .app
    .store
    .ask("it", "example.org/thing", "Thing-7B", "https://example.org/thing", Shape::Manual)
    .unwrap();
  file(&ground, 1, 501, narrated(manual_document()));
  ground.world.lock().unwrap().voice_refusing = true;

  settle(&ground).await;

  let asked = ground.asked_path("voice", "POST", "/syntheses");
  assert_eq!(asked.len(), 2, "each language asked once");
  for synthesis in &asked {
    assert!(
      synthesis.body.get("spoken").is_none(),
      "the voice unit derives the spoken words: {:?}",
      synthesis.body
    );
  }
  let languages: Vec<&str> =
    asked.iter().filter_map(|synthesis| synthesis.body["language"].as_str()).collect();
  assert_eq!(languages, ["ko", "en"], "each narration is said in its own language");
  assert!(ground.world.lock().unwrap().attachments.is_empty());
  let waiting = ground.app.store.item(item.id).unwrap();
  assert_eq!(waiting.state, ItemState::Finishing);
  assert_eq!(waiting.pending, ["narration-ko", "narration-en"]);
  let narrated = logged(&ground, "narration");
  assert!(
    narrated
      .iter()
      .any(|text| text.contains("unconverted-run") && text.contains("Thing-7B")),
    "the run's log carries the refusal's code and sentence: {narrated:?}"
  );
  assert!(orders(&ground).is_empty());

  ground.world.lock().unwrap().voice_refusing = false;
  settle(&ground).await;

  let told = ground.app.store.item(item.id).unwrap();
  assert_eq!(told.narrations, ["ko", "en"]);
  assert_eq!(orders(&ground).len(), 1);
}

#[tokio::test]
async fn a_missing_english_sound_is_logged_and_retried_on_the_retry_wait() {
  let ground = support::ground(|s| s.retry_minutes = 0).await;
  give_owner_room(&ground.app).await;
  let item = ground
    .app
    .store
    .ask("it", "example.org/thing", "Thing-7B", "https://example.org/thing", Shape::Manual)
    .unwrap();
  file(&ground, 1, 501, narrated(manual_document()));
  ground.world.lock().unwrap().voice_failing_on = Some(SCRIPT.to_string());

  settle(&ground).await;

  let attached: Vec<String> =
    ground.world.lock().unwrap().attachments.iter().map(|a| a.name.clone()).collect();
  assert_eq!(attached, ["narration-ko.ogg"], "the Korean sound alone");
  let waiting = ground.app.store.item(item.id).unwrap();
  assert_eq!(waiting.narrations, ["ko"]);
  assert_eq!(
    (waiting.state, waiting.pending.clone()),
    (ItemState::Finishing, vec!["narration-en".to_string()])
  );
  let state = ground.app.store.page_state(item.id).unwrap();
  assert_eq!(state.narration_tries, 1);
  assert!(state.retry_at.is_some(), "the English sound is asked again");
  assert!(orders(&ground).is_empty(), "the report waits for the English sound");
  let narrated = logged(&ground, "narration");
  let failed = narrated.iter().find(|text| text.contains("en narration was not made"));
  let said = failed.unwrap_or_else(|| panic!("the run's log says why: {narrated:?}"));
  let size = SCRIPT.chars().count().to_string();
  assert!(said.contains(&size), "the synthesis is named with its text's length: {said}");

  ground.world.lock().unwrap().voice_failing_on = None;
  settle(&ground).await;

  let attached: Vec<String> =
    ground.world.lock().unwrap().attachments.iter().map(|a| a.name.clone()).collect();
  assert_eq!(attached, ["narration-ko.ogg", "narration-en.ogg"]);
  assert_eq!(ground.app.store.item(item.id).unwrap().narrations, ["ko", "en"]);
  assert_eq!(ground.spoken().iter().filter(|text| text.as_str() == SCRIPT).count(), 2);
  let state = ground.app.store.page_state(item.id).unwrap();
  assert_eq!((state.narration_tries, state.retry_at), (2, None));
  assert_eq!(orders(&ground).len(), 1);
}

#[tokio::test]
async fn a_page_a_stopped_daemon_left_finishing_is_finished_and_told_by_the_next() {
  let ground = support::ground(|s| s.retry_minutes = 0).await;
  give_owner_room(&ground.app).await;
  let item = ground
    .app
    .store
    .ask("it", "example.org/thing", "Thing-7B", "https://example.org/thing", Shape::Manual)
    .unwrap();
  file(&ground, 1, 501, narrated(manual_document()));
  ground.world.lock().unwrap().voice_failing = true;
  settle(&ground).await;
  assert_eq!(ground.app.store.item(item.id).unwrap().state, ItemState::Finishing);
  ground.world.lock().unwrap().voice_failing = false;

  let settings = (*ground.app.settings).clone();
  let store = Store::open(&settings.store_dir()).unwrap();
  let next = App::new(settings, store, "test-token".into());
  take_up(&next);
  for handle in tick(&next).await {
    handle.await.unwrap();
  }

  let told = next.store.item(item.id).unwrap();
  assert_eq!(
    (told.state, told.narrations.clone()),
    (ItemState::Fit, vec!["ko".to_string(), "en".to_string()])
  );
  assert_eq!(ground.run_seats().len(), 1, "the run is not asked again");
  assert_eq!(orders(&ground).len(), 1);
}

#[tokio::test]
async fn a_script_longer_than_the_voice_speaks_is_spoken_to_its_last_whole_sentence_and_logged() {
  let ground = support::ground(|_| {}).await;
  give_owner_room(&ground.app).await;
  ground
    .app
    .store
    .ask("it", "example.org/thing", "Thing-7B", "https://example.org/thing", Shape::Manual)
    .unwrap();
  let script = prose(VOICE_MOST_CHARACTERS + 300);
  file(&ground, 1, 501, format!("{}\nLISTEN:\n{script}\n", manual_document()));

  settle(&ground).await;

  let spoken = ground.spoken();
  assert_eq!(spoken.len(), 2, "both languages are spoken");
  for text in &spoken {
    assert!(
      text.chars().count() <= VOICE_MOST_CHARACTERS,
      "within the voice: {}",
      text.chars().count()
    );
    assert!(text.ends_with('.'), "cut at a sentence's end");
  }
  let english = spoken.iter().find(|text| !text.starts_with(KO)).unwrap();
  assert!(script.starts_with(english.as_str()));
  assert_eq!(ground.world.lock().unwrap().attachments.len(), 2);
  let logged = ground.app.store.research_log(1).unwrap();
  assert!(
    logged
      .iter()
      .any(|line| line.kind == "narration" && line.text.contains("more than the voice speaks")),
    "the run's log says the script was cut"
  );
  assert_eq!(speakable("One. Two.", 5), "One.");
  assert_eq!(speakable("Onetwothree four", 5), "Onetw", "no sentence and no word fits");
  assert_eq!(speakable(" Short. ", 40), "Short.");
}

#[tokio::test]
async fn a_narration_waits_for_its_synthesis_however_long_and_logs_each_new_reason_once() {
  let ground = support::ground(|_| {}).await;
  give_owner_room(&ground.app).await;
  let item = ground
    .app
    .store
    .ask("it", "example.org/thing", "Thing-7B", "https://example.org/thing", Shape::Manual)
    .unwrap();
  file(&ground, 1, 501, narrated(manual_document()));
  let reasons = ["Every card of voice is in use.", "Voice is starting on a card."];
  ground.world.lock().unwrap().voice_waiting_on =
    vec![reasons[0].into(), reasons[0].into(), reasons[1].into()];

  settle(&ground).await;

  let attached: Vec<String> =
    ground.world.lock().unwrap().attachments.iter().map(|a| a.name.clone()).collect();
  assert_eq!(
    attached,
    ["narration-ko.ogg", "narration-en.ogg"],
    "both are spoken once their wait ends"
  );
  assert_eq!(ground.app.store.item(item.id).unwrap().narrations, ["ko", "en"]);
  let logged: Vec<String> = ground
    .app
    .store
    .research_log(1)
    .unwrap()
    .into_iter()
    .filter(|line| line.kind == "narration")
    .map(|line| line.text)
    .collect();
  for code in ["ko", "en"] {
    let said: Vec<&String> = logged
      .iter()
      .filter(|text| text.starts_with(&format!("The {code} narration waits")))
      .collect();
    assert_eq!(said.len(), 2, "each reason once as it changes: {logged:?}");
    for (line, reason) in said.iter().zip(reasons) {
      assert!(line.ends_with(reason), "{line}");
    }
  }
}

/// Waits, a little at a time, until `done` holds.
async fn until(what: &str, done: impl Fn() -> bool) {
  for _ in 0..400 {
    if done() {
      return;
    }
    tokio::time::sleep(std::time::Duration::from_millis(25)).await;
  }
  panic!("{what} never came");
}

/// How often the voice unit was asked how synthesis `id` stands.
fn readings(ground: &Ground, id: usize) -> usize {
  ground.asked_path("voice", "GET", &format!("/syntheses/{id}")).len()
}

/// A daemon started over the same store and stand-ins, as after a restart.
fn restarted(ground: &Ground) -> std::sync::Arc<App> {
  let settings = (*ground.app.settings).clone();
  let store = Store::open(&settings.store_dir()).unwrap();
  App::new(settings, store, "test-token".into())
}

#[tokio::test]
async fn a_narration_whose_synthesis_still_waits_is_asked_nothing_beside_it_on_any_pass_or_restart() {
  let ground = support::ground(|s| s.retry_minutes = 0).await;
  give_owner_room(&ground.app).await;
  let item = ground
    .app
    .store
    .ask("it", "example.org/thing", "Thing-7B", "https://example.org/thing", Shape::Manual)
    .unwrap();
  file(&ground, 1, 501, narrated(manual_document()));
  ground.world.lock().unwrap().voice_holding = true;

  let mut started = tick(&ground.app).await;
  until("the Korean synthesis's wait", || readings(&ground, 1) > 0).await;
  for _ in 0..4 {
    started.extend(tick(&ground.app).await);
    let read = readings(&ground, 1);
    until("another reading", || readings(&ground, 1) > read).await;
  }

  assert_eq!(ground.spoken().len(), 1, "nothing is asked beside the synthesis on its way");
  let state = ground.app.store.page_state(item.id).unwrap();
  assert_eq!(state.narration_tries, 1, "the page is finished by one try at a time");

  // The daemon stops while the synthesis waits, and the next follows the one it kept.
  for handle in &started {
    handle.abort();
  }
  let next = restarted(&ground);
  let read = readings(&ground, 1);
  let mut again = tick(&next).await;
  until("the kept synthesis read again", || readings(&ground, 1) > read).await;
  again.extend(tick(&next).await);
  assert_eq!(ground.spoken().len(), 1, "the synthesis kept is followed, not asked again");

  ground.world.lock().unwrap().voice_holding = false;
  for handle in again {
    handle.await.unwrap();
  }

  let spoken = ground.spoken();
  assert_eq!(spoken.len(), 2, "one synthesis for each narration: {spoken:?}");
  assert!(spoken[0].starts_with(KO) && spoken[1] == SCRIPT);
  let attached: Vec<String> =
    ground.world.lock().unwrap().attachments.iter().map(|a| a.name.clone()).collect();
  assert_eq!(attached, ["narration-ko.ogg", "narration-en.ogg"]);
  assert_eq!(next.store.item(item.id).unwrap().narrations, ["ko", "en"]);
  assert_eq!(next.store.synthesis(501, "ko").unwrap(), None, "nothing is kept once attached");
  assert_eq!(orders(&ground).len(), 1);
}

#[tokio::test]
async fn a_kept_synthesis_that_failed_or_is_gone_is_asked_again_once() {
  for gone in [false, true] {
    let ground = support::ground(|s| s.retry_minutes = 0).await;
    give_owner_room(&ground.app).await;
    let item = ground
      .app
      .store
      .ask("it", "example.org/thing", "Thing-7B", "https://example.org/thing", Shape::Manual)
      .unwrap();
    file(&ground, 1, 501, narrated(manual_document()));
    ground.world.lock().unwrap().voice_holding = true;
    let started = tick(&ground.app).await;
    until("the Korean synthesis's wait", || readings(&ground, 1) > 0).await;
    for handle in &started {
      handle.abort();
    }
    let kept = ground.app.store.synthesis(501, "ko").unwrap();
    let size = ground.spoken()[0].chars().count() as i64;
    assert_eq!(kept, Some((1, size)), "the synthesis is kept with its text's length");
    {
      let mut world = ground.world.lock().unwrap();
      world.voice_holding = false;
      if gone {
        world.voice_gone.push(1);
      } else {
        world.voice_failed.push(1);
      }
    }

    let next = restarted(&ground);
    for handle in tick(&next).await {
      handle.await.unwrap();
    }

    let spoken = ground.spoken();
    assert_eq!(spoken.len(), 3, "the Korean asked once more, the English once: {spoken:?}");
    assert_eq!(spoken[0], spoken[1]);
    assert_eq!(spoken[2], SCRIPT);
    let attached = ground.world.lock().unwrap().attachments.len();
    assert_eq!(attached, 2);
    assert_eq!(next.store.item(item.id).unwrap().narrations, ["ko", "en"]);
    assert_eq!(orders(&ground).len(), 1);
  }
}

#[tokio::test]
async fn a_synthesis_is_followed_through_readings_the_voice_unit_does_not_answer() {
  let ground = support::ground(|_| {}).await;
  give_owner_room(&ground.app).await;
  let item = ground
    .app
    .store
    .ask("it", "example.org/thing", "Thing-7B", "https://example.org/thing", Shape::Manual)
    .unwrap();
  file(&ground, 1, 501, narrated(manual_document()));
  ground.world.lock().unwrap().voice_down = true;

  let started = tick(&ground.app).await;
  until("readings the voice unit does not answer", || readings(&ground, 1) >= 3).await;
  assert_eq!(ground.spoken().len(), 1);
  ground.world.lock().unwrap().voice_down = false;
  for handle in started {
    handle.await.unwrap();
  }

  assert_eq!(ground.spoken().len(), 2, "no narration is asked twice");
  assert_eq!(ground.app.store.item(item.id).unwrap().narrations, ["ko", "en"]);
  assert_eq!(orders(&ground).len(), 1);
}

#[tokio::test]
async fn a_whole_page_finished_again_finds_its_sounds_attached_and_is_drawn_whole_untold() {
  let ground = support::ground(|s| s.retry_minutes = 0).await;
  give_owner_room(&ground.app).await;
  let item = ground
    .app
    .store
    .ask("it", "example.org/thing", "Thing-7B", "https://example.org/thing", Shape::Manual)
    .unwrap();
  file(&ground, 1, 501, narrated(manual_document()));
  settle(&ground).await;
  assert_eq!(orders(&ground).len(), 1);
  let drawn = ground.rewrites(501).len();

  // What the store's eleventh step sets on a page that plays a narration.
  let db = ground.app.settings.store_dir().join("research.db");
  let conn = rusqlite::Connection::open(db).unwrap();
  conn
    .execute("update queue set narrations = '[]', pending_retry_at = 0 where id = ?1", [item.id])
    .unwrap();
  drop(conn);
  settle(&ground).await;

  let rewrites = ground.rewrites(501);
  assert_eq!(rewrites.len(), drawn + 1, "the page is drawn again");
  let html = rewrites.last().unwrap()["html"].as_str().unwrap();
  for name in ["narration-ko.ogg", "narration-en.ogg"] {
    assert!(html.contains(name), "the listen band plays {name}");
  }
  assert_eq!(rewrites.last().unwrap()["labels"]["languages"], "ko,en");
  assert_eq!(ground.spoken().len(), 2, "no sound is made again");
  let told = ground.app.store.item(item.id).unwrap();
  assert_eq!(
    (told.narrations.clone(), told.pending.clone()),
    (vec!["ko".to_string(), "en".to_string()], vec![])
  );
  assert_eq!(orders(&ground).len(), 1, "a report told is not told again");
}

#[tokio::test]
async fn a_result_holding_none_of_its_shape_s_sections_is_not_filed_or_told_and_the_try_waits() {
  let ground = support::ground(|_| {}).await;
  give_owner_room(&ground.app).await;
  let item = ground
    .app
    .store
    .ask("it", "example.org/thing", "Thing-7B", "https://example.org/thing", Shape::Manual)
    .unwrap();
  file(&ground, 1, 501, "<tool_call>\n<function=web_fetch>\n</function>\n</tool_call>\n".into());

  settle(&ground).await;

  assert!(ground.run_documents().is_empty(), "nothing is filed");
  let run = ground.app.store.research_run(1).unwrap();
  assert_eq!((run.outcome.as_deref(), run.document_id), (Some("failed"), None));
  let error = run.error.unwrap();
  let plan = &words::document().manual[0].plan_heading;
  assert!(error.contains(plan.as_str()), "the error names what was missing: {error}");
  let waiting = ground.app.store.item(item.id).unwrap();
  assert_eq!(waiting.state, ItemState::Waiting);
  assert!(waiting.error.unwrap().contains(plan.as_str()));
  assert!(orders(&ground).is_empty(), "no notice goes out");
}

/// The words a follow-up of document 812 is asked with.
const BECAUSE: &str = "The manual names a lighter variant; what is it for?";

#[tokio::test]
async fn a_follow_up_is_asked_and_drawn_with_the_document_it_follows_and_told_naming_it() {
  let ground = support::ground(|_| {}).await;
  give_owner_room(&ground.app).await;
  let parent_korean = format!("{KO}Parent-7B manual");
  ground
    .app
    .store
    .keep_rendering(
      812,
      &Rendering {
        title: parent_korean.clone(),
        description: None,
        body: String::new(),
      },
    )
    .unwrap();
  let asked = Asked {
    because: Some(BECAUSE.into()),
    parent: Some(Parent {
      item_id: Some(40),
      document_id: Some(812),
      title: Some("Parent-7B manual".into()),
      share_url: Some("https://share.example/d812".into()),
    }),
  };
  let item = ground
    .app
    .store
    .ask_from(
      "it",
      "example.org/thing",
      "Thing-7B",
      "https://example.org/thing",
      Shape::Manual,
      &asked,
    )
    .unwrap();
  file(&ground, 1, 501, manual_document());

  settle(&ground).await;

  let english = research_server::page::origin_line(&item, "en", None).unwrap();
  for part in ["812", "Parent-7B manual", "https://share.example/d812", BECAUSE] {
    assert!(english.contains(part), "{part} missing from {english}");
  }
  let query = ground.turns_named("research-1")[0]["prompt"].as_str().unwrap().to_string();
  assert!(query.contains(&english), "the question hands the run the origin: {query}");
  let asked_line = ground
    .app
    .store
    .research_log(1)
    .unwrap()
    .into_iter()
    .find(|l| l.kind == "asked")
    .unwrap();
  assert!(asked_line.text.contains(&english), "the log's asked line says it: {}", asked_line.text);

  let filed = ground.run_documents();
  let html = filed[0]["html"].as_str().unwrap();
  let label = &words::page().language("en").unwrap().origin;
  let origin_at = html.find("<div class=\"origin\">").expect("the English page prints the origin");
  assert!(origin_at < html.find("</header>").unwrap(), "under the title, in the head");
  assert!(html.contains(&format!("<strong>{label}:</strong>")));
  assert!(html.contains("<a href=\"https://share.example/d812\">Parent-7B manual</a>"), "{html}");
  let opening = html.find(support::OPENING).unwrap();
  assert!(opening < html.find(support::VERDICT).unwrap(), "the page opens with what it is");

  let rewrites = ground.rewrites(501);
  let page = rewrites[0]["html"].as_str().unwrap();
  assert!(page.contains("<div class=\"origin\" data-language=\"ko\" lang=\"ko\">"));
  assert!(page.contains("<div class=\"origin\" data-language=\"en\" lang=\"en\">"));
  assert!(
    page.contains(&format!(">{parent_korean}</a>")),
    "the Korean side names the parent's: {page}"
  );

  let told = orders(&ground);
  assert_eq!(told.len(), 1);
  let order = told[0]["said"]["order"].as_str().unwrap();
  let follows = words::fill(&words::report().follows, &[("parent", &parent_korean)]);
  assert!(order.starts_with(&format!("{KO}Report 501{follows} — ")), "{order}");
  assert_eq!(order.matches("https://").count(), 1, "the parent's link stands on the page: {order}");
  assert_eq!(ground.app.store.item(item.id).unwrap().state, ItemState::Fit);
}

#[tokio::test]
async fn a_result_with_no_opening_is_asked_again_whole_and_filed_once_it_says_what_its_subject_is() {
  let ground = support::ground(|_| {}).await;
  give_owner_room(&ground.app).await;
  ground
    .app
    .store
    .ask("it", "example.org/thing", "Thing-7B", "https://example.org/thing", Shape::Manual)
    .unwrap();
  file(&ground, 1, 501, narrated(manual_document()));
  let whole = result_text(&ground.world.lock().unwrap().filed[&1]);
  let headless = format!(
    "ANSWERED: yes :: the document\nTITLE: Report 501\nTOPIC: It is a thing.\n\nVERDICT:\n{}\n\n{}",
    support::VERDICT,
    narrated(manual_document())
  );
  {
    let mut world = ground.world.lock().unwrap();
    world.raw.insert(1, headless);
    world.whole = Some(whole);
  }

  settle(&ground).await;

  let asked = ground.turns_named("research-1-whole");
  assert_eq!(asked.len(), 1, "the whole document is asked once");
  let prompt = asked[0]["prompt"].as_str().unwrap();
  let opening = &words::document().opening.manual;
  assert!(
    prompt.contains("opening section") && prompt.contains(&opening[0]),
    "what it lacks: {prompt}"
  );
  assert!(!prompt.contains("ANSWERED: line"), "its head stands: {prompt}");
  let filed = ground.run_documents();
  assert_eq!(filed.len(), 1, "the whole answer is filed");
  assert!(filed[0]["html"].as_str().unwrap().contains(support::OPENING));
}

#[test]
fn a_result_without_its_opening_is_short_and_one_holding_it_in_either_language_is_not() {
  let opening = &words::document().opening.manual;
  let sections = words::document().sections.manual.clone();
  let answer = &words::search_document("en").answer;
  let w = &words::document().manual[0];
  let result = |opening_heading: Option<&str>| {
    let open = opening_heading.map(|h| format!("SECTION: {h}\nIt is a model.\n\n")).unwrap_or_default();
    format!(
      "ANSWERED: yes :: x\nTITLE: T\n\nVERDICT:\nIt works.\n\n{open}SECTION: 7. {}\n- {} n3\n\nSECTION: 9. {}\n- re-measure\n",
      w.heading, w.label, w.plan_heading
    )
  };
  let short = |text: String| {
    research_server::document::short_of(
      &draw(&text, words::search_document("en")),
      &sections,
      opening,
      answer,
    )
  };
  let lacking = short(result(None)).expect("a result with no opening is short");
  assert!(lacking.contains("opening section") && lacking.contains(&opening[1]), "{lacking}");
  assert_eq!(short(result(Some(&format!("0. {}", opening[0])))), None);
  assert_eq!(short(result(Some(&opening[1]))), None);
  let advisory = research_server::document::short_of(
    &draw(&result(None), words::search_document("en")),
    &[],
    words::document().opening.of(Shape::Advisory),
    answer,
  );
  assert_eq!(advisory, None, "a shape listing no opening asks for none");
}

/// The tail of a manual: its plan's continuation alone, with no head and no answer.
fn tail_only() -> String {
  let w = &words::document().manual[0];
  format!("SECTION: {} (continued)\nStage 2, the quality path.\n", w.plan_heading)
}

#[tokio::test]
async fn a_final_answer_in_two_text_blocks_is_filed_whole() {
  let ground = support::ground(|_| {}).await;
  give_owner_room(&ground.app).await;
  let item = ground
    .app
    .store
    .ask("it", "example.org/thing", "Thing-7B", "https://example.org/thing", Shape::Manual)
    .unwrap();
  file(&ground, 1, 501, narrated(manual_document()));
  ground.world.lock().unwrap().two_blocks = true;

  settle(&ground).await;

  assert!(ground.turns_named("research-1-whole").is_empty(), "nothing is asked again");
  let filed = ground.run_documents();
  assert_eq!(filed.len(), 1, "the page is filed");
  assert_eq!(filed[0]["title"], "Report 501");
  let html = filed[0]["html"].as_str().unwrap();
  let w = &words::document().manual[0];
  for held in [w.heading.as_str(), w.plan_heading.as_str(), support::VERDICT] {
    assert!(html.contains(held), "the first block stands with the second: {held}");
  }
  let written = ground.app.store.research_run(1).unwrap().written.unwrap();
  assert!(written.starts_with("ANSWERED:"), "the run keeps the whole: {written}");
  assert_eq!(ground.app.store.item(item.id).unwrap().title.as_deref(), Some("Report 501"));
}

#[tokio::test]
async fn a_result_holding_only_its_tail_is_asked_again_whole_and_filed_when_it_answers_whole() {
  let ground = support::ground(|_| {}).await;
  give_owner_room(&ground.app).await;
  let item = ground
    .app
    .store
    .ask("it", "example.org/thing", "Thing-7B", "https://example.org/thing", Shape::Manual)
    .unwrap();
  file(&ground, 1, 501, narrated(manual_document()));
  let whole = result_text(&ground.world.lock().unwrap().filed[&1]);
  {
    let mut world = ground.world.lock().unwrap();
    world.raw.insert(1, tail_only());
    world.whole = Some(whole);
  }

  settle(&ground).await;

  let asked = ground.turns_named("research-1-whole");
  assert_eq!(asked.len(), 1, "the whole document is asked once");
  let prompt = asked[0]["prompt"].as_str().unwrap();
  let question = &words::runner().whole.question;
  let opening = &question[..question.find("{missing}").unwrap()];
  assert!(prompt.starts_with(opening), "{prompt}");
  assert!(
    prompt.contains("ANSWERED: line") && prompt.contains("TITLE: line"),
    "what it lacks: {prompt}"
  );
  assert!(
    prompt.contains(&tail_only().trim().to_string()),
    "the answer written last is handed: {prompt}"
  );
  let filed = ground.run_documents();
  assert_eq!(filed.len(), 1, "the whole answer is filed");
  assert_eq!(filed[0]["title"], "Report 501");
  let html = filed[0]["html"].as_str().unwrap();
  assert!(!html.contains("(continued)"), "the tail is not what is filed");
  assert!(html.contains(&words::document().manual[0].heading));
  assert_eq!(ground.app.store.item(item.id).unwrap().state, ItemState::Fit);
  let logged = ground.app.store.research_log(1).unwrap();
  assert!(logged.iter().any(|line| line.kind == "whole"), "the log says it was asked again");
}

#[tokio::test]
async fn a_result_still_short_after_its_whole_ask_is_not_filed_and_names_what_it_lacks() {
  let ground = support::ground(|_| {}).await;
  give_owner_room(&ground.app).await;
  let item = ground
    .app
    .store
    .ask("it", "example.org/thing", "Thing-7B", "https://example.org/thing", Shape::Manual)
    .unwrap();
  file(&ground, 1, 501, narrated(manual_document()));
  ground.world.lock().unwrap().raw.insert(1, tail_only());

  settle(&ground).await;

  assert_eq!(ground.turns_named("research-1-whole").len(), 1, "the whole document is asked once");
  assert!(ground.run_documents().is_empty(), "nothing is filed");
  let run = ground.app.store.research_run(1).unwrap();
  assert_eq!((run.outcome.as_deref(), run.document_id), (Some("failed"), None));
  let error = run.error.unwrap();
  let answer = &words::search_document("en").answer;
  for lacking in ["ANSWERED:", "TITLE:", answer.as_str(), "(continued)"] {
    assert!(error.contains(lacking), "the error names {lacking}: {error}");
  }
  let waiting = ground.app.store.item(item.id).unwrap();
  assert_eq!(waiting.state, ItemState::Waiting);
  assert!(waiting.error.unwrap().contains("ANSWERED:"));
  assert!(orders(&ground).is_empty(), "no notice goes out");
}

#[tokio::test]
async fn a_note_the_translator_answers_as_text_never_reaches_the_page() {
  let ground = support::ground(|_| {}).await;
  give_owner_room(&ground.app).await;
  ground
    .app
    .store
    .ask("it", "example.org/thing", "Thing-7B", "https://example.org/thing", Shape::Manual)
    .unwrap();
  file(&ground, 1, 501, manual_document());
  ground.world.lock().unwrap().leaking = true;

  settle(&ground).await;

  let rewrites = ground.rewrites(501);
  assert_eq!(rewrites[0]["title"], format!("{KO}Report 501").as_str());
  let html = rewrites[0]["html"].as_str().unwrap();
  assert!(!html.contains("Note: "), "the note was taken out");
  let order = orders(&ground)[0]["said"]["order"].as_str().unwrap().to_string();
  assert!(!order.contains("Note: "), "{order}");
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
  let content = format!("## {}\n\n{}\n", answer_heading(), prose(150));
  file(&ground, 1, 60, content.clone());
  ground.world.lock().unwrap().too_large_over = Some(200);

  settle(&ground).await;

  let asks = translations(&ground);
  let texts: Vec<&str> = asks.iter().map(|a| a["text"].as_str().unwrap()).collect();
  let part = format!("# Report 60\n\n{}", english_body(60, &content));
  let whole = texts.iter().position(|t| *t == part).expect("the part was asked whole first");
  let halves = &texts[whole + 1..];
  assert_eq!(halves.len(), 2, "{halves:#?}");
  assert_eq!(halves.concat(), part, "the halves are the part, in order");
  let html = ground.rewrites(60)[0]["html"].as_str().unwrap().to_string();
  assert!(html.contains(&format!("{KO}Report 60</h1>")));
  assert_eq!(ground.app.store.item(item.id).unwrap().languages, ["ko", "en"]);
}

#[tokio::test]
async fn a_part_the_translator_cuts_short_is_halved_and_asked_again() {
  let ground = support::ground(|s| s.translation.part_chars = 400).await;
  give_owner_room(&ground.app).await;
  let item = ground
    .app
    .store
    .ask("it", "example.org/h", "H", "https://example.org/h", Shape::Investigation)
    .unwrap();
  let content = format!("## {}\n\n{}\n", answer_heading(), prose(150));
  file(&ground, 1, 60, content.clone());
  ground.world.lock().unwrap().cut_over = Some(200);

  settle(&ground).await;

  let asks = translations(&ground);
  let texts: Vec<&str> = asks.iter().map(|a| a["text"].as_str().unwrap()).collect();
  let part = format!("# Report 60\n\n{}", english_body(60, &content));
  let whole = texts.iter().position(|t| *t == part).expect("the part was asked whole first");
  let halves = &texts[whole + 1..];
  assert_eq!(halves.len(), 2, "{halves:#?}");
  assert_eq!(halves.concat(), part, "the halves are the part, in order");
  let kept = ground.app.store.rendering(60).unwrap().expect("the Korean side is kept");
  assert!(kept.body.trim_end().ends_with('.'), "the Korean side is whole: {}", kept.body);
  assert_eq!(ground.app.store.item(item.id).unwrap().languages, ["ko", "en"]);
}

#[tokio::test]
async fn a_description_that_is_the_item_s_name_stands_untranslated_on_the_korean_side() {
  let ground = support::ground(|_| {}).await;
  give_owner_room(&ground.app).await;
  let name = "example-lab/Example-Model-7B";
  ground
    .app
    .store
    .ask("it", "example.org/model", name, "https://example.org/model", Shape::Manual)
    .unwrap();
  ground.world.lock().unwrap().filed.insert(
    1,
    Filed {
      id: 70,
      title: "Report 70".into(),
      description: name.into(),
      content: manual_document(),
    },
  );

  settle(&ground).await;

  assert!(translations(&ground).iter().all(|a| a["text"] != name), "the name is never translated");
  let kept = ground.app.store.rendering(70).unwrap().expect("the Korean side is kept");
  assert_eq!(kept.description.as_deref(), Some(name));
}

#[tokio::test]
async fn a_korean_side_no_road_carries_holds_the_report_until_a_retry_draws_it_into_the_same_page() {
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

  assert!(orders(&ground).is_empty(), "nobody is told while the Korean side is pending");
  let pending = ground.app.store.item(item.id).unwrap();
  assert_eq!(
    (pending.state, pending.languages.clone()),
    (ItemState::Finishing, vec!["en".to_string()])
  );
  assert_eq!(pending.pending, ["korean"], "a result with no script waits for no narration");
  let state = ground.app.store.page_state(item.id).unwrap();
  assert_eq!(state.translation_tries, 1);
  assert!(state.retry_at.is_some());
  assert!(ground.rewrites(501).is_empty(), "the English page stands alone");
  let filed = logged(&ground, "filed");
  assert!(filed.iter().any(|text| text.starts_with("The Korean side is pending:")), "{filed:?}");
  assert!(filed.iter().any(|text| text.contains("lacks the Korean side")), "{filed:?}");

  settle(&ground).await;

  assert_eq!(ground.run_seats().len(), 1, "the run is not asked again");
  assert_eq!(ground.run_documents().len(), 1, "no second document is filed");
  let rewrites = ground.rewrites(501);
  assert_eq!(rewrites.len(), 1, "the same document is drawn again");
  assert_eq!(rewrites[0]["labels"]["languages"], "ko,en");
  let told = orders(&ground);
  assert_eq!(told.len(), 1, "the report is told once its page is whole");
  assert_eq!(told[0]["kind"], format!("research-report:{}", item.id));
  let order = told[0]["said"]["order"].as_str().unwrap();
  assert!(
    order.starts_with(&format!("{KO}Report 501 — {KO}It is a thing. https://share.example/d501")),
    "{order}"
  );
  assert_eq!(order.matches("https://").count(), 1, "one link: {order}");
  assert!(order.contains(&report.fit), "a fit item's report keeps what acts on it: {order}");
  let rendered = ground.app.store.item(item.id).unwrap();
  assert_eq!(
    (rendered.state, rendered.languages.clone()),
    (ItemState::Fit, vec!["ko".to_string(), "en".to_string()])
  );
  assert!(rendered.pending.is_empty());
  let state = ground.app.store.page_state(item.id).unwrap();
  assert_eq!((state.translation_tries, state.retry_at), (2, None));
  let reports = ground.app.store.reports(item.id).unwrap();
  assert_eq!(reports, [("room:7".to_string(), "report".to_string(), "sent".to_string())]);

  settle(&ground).await;
  assert_eq!(orders(&ground).len(), 1, "a report is told once");
}

#[tokio::test]
async fn a_korean_side_failing_past_the_retries_is_still_asked_on_every_retry_wait() {
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
  file(&ground, 1, 70, format!("# N\n\n## {}\n\nIt is a thing.\n", answer_heading()));
  ground.world.lock().unwrap().translation_answers.extend([503, 503, 503]);

  for _ in 0..3 {
    settle(&ground).await;
  }

  assert_eq!(translations(&ground).len(), 3, "the first try and every retry, past `retries`");
  let state = ground.app.store.page_state(item.id).unwrap();
  assert_eq!(state.translation_tries, 3);
  assert!(state.retry_at.is_some(), "it is not given up");
  assert!(orders(&ground).is_empty());
  let waiting = ground.app.store.item(item.id).unwrap();
  assert_eq!(
    (waiting.state, waiting.languages.clone()),
    (ItemState::Finishing, vec!["en".to_string()])
  );
  assert!(ground.rewrites(70).is_empty());

  settle(&ground).await;

  let told = ground.app.store.item(item.id).unwrap();
  assert_eq!(
    (told.state, told.languages.clone()),
    (ItemState::Reported, vec!["ko".to_string(), "en".to_string()])
  );
  assert_eq!(ground.rewrites(70).len(), 1);
  assert_eq!(orders(&ground).len(), 1);
}

#[tokio::test]
async fn an_ask_agency_cannot_carry_now_is_asked_again_at_once_and_not_counted() {
  let ground = support::ground(|s| s.retry_minutes = 60).await;
  give_owner_room(&ground.app).await;
  let item = ground
    .app
    .store
    .ask("it", "example.org/a", "A", "https://example.org/a", Shape::Investigation)
    .unwrap();
  ground.world.lock().unwrap().session_answers.push_back(503);
  // The refused ask opened run 1 and closed it; the second try is run 2.
  file(&ground, 2, 9, format!("# An investigation\n\n## {}\n\nIt is.\n", answer_heading()));

  settle(&ground).await;
  let waiting = ground.app.store.item(item.id).unwrap();
  assert_eq!((waiting.state, waiting.tries, waiting.retry_at), (ItemState::Waiting, 0, None));
  let error = waiting.error.unwrap();
  assert!(error.contains("503") && error.contains("not counted"), "{error}");

  settle(&ground).await;
  let reported = ground.app.store.item(item.id).unwrap();
  assert_eq!((reported.state, reported.tries), (ItemState::Reported, 1));
  let runs = ground.app.store.runs(Some(item.id), None, None, 10).unwrap().items;
  let states: Vec<&str> = runs.iter().map(|r| r.state.as_str()).collect();
  assert_eq!(states, ["closed", "refused"]);
  assert_eq!(runs[1].run_id, None);
}

#[tokio::test]
async fn an_ask_agency_refuses_for_what_it_asks_is_counted_and_waits() {
  let ground = support::ground(|s| s.retry_minutes = 60).await;
  let item = ground
    .app
    .store
    .ask("it", "example.org/a", "A", "https://example.org/a", Shape::Investigation)
    .unwrap();
  ground.world.lock().unwrap().session_answers.push_back(422);

  settle(&ground).await;
  let waiting = ground.app.store.item(item.id).unwrap();
  assert_eq!((waiting.state, waiting.tries), (ItemState::Waiting, 1));
  assert!(waiting.retry_at.unwrap() > now_ms() + 50 * 60_000, "it waits retry_minutes");
  assert!(waiting.error.unwrap().contains("422"));
  settle(&ground).await;
  assert_eq!(ground.run_seats().len(), 1, "nothing is asked before its wait ends");
}

#[tokio::test]
async fn a_run_whose_turn_agency_cannot_carry_is_cut_and_asked_again_at_once_uncounted() {
  let ground = support::ground(|s| s.retry_minutes = 60).await;
  let item = ground
    .app
    .store
    .ask("it", "example.org/c", "C", "https://example.org/c", Shape::Investigation)
    .unwrap();
  ground.world.lock().unwrap().turns_lost.insert(1);
  file(&ground, 2, 9, format!("# An investigation\n\n## {}\n\nIt is.\n", answer_heading()));

  settle(&ground).await;
  let cut = ground.app.store.research_run(1).unwrap();
  assert_eq!(cut.outcome.as_deref(), Some("cut"));
  assert!(cut.error.unwrap().contains("503"));
  let waiting = ground.app.store.item(item.id).unwrap();
  assert_eq!((waiting.state, waiting.tries, waiting.retry_at), (ItemState::Waiting, 0, None));
  assert!(waiting.error.unwrap().contains("not counted"));

  settle(&ground).await;
  let reported = ground.app.store.item(item.id).unwrap();
  assert_eq!((reported.state, reported.tries), (ItemState::Reported, 1));
  let tries = ground.app.store.runs(Some(item.id), None, None, 10).unwrap().items;
  let states: Vec<&str> = tries.iter().map(|r| r.state.as_str()).collect();
  assert_eq!(states, ["closed", "cut"]);
}

#[tokio::test]
async fn a_turn_that_wrote_nothing_is_cut_where_agency_no_longer_held_the_seat_and_counted_where_it_did() {
  let ground = support::ground(|s| s.retry_minutes = 60).await;
  let item = ground
    .app
    .store
    .ask("it", "example.org/d", "D", "https://example.org/d", Shape::Investigation)
    .unwrap();
  {
    let mut world = ground.world.lock().unwrap();
    world.failing.extend(1..=2);
    world.seats_dropped = true;
  }
  settle(&ground).await;
  let cut = ground.app.store.research_run(1).unwrap();
  assert_eq!(cut.outcome.as_deref(), Some("cut"));
  let error = cut.error.unwrap();
  assert!(error.contains("wrote nothing") && error.contains("no longer held"), "{error}");
  let waiting = ground.app.store.item(item.id).unwrap();
  assert_eq!((waiting.state, waiting.tries, waiting.retry_at), (ItemState::Waiting, 0, None));

  // With its seat held, a turn that wrote nothing is the run's own failure.
  ground.world.lock().unwrap().seats_dropped = false;
  settle(&ground).await;
  let failed = ground.app.store.research_run(2).unwrap();
  assert_eq!(failed.outcome.as_deref(), Some("failed"));
  let counted = ground.app.store.item(item.id).unwrap();
  assert_eq!((counted.state, counted.tries), (ItemState::Waiting, 1));
  assert!(counted.retry_at.is_some());
}

#[tokio::test]
async fn an_item_failing_past_its_retries_is_stuck_told_once_handed_to_the_helper_and_still_asked() {
  let ground = support::ground(|s| {
    s.retry_minutes = 0;
    s.retries = 3;
  })
  .await;
  give_owner_room(&ground.app).await;
  let item = ground
    .app
    .store
    .ask("it", "example.org/b", "B", "https://example.org/b", Shape::Investigation)
    .unwrap();
  ground.world.lock().unwrap().failing.extend(1..=5);
  for _ in 0..4 {
    settle(&ground).await;
  }
  let stuck = ground.app.store.item(item.id).unwrap();
  assert_eq!((stuck.state, stuck.tries, stuck.repair_item_id), (ItemState::Stuck, 4, None));
  let told = orders(&ground);
  assert_eq!(told.len(), 1);
  let expected = words::fill(&words::report().stuck, &[("name", "B"), ("tries", "4")]);
  assert_eq!(told[0]["said"]["order"], expected.as_str());

  // The next pass files the repair before it asks the item again, and asks it all the same.
  settle(&ground).await;
  let items = ground.world.lock().unwrap().job_items.clone();
  assert_eq!(items.len(), 1);
  let repair = &items[0];
  assert_eq!(
    (repair["queueName"].as_str(), repair["assigneeName"].as_str()),
    (Some("todo"), Some("helper"))
  );
  assert_eq!(repair["labels"]["lane"], "repair");
  assert_eq!(repair["labels"]["research-item"], item.id.to_string().as_str());
  assert!(repair["title"].as_str().unwrap().contains("B"));
  let brief = repair["brief"].as_str().unwrap();
  assert!(brief.contains("https://example.org/b") && brief.contains("research run 4"), "{brief}");
  assert!(brief.contains("wrote nothing") && brief.contains(&format!("PATCH /queue/{}", item.id)));
  assert_eq!(ground.run_seats().len(), 5, "a stuck item is asked again on its retry wait");
  let still = ground.app.store.item(item.id).unwrap();
  assert_eq!((still.state, still.tries, still.repair_item_id), (ItemState::Stuck, 5, Some(601)));
  assert_eq!(orders(&ground).len(), 1, "it is told once");

  // The fix lands without a word: the next try files its document and the item is reported.
  file(&ground, 6, 9, format!("# An investigation\n\n## {}\n\nIt is.\n", answer_heading()));
  settle(&ground).await;
  assert_eq!(ground.world.lock().unwrap().job_items.len(), 1, "one repair for the stretch");
  let reported = ground.app.store.item(item.id).unwrap();
  assert_eq!((reported.state, reported.repair_item_id), (ItemState::Reported, Some(601)));
}

#[tokio::test]
async fn a_stuck_item_leaves_only_by_dismissal_and_is_asked_again_afresh() {
  let ground = support::ground(|s| {
    s.retry_minutes = 0;
    s.retries = 0;
  })
  .await;
  let store = &ground.app.store;
  let item = store
    .ask("it", "example.org/s", "S", "https://example.org/s", Shape::Investigation)
    .unwrap();
  ground.world.lock().unwrap().failing.extend(1..=3);
  settle(&ground).await;
  assert_eq!(store.item(item.id).unwrap().state, ItemState::Stuck);
  settle(&ground).await;
  let repaired = store.item(item.id).unwrap();
  assert_eq!(
    (repaired.state, repaired.tries, repaired.repair_item_id),
    (ItemState::Stuck, 2, Some(601))
  );

  let again = store.ask_again(item.id).unwrap();
  assert_eq!(
    (again.state, again.tries, again.retry_at, again.repair_item_id),
    (ItemState::Waiting, 0, None, None)
  );
  settle(&ground).await;
  assert_eq!(store.item(item.id).unwrap().state, ItemState::Stuck, "stuck anew after its retries");
  assert!(store.ask_again(item.id).is_ok());
  assert!(store.ask_again(item.id).is_err(), "only a stuck item is asked again");

  ground.world.lock().unwrap().failing.extend(4..=9);
  settle(&ground).await;
  store.dismiss(item.id).unwrap();
  let seats = ground.run_seats().len();
  settle(&ground).await;
  settle(&ground).await;
  assert_eq!(store.item(item.id).unwrap().state, ItemState::Dismissed);
  assert_eq!(ground.run_seats().len(), seats, "a dismissed item is asked no more");
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
  let flaw = &words::document().sections.advisory[0];
  let document = format!(
    "# SA-1\n\n## {flaw}\n\nIt overflows.\n\n| {} | {} | {} | {} | {} |\n|---|---|---|---|---|\n| libexample | 1.4.1 | services/alpha | node-a | {} |\n| libexample | 1.4.1 | services/beta | | {} |\n\n## {}\n\n1. upgrade\n",
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
    "# Brief\n\n## {}\n\n| {} | {} | {} | {} | {} |\n|---|---|---|---|---|\n| index-a | 2,650.1 | pt | 2026-10-29 | [page](https://example.org/e1) |\n",
    words::document().sections.brief[0],
    w.name,
    w.value,
    w.unit,
    w.as_of,
    w.source
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
  store.fail_try(run, "earlier", 0, false).unwrap();
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
  assert_eq!(ground.asked_path("agency", "DELETE", "/sessions/search@apart:9").len(), 1);
  let told = orders(&ground);
  assert_eq!(told.len(), 1, "a run the deep dive asked tells no room");
  assert_eq!(
    (told[0]["room"].as_i64(), told[0]["kind"].as_str()),
    (Some(5), Some("research-ended:2"))
  );
  assert!(told[0]["said"]["order"].as_str().unwrap().contains("Half of it."));
  assert!(told[0].get("run").is_none());

  let retried = store.item(asked.id).unwrap();
  assert_eq!(
    (retried.state, retried.tries, retried.retry_at),
    (ItemState::Waiting, 0, None),
    "uncounted"
  );
  let tries = store.runs(Some(asked.id), None, None, 10).unwrap().items;
  assert_eq!(tries[0].state.as_str(), "cut");
  assert!(ground.run_seats().is_empty(), "an asked run is not asked again");
  let waiting = store.item(never.id).unwrap();
  assert_eq!((waiting.state, waiting.tries), (ItemState::Waiting, 0));
}

#[tokio::test(start_paused = true)]
async fn a_translation_answered_past_ten_minutes_is_waited_on_with_no_clock() {
  let ground = support::ground(|_| {}).await;
  ground.world.lock().unwrap().translation_wait_ms = 700_000;
  let asked = tokio::time::Instant::now();
  let text =
    translate_part(&ground.app.peers, "Korean", "A part whose card is granted late.", "", "Thing-7B")
      .await
      .unwrap();
  assert_eq!(text, translated("A part whose card is granted late."));
  assert!(
    asked.elapsed() >= std::time::Duration::from_secs(700),
    "the slow translation was waited on to its answer"
  );
  assert_eq!(translations(&ground).len(), 1, "one ask, never cut and asked again");
}
