//! The research namespace's tool (`docs/api.md`, 「The tools」): one executable answering every entry of the
//! namespace by calling the daemon's routes, at the address and with the token the daemon's settings name
//! under the data root the session record's `BMO_DATA_DIR` names; the two skill readers alone read no route
//! but the skills placed beside the executable and the asker's own rules under the data root. It speaks the
//! tool host's call protocol 1: one request on standard input, one tool result on standard output.

use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};

use bmo_research_client::*;
use pulldown_cmark::{Event, Options, Parser, Tag, TagEnd};
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
  "skill",
];

/// The folder under which a skill is read: `prompts/skills` under the tool's working folder or the nearest
/// folder above it holding one.
pub const SKILLS: &str = "prompts/skills";

/// The file under the data root holding each person's own rules (`docs/settings.md`, 「A person's own rules」).
pub const PEOPLE: &str = "research/people.toml";

/// The asker a run or a call names nobody as.
pub const NOBODY: &str = "the owner";

/// The line a person's own rule is answered under, after the skill it stands in.
pub const OWN_RULE: &str = "THE ASKER'S OWN RULE:";

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
  if name == "rulebook" || name == "skill" {
    let here = std::env::current_dir().unwrap_or_default();
    let root = data_root(session);
    let read = if name == "rulebook" {
      rulebook(&here, &root, &session["labels"], &arguments)
    } else {
      skill(&here, &root, &arguments)
    };
    return match read {
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

/// The calendar a topic's arguments name: `calendar` with its `realm` and `calendarId`, the realm the owner's
/// own where none is named.
fn calendar_of(arguments: &Value) -> Option<CalendarTarget> {
  let named = arguments.get("calendar")?.as_object()?;
  Some(CalendarTarget {
    realm: named.get("realm").and_then(Value::as_str).unwrap_or("mine").to_owned(),
    calendar_id: named.get("calendarId").and_then(Value::as_str)?.to_owned(),
  })
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
        options: text(arguments, "options").map(serde_json::Value::String),
        room_id: arguments["roomId"].as_i64(),
        provider: text(arguments, "provider"),
        model: text(arguments, "model"),
      };
      let run = client.ask_research(&ask).await?;
      Ok(format!(
        "Research run {} on {} is {} on {} · {}. Its result comes back to the room once it ends.",
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

/// The folder holding `prompts/skills`: `here` or the nearest folder above it.
pub fn skills_folder(here: &Path) -> Option<PathBuf> {
  here.ancestors().map(|folder| folder.join(SKILLS)).find(|skills| skills.is_dir())
}

/// The data root the session record's `BMO_DATA_DIR` names, else the platform's own under the home folder.
pub fn data_root(session: &Value) -> PathBuf {
  match session["variables"]["BMO_DATA_DIR"].as_str().filter(|r| !r.is_empty()) {
    Some(root) => PathBuf::from(root),
    None => PathBuf::from(std::env::var_os("HOME").unwrap_or_default()).join(".local/share/bmo/data"),
  }
}

/// One rulebook the session record's `labels.research.rulebooks` names, whole, read from the skills folder
/// found from `here`, with the asker's own rule for it where they keep one; a name the record does not name
/// is refused with the names it does. The asker is the record's `owner`, the owner where it names none.
pub fn rulebook(
  here: &Path,
  data_root: &Path,
  labels: &Value,
  arguments: &Value,
) -> Result<String, String> {
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
  let asker = labels["research"]["owner"].as_str().filter(|a| !a.trim().is_empty()).unwrap_or(NOBODY);
  let folder = skills_folder(here).ok_or_else(|| format!("no {SKILLS} stands above {}", here.display()))?;
  with_own_rule(&folder, data_root, &name, asker)
}

/// One skill placed in the skills folder found from `here`, whole, with the asker's own rule for it where
/// they keep one; a name no skill is placed under is refused with the names that are. The asker is the
/// argument `asker`, the owner where it names none.
pub fn skill(here: &Path, data_root: &Path, arguments: &Value) -> Result<String, String> {
  let name = need_text(arguments, "name")?;
  let asker = arguments["asker"].as_str().filter(|a| !a.trim().is_empty()).unwrap_or(NOBODY);
  let folder = skills_folder(here).ok_or_else(|| format!("no {SKILLS} stands above {}", here.display()))?;
  let placed = placed_skills(&folder);
  if !placed.contains(&name) {
    return Err(format!("{name} is no research skill: {}", placed.join(", ")));
  }
  with_own_rule(&folder, data_root, &name, asker)
}

/// The names of the skills placed in `folder`, in order.
pub fn placed_skills(folder: &Path) -> Vec<String> {
  let mut names: Vec<String> = std::fs::read_dir(folder)
    .map(|entries| {
      entries
        .filter_map(Result::ok)
        .filter(|entry| entry.path().join("SKILL.md").is_file())
        .filter_map(|entry| entry.file_name().into_string().ok())
        .collect()
    })
    .unwrap_or_default();
  names.sort();
  names
}

/// The skill `name` under `folder`, its front matter left out, then the asker's own rule for it under
/// [`OWN_RULE`] where they keep one; a file of the people's rules that does not read is said in its place.
fn with_own_rule(folder: &Path, data_root: &Path, name: &str, asker: &str) -> Result<String, String> {
  let path = folder.join(name).join("SKILL.md");
  let text = std::fs::read_to_string(&path).map_err(|e| format!("{}: {e}", path.display()))?;
  let whole = format!("SKILL {name}:\n{}", skill_body(&text));
  Ok(match own_rule(data_root, asker, name) {
    Ok(Some(rule)) => format!("{}\n\n{OWN_RULE}\n{}\n", whole.trim_end(), rule.trim()),
    Ok(None) => whole,
    Err(why) => format!(
      "{}\n\nThe asker's own rule could not be read ({why}): apply the default rule and say so.\n",
      whole.trim_end()
    ),
  })
}

/// A `SKILL.md`'s body: what follows its front matter block, as the markdown reader finds the block, with
/// the blank lines after it left out; a text with no front matter is its own body.
pub fn skill_body(text: &str) -> &str {
  let mut parser = Parser::new_ext(text, Options::ENABLE_YAML_STYLE_METADATA_BLOCKS)
    .into_offset_iter()
    .peekable();
  if !matches!(parser.peek(), Some((Event::Start(Tag::MetadataBlock(_)), _))) {
    return text;
  }
  let end = parser
    .find_map(|(event, range)| matches!(event, Event::End(TagEnd::MetadataBlock(_))).then_some(range.end))
    .unwrap_or(0);
  text[end..].trim_start_matches(['\n', '\r'])
}

/// Each person who keeps rules of their own: the names an asker is named by, and their rule for each
/// skill's kind of research, by the skill's name.
#[derive(Debug, Default, Deserialize)]
#[serde(deny_unknown_fields)]
struct People {
  #[serde(default)]
  person: Vec<Person>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct Person {
  names: Vec<String>,
  #[serde(default)]
  rules: BTreeMap<String, String>,
}

/// The asker's own rule for the skill `name`, read from [`PEOPLE`] under the data root: none where the file
/// does not stand, names no person by the asker's name, or that person keeps no rule for the skill.
pub fn own_rule(data_root: &Path, asker: &str, name: &str) -> Result<Option<String>, String> {
  let path = data_root.join(PEOPLE);
  let people: People = match std::fs::read_to_string(&path) {
    Ok(text) => toml::from_str(&text).map_err(|e| format!("{}: {e}", path.display()))?,
    Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(None),
    Err(e) => return Err(format!("{}: {e}", path.display())),
  };
  let asker = asker.trim().to_lowercase();
  Ok(
    people
      .person
      .into_iter()
      .find(|person| person.names.iter().any(|named| named.trim().to_lowercase() == asker))
      .and_then(|mut person| person.rules.remove(name))
      .filter(|rule| !rule.trim().is_empty()),
  )
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
        .map(|c| format!("{} ({}, {}): {}", c.name, c.shape.as_str(), c.state.as_str(), c.purpose))
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
          cadence_minutes: arguments["cadenceMinutes"].as_i64(),
          gather_minutes: arguments["gatherMinutes"].as_i64(),
          provider: text(arguments, "provider"),
          model: text(arguments, "model"),
          instruction: text(arguments, "instruction"),
          calendar: calendar_of(arguments),
        })
        .await?;
      Ok(format!("Category {} stands, as {}.", category.name, category.shape.as_str()))
    }
    "change" => {
      owner(is_owner, "Changing a category")?;
      let name = need_text(arguments, "name")?;
      let change = CategoryChange {
        purpose: text(arguments, "purpose"),
        cadence_minutes: arguments["cadenceMinutes"].as_i64(),
        gather_minutes: arguments["gatherMinutes"].as_i64(),
        provider: arguments["provider"].as_str().map(String::from),
        model: arguments["model"].as_str().map(String::from),
        instruction: text(arguments, "instruction"),
        // A calendar named is set, `calendar: null` takes it away, and one left out is kept.
        calendar: match &arguments["calendar"] {
          Value::Null if arguments.get("calendar").is_some() => Some(None),
          _ => calendar_of(arguments).map(Some),
        },
      };
      let category = client.change_category(&name, &change).await?;
      Ok(format!(
        "Category {} changed: every {} minutes.",
        category.name, category.cadence_minutes
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
