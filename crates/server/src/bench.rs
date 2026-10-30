//! The bench watch (`docs/api.md`, 「The bench」, and `docs/design.md`, 「The n3–n6 restore path」):
//! whether a bench's ticket stands on a node a watched model stands on, read from the arbiter master's
//! cluster picture; the wait on the model's health once the bench is gone, told to the owner's room once
//! where it lasts past `restore_wait_minutes`; the holds the state answers; and whether a failed try was
//! the bench's.

use std::collections::BTreeMap;
use std::sync::Mutex;

use bmo_research_client::{Hold, HoldReason};

use crate::App;
use crate::config::Watched;
use crate::store::now_ms;
use crate::words::{self, fill};

/// How one watched model stands.
#[derive(Debug, Clone, Default)]
struct Standing {
  /// The node a bench's ticket stands on, and when it was first seen there.
  bench: Option<(String, i64)>,
  /// When a bench was last seen on one of the model's nodes.
  bench_seen_at: Option<i64>,
  /// When the wait on the model's health began, while it lasts.
  restore_since: Option<i64>,
  /// Whether the owner's room was told the model stays down, in this wait.
  reported: bool,
}

impl Standing {
  fn holds(&self) -> bool {
    self.bench.is_some() || self.restore_since.is_some()
  }
}

/// What the watch keeps between its readings: each watched model's standing by its registration, and the
/// last failure to read the master, said once while it stays the same.
#[derive(Default)]
pub struct Watch {
  standing: Mutex<BTreeMap<String, Standing>>,
  failure: Mutex<Option<String>>,
}

impl Watch {
  fn standing(&self, registration: &str) -> Standing {
    self.standing.lock().unwrap().get(registration).cloned().unwrap_or_default()
  }
}

/// One reading: the master's tickets looked at for each watched model's nodes, and the health of every
/// model whose bench is gone asked. A master that cannot be read leaves each bench as it stood.
pub async fn look(app: &App) {
  let watch = &app.settings.bench_watch;
  if watch.models.is_empty() {
    return;
  }
  let tickets = match app.peers.tickets().await {
    Ok(tickets) => {
      *app.watch.failure.lock().unwrap() = None;
      Some(tickets)
    }
    Err(why) => {
      let mut failure = app.watch.failure.lock().unwrap();
      if failure.as_deref() != Some(why.as_str()) {
        eprintln!("bmo-research: the bench watch could not read the arbiter: {why}");
        *failure = Some(why);
      }
      None
    }
  };
  let now = now_ms();
  for model in &watch.models {
    let bench = tickets.as_ref().map(|tickets| bench_node(&watch.occupant, model, tickets));
    let waiting = {
      let mut all = app.watch.standing.lock().unwrap();
      let standing = all.entry(model.registration.clone()).or_default();
      match bench {
        Some(Some(node)) => {
          let since = standing.bench.as_ref().map_or(now, |(_, since)| *since);
          standing.bench = Some((node, since));
          standing.bench_seen_at = Some(now);
          standing.restore_since = None;
          standing.reported = false;
        }
        Some(None) if standing.bench.is_some() => {
          standing.bench = None;
          standing.restore_since = Some(now);
        }
        Some(None) | None => {}
      }
      standing.bench.is_none() && standing.restore_since.is_some()
    };
    if waiting {
      restore(app, model).await;
    }
  }
}

/// The first of the model's nodes a ticket of the bench's occupant stands on, or none.
fn bench_node(occupant: &str, model: &Watched, tickets: &BTreeMap<String, Vec<String>>) -> Option<String> {
  model
    .nodes
    .iter()
    .find(|node| tickets.get(*node).is_some_and(|services| services.iter().any(|s| s == occupant)))
    .cloned()
}

/// The model's health asked: healthy, its wait ends; not, a wait begins where none stands, and the owner's
/// room is told once when it has lasted `restore_wait_minutes`.
async fn restore(app: &App, model: &Watched) {
  let healthy = app.peers.healthy(&app.settings.bench_watch.provider, &model.registration).await;
  let now = now_ms();
  let waited = {
    let mut all = app.watch.standing.lock().unwrap();
    let standing = all.entry(model.registration.clone()).or_default();
    if standing.bench.is_some() {
      return;
    }
    if healthy == Ok(true) {
      standing.restore_since = None;
      standing.reported = false;
      return;
    }
    let since = *standing.restore_since.get_or_insert(now);
    let due = now - since >= app.settings.restore_wait_minutes.max(0) * 60_000;
    if !due || standing.reported {
      return;
    }
    standing.reported = true;
    (now - since) / 60_000
  };
  tell(app, model, waited).await;
}

/// The owner's private room told that the model has not come back.
async fn tell(app: &App, model: &Watched, minutes: i64) {
  let notice = fill(
    &words::report().restore,
    &[
      ("model", &model.registration),
      ("nodes", &model.nodes.join(", ")),
      ("minutes", &minutes.to_string()),
    ],
  );
  let kind = format!("research-restore:{}", model.registration);
  let summary = format!("research restore: {}", model.registration);
  let said = match app.peers.owner_room().await {
    Ok(room) => app.peers.instruct(room, &kind, &notice, &summary).await.map(|_| ()),
    Err(why) => Err(why),
  };
  if let Err(why) = said {
    eprintln!(
      "bmo-research: the owner's room could not be told {} stays down: {why}",
      model.registration
    );
  }
}

/// Whether a road is held now: a bench stands on its model's nodes, or the queue waits on its health.
pub fn held(app: &App, provider: &str, model: &str) -> bool {
  app
    .settings
    .bench_watch
    .model(provider, model)
    .is_some_and(|watched| app.watch.standing(&watched.registration).holds())
}

/// What holds the queue now, each with the categories whose road it is.
pub fn holds(app: &App) -> Vec<Hold> {
  let categories = app.store.categories().unwrap_or_default();
  let mut holds = Vec::new();
  for model in &app.settings.bench_watch.models {
    let standing = app.watch.standing(&model.registration);
    let (reason, node_name, since) = match (&standing.bench, standing.restore_since) {
      (Some((node, since)), _) => (HoldReason::Bench, Some(node.clone()), *since),
      (None, Some(since)) => (HoldReason::Restore, None, since),
      (None, None) => continue,
    };
    let category_names = categories
      .iter()
      .filter(|c| {
        let (provider, road) = crate::dive::road(app, c);
        app
          .settings
          .bench_watch
          .model(&provider, &road)
          .is_some_and(|w| w.registration == model.registration)
      })
      .map(|c| c.name.clone())
      .collect();
    holds.push(Hold {
      registration: model.registration.clone(),
      reason,
      node_name,
      since,
      reported: standing.reported,
      category_names,
    });
  }
  holds
}

/// Whether a try of the road, asked at `started_at`, that has just failed was the bench's: read afresh, a
/// bench stands on the model's nodes, one was seen there since the try was asked, or the model does not
/// answer its health, which holds the queue as a wait after a bench does. A road not watched never is.
pub async fn evicted(app: &App, provider: &str, model: &str, started_at: i64) -> bool {
  let Some(watched) = app.settings.bench_watch.model(provider, model) else {
    return false;
  };
  look(app).await;
  let standing = app.watch.standing(&watched.registration);
  if standing.holds() || standing.bench_seen_at.is_some_and(|at| at >= started_at) {
    return true;
  }
  restore(app, watched).await;
  app.watch.standing(&watched.registration).holds()
}
