//! The `references` research key (`docs/runner.md`, 「The references dossier」): the options it takes, the
//! dossier its answer must be with each licence line kept as metadata that drops no candidate, the language of
//! the person who asked for the draw, and a run end to end filed in the archive and told to no room.

use crate::support;

use std::time::Duration;

use bmo_research_client::*;
use research_server::reading;
use research_server::references::{self, ReferenceAsk};
use research_server::words;
use serde_json::{Value, json};

// The subject's name in Hangul, built from its code points so this file holds no Hangul of its own.
fn korean() -> String {
  [0xCE58_u32, 0xC774, 0xCE74, 0xC640].into_iter().filter_map(char::from_u32).collect()
}

const DOSSIER: &str = r#"
```json
{
  "subject": {"name": "Chiikawa", "aliases": ["Chiikawa", "Chiikawa-chan"], "kind": "character"},
  "description": "A small round white character with round ears and a tiny face.",
  "features": ["round ears", "a tiny face"],
  "constraints": ["Do not draw the character with a human body."],
  "candidates": [
    {
      "url": "https://upload.example.org/chiikawa.png",
      "pageUrl": "https://commons.example.org/wiki/File:Chiikawa.png",
      "title": "Chiikawa",
      "creator": "A maker",
      "publisher": "Wikimedia Commons",
      "retrievedAt": 0,
      "licence": {"id": "CC-BY-4.0", "url": "https://creativecommons.org/licenses/by/4.0/", "attribution": "A maker, CC BY 4.0"},
      "mediaType": "image/png",
      "width": 1200,
      "height": 1200,
      "note": "A clear view of the face"
    },
    {
      "url": "https://upload.example.org/chiikawa-nc.jpg",
      "pageUrl": "https://example.org/nc",
      "title": "Chiikawa NC",
      "creator": "Another maker",
      "publisher": "Example",
      "retrievedAt": 0,
      "licence": {"id": "CC-BY-NC-4.0", "url": null, "attribution": "Another maker"},
      "mediaType": "image/jpeg",
      "width": 800,
      "height": 800,
      "note": "A side view"
    },
    {
      "url": "https://upload.example.org/no-size.jpg",
      "pageUrl": "https://example.org/no-size",
      "title": "No size",
      "creator": "",
      "publisher": "",
      "retrievedAt": 0,
      "licence": {"id": "CC0-1.0", "url": null, "attribution": ""},
      "mediaType": "image/jpeg",
      "note": "No dimensions"
    },
    {
      "url": "https://upload.example.org/chiikawa.png",
      "pageUrl": "https://commons.example.org/wiki/File:Chiikawa.png",
      "title": "The same picture again",
      "creator": "A maker",
      "publisher": "Wikimedia Commons",
      "retrievedAt": 0,
      "licence": {"id": "CC-BY-4.0", "url": null, "attribution": "A maker"},
      "mediaType": "image/png",
      "width": 1200,
      "height": 1200,
      "note": "A duplicate"
    },
    {
      "url": "https://upload.example.org/chiikawa-unstated.png",
      "pageUrl": "https://example.org/unstated",
      "title": "Chiikawa, no licence stated",
      "creator": "",
      "publisher": "Example",
      "retrievedAt": 0,
      "mediaType": "image/png",
      "width": 1024,
      "height": 768,
      "note": "A view from behind"
    }
  ],
  "sources": [{"url": "https://example.org/character", "what": "The character's appearance"}],
  "unresolved": []
}
```
"#;

fn ask(language: &str) -> ReferenceAsk {
  ReferenceAsk {
    subject_key: "chiikawa-1a2b3c4d".into(),
    kind: "character".into(),
    language: language.into(),
    candidate_limit: 12,
    image_engines: references::ENGINES.iter().map(|engine| (*engine).to_owned()).collect(),
  }
}

#[test]
fn the_options_name_the_subject_the_kind_the_language_the_limit_and_the_engines() {
  let ask = references::ask(&ResearchRunAsk {
    key: "references".into(),
    query: format!("{} (Chiikawa) reference dossier", korean()),
    options: Some(json!({
      "subjectKey": "chiikawa-1a2b3c4d",
      "kind": "character",
      "language": "ko",
      "candidateLimit": 8,
      "requireLicence": false,
      "imageEngines": ["openverse", "unsplash"]
    })),
    ..ResearchRunAsk::default()
  })
  .unwrap();
  assert_eq!(
    (ask.subject_key.as_str(), ask.kind.as_str(), ask.language.as_str(), ask.candidate_limit),
    ("chiikawa-1a2b3c4d", "character", "ko", 8)
  );
  assert_eq!(ask.image_engines, ["openverse", "unsplash"]);

  // The same options as a string, and the language the question is asked in where none is named.
  let ask = references::ask(&ResearchRunAsk {
    key: "references".into(),
    query: korean(),
    options: Some(json!(
      "{\"subjectKey\":\"chiikawa-1a2b3c4d\",\"kind\":\"character\",\"candidateLimit\":12}"
    )),
    ..ResearchRunAsk::default()
  })
  .unwrap();
  assert_eq!((ask.language.as_str(), ask.candidate_limit), ("ko", 12));
  assert_eq!(ask.image_engines, references::ENGINES);

  for options in [
    None,
    Some(json!("not json")),
    Some(json!({"kind": "character"})),
    Some(json!({"subjectKey": "chiikawa-1a2b3c4d", "kind": "animal"})),
    Some(json!({"subjectKey": "chiikawa-1a2b3c4d", "kind": "character", "candidateLimit": 0})),
    Some(json!({"subjectKey": "chiikawa-1a2b3c4d", "kind": "character", "candidateLimit": 25})),
    Some(json!({"subjectKey": "chiikawa-1a2b3c4d", "kind": "character", "language": "fr"})),
    Some(json!({"subjectKey": "chiikawa-1a2b3c4d", "kind": "character", "imageEngines": ["flickr"]})),
    Some(json!({"subjectKey": "chiikawa-1a2b3c4d", "kind": "character", "imageEngines": []})),
  ] {
    assert!(
      references::ask(&ResearchRunAsk {
        key: "references".into(),
        query: "Q".into(),
        options,
        ..ResearchRunAsk::default()
      })
      .is_err(),
      "the options do not stand"
    );
  }
}

#[test]
fn the_dossier_is_the_last_fenced_block_held_to_its_fields_and_no_licence_drops_a_candidate() {
  let now = 1_700_000_000_000;
  let dossier = references::dossier(DOSSIER, &ask("en"), now).expect("the dossier reads");
  assert_eq!(
    (dossier.subject.name.as_str(), dossier.subject.kind.as_str()),
    ("Chiikawa", "character")
  );
  let urls: Vec<&str> = dossier.candidates.iter().map(|candidate| candidate.url.as_str()).collect();
  assert_eq!(
    urls,
    [
      "https://upload.example.org/chiikawa.png",
      "https://upload.example.org/chiikawa-nc.jpg",
      "https://upload.example.org/chiikawa-unstated.png"
    ],
    "a non-commercial licence and no licence line drop no candidate; a missing dimension and a repeat do"
  );
  let candidate = &dossier.candidates[0];
  assert_eq!(candidate.licence.id, "CC-BY-4.0");
  assert_eq!((candidate.width, candidate.height), (1200, 1200));
  assert_eq!(candidate.retrieved_at, now, "the run's read moment where the answer names none");
  let restricted = &dossier.candidates[1].licence;
  assert_eq!(
    (restricted.id.as_str(), restricted.attribution.as_str()),
    ("CC-BY-NC-4.0", "Another maker"),
    "the licence line stands as the source states it"
  );
  let unstated = &dossier.candidates[2].licence;
  assert_eq!(
    (unstated.id.as_str(), unstated.url.as_deref(), unstated.attribution.as_str()),
    ("", None, "")
  );
  assert_eq!(dossier.unresolved.len(), 1, "{:?}", dossier.unresolved);
  assert!(dossier.unresolved[0].contains("pixel dimensions"), "{:?}", dossier.unresolved);

  let words = words::document().reference("en");
  let body = references::render(&dossier, words);
  assert!(body.contains(&format!("## {}", words.candidates)));
  assert!(body.contains("| CC-BY-NC-4.0 (Another maker) |"), "the table reports the licence line");
  assert!(body.contains(&format!("| {} |\n", words.none)), "an unstated licence reads as none");
  let (_, block) = reading::last_fenced(&body).expect("the body closes with its JSON");
  let parsed: Value = serde_json::from_str(&block).expect("the last block is JSON");
  assert_eq!(parsed["subject"]["name"], "Chiikawa");
  assert_eq!(parsed["candidates"].as_array().unwrap().len(), 3);
  assert_eq!(parsed["candidates"][1]["licence"]["id"], "CC-BY-NC-4.0");
  assert_eq!(parsed["candidates"][2]["licence"]["id"], "");
  assert_eq!(parsed["candidates"][0]["pageUrl"], "https://commons.example.org/wiki/File:Chiikawa.png");
  assert_eq!(parsed["candidates"][0]["licence"]["id"], "CC-BY-4.0");
}

#[test]
fn a_korean_dossier_is_written_in_korean_and_an_answer_with_no_dossier_is_not_one() {
  let english = DOSSIER;
  assert!(
    references::dossier(english, &ask("ko"), 1)
      .unwrap_err()
      .contains("not written in Korean"),
    "an English description is not a Korean dossier"
  );

  let korean = english.replace(
    "A small round white character with round ears and a tiny face.",
    &format!("{} is a small round white character with round ears and a tiny face.", korean()),
  );
  let dossier = references::dossier(&korean, &ask("ko"), 1).expect("the Korean dossier reads");
  assert_eq!(dossier.candidates.len(), 3);

  assert!(references::dossier("prose alone, no block", &ask("en"), 1).is_err());
}

async fn served() -> (support::Ground, String, ResearchClient) {
  let ground = support::ground(|_| {}).await;
  let base = support::serve(research_server::routes::router(ground.app.clone())).await;
  let client = ResearchClient::new(&base).with_token("test-token");
  (ground, base, client)
}

async fn closed(ground: &support::Ground, run: i64) -> ResearchRunView {
  for _ in 0..1500 {
    let row = ground.app.store.research_run(run).unwrap();
    if row.state == ResearchRunState::Closed {
      return row.view();
    }
    tokio::time::sleep(Duration::from_millis(10)).await;
  }
  panic!("references run {run} did not close");
}

#[tokio::test]
async fn a_references_run_is_asked_filed_as_a_dossier_and_told_to_no_room() {
  let (ground, _base, client) = served().await;
  ground.world.lock().unwrap().raw.insert(1, DOSSIER.into());

  let asked = client
    .ask_research(&ResearchRunAsk {
      key: "references".into(),
      query: format!("{} (Chiikawa) reference dossier", korean()),
      asker: Some("paint".into()),
      options: Some(json!({
        "subjectKey": "chiikawa-1a2b3c4d",
        "kind": "character",
        "language": "en",
        "candidateLimit": 12,
        "requireLicence": false
      })),
      ..ResearchRunAsk::default()
    })
    .await
    .unwrap();
  assert_eq!((asked.key.as_str(), asked.state), ("references", ResearchRunState::Running));

  let run = closed(&ground, 1).await;
  assert_eq!((run.outcome.as_deref(), run.document_id), (Some("answered"), Some(901)));
  assert!(run.narrations.is_empty() && run.pending.is_empty(), "a dossier carries no narration");
  assert!(
    ground.asked_path("receiver", "POST", "/instructions").is_empty(),
    "no room is told of a dossier"
  );
  assert!(
    ground.turns_named("research-1-narration").is_empty(),
    "a references run asks no narration"
  );

  let filed = ground.run_documents();
  assert_eq!(filed.len(), 1);
  let document = &filed[0];
  assert_eq!(document["kind"], "search");
  assert_eq!(document["labels"]["key"], "references");
  assert_eq!(document["labels"]["category"], "references");
  assert_eq!(document["labels"]["paint.subject-key"], "chiikawa-1a2b3c4d");
  assert_eq!(document["labels"]["paint.schema"], "references-v1");
  assert_eq!(document["labels"]["languages"], "en");
  assert_eq!(document["actorName"], "research");

  let body = document["body"].as_str().unwrap();
  let (_, block) = reading::last_fenced(body).expect("the dossier closes with its JSON");
  let dossier: Value = serde_json::from_str(&block).unwrap();
  assert_eq!(dossier["subject"]["kind"], "character");
  let licences: Vec<(&str, &str)> = dossier["candidates"]
    .as_array()
    .unwrap()
    .iter()
    .map(|candidate| {
      let licence = &candidate["licence"];
      (licence["id"].as_str().unwrap(), licence["attribution"].as_str().unwrap())
    })
    .collect();
  assert_eq!(
    licences,
    [("CC-BY-4.0", "A maker, CC BY 4.0"), ("CC-BY-NC-4.0", "Another maker"), ("", "")],
    "an ask naming requireLicence false is run, and the filed dossier keeps every licence line as metadata"
  );
  assert!(
    document["body"]
      .as_str()
      .unwrap()
      .contains("Appearance and the features that must survive")
  );

  let sessions = ground
    .asked_path("agency", "POST", "/sessions")
    .into_iter()
    .map(|asked| asked.body)
    .collect::<Vec<Value>>();
  let first = &sessions[0];
  let system = first["systemPrompt"].as_str().unwrap();
  assert!(
    system.contains("SKILL bmo-research-references:\n"),
    "the references rulebook is handed whole"
  );
  assert!(!system.contains("prompts/search/task.md"), "no search task binds a references run");
  assert_eq!(first["labels"]["research"]["rulebooks"], json!(["bmo-research-references"]));
  let turn = ground.turns_named("research-1").remove(0);
  let prompt = turn["prompt"].as_str().unwrap();
  for said in [
    "Reference dossier task",
    "Subject key: chiikawa-1a2b3c4d",
    "Subject kind: character",
    "openverse, wikicommons.images, unsplash",
  ] {
    assert!(prompt.contains(said), "{said} stands in the request");
  }
}

#[tokio::test]
async fn a_korean_dossier_whose_answer_is_english_is_asked_again_whole() {
  let (ground, _base, client) = served().await;
  let korean_dossier = DOSSIER.replace(
    "A small round white character with round ears and a tiny face.",
    &format!("{} is a small round white character with round ears and a tiny face.", korean()),
  );
  {
    let mut world = ground.world.lock().unwrap();
    world.raw.insert(1, DOSSIER.into());
    world.whole = Some(korean_dossier);
  }

  client
    .ask_research(&ResearchRunAsk {
      key: "references".into(),
      query: format!("{} (Chiikawa) reference dossier", korean()),
      asker: Some("paint".into()),
      options: Some(json!({
        "subjectKey": "chiikawa-1a2b3c4d",
        "kind": "character",
        "language": "ko"
      })),
      ..ResearchRunAsk::default()
    })
    .await
    .unwrap();

  let run = closed(&ground, 1).await;
  assert_eq!(run.outcome.as_deref(), Some("answered"));
  assert_eq!(
    ground.turns_named("research-1-whole").len(),
    1,
    "an English dossier for a Korean draw is asked again whole"
  );
  assert!(ground.turns_named("research-1-narration").is_empty());

  let filed = ground.run_documents();
  let body = filed[0]["body"].as_str().unwrap();
  assert!(body.contains(&format!("{} is a small round", korean())), "the Korean dossier is filed");
  assert_eq!(filed[0]["labels"]["languages"], "ko");
  let (_, block) = reading::last_fenced(body).expect("the dossier closes with its JSON");
  let dossier: Value = serde_json::from_str(&block).unwrap();
  assert_eq!(dossier["subject"]["name"], "Chiikawa");
  assert_eq!(dossier["candidates"].as_array().unwrap().len(), 3);
}
