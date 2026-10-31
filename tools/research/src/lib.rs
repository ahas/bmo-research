//! The research namespace's tool (`docs/api.md`, 「The tools」): one executable answering every entry of the
//! namespace by calling the daemon's routes, at the address and with the token the daemon's settings name
//! under the data root the session record's `BMO_DATA_DIR` names; the rulebook reader alone reads no route but
//! the rulebooks placed beside the executable. It speaks the tool host's call protocol 1: one request on
//! standard input, one tool result on standard output.

use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

use bmo_research_client::*;
use serde::Deserialize;
use serde_json::{Value, json};

/// The protocol version this tool speaks.
pub const PROTOCOL: u64 = 1;

/// The namespace every entry stands under.
pub const NAMESPACE: &str = "research";

/// The capability a session must also hold for an entry's changing actions.
pub const OWNER: &str = "research.owner";

/// Every entry this executable answers, by its name in the namespace.
pub const ENTRIES: &[&str] = &[
  "ask",
  "queue",
  "taken",
  "dismiss",
  "categories",
  "sources",
  "recipients",
  "rooms",
  "stop",
  "resume",
  "run",
  "rulebook",
];

/// The folder under which a rulebook is read: `prompts/rules` under the tool's working folder or the nearest
/// folder above it holding one.
pub const RULES: &str = "prompts/rules";

/// A tool result: text for the model, marked as an error where it is a refusal.
pub fn result(text: impl Into<String>, error: bool) -> Value {
  json!({ "content": [{ "type": "text", "text": text.into() }], "isError": error })
}

/// The daemon's address and token, read from its settings under the data root.
#[derive(Debug, Clone)]
pub struct Daemon {
  pub address: String,
  pub token: String,
}

#[derive(Deserialize)]
struct DaemonSettings {
  #[serde(default = "default_listen")]
  listen: String,
  #[serde(default = "default_token_file")]
  token_file: String,
}

fn default_listen() -> String {
  "127.0.0.1:3238".into()
}

fn default_token_file() -> String {
  "research.token".into()
}

impl Daemon {
  /// The daemon the settings under `data_root` name: `research/settings.toml`, its `listen` and the token
  /// file under `env/` its `token_file` names, each at its default where the file names none.
  pub fn from_data_root(data_root: &Path) -> Result<Daemon, String> {
    let path = data_root.join("research/settings.toml");
    let settings: DaemonSettings = match std::fs::read_to_string(&path) {
      Ok(text) => toml::from_str(&text).map_err(|e| format!("{}: {e}", path.display()))?,
      Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
        toml::from_str("").map_err(|e| e.to_string())?
      }
      Err(e) => return Err(format!("{}: {e}", path.display())),
    };
    let named = PathBuf::from(&settings.token_file);
    let token_path = if named.is_absolute() { named } else { data_root.join("env").join(named) };
    let token = std::fs::read_to_string(&token_path)
      .map(|t| t.trim().to_string())
      .map_err(|e| format!("{}: {e}", token_path.display()))?;
    Ok(Daemon {
      address: format!("http://{}", settings.listen),
      token,
    })
  }

  pub fn client(&self) -> ResearchClient {
    ResearchClient::new(&self.address).with_token(&self.token)
  }
}

/// One request read off its JSON: a call answered with a tool result, any other kind refused.
pub async fn answer(handed: Value) -> Value {
  if handed["protocol"].as_u64() != Some(PROTOCOL) {
    return result(format!("ERROR: this tool speaks protocol {PROTOCOL}"), true);
  }
  match handed["kind"].as_str() {
    Some("call") => {}
    Some("resources_list") => return json!({ "resources": [] }),
    Some("resources_read") => return json!({ "error": "this tool serves no resources" }),
    other => {
      return result(
        format!("ERROR: this tool answers calls alone, not {}", other.unwrap_or("none")),
        true,
      );
    }
  }
  let entry = handed["entry"].as_str().unwrap_or_default();
  let Some(name) = entry.strip_prefix("research/").filter(|n| ENTRIES.contains(n)) else {
    return result(format!("ERROR: this tool holds no entry {entry}"), true);
  };
  let session = &handed["session"];
  let arguments = handed.get("arguments").cloned().unwrap_or_else(|| json!({}));
  if name == "rulebook" {
    let here = std::env::current_dir().unwrap_or_default();
    return match rulebook(&here, &session["labels"], &arguments) {
      Ok(text) => result(text, false),
      Err(why) => result(why, true),
    };
  }
  let capabilities: BTreeSet<String> = session["capabilities"]
    .as_array()
    .map(|c| c.iter().filter_map(|c| c.as_str().map(String::from)).collect())
    .unwrap_or_default();
  let Some(root) = session["variables"]["BMO_DATA_DIR"].as_str().filter(|r| !r.is_empty()) else {
    return result("ERROR: the session record names no BMO_DATA_DIR", true);
  };
  let daemon = match Daemon::from_data_root(Path::new(root)) {
    Ok(d) => d,
    Err(why) => return result(format!("ERROR: the daemon's settings could not be read: {why}"), true),
  };
  match call(&daemon.client(), name, &arguments, capabilities.contains(OWNER)).await {
    Ok(text) => result(text, false),
    Err(why) => result(why, true),
  }
}

fn text(arguments: &Value, key: &str) -> Option<String> {
  arguments[key].as_str().map(str::trim).filter(|v| !v.is_empty()).map(String::from)
}

fn need_text(arguments: &Value, key: &str) -> Result<String, String> {
  text(arguments, key).ok_or_else(|| format!("the call names no {key}"))
}

fn need_number(arguments: &Value, key: &str) -> Result<i64, String> {
  arguments[key].as_i64().ok_or_else(|| format!("the call names no {key}"))
}

fn words(arguments: &Value, key: &str) -> Vec<String> {
  arguments[key]
    .as_array()
    .map(|w| w.iter().filter_map(|w| w.as_str().map(String::from)).collect())
    .unwrap_or_default()
}

/// The owner's capability, asked of a changing action.
fn owner(held: bool, what: &str) -> Result<(), String> {
  if held {
    Ok(())
  } else {
    Err(format!("{what} is the owner's: this session does not hold {OWNER}"))
  }
}

/// One entry called with its arguments: the text the model reads, or the refusal.
pub async fn call(
  client: &ResearchClient,
  name: &str,
  arguments: &Value,
  is_owner: bool,
) -> Result<String, String> {
  match name {
    "ask" => {
      let shape = match text(arguments, "shape") {
        Some(word) => Some(Shape::parse(&word).ok_or_else(|| format!("{word} is no shape"))?),
        None => None,
      };
      let item = client
        .ask(&Ask {
          name: need_text(arguments, "name")?,
          address: need_text(arguments, "address")?,
          category_name: text(arguments, "category"),
          shape,
          because: text(arguments, "because"),
          follows_item_id: arguments.get("follows").and_then(Value::as_i64),
          follows_document_id: arguments.get("document").and_then(Value::as_i64),
        })
        .await?;
      let follows = match (item.origin.item_id, item.origin.document_id) {
        (Some(parent), _) => format!(", a follow-up of #{parent}"),
        (None, Some(document)) => format!(", a follow-up of document {document}"),
        (None, None) => String::new(),
      };
      Ok(format!(
        "Queued #{} {} in {} as a {}{follows}.",
        item.id,
        item.name,
        item.category_name,
        item.shape.as_str()
      ))
    }
    "queue" => {
      let states: Vec<ItemState> = text(arguments, "state")
        .unwrap_or_default()
        .split(',')
        .filter(|w| !w.trim().is_empty())
        .map(|w| ItemState::parse(w.trim()).ok_or_else(|| format!("{w} is no item state")))
        .collect::<Result<_, _>>()?;
      let page = client.queue(text(arguments, "category").as_deref(), &states, None, 50).await?;
      let mut lines: Vec<String> = client
        .state()
        .await?
        .holds
        .iter()
        .map(|hold| format!("Held: {}.", hold.line()))
        .collect();
      if page.items.is_empty() {
        lines.push("The queue holds nothing of that.".into());
      }
      lines.extend(page.items.iter().map(item_line));
      Ok(lines.join("\n"))
    }
    "taken" => {
      owner(is_owner, "Marking an item taken")?;
      let change = ItemChange {
        state: Some(ItemState::Taken),
        job_item_id: Some(need_number(arguments, "job")?),
        front: None,
      };
      let item = client.change_item(need_number(arguments, "item")?, &change).await?;
      Ok(format!(
        "#{} {} is taken by job item {}.",
        item.id,
        item.name,
        item.job_item_id.unwrap_or_default()
      ))
    }
    "dismiss" => {
      owner(is_owner, "Dismissing an item")?;
      let change = ItemChange {
        state: Some(ItemState::Dismissed),
        ..ItemChange::default()
      };
      let item = client.change_item(need_number(arguments, "item")?, &change).await?;
      Ok(format!("#{} {} is dismissed.", item.id, item.name))
    }
    "categories" => categories(client, arguments, is_owner).await,
    "sources" => sources(client, arguments, is_owner).await,
    "recipients" => recipients(client, arguments, is_owner, false).await,
    "rooms" => recipients(client, arguments, is_owner, true).await,
    "stop" | "resume" => {
      owner(is_owner, "Stopping and resuming")?;
      let state = if name == "stop" { Running::Stopped } else { Running::Running };
      let answered = client
        .set_state(&StateChange {
          state,
          category_name: text(arguments, "category"),
        })
        .await?;
      let lines: Vec<String> = answered
        .categories
        .iter()
        .map(|c| format!("{}: {}", c.category_name, c.state.as_str()))
        .collect();
      Ok(lines.join("\n"))
    }
    "run" => {
      let ask = ResearchRunAsk {
        key: need_text(arguments, "key")?,
        query: need_text(arguments, "query")?,
        asker: text(arguments, "asker"),
        options: text(arguments, "options"),
        room_id: arguments["roomId"].as_i64(),
        provider: text(arguments, "provider"),
        model: text(arguments, "model"),
      };
      let run = client.ask_research(&ask).await?;
      let closes = run
        .closes_at
        .map(|at| format!(", its web tools closing at {at} (epoch ms)"))
        .unwrap_or_default();
      Ok(format!(
        "Research run {} on {} is {} on {} · {}{closes}. Its result comes back to the room once it ends.",
        run.run_id,
        run.key,
        run.state.as_str(),
        run.provider,
        run.model
      ))
    }
    other => Err(format!("this tool holds no entry research/{other}")),
  }
}

/// The folder holding `prompts/rules`: `here` or the nearest folder above it.
pub fn rules_folder(here: &Path) -> Option<PathBuf> {
  here.ancestors().map(|folder| folder.join(RULES)).find(|rules| rules.is_dir())
}

/// One rulebook the session record's `labels.research.rulebooks` names, whole, read from the rules folder
/// found from `here`; a name the record does not name is refused with the names it does.
pub fn rulebook(here: &Path, labels: &Value, arguments: &Value) -> Result<String, String> {
  let name = need_text(arguments, "name")?;
  let name = name.trim_end_matches(".md").rsplit('/').next().unwrap_or_default().to_string();
  let handed: Vec<String> = labels["research"]["rulebooks"]
    .as_array()
    .map(|names| names.iter().filter_map(|n| n.as_str().map(String::from)).collect())
    .unwrap_or_default();
  if !handed.contains(&name) {
    return Err(if handed.is_empty() {
      format!("this run was handed no rulebook, so {name} is not read")
    } else {
      format!("{name} is no rulebook this run was handed: {}", handed.join(", "))
    });
  }
  let folder = rules_folder(here).ok_or_else(|| format!("no {RULES} stands above {}", here.display()))?;
  let path = folder.join(format!("{name}.md"));
  let text = std::fs::read_to_string(&path).map_err(|e| format!("{}: {e}", path.display()))?;
  Ok(format!("RULEBOOK prompts/rules/{name}.md:\n{text}"))
}

fn item_line(item: &Item) -> String {
  let mut line = format!(
    "#{} [{}] {} {} — {}",
    item.id,
    item.category_name,
    item.state.as_str(),
    item.shape.as_str(),
    item.name
  );
  if let Some(link) = &item.share_url {
    line.push(' ');
    line.push_str(link);
  }
  line
}

async fn categories(client: &ResearchClient, arguments: &Value, is_owner: bool) -> Result<String, String> {
  match text(arguments, "action").as_deref().unwrap_or("list") {
    "list" => {
      let page = client.categories().await?;
      let lines: Vec<String> = page
        .items
        .iter()
        .map(|c| {
          format!("{} ({}, cap {}, {}): {}", c.name, c.shape.as_str(), c.cap, c.state.as_str(), c.purpose)
        })
        .collect();
      Ok(lines.join("\n"))
    }
    "add" => {
      owner(is_owner, "Adding a category")?;
      let word = need_text(arguments, "shape")?;
      let shape = CategoryShape::parse(&word).ok_or_else(|| format!("{word} is no category shape"))?;
      let category = client
        .add_category(&NewCategory {
          name: need_text(arguments, "name")?,
          purpose: need_text(arguments, "purpose")?,
          shape,
          cap: arguments["cap"].as_i64(),
          cadence_minutes: arguments["cadenceMinutes"].as_i64(),
          gather_minutes: arguments["gatherMinutes"].as_i64(),
          provider: text(arguments, "provider"),
          model: text(arguments, "model"),
        })
        .await?;
      Ok(format!("Category {} stands, as {}.", category.name, category.shape.as_str()))
    }
    "change" => {
      owner(is_owner, "Changing a category")?;
      let name = need_text(arguments, "name")?;
      let change = CategoryChange {
        purpose: text(arguments, "purpose"),
        cap: arguments["cap"].as_i64(),
        cadence_minutes: arguments["cadenceMinutes"].as_i64(),
        gather_minutes: arguments["gatherMinutes"].as_i64(),
        provider: arguments["provider"].as_str().map(String::from),
        model: arguments["model"].as_str().map(String::from),
      };
      let category = client.change_category(&name, &change).await?;
      Ok(format!(
        "Category {} changed: cap {}, every {} minutes.",
        category.name, category.cap, category.cadence_minutes
      ))
    }
    "remove" => {
      owner(is_owner, "Removing a category")?;
      let name = need_text(arguments, "name")?;
      client.remove_category(&name).await?;
      Ok(format!("Category {name} is taken away."))
    }
    other => Err(format!("{other} is no action of research/categories")),
  }
}

async fn sources(client: &ResearchClient, arguments: &Value, is_owner: bool) -> Result<String, String> {
  match text(arguments, "action").as_deref().unwrap_or("list") {
    "list" => {
      let page = client.sources(text(arguments, "category").as_deref(), None, 200).await?;
      if page.items.is_empty() {
        return Ok("No source is registered.".into());
      }
      let lines: Vec<String> = page
        .items
        .iter()
        .map(|s| {
          let failing = if s.failure_count > 0 {
            format!(" (failing {}x)", s.failure_count)
          } else {
            String::new()
          };
          format!(
            "#{} [{}] {} {} {}{failing}",
            s.id,
            s.category_name,
            s.kind,
            s.address,
            s.query.clone().unwrap_or_default()
          )
        })
        .collect();
      Ok(lines.join("\n"))
    }
    "add" => {
      owner(is_owner, "Adding a source")?;
      let require = words(arguments, "require");
      let exclude = words(arguments, "exclude");
      let filter = (!require.is_empty() || !exclude.is_empty()).then_some(Filter { require, exclude });
      let source = client
        .add_source(&NewSource {
          category_name: need_text(arguments, "category")?,
          kind: need_text(arguments, "kind")?,
          address: text(arguments, "address"),
          query: text(arguments, "query"),
          pattern: text(arguments, "pattern"),
          strategy: text(arguments, "strategy"),
          filter,
          every_minutes: arguments["everyMinutes"].as_i64(),
          limit: arguments["limit"].as_i64(),
        })
        .await?;
      Ok(format!("Source #{} reads {} for {}.", source.id, source.address, source.category_name))
    }
    "remove" => {
      owner(is_owner, "Removing a source")?;
      let id = need_number(arguments, "source")?;
      client.remove_source(id).await?;
      Ok(format!("Source #{id} is taken away."))
    }
    other => Err(format!("{other} is no action of research/sources")),
  }
}

/// The recipients of a category, or, for `rooms`, those that are rooms alone.
async fn recipients(
  client: &ResearchClient,
  arguments: &Value,
  is_owner: bool,
  rooms: bool,
) -> Result<String, String> {
  let category = need_text(arguments, "category")?;
  let entry = if rooms { "research/rooms" } else { "research/recipients" };
  let room = arguments["room"].as_i64();
  let agent = if rooms { None } else { text(arguments, "agent") };
  match text(arguments, "action").as_deref().unwrap_or("list") {
    "list" => {
      let page = client.recipients(&category).await?;
      let lines: Vec<String> = page
        .items
        .iter()
        .filter(|r| !rooms || r.kind == RecipientKind::Room)
        .map(|r| {
          let who = r
            .room_id
            .map(|id| format!("room {id}"))
            .or(r.agent_name.clone().map(|a| format!("agent {a}")));
          format!("#{} {} ({})", r.id, who.unwrap_or_default(), r.origin)
        })
        .collect();
      if lines.is_empty() {
        Ok(format!("{category} has no recipient."))
      } else {
        Ok(lines.join("\n"))
      }
    }
    "add" => {
      owner(is_owner, "Adding a recipient")?;
      let new = match (room, agent) {
        (Some(room_id), None) => NewRecipient {
          kind: RecipientKind::Room,
          room_id: Some(room_id),
          agent_name: None,
        },
        (None, Some(agent)) => NewRecipient {
          kind: RecipientKind::Agent,
          room_id: None,
          agent_name: Some(agent),
        },
        _ => return Err(format!("{entry} add names a room or an agent, one of them")),
      };
      let added = client.add_recipient(&category, &new).await?;
      Ok(format!("Recipient #{} hears {category}.", added.id))
    }
    "remove" => {
      owner(is_owner, "Removing a recipient")?;
      let page = client.recipients(&category).await?;
      let found = page.items.iter().find(|r| match (room, &agent) {
        (Some(room_id), _) => r.room_id == Some(room_id),
        (None, Some(agent)) => r.agent_name.as_deref() == Some(agent.as_str()),
        (None, None) => false,
      });
      let Some(found) = found else {
        return Err(format!("{category} holds no such recipient"));
      };
      client.remove_recipient(&category, found.id).await?;
      Ok(format!("Recipient #{} no longer hears {category}.", found.id))
    }
    other => Err(format!("{other} is no action of {entry}")),
  }
}
