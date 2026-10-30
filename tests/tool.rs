//! The research tool (`docs/api.md`, 「The tools」): its manifest held to the executable's entries, a call
//! handed the way the tool host hands it answered against a served daemon, and the owner's actions refused to
//! a session that does not hold the owner's capability.

mod support;

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
  assert_eq!(manifest["tool"]["name"].as_str(), Some("research"));
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
    let owner_only = ["taken", "dismiss", "stop", "resume"].contains(&entry["name"].as_str().unwrap());
    assert_eq!(capabilities, [if owner_only { research_tool::OWNER } else { "research.ask" }]);
  }
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
