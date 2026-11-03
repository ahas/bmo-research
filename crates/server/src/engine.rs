//! The engine a run needs (`docs/api.md`, 「The engine」): a place on a watched road asks the arbiter's
//! master for one card of its engine's service and waits for the grant with no clock of its own, and the try
//! it takes holds the card for its run and gives it back as the try ends; a branch of such a run asks its own
//! within the run's; what the daemon left behind given back; the holds the state answers; the owner's room
//! told once where a card does not come; and why a failed try was the arbiter's, where it was.

use std::collections::BTreeMap;
use std::sync::Mutex;
use std::time::Duration;

use bmo_research_client::{Hold, HoldReason};
use serde_json::{Value, json};

use crate::App;
use crate::config::Watched;
use crate::peers::Demand;
use crate::pull::{self, Road};
use crate::store::now_ms;
use crate::words::{self, fill};

/// How long one reading of a demand that waits holds its answer open at the master.
const HOLD_OPEN_MS: u64 = 25_000;

/// How long the daemon waits before it reads again a demand that answered still waiting, as the master's
/// `Retry-After` says.
const LOOK_AGAIN: Duration = Duration::from_secs(1);

/// A place waiting for its card.
#[derive(Debug, Clone)]
struct Waiting {
  road: Road,
  registration: String,
  service: String,
  demand: Option<i64>,
  cause: Option<String>,
  cause_text: Option<String>,
  since: i64,
  reported: bool,
}

/// What the engine watch keeps between its readings: each place waiting for its card, by its request key,
/// and the last failure to reach the master, said once while it stays the same.
#[derive(Default)]
pub struct Waits {
  waiting: Mutex<BTreeMap<String, Waiting>>,
  failure: Mutex<Option<String>>,
}

impl Waits {
  fn change(&self, place: &str, change: impl FnOnce(&mut Waiting)) {
    if let Some(waiting) = self.waiting.lock().unwrap().get_mut(place) {
      change(waiting);
    }
  }

  /// A failure to reach the master, said in the log once while it stays the same.
  fn failed(&self, why: String) {
    let mut failure = self.failure.lock().unwrap();
    if failure.as_deref() != Some(why.as_str()) {
      eprintln!("bmo-research: the arbiter's master: {why}");
      *failure = Some(why);
    }
  }

  fn reached(&self) {
    *self.failure.lock().unwrap() = None;
  }
}

/// The watched model a road goes out on, or none where it is not watched or the watch names no master.
pub fn watched(app: &App, provider: &str, model: &str) -> Option<Watched> {
  app.settings.engine.model(provider, model).cloned()
}

/// A place's card, asked of the master and waited on until it is granted, however long that takes: the
/// demand's id, kept in the store, or none where the place's road had no try due any longer first, its
/// demand then given back.
pub async fn card(app: &App, place: &str, road: &Road, model: &Watched) -> Option<i64> {
  let since = now_ms();
  app.waits.waiting.lock().unwrap().insert(
    place.to_owned(),
    Waiting {
      road: road.clone(),
      registration: model.registration.clone(),
      service: model.service.clone(),
      demand: None,
      cause: None,
      cause_text: None,
      since,
      reported: false,
    },
  );
  let ask = json!({
    "serviceName": model.service,
    "count": 1,
    "workKey": format!("research:place:{place}"),
    "ref": format!("research place {place}"),
  });
  let mut held: Option<i64> = None;
  let granted = loop {
    if !pull::wanted(app, road) {
      if let Some(id) = held {
        give_back(app, id).await;
      }
      break None;
    }
    let read = match held {
      None => app.peers.ask_demand(&ask).await.map(Some),
      Some(id) => app.peers.read_demand(id, HOLD_OPEN_MS).await,
    };
    let demand = match read {
      Ok(Some(demand)) => demand,
      Ok(None) => {
        if let Some(id) = held.take() {
          let _ = app.store.drop_demand(id);
        }
        tokio::time::sleep(LOOK_AGAIN).await;
        continue;
      }
      Err(why) => {
        app.waits.failed(why);
        tokio::time::sleep(Duration::from_secs(app.settings.tick_seconds.max(1))).await;
        continue;
      }
    };
    app.waits.reached();
    if held != Some(demand.id) {
      let _ = app.store.keep_demand(demand.id, None, None, None, &model.service);
      held = Some(demand.id);
    }
    if demand.granted() {
      break Some(demand.id);
    }
    waits(app, place, &demand).await;
    tokio::time::sleep(LOOK_AGAIN).await;
  };
  app.waits.waiting.lock().unwrap().remove(place);
  granted
}

/// A waiting place's hold set from its demand's answer, and the owner's room told once where the card has
/// not come past `restore_wait_minutes`.
async fn waits(app: &App, place: &str, demand: &Demand) {
  let reason = demand.reason.clone().unwrap_or_default();
  let due = app.settings.restore_wait_minutes.max(0) * 60_000;
  let mut told = None;
  app.waits.change(place, |waiting| {
    waiting.demand = Some(demand.id);
    waiting.cause = reason.code.clone();
    waiting.cause_text = reason.text.clone();
    let waited = now_ms() - waiting.since;
    if !waiting.reported && waited >= due {
      waiting.reported = true;
      told = Some((waiting.clone(), waited / 60_000));
    }
  });
  if let Some((waiting, minutes)) = told {
    tell(app, &waiting, minutes).await;
  }
}

/// The owner's private room told that a run's engine has not come.
async fn tell(app: &App, waiting: &Waiting, minutes: i64) {
  let notice = fill(
    &words::report().restore,
    &[
      ("model", &waiting.registration),
      ("service", &waiting.service),
      ("minutes", &minutes.to_string()),
    ],
  );
  let kind = format!("research-restore:{}", waiting.registration);
  let summary = format!("research restore: {}", waiting.registration);
  let said = match app.peers.owner_room().await {
    Ok(room) => app.peers.instruct(room, &kind, &notice, &summary).await.map(|_| ()),
    Err(why) => Err(why),
  };
  if let Err(why) = said {
    eprintln!(
      "bmo-research: the owner's room could not be told {} has not come: {why}",
      waiting.registration
    );
  }
}

/// A demand given back to the master and taken out of the store; one the master could not be asked to take
/// back stays kept, given back on a later pass.
pub async fn give_back(app: &App, id: i64) {
  match app.peers.give_back_demand(id).await {
    Ok(()) => {
      let _ = app.store.drop_demand(id);
    }
    Err(why) => eprintln!("bmo-research: demand {id} was not given back: {why}"),
  }
}

/// The demand a try holds given back, where it holds one.
pub async fn give_back_try(app: &App, try_id: i64) {
  if let Ok(Some(id)) = app.store.try_demand(try_id) {
    give_back(app, id).await;
  }
}

/// Every demand the store holds given back, at the daemon's start, when no run it asked is still under way.
/// Answers how many were.
pub async fn give_back_all(app: &App) -> usize {
  let held = app.store.demands().unwrap_or_default();
  for id in &held {
    give_back(app, *id).await;
  }
  held.len()
}

/// Every demand left behind given back: a try's whose try has ended, a branch's whose run has closed. A
/// place's that has taken no try is its place's to give back.
pub async fn give_back_left(app: &App) {
  for id in app.store.demands_left().unwrap_or_default() {
    give_back(app, id).await;
  }
}

/// Why a try that has just failed was the arbiter's, where it was: its demand, read afresh, names who took
/// its card or no longer stands granted, the master no longer holds it, or the master cannot be reached. A
/// try holding no demand never is.
pub async fn evicted(app: &App, try_id: i64) -> Option<String> {
  let Ok(Some(id)) = app.store.try_demand(try_id) else {
    return None;
  };
  match app.peers.read_demand(id, 0).await {
    Ok(Some(demand)) if demand.taken() => Some("the engine's card was taken".into()),
    Ok(Some(_)) => None,
    Ok(None) => Some(format!("the arbiter no longer holds the run's demand {id}")),
    Err(why) => Some(format!("the arbiter could not be asked for the run's demand: {why}")),
  }
}

/// Why a branch got no card.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Uncarded {
  /// The run stopped running before the card was granted.
  Ended,
  /// The master could not be asked.
  Failed(String),
}

/// A branch's card: none where its run holds no demand or the branch's road is not watched; else its own
/// demand, asked within the run's and kept in the store, waited on until it is granted, however long that
/// takes, while the run is running.
pub async fn branch_card(
  app: &App,
  run_id: i64,
  provider: &str,
  model: &str,
  index: i64,
) -> Result<Option<i64>, Uncarded> {
  let Some(model) = watched(app, provider, model) else {
    return Ok(None);
  };
  let Ok(Some(within)) = app.store.run_demand(run_id) else {
    return Ok(None);
  };
  let ask: Value = json!({
    "serviceName": model.service,
    "count": 1,
    "within": within,
    "workKey": format!("research:{run_id}:branch:{index}"),
    "ref": format!("research {run_id} branch {index}"),
  });
  let mut held: Option<i64> = None;
  loop {
    if !crate::runner::runs(app, run_id) {
      if let Some(id) = held {
        give_back(app, id).await;
      }
      return Err(Uncarded::Ended);
    }
    let read = match held {
      None => app.peers.ask_demand(&ask).await.map(Some),
      Some(id) => app.peers.read_demand(id, HOLD_OPEN_MS).await,
    };
    let demand = match read {
      Ok(Some(demand)) => demand,
      Ok(None) => {
        if let Some(id) = held.take() {
          let _ = app.store.drop_demand(id);
        }
        tokio::time::sleep(LOOK_AGAIN).await;
        continue;
      }
      Err(why) => {
        if let Some(id) = held {
          give_back(app, id).await;
        }
        return Err(Uncarded::Failed(why));
      }
    };
    if held != Some(demand.id) {
      let _ = app.store.keep_demand(demand.id, None, Some(run_id), Some(within), &model.service);
      held = Some(demand.id);
    }
    if demand.granted() {
      return Ok(Some(demand.id));
    }
    tokio::time::sleep(LOOK_AGAIN).await;
  }
}

/// The places waiting for their card now, in the order they were asked, each with the running categories on
/// its road.
pub fn holds(app: &App) -> Vec<Hold> {
  let waiting: Vec<(String, Waiting)> = app
    .waits
    .waiting
    .lock()
    .unwrap()
    .iter()
    .map(|(place, waiting)| (place.clone(), waiting.clone()))
    .collect();
  let mut holds: Vec<Hold> = waiting
    .into_iter()
    .map(|(place, waiting)| Hold {
      reason: HoldReason::WaitingCard,
      category_names: pull::categories_on(app, &waiting.road),
      place,
      registration: waiting.registration.clone(),
      service_name: waiting.service.clone(),
      demand_id: waiting.demand,
      cause: waiting.cause.clone(),
      cause_text: waiting.cause_text.clone(),
      since: waiting.since,
      reported: waiting.reported,
    })
    .collect();
  holds.sort_by(|a, b| (a.since, &a.place).cmp(&(b.since, &b.place)));
  holds
}
