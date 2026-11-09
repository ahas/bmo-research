//! The routes `docs/api.md` states, walked over a served daemon: the token, the wire format's failures, and
//! each resource's members and refusals.

use crate::support;

use bmo_research_client::*;
use reqwest::StatusCode;
use serde_json::json;

async fn served() -> (support::Ground, String, ResearchClient) {
  let ground = support::ground(|_| {}).await;
  let base = support::serve(research_server::routes::router(ground.app.clone())).await;
  let client = ResearchClient::new(&base).with_token("test-token");
  (ground, base, client)
}

async fn raw(
  base: &str,
  method: reqwest::Method,
  path: &str,
  body: Option<serde_json::Value>,
) -> (StatusCode, serde_json::Value) {
  let mut request = reqwest::Client::new()
    .request(method, format!("{base}{path}"))
    .bearer_auth("test-token");
  if let Some(body) = body {
    request = request.json(&body);
  }
  let answer = request.send().await.unwrap();
  let status = answer.status();
  (status, answer.json().await.unwrap_or_default())
}

#[tokio::test]
async fn a_request_without_the_token_is_refused_and_an_unknown_route_is_no_route() {
  let (_ground, base, client) = served().await;
  let err = ResearchClient::new(&base).health().await.unwrap_err();
  assert!(err.starts_with("401 unauthorized"), "{err}");
  assert!(client.health().await.unwrap().ready);
  let (status, body) = raw(&base, reqwest::Method::GET, "/nowhere", None).await;
  assert_eq!((status, body["code"].as_str()), (StatusCode::NOT_FOUND, Some("no-route")));
}

#[tokio::test]
async fn categories_are_listed_added_with_the_owner_s_room_changed_and_taken_away() {
  let (_ground, base, client) = served().await;
  assert_eq!(client.categories().await.unwrap().items.len(), 3);
  let new = NewCategory {
    name: "health".into(),
    purpose: "Medical news.".into(),
    shape: CategoryShape::ManualOrInvestigation,
    cadence_minutes: Some(30),
    gather_minutes: None,
    provider: Some("claude-code".into()),
    model: None,
    instruction: None,
    calendar: None,
  };
  let made = client.add_category(&new).await.unwrap();
  assert_eq!((made.cadence_minutes, made.provider.as_deref()), (30, Some("claude-code")));
  let recipients = client.recipients("health").await.unwrap().items;
  assert_eq!(recipients.len(), 1);
  assert_eq!((recipients[0].room_id, recipients[0].origin.as_str()), (Some(7), "owner"));
  assert!(client.add_category(&new).await.unwrap_err().starts_with("409 conflict"));
  let mut bad = new.clone();
  bad.name = "Health!".into();
  assert!(client.add_category(&bad).await.unwrap_err().starts_with("400 bad-request"));
  let (status, _) =
    raw(&base, reqwest::Method::PATCH, "/categories/health", Some(json!({"gatherMinutes": 60}))).await;
  assert_eq!(status, StatusCode::BAD_REQUEST, "only a brief gathers");
  let changed = client
    .change_category(
      "health",
      &CategoryChange {
        provider: Some(String::new()),
        cadence_minutes: Some(45),
        ..CategoryChange::default()
      },
    )
    .await
    .unwrap();
  assert_eq!((changed.cadence_minutes, changed.provider), (45, None));
  client.remove_category("health").await.unwrap();
  assert!(client.category("health").await.unwrap_err().starts_with("404 not-found"));
}

#[tokio::test]
async fn the_state_stops_one_category_or_every_one() {
  let (_ground, _base, client) = served().await;
  let state = client
    .set_state(&StateChange {
      state: Running::Stopped,
      category_name: Some("economy".into()),
    })
    .await
    .unwrap();
  assert_eq!(state.state, Running::Running);
  assert!(state.changed_at.is_some());
  assert!(
    state
      .categories
      .iter()
      .any(|c| c.category_name == "economy" && c.state == Running::Stopped)
  );
  let err = client
    .set_state(&StateChange {
      state: Running::Stopped,
      category_name: Some("nothing".into()),
    })
    .await
    .unwrap_err();
  assert!(err.starts_with("404 not-found"), "{err}");
  let all = client
    .set_state(&StateChange {
      state: Running::Stopped,
      category_name: None,
    })
    .await
    .unwrap();
  assert_eq!(all.state, Running::Stopped);
}

#[tokio::test]
async fn a_page_source_is_registered_with_its_pattern_and_strategy_and_refused_when_malformed() {
  let (_ground, base, client) = served().await;
  let page = client
    .add_source(&NewSource {
      category_name: "it".into(),
      kind: "page".into(),
      address: Some("https://example-lab.org/news".into()),
      pattern: Some(" /news/ ".into()),
      ..NewSource::default()
    })
    .await
    .unwrap();
  assert_eq!(
    (page.pattern.as_deref(), page.strategy.as_deref(), page.query.as_deref()),
    (Some("/news/"), Some("direct"), None)
  );
  for (body, status) in [
    (
      json!({"categoryName": "it", "kind": "page", "pattern": "/news/"}),
      StatusCode::BAD_REQUEST,
    ),
    (
      json!({"categoryName": "it", "kind": "page", "address": "https://example.org/n"}),
      StatusCode::BAD_REQUEST,
    ),
    (
      json!({"categoryName": "it", "kind": "page", "address": "https://example.org/n", "pattern": "news/("}),
      StatusCode::BAD_REQUEST,
    ),
    (
      json!({"categoryName": "it", "kind": "page", "address": "https://example.org/n", "pattern": "/n/", "strategy": "direct-only"}),
      StatusCode::BAD_REQUEST,
    ),
    (
      json!({"categoryName": "it", "kind": "rss", "address": "https://example.org/f", "pattern": "/n/"}),
      StatusCode::BAD_REQUEST,
    ),
  ] {
    let (answered, failure) = raw(&base, reqwest::Method::POST, "/sources", Some(body.clone())).await;
    assert_eq!(answered, status, "{body}");
    assert!(failure["code"].is_string() && failure["error"].is_string());
  }
  let changed = client
    .change_source(
      page.id,
      &SourceChange {
        pattern: Some(r"example-lab\.org/news/[a-z-]+$".into()),
        strategy: Some("browser".into()),
        ..SourceChange::default()
      },
    )
    .await
    .unwrap();
  assert_eq!(
    (changed.pattern.as_deref(), changed.strategy.as_deref()),
    (Some(r"example-lab\.org/news/[a-z-]+$"), Some("browser"))
  );
  let path = format!("/sources/{}", page.id);
  let (status, _) = raw(&base, reqwest::Method::PATCH, &path, Some(json!({"strategy": "chrome"}))).await;
  assert_eq!(status, StatusCode::BAD_REQUEST);
  let feed = client
    .add_source(&NewSource {
      category_name: "it".into(),
      kind: "rss".into(),
      address: Some("https://example.org/feed.xml".into()),
      ..NewSource::default()
    })
    .await
    .unwrap();
  assert_eq!((feed.pattern, feed.strategy), (None, None));
  let path = format!("/sources/{}", feed.id);
  let (status, _) = raw(&base, reqwest::Method::PATCH, &path, Some(json!({"pattern": "/news/"}))).await;
  assert_eq!(status, StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn sources_are_registered_with_their_kind_s_address_and_refused_when_malformed() {
  let (_ground, base, client) = served().await;
  let hub = client
    .add_source(&NewSource {
      category_name: "it".into(),
      kind: "huggingface-trending".into(),
      ..NewSource::default()
    })
    .await
    .unwrap();
  assert_eq!((hub.address.as_str(), hub.limit), ("https://huggingface.co/api/models", 30));
  for (body, status) in [
    (json!({"categoryName": "it", "kind": "page"}), StatusCode::BAD_REQUEST),
    (json!({"categoryName": "it", "kind": "arxiv"}), StatusCode::BAD_REQUEST),
    (json!({"categoryName": "it", "kind": "rss"}), StatusCode::BAD_REQUEST),
    (
      json!({"categoryName": "it", "kind": "rss", "address": "https://example.org/f", "limit": 0}),
      StatusCode::BAD_REQUEST,
    ),
    (
      json!({"categoryName": "nothing", "kind": "rss", "address": "https://example.org/f"}),
      StatusCode::NOT_FOUND,
    ),
    (json!({"kind": "rss"}), StatusCode::BAD_REQUEST),
  ] {
    let (answered, failure) = raw(&base, reqwest::Method::POST, "/sources", Some(body.clone())).await;
    assert_eq!(answered, status, "{body}");
    assert!(failure["code"].is_string() && failure["error"].is_string());
  }
  let changed = client
    .change_source(
      hub.id,
      &SourceChange {
        filter: Some(Filter {
          require: vec!["gguf".into()],
          exclude: vec![],
        }),
        every_minutes: Some(120),
        ..SourceChange::default()
      },
    )
    .await
    .unwrap();
  assert_eq!((changed.filter.require.len(), changed.every_minutes), (1, Some(120)));
  assert_eq!(client.sources(Some("it"), None, 50).await.unwrap().items.len(), 1);
  assert!(client.sources(Some("security"), None, 50).await.unwrap().items.is_empty());
  client.remove_source(hub.id).await.unwrap();
  assert!(client.source(hub.id).await.unwrap_err().starts_with("404"));
  let (status, _) = raw(&base, reqwest::Method::GET, "/sources/abc", None).await;
  assert_eq!(status, StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn a_source_answers_what_its_readings_left() {
  let (ground, base, client) = served().await;
  let feed = client
    .add_source(&NewSource {
      category_name: "security".into(),
      kind: "rss".into(),
      address: Some(format!("{}/feed.xml", ground.sources)),
      ..NewSource::default()
    })
    .await
    .unwrap();
  let (_, unread) = raw(&base, reqwest::Method::GET, &format!("/sources/{}", feed.id), None).await;
  for (field, value) in [
    ("readAt", json!(null)),
    ("scanCount", json!(0)),
    ("lastYield", json!(null)),
    ("lastTaken", json!(null)),
    ("lastError", json!(null)),
  ] {
    assert_eq!(unread[field], value, "{field}");
  }
  research_server::scan::scan(&ground.app).await.unwrap();
  let read = client.source(feed.id).await.unwrap();
  assert!(read.read_at.is_some());
  assert_eq!(read.scan_count, 1);
  assert!(read.last_yield.is_some_and(|n| n > 0) && read.last_taken == read.last_yield, "{read:?}");
  let (_, listed) = raw(&base, reqwest::Method::GET, "/sources?categoryName=security", None).await;
  assert_eq!(listed["items"][0]["scanCount"], 1);
  assert_eq!(listed["items"][0]["lastTaken"], json!(read.last_taken));
}

#[tokio::test]
async fn the_queue_takes_asks_dismissals_takings_and_front_moves() {
  let (_ground, base, client) = served().await;
  let ask = |name: &str, category: Option<&str>, shape: Option<Shape>| Ask {
    name: name.into(),
    address: format!("https://example.org/{name}"),
    category_name: category.map(String::from),
    shape,
    ..Ask::default()
  };
  let one = client.ask(&ask("one", None, None)).await.unwrap();
  assert_eq!((one.category_name.as_str(), one.shape, one.asked), ("it", Shape::Manual, true));
  let two = client.ask(&ask("two", None, Some(Shape::Investigation))).await.unwrap();
  assert_eq!(two.shape, Shape::Investigation);
  assert!(client.ask(&ask("one", None, None)).await.unwrap_err().starts_with("409 conflict"));
  assert!(
    client
      .ask(&ask("three", None, Some(Shape::Advisory)))
      .await
      .unwrap_err()
      .starts_with("400")
  );
  assert!(
    client
      .ask(&ask("three", Some("security"), Some(Shape::Manual)))
      .await
      .unwrap_err()
      .starts_with("400")
  );
  assert!(client.ask(&ask("three", Some("nothing"), None)).await.unwrap_err().starts_with("404"));
  let advisory = client.ask(&ask("three", Some("security"), None)).await.unwrap();
  assert_eq!(advisory.shape, Shape::Advisory);

  let moved = client
    .change_item(
      two.id,
      &ItemChange {
        front: Some(true),
        ..ItemChange::default()
      },
    )
    .await
    .unwrap();
  let waiting = client.queue(Some("it"), &[ItemState::Waiting], None, 50).await.unwrap().items;
  assert_eq!(waiting[0].id, moved.id);
  let page = client.queue(None, &[], Some(waiting[0].id), 1).await.unwrap();
  assert_eq!((page.items.len(), page.more), (1, true));

  let (status, _) = raw(
    &base,
    reqwest::Method::PATCH,
    &format!("/queue/{}", one.id),
    Some(json!({"state": "taken"})),
  )
  .await;
  assert_eq!(status, StatusCode::BAD_REQUEST, "taken names its job item");
  let err = client
    .change_item(
      one.id,
      &ItemChange {
        state: Some(ItemState::Taken),
        job_item_id: Some(9),
        front: None,
      },
    )
    .await
    .unwrap_err();
  assert!(err.starts_with("409 conflict"), "a waiting item is not taken: {err}");
  let (status, _) = raw(
    &base,
    reqwest::Method::PATCH,
    &format!("/queue/{}", one.id),
    Some(json!({"state": "running"})),
  )
  .await;
  assert_eq!(status, StatusCode::CONFLICT);
  let (status, _) = raw(
    &base,
    reqwest::Method::PATCH,
    &format!("/queue/{}", one.id),
    Some(json!({"state": "waiting"})),
  )
  .await;
  assert_eq!(status, StatusCode::CONFLICT, "only a stuck item is asked again");
  let (status, _) =
    raw(&base, reqwest::Method::PATCH, &format!("/queue/{}", one.id), Some(json!({}))).await;
  assert_eq!(status, StatusCode::BAD_REQUEST);
  let dismissed = client
    .change_item(
      one.id,
      &ItemChange {
        state: Some(ItemState::Dismissed),
        ..ItemChange::default()
      },
    )
    .await
    .unwrap();
  assert_eq!(dismissed.state, ItemState::Dismissed);
  let (status, _) = raw(&base, reqwest::Method::GET, "/queue?state=sleeping", None).await;
  assert_eq!(status, StatusCode::BAD_REQUEST);
  assert!(client.item(999).await.unwrap_err().starts_with("404"));
}

#[tokio::test]
async fn an_ask_says_why_and_a_follow_up_names_the_research_it_follows_from() {
  let (ground, _base, client) = served().await;
  let ask = |name: &str| Ask {
    name: name.into(),
    address: format!("https://example.org/{name}"),
    because: Some(format!("  why {name}  ")),
    ..Ask::default()
  };
  let parent = client.ask(&ask("parent")).await.unwrap();
  assert_eq!(
    (parent.origin.kind, parent.origin.because.as_deref()),
    (OriginKind::Asked, Some("why parent"))
  );
  let follows = |name: &str, item: Option<i64>, document: Option<i64>| Ask {
    follows_item_id: item,
    follows_document_id: document,
    ..ask(name)
  };
  let early = client.ask(&follows("early", Some(parent.id), None)).await.unwrap_err();
  assert!(early.starts_with("409 conflict"), "a parent with no document yet: {early}");
  assert!(client.ask(&follows("lost", Some(999), None)).await.unwrap_err().starts_with("404"));

  ground.app.store.start_try("it", &[parent.id]).unwrap();
  let filed = research_server::store::Filed {
    document_id: 812,
    title: "Parent manual".into(),
    share_url: "https://share.example/d812".into(),
    ..Default::default()
  };
  ground.app.store.file(parent.id, &filed).unwrap();
  let mismatched = client.ask(&follows("mixed", Some(parent.id), Some(900))).await.unwrap_err();
  assert!(mismatched.starts_with("400"), "{mismatched}");
  let child = client.ask(&follows("child", None, Some(812))).await.unwrap();
  let origin = client.item(child.id).await.unwrap().origin;
  assert_eq!(origin.kind, OriginKind::FollowUp);
  assert_eq!(
    (origin.item_id, origin.document_id, origin.title.as_deref(), origin.share_url.as_deref()),
    (Some(parent.id), Some(812), Some("Parent manual"), Some("https://share.example/d812"))
  );
  assert_eq!(origin.because.as_deref(), Some("why child"));
  let listed = client.queue(Some("it"), &[ItemState::Waiting], None, 50).await.unwrap().items;
  assert_eq!(listed.iter().find(|item| item.id == child.id).map(|item| &item.origin), Some(&origin));
}

#[tokio::test]
async fn recipients_stack_runs_and_facts_answer_their_shapes() {
  let (_ground, base, client) = served().await;
  let agent = client
    .add_recipient(
      "economy",
      &NewRecipient {
        kind: RecipientKind::Agent,
        room_id: None,
        agent_name: Some("trader".into()),
      },
    )
    .await
    .unwrap();
  assert_eq!(agent.kind, RecipientKind::Agent);
  let (status, _) = raw(
    &base,
    reqwest::Method::POST,
    "/categories/economy/recipients",
    Some(json!({"kind": "room", "agentName": "x"})),
  )
  .await;
  assert_eq!(status, StatusCode::BAD_REQUEST);
  client.remove_recipient("economy", agent.id).await.unwrap();
  assert!(client.remove_recipient("economy", agent.id).await.unwrap_err().starts_with("404"));

  let (status, _) =
    raw(&base, reqwest::Method::POST, "/stack", Some(json!({"component": "libexample"}))).await;
  assert_eq!(status, StatusCode::BAD_REQUEST, "a row names a place or a repository");
  let row = client
    .add_stack_row(&NewStackRow {
      component: "libexample".into(),
      version: Some("1.4.1".into()),
      repository: Some("services/alpha".into()),
      worker: Some("worker-a".into()),
      place: None,
    })
    .await
    .unwrap();
  assert_eq!(client.stack().await.unwrap().items, std::slice::from_ref(&row));
  client.remove_stack_row(row.id).await.unwrap();

  assert!(client.runs(None, None, None, 10).await.unwrap().items.is_empty());
  assert!(client.run(5).await.unwrap_err().starts_with("404"));
  assert!(client.facts(None, None, 10).await.unwrap().items.is_empty());
}

// A topic is a category whose question is the owner's own (docs/design.md, 「Categories」): it names its
// instruction, gathers on its own period and may name the calendar its dated findings are written to.
#[tokio::test]
async fn a_topic_is_registered_with_its_instruction_and_its_calendar_and_changed() {
  let (_ground, base, client) = served().await;
  let topic = NewCategory {
    name: "ai-events".into(),
    purpose: "AI events in the country".into(),
    shape: CategoryShape::Topic,
    cadence_minutes: None,
    gather_minutes: Some(30),
    provider: None,
    model: None,
    instruction: Some("Find AI conferences and meetups announced for the coming months.".into()),
    calendar: Some(CalendarTarget {
      realm: "mine".into(),
      calendar_id: "work".into(),
    }),
  };
  let made = client.add_category(&topic).await.unwrap();
  assert_eq!((made.shape, made.gather_minutes), (CategoryShape::Topic, Some(30)));
  assert_eq!(made.instruction.as_deref(), topic.instruction.as_deref());
  assert_eq!(made.calendar, topic.calendar);

  // A topic gathers daily where it names no period, and stands with no calendar where it names none.
  let plain = client
    .add_category(&NewCategory {
      name: "notes".into(),
      gather_minutes: None,
      calendar: None,
      ..topic.clone()
    })
    .await
    .unwrap();
  assert_eq!((plain.gather_minutes, plain.calendar), (Some(1440), None));

  // What a topic may not be registered as, each refused with nothing written.
  let refused = [
    NewCategory {
      name: "no-instruction".into(),
      instruction: None,
      ..topic.clone()
    },
    NewCategory {
      name: "blank-instruction".into(),
      instruction: Some("  ".into()),
      ..topic.clone()
    },
    NewCategory {
      name: "bad-realm".into(),
      calendar: Some(CalendarTarget {
        realm: "theirs".into(),
        calendar_id: "work".into(),
      }),
      ..topic.clone()
    },
    NewCategory {
      name: "no-calendar-id".into(),
      calendar: Some(CalendarTarget {
        realm: "organization".into(),
        calendar_id: " ".into(),
      }),
      ..topic.clone()
    },
    // An instruction and a calendar are a topic's alone.
    NewCategory {
      name: "not-a-topic".into(),
      shape: CategoryShape::Advisory,
      gather_minutes: None,
      ..topic.clone()
    },
  ];
  for new in &refused {
    assert!(
      client.add_category(new).await.unwrap_err().starts_with("400 bad-request"),
      "{}",
      new.name
    );
    assert!(client.category(&new.name).await.is_err(), "{} was not written", new.name);
  }

  // Its instruction and its period change, its calendar is taken away by null and kept where left out.
  let changed = client
    .change_category(
      "ai-events",
      &CategoryChange {
        instruction: Some("Find AI conferences only.".into()),
        gather_minutes: Some(1440),
        ..CategoryChange::default()
      },
    )
    .await
    .unwrap();
  assert_eq!(changed.instruction.as_deref(), Some("Find AI conferences only."));
  assert_eq!((changed.gather_minutes, changed.calendar.is_some()), (Some(1440), true));
  let (status, body) =
    raw(&base, reqwest::Method::PATCH, "/categories/ai-events", Some(json!({"calendar": null}))).await;
  assert_eq!(status, StatusCode::OK);
  assert_eq!(body["calendar"], serde_json::Value::Null);
  assert_eq!(body["instruction"], "Find AI conferences only.");

  // A topic's instruction is never emptied, and no other shape takes one.
  let (status, _) = raw(
    &base,
    reqwest::Method::PATCH,
    "/categories/ai-events",
    Some(json!({"instruction": " "})),
  )
  .await;
  assert_eq!(status, StatusCode::BAD_REQUEST);
  let (status, _) =
    raw(&base, reqwest::Method::PATCH, "/categories/it", Some(json!({"instruction": "x"}))).await;
  assert_eq!(status, StatusCode::BAD_REQUEST);
  let (status, _) = raw(
    &base,
    reqwest::Method::PATCH,
    "/categories/it",
    Some(json!({"calendar": {"realm": "mine", "calendarId": "work"}})),
  )
  .await;
  assert_eq!(status, StatusCode::BAD_REQUEST);
}
