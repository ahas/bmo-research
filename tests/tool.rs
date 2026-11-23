//! The research tool (`docs/api.md`, 「The tools」): its manifest held to the executable's entries, a call
//! handed the way the tool host hands it answered against a served daemon, the owner's actions refused to a
//! session that does not hold the owner's capability, a research sent out, and a skill read whole where the
//! record hands it or the chat asks it, with the asker's own rule after it where they keep one.

use crate::support;

use std::path::PathBuf;

use serde_json::{Value, json};

fn manifest() -> toml::Table {
  let text =
    std::fs::read_to_string(PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tools/research/tool.toml"))
      .expect("the manifest reads");
  text.parse().expect("the manifest is TOML")
}

#[test]
fn the_manifest_names_exactly_the_executable_s_entries_with_their_schemas() {
  let manifest = manifest();
  assert_eq!(manifest["tool"]["name"].as_str(), Some("researcher"));
  assert_eq!(manifest["tool"]["protocol"].as_integer(), Some(1));
  let entries = manifest["entry"].as_array().unwrap();
  let names: Vec<&str> = entries.iter().map(|e| e["name"].as_str().unwrap()).collect();
  assert_eq!(names, research_tool::ENTRIES);
  let folder = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tools/research");
  for entry in entries {
    assert_eq!(entry["namespace"].as_str(), Some(research_tool::NAMESPACE));
    let input = entry["input"].as_str().unwrap();
    let schema: Value =
      serde_json::from_str(&std::fs::read_to_string(folder.join(input)).unwrap()).unwrap();
    assert_eq!(schema["type"], "object", "{input}");
    let capabilities: Vec<&str> =
      entry["capabilities"].as_array().unwrap().iter().map(|c| c.as_str().unwrap()).collect();
    let expected = match entry["name"].as_str().unwrap() {
      "taken" | "dismiss" | "stop" | "resume" => research_tool::OWNER,
      "rulebook" => "web.rulebooks",
      _ => "research.ask",
    };
    assert_eq!(capabilities, [expected]);
  }
}

#[test]
fn the_categories_entry_declares_every_field_a_category_is_added_with() {
  let folder = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tools/research");
  let schema: Value =
    serde_json::from_str(&std::fs::read_to_string(folder.join("categories.json")).unwrap()).unwrap();

  // The entry takes no argument it does not declare, so a field the executable reads and the declaration
  // leaves out could never be handed.
  assert_eq!(schema["additionalProperties"], false);

  for field in [
    "action",
    "name",
    "shape",
    "purpose",
    "provider",
    "model",
    "cadenceMinutes",
    "gatherMinutes",
    "instruction",
    "calendar",
  ] {
    assert!(schema["properties"].get(field).is_some(), "{field}");
  }

  let shapes: Vec<&str> = schema["properties"]["shape"]["enum"]
    .as_array()
    .unwrap()
    .iter()
    .map(|one| one.as_str().unwrap())
    .collect();

  assert_eq!(shapes, ["manual-or-investigation", "advisory", "brief", "topic"]);
  assert_eq!(schema["properties"]["calendar"]["required"], json!(["realm", "calendarId"]));
}

/// A served daemon, its settings and token written under a data root the way the deployment writes them.
async fn daemon() -> (support::Ground, PathBuf) {
  let ground = support::ground(|_| {}).await;
  let base = support::serve(research_server::routes::router(ground.app.clone())).await;
  let root = ground.root.clone();
  std::fs::create_dir_all(root.join("research")).unwrap();
  let listen = base.trim_start_matches("http://");
  std::fs::write(root.join("research/settings.toml"), format!("listen = \"{listen}\"\n")).unwrap();
  std::fs::write(root.join("env/research.token"), "test-token\n").unwrap();
  (ground, root)
}

fn handed(root: &std::path::Path, entry: &str, arguments: Value, capabilities: &[&str]) -> Value {
  json!({
    "protocol": 1,
    "kind": "call",
    "entry": entry,
    "arguments": arguments,
    "session": {
      "holderName": "a-chat-agent",
      "namespaces": ["research"],
      "capabilities": capabilities,
      "variables": { "BMO_DATA_DIR": root.to_string_lossy() },
      "labels": {},
    },
    "callback": null,
  })
}

fn text(answer: &Value) -> &str {
  answer["content"][0]["text"].as_str().unwrap_or_default()
}

#[tokio::test]
async fn an_ask_and_the_queue_answer_a_session_holding_the_ask_capability() {
  let (_ground, root) = daemon().await;
  let asking = ["research.ask"];
  let asked = research_tool::answer(handed(
    &root,
    "research/ask",
    json!({"name": "Thing-7B", "address": "https://example.org/thing"}),
    &asking,
  ))
  .await;
  assert_eq!(asked["isError"], false, "{asked}");
  assert!(text(&asked).contains("Thing-7B"));
  let queue =
    research_tool::answer(handed(&root, "research/queue", json!({"state": "waiting"}), &asking)).await;
  assert!(text(&queue).contains("Thing-7B") && text(&queue).contains("[it]"), "{queue}");
  let sources = research_tool::answer(handed(&root, "research/sources", json!({}), &asking)).await;
  assert_eq!(text(&sources), "No source is registered.");
  let rooms =
    research_tool::answer(handed(&root, "research/rooms", json!({"category": "it"}), &asking)).await;
  assert_eq!(rooms["isError"], false);
}

#[tokio::test]
async fn the_owner_s_actions_are_refused_without_the_owner_s_capability() {
  let (ground, root) = daemon().await;
  let asking = ["research.ask"];
  for (entry, arguments) in [
    (
      "research/sources",
      json!({"action": "add", "category": "it", "kind": "rss", "address": "https://example.org/f"}),
    ),
    ("research/categories", json!({"action": "remove", "name": "economy"})),
    ("research/recipients", json!({"action": "add", "category": "it", "agent": "worker-a"})),
    ("research/stop", json!({})),
  ] {
    let answer = research_tool::answer(handed(&root, entry, arguments, &asking)).await;
    assert_eq!(answer["isError"], true, "{entry}");
    assert!(text(&answer).contains(research_tool::OWNER), "{entry}: {answer}");
  }
  assert!(ground.app.store.sources(None, None, 10).unwrap().items.is_empty());

  let owner = ["research.ask", research_tool::OWNER];
  let added = research_tool::answer(handed(
    &root,
    "research/sources",
    json!({"action": "add", "category": "security", "kind": "rss", "address": "https://example.org/f", "require": ["libexample"]}),
    &owner,
  ))
  .await;
  assert_eq!(added["isError"], false, "{added}");
  let source = &ground.app.store.sources(Some("security"), None, 10).unwrap().items[0];
  assert_eq!(source.filter.require, ["libexample"]);
  let agent = research_tool::answer(handed(
    &root,
    "research/recipients",
    json!({"action": "add", "category": "security", "agent": "worker-a"}),
    &owner,
  ))
  .await;
  assert_eq!(agent["isError"], false, "{agent}");
  let removed = research_tool::answer(handed(
    &root,
    "research/recipients",
    json!({"action": "remove", "category": "security", "agent": "worker-a"}),
    &owner,
  ))
  .await;
  assert_eq!(removed["isError"], false, "{removed}");
  let stopped =
    research_tool::answer(handed(&root, "research/stop", json!({"category": "economy"}), &owner)).await;
  assert!(text(&stopped).contains("economy: stopped"), "{stopped}");
}

#[tokio::test]
async fn a_request_that_is_no_call_or_names_no_entry_is_refused() {
  let (_ground, root) = daemon().await;
  let mut other = handed(&root, "research/ask", json!({}), &[]);
  other["kind"] = json!("claim");
  assert_eq!(research_tool::answer(other).await["isError"], true);
  let unknown =
    research_tool::answer(handed(&root, "research/approve", json!({}), &["research.ask"])).await;
  assert!(text(&unknown).contains("no entry"));
  let missing =
    research_tool::answer(handed(&root, "research/ask", json!({"name": "x"}), &["research.ask"])).await;
  assert!(text(&missing).contains("address"));
  let listed = research_tool::answer(json!({"protocol": 1, "kind": "resources_list"})).await;
  assert_eq!(listed, json!({"resources": []}));
}

#[tokio::test]
async fn a_research_is_sent_out_through_the_runner() {
  let (ground, root) = daemon().await;
  let answer = research_tool::answer(handed(
    &root,
    "research/run",
    json!({"key": "search:basic", "query": "What does Thing-7B cost?", "roomId": 5}),
    &["research.ask"],
  ))
  .await;
  assert_eq!(answer["isError"], false, "{answer}");
  assert!(text(&answer).starts_with("Research run 1 on search:basic is "), "{answer}");
  assert!(text(&answer).contains("registered · slot-model"), "{answer}");
  let slots = ground.run_slots();
  assert_eq!(
    (slots[0]["role"].as_str(), slots[0]["source"].as_str()),
    (Some("voyager"), Some("search"))
  );
  let refused = research_tool::answer(handed(
    &root,
    "research/run",
    json!({"key": "search:deep", "query": "Q"}),
    &["research.ask"],
  ))
  .await;
  assert_eq!(refused["isError"], true);
  assert!(text(&refused).contains("400"), "{refused}");
}

fn rulebook_call(labels: Value, name: &str) -> Value {
  json!({
    "protocol": 1,
    "kind": "call",
    "entry": "research/rulebook",
    "arguments": {"name": name},
    "session": {"holderName": "search", "capabilities": ["web.rulebooks"], "variables": {}, "labels": labels},
  })
}

#[tokio::test]
async fn a_rulebook_the_record_does_not_hand_is_refused_with_the_names_it_does() {
  let labels = json!({"research": {"rulebooks": ["bmo-research-products", "bmo-research-deep-dives"]}});
  let refused = research_tool::answer(rulebook_call(labels, "bmo-research-routine")).await;
  assert_eq!(refused["isError"], true);
  assert!(text(&refused).contains("bmo-research-products, bmo-research-deep-dives"), "{refused}");
  let none = research_tool::answer(rulebook_call(json!({}), "bmo-research-routine")).await;
  assert_eq!(none["isError"], true);
  assert!(text(&none).contains("handed no rulebook"), "{none}");
}

/// A deploy tree with two skills placed under `prompts/skills/`, and a data root whose people's rules give
/// one person a restaurant rule of their own; answered as the tree's `bin/` and the data root.
fn placed() -> (PathBuf, PathBuf) {
  let tree = support::scratch().join("placed");
  for name in ["bmo-research-restaurants", "bmo-research-products"] {
    std::fs::create_dir_all(tree.join("prompts/skills").join(name)).unwrap();
    std::fs::write(
      tree.join("prompts/skills").join(name).join("SKILL.md"),
      format!("---\nname: {name}\ndescription: \"When it is read.\"\nbundle: worker\n---\n\n# {name}\n\nThe default rule.\n"),
    )
    .unwrap();
  }
  let inside = tree.join("bin");
  std::fs::create_dir_all(&inside).unwrap();
  let root = support::scratch().join("data");
  std::fs::create_dir_all(root.join("research")).unwrap();
  std::fs::write(
    root.join(research_tool::PEOPLE),
    "[[person]]\nnames = [\"the owner\", \"Sam\"]\n[person.rules]\nbmo-research-restaurants = \"\"\"\n## Sam's rule\n\nFive checks.\n\"\"\"\n",
  )
  .unwrap();
  (inside, root)
}

#[test]
fn a_rulebook_the_record_hands_is_its_skill_whole_with_the_asker_s_own_rule_after_it() {
  let (inside, root) = placed();
  assert_eq!(
    research_tool::skills_folder(&inside),
    Some(inside.parent().unwrap().join("prompts/skills"))
  );
  let handed = |owner: Value| json!({"research": {"rulebooks": ["bmo-research-restaurants", "bmo-research-products"], "owner": owner}});
  let read = |labels: Value, name: &str| {
    research_tool::rulebook(&inside, &root, &labels, &json!({"name": name})).unwrap()
  };
  assert_eq!(
    read(handed(Value::Null), "bmo-research-products"),
    "SKILL bmo-research-products:\n# bmo-research-products\n\nThe default rule.\n",
    "the front matter is left out, and a skill the asker keeps no rule for is the skill alone"
  );
  let own = "SKILL bmo-research-restaurants:\n# bmo-research-restaurants\n\nThe default rule.\n\n\
             THE ASKER'S OWN RULE:\n## Sam's rule\n\nFive checks.\n";
  assert_eq!(
    read(handed(Value::Null), "bmo-research-restaurants"),
    own,
    "an ask naming nobody is the owner's"
  );
  assert_eq!(
    read(handed(json!(" sam ")), "bmo-research-restaurants"),
    own,
    "a name is matched as said"
  );
  assert_eq!(
    read(handed(json!("Lee")), "bmo-research-restaurants"),
    "SKILL bmo-research-restaurants:\n# bmo-research-restaurants\n\nThe default rule.\n",
    "a person keeping no rule of their own is held to the default rule"
  );

  std::fs::write(root.join(research_tool::PEOPLE), "[[person]]\nname = 1\n").unwrap();
  let unread = read(handed(json!("Sam")), "bmo-research-restaurants");
  assert!(
    unread.contains("could not be read") && unread.contains("apply the default rule"),
    "{unread}"
  );
}

#[test]
fn a_research_skill_is_read_whole_for_the_asker_the_chat_names_and_any_other_name_refused() {
  let (inside, root) = placed();
  let read = |arguments: Value| research_tool::skill(&inside, &root, &arguments);
  let own = read(json!({"name": "bmo-research-restaurants", "asker": "Sam"})).unwrap();
  assert!(own.ends_with("THE ASKER'S OWN RULE:\n## Sam's rule\n\nFive checks.\n"), "{own}");
  let other = read(json!({"name": "bmo-research-restaurants", "asker": "Lee"})).unwrap();
  assert!(!other.contains("THE ASKER'S OWN RULE"), "{other}");
  let refused = read(json!({"name": "bmo-research-anything"})).unwrap_err();
  assert!(refused.contains("bmo-research-products, bmo-research-restaurants"), "{refused}");
}
