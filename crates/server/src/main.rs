//! `bmo-research`: the research unit's daemon and its command (`docs/api.md`, 「The command」).

use std::process::ExitCode;

use bmo_research_client::*;
use clap::{Parser, Subcommand};
use research_server::App;
use research_server::config::Settings;
use research_server::store::Store;
use research_server::words::fill;

#[derive(Parser)]
#[command(
  name = "bmo-research",
  version = research_server::VERSION,
  about = "The research unit: the queue, the scan, the deep dive and the notices"
)]
struct Cli {
  #[command(subcommand)]
  step: Step,
}

#[derive(Subcommand)]
enum Step {
  /// Answers the noun this command carries: `research`.
  Nouns,
  /// Runs the daemon.
  Serve,
  /// Answers 0 once the daemon answers ready: the deployment's health command.
  Health,
  /// Prints the OpenAPI document.
  Openapi,
  /// Brings the store's schema to the version this binary holds.
  Migrate,
  /// Draws every filed page again where it differs from what this binary draws, and prints how many it set.
  Redraw,
  /// Prints the skills the runner composes a run of, one name a line: what a deployment places.
  Skills,
  /// The unit's own verbs, as `bmo research` hands them.
  Research {
    #[command(subcommand)]
    verb: Verb,
  },
}

#[derive(Subcommand)]
enum Verb {
  /// Whether the loop runs, and each category's part of the queue.
  State {
    #[arg(long)]
    json: bool,
  },
  /// Stops every category, or the one named.
  Stop { category: Option<String> },
  /// Resumes every category, or the one named.
  Resume { category: Option<String> },
  /// The categories.
  Categories {
    #[arg(long)]
    json: bool,
  },
  /// The queue in its order.
  Queue {
    #[arg(long)]
    category: Option<String>,
    #[arg(long)]
    state: Option<String>,
    #[arg(long, default_value_t = 50)]
    limit: i64,
    #[arg(long)]
    json: bool,
  },
  /// Asks for a thing by its name and its maker's address.
  Ask {
    name: String,
    address: String,
    #[arg(long)]
    category: Option<String>,
    #[arg(long)]
    shape: Option<String>,
    /// The words the ask is made with: why it is asked.
    #[arg(long)]
    because: Option<String>,
    /// For a follow-up: the item it follows from.
    #[arg(long)]
    follows: Option<i64>,
    /// For a follow-up: the document it follows from.
    #[arg(long)]
    document: Option<i64>,
  },
  /// Marks an item taken by the job item filed for it.
  Taken { item: i64, job: i64 },
  /// Dismisses an item.
  Dismiss { item: i64 },
  /// Moves a waiting item to the front of its category.
  Front { item: i64 },
  /// The registered sources.
  Sources {
    #[arg(long)]
    category: Option<String>,
    #[arg(long)]
    json: bool,
  },
  /// A category's recipients.
  Recipients {
    category: String,
    #[arg(long)]
    json: bool,
  },
  /// The tries, newest first.
  Runs {
    #[arg(long)]
    item: Option<i64>,
    #[arg(long, default_value_t = 20)]
    limit: i64,
    #[arg(long)]
    json: bool,
  },
  /// The stack inventory.
  Stack {
    #[arg(long)]
    json: bool,
  },
  /// The facts the briefs kept.
  Facts {
    #[arg(long)]
    name: Option<String>,
    #[arg(long, default_value_t = 50)]
    limit: i64,
    #[arg(long)]
    json: bool,
  },
}

#[tokio::main]
async fn main() -> ExitCode {
  let said = match Cli::parse().step {
    Step::Nouns => {
      println!("research\t");
      Ok(())
    }
    Step::Serve => research_server::daemon::run().await,
    Step::Health => health().await,
    Step::Openapi => {
      print!("{}", research_server::openapi::openapi_json());
      Ok(())
    }
    Step::Migrate => migrate(),
    Step::Redraw => redraw().await,
    Step::Skills => {
      research_server::words::skills().iter().for_each(|name| println!("{name}"));
      Ok(())
    }
    Step::Research { verb } => command(verb).await,
  };
  match said {
    Ok(()) => ExitCode::SUCCESS,
    Err(why) => {
      eprintln!("bmo-research: {why}");
      ExitCode::FAILURE
    }
  }
}

fn migrate() -> Result<(), String> {
  let settings = Settings::load()?;
  let store = Store::open(&settings.store_dir())?;
  println!("schema version {}", store.version()?);
  Ok(())
}

async fn redraw() -> Result<(), String> {
  let settings = Settings::load()?;
  let token = settings.read_token()?;
  let store = Store::open(&settings.store_dir())?;
  let app = App::new(settings, store, token);
  let set = research_server::dive::redraw_filed(&app).await;
  println!("{set} pages drawn again");
  Ok(())
}

fn client() -> Result<(Settings, ResearchClient), String> {
  let settings = Settings::load()?;
  let token = settings.read_token()?;
  let client = ResearchClient::new(format!("http://{}", settings.listen)).with_token(token);
  Ok((settings, client))
}

async fn health() -> Result<(), String> {
  let (settings, client) = client()?;
  let health = client.health().await?;
  if health.ready {
    println!("ready {} {}", health.version, settings.listen);
    Ok(())
  } else {
    Err(health.reason.unwrap_or_else(|| "not ready".into()))
  }
}

fn print_json<T: serde::Serialize>(value: &T) -> Result<(), String> {
  println!("{}", serde_json::to_string_pretty(value).map_err(|e| e.to_string())?);
  Ok(())
}

async fn command(verb: Verb) -> Result<(), String> {
  let (_, client) = client()?;
  match verb {
    Verb::State { json } => {
      let state = client.state().await?;
      if json {
        return print_json(&state);
      }
      println!("{}", state.state.as_str());
      for c in state.categories {
        println!(
          "  {:<12} {:<8} waiting {} running {} finishing {} stuck {}",
          c.category_name,
          c.state.as_str(),
          c.waiting_count,
          c.running_count,
          c.finishing_count,
          c.stuck_count
        );
      }
      for hold in state.holds {
        println!("  held: {}", hold.line());
      }
      Ok(())
    }
    Verb::Stop { category } => change_state(&client, Running::Stopped, category).await,
    Verb::Resume { category } => change_state(&client, Running::Running, category).await,
    Verb::Categories { json } => {
      let page = client.categories().await?;
      if json {
        return print_json(&page);
      }
      for c in page.items {
        println!("{:<12} {:<24} {} — {}", c.name, c.shape.as_str(), c.state.as_str(), c.purpose);
      }
      Ok(())
    }
    Verb::Queue {
      category,
      state,
      limit,
      json,
    } => {
      let states: Vec<ItemState> = state
        .unwrap_or_default()
        .split(',')
        .filter_map(|w| ItemState::parse(w.trim()))
        .collect();
      let page = client.queue(category.as_deref(), &states, None, limit).await?;
      if json {
        return print_json(&page);
      }
      for i in page.items {
        let mut links: Vec<String> = i.share_url.into_iter().collect();
        if !i.pending.is_empty() {
          links.push(format!("(pending: {})", i.pending.join(", ")));
        }

        println!(
          "#{} {:<10} {:<9} {:<13} {} {}",
          i.id,
          i.category_name,
          i.state.as_str(),
          i.shape.as_str(),
          i.name,
          links.join(" ")
        );
      }
      Ok(())
    }
    Verb::Ask {
      name,
      address,
      category,
      shape,
      because,
      follows,
      document,
    } => {
      let shape = match shape {
        Some(word) => Some(Shape::parse(&word).ok_or_else(|| format!("{word} is no shape"))?),
        None => None,
      };
      let item = client
        .ask(&Ask {
          name,
          address,
          category_name: category,
          shape,
          because,
          follows_item_id: follows,
          follows_document_id: document,
        })
        .await?;
      println!("#{} queued in {} as a {}", item.id, item.category_name, item.shape.as_str());
      Ok(())
    }
    Verb::Taken { item, job } => {
      let change = ItemChange {
        state: Some(ItemState::Taken),
        job_item_id: Some(job),
        front: None,
      };
      let item = client.change_item(item, &change).await?;
      println!("#{} taken by job item {job}", item.id);
      Ok(())
    }
    Verb::Dismiss { item } => {
      let change = ItemChange {
        state: Some(ItemState::Dismissed),
        ..ItemChange::default()
      };
      let item = client.change_item(item, &change).await?;
      println!("#{} dismissed", item.id);
      Ok(())
    }
    Verb::Front { item } => {
      let change = ItemChange {
        front: Some(true),
        ..ItemChange::default()
      };
      let item = client.change_item(item, &change).await?;
      println!("#{} at priority {}", item.id, item.priority);
      Ok(())
    }
    Verb::Sources { category, json } => {
      let page = client.sources(category.as_deref(), None, 200).await?;
      if json {
        return print_json(&page);
      }
      let columns = &research_server::words::report().sources;
      let now = research_server::store::now_ms();
      for s in page.items {
        let read = match s.read_at {
          Some(at) => fill(&columns.read, &[("minutes", &((now - at).max(0) / 60_000).to_string())]),
          None => columns.unread.clone(),
        };
        let taken = match (s.last_taken, s.last_yield) {
          (Some(taken), Some(candidates)) => fill(
            &columns.taken,
            &[("taken", &taken.to_string()), ("candidates", &candidates.to_string())],
          ),
          _ => columns.untaken.clone(),
        };
        println!(
          "#{} {:<10} {:<20} {} {} failures {} {read} {taken}",
          s.id,
          s.category_name,
          s.kind,
          s.address,
          s.query.unwrap_or_default(),
          s.failure_count
        );
      }
      Ok(())
    }
    Verb::Recipients { category, json } => {
      let page = client.recipients(&category).await?;
      if json {
        return print_json(&page);
      }
      for r in page.items {
        let who = r.room_id.map(|id| id.to_string()).or(r.agent_name).unwrap_or_default();
        println!("#{} {} {} ({})", r.id, r.kind.as_str(), who, r.origin);
      }
      Ok(())
    }
    Verb::Runs { item, limit, json } => {
      let page = client.runs(item, None, None, limit).await?;
      if json {
        return print_json(&page);
      }
      for r in page.items {
        println!(
          "#{} {:<10} item {} try {} {} run {} {}",
          r.id,
          r.category_name,
          r.item_id,
          r.attempt,
          r.state,
          r.run_id.map(|id| id.to_string()).unwrap_or_else(|| "-".into()),
          r.error.unwrap_or_default()
        );
      }
      Ok(())
    }
    Verb::Stack { json } => {
      let page = client.stack().await?;
      if json {
        return print_json(&page);
      }
      for r in page.items {
        println!(
          "#{} {} {} {} {} {}",
          r.id,
          r.component,
          r.version.unwrap_or_default(),
          r.place.unwrap_or_default(),
          r.repository.unwrap_or_default(),
          r.worker.unwrap_or_default()
        );
      }
      Ok(())
    }
    Verb::Facts { name, limit, json } => {
      let page = client.facts(name.as_deref(), None, limit).await?;
      if json {
        return print_json(&page);
      }
      for f in page.items {
        println!(
          "{} {} {} {} {}",
          f.as_of.unwrap_or_default(),
          f.name,
          f.value,
          f.unit.unwrap_or_default(),
          f.source_address.unwrap_or_default()
        );
      }
      Ok(())
    }
  }
}

async fn change_state(
  client: &ResearchClient,
  state: Running,
  category: Option<String>,
) -> Result<(), String> {
  let answered = client
    .set_state(&StateChange {
      state,
      category_name: category,
    })
    .await?;
  println!("{}", answered.state.as_str());
  Ok(())
}
