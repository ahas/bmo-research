//! A node pulls (`docs/design.md`, 「A node pulls」; `docs/api.md`, 「The deep dive」): a place of the search
//! line's role asked for each try due on a road, never more across every road than the role's cap less what
//! the daemon holds or waits for; a place on a watched road asking its card first, then its slot, and waiting
//! in agency's line however long it takes; a place let in taking the next try on its road and carrying it on
//! its slot and card; a place whose road has nothing due given back; and the pull woken by what changes it,
//! never by a clock.

use std::collections::BTreeMap;
use std::sync::{Arc, Mutex};

use bmo_research_client::{CategoryShape, Running};
use serde_json::json;
use tokio::sync::Notify;
use tokio::task::JoinHandle;

use crate::App;
use crate::dive;
use crate::engine;
use crate::runner::{self, Handed, Slot};
use crate::store::now_ms;

/// The owner every place is asked under, whose release gives back every place at the daemon's start.
pub const OWNER: &str = "research:places";

/// A road: the provider and the model a category's tries go out on.
pub type Road = (String, String);

/// The places asked and holding no try yet, by road; what wakes the pull; and the last failure to ask for a
/// place, said in the log once while it stays the same.
#[derive(Default)]
pub struct Places {
  waiting: Mutex<BTreeMap<Road, i64>>,
  wake: Notify,
  failure: Mutex<Option<String>>,
}

impl Places {
  /// The pull woken: a place let in, an item arriving in the queue or a try's run ending.
  pub fn wake(&self) {
    self.wake.notify_one();
  }

  fn failed(&self, why: String) {
    let mut failure = self.failure.lock().unwrap_or_else(|e| e.into_inner());
    if failure.as_deref() != Some(why.as_str()) {
      eprintln!("bmo-research: a place: {why}");
      *failure = Some(why);
    }
  }

  fn reached(&self) {
    *self.failure.lock().unwrap_or_else(|e| e.into_inner()) = None;
  }

  /// How many places wait on each road now.
  pub fn waiting(&self) -> BTreeMap<Road, i64> {
    self.waiting.lock().unwrap_or_else(|e| e.into_inner()).clone()
  }
}

/// The running categories on a road, by name.
pub fn categories_on(app: &App, road: &Road) -> Vec<String> {
  app
    .store
    .categories()
    .unwrap_or_default()
    .into_iter()
    .filter(|category| category.state == Running::Running && dive::road(app, category) == *road)
    .map(|category| category.name)
    .collect()
}

/// The tries due on each road the running categories go out on, in the order the categories stand: each
/// waiting or stuck item whose retry is due, and a brief's whole gathering once it is due.
fn due(app: &App) -> Vec<(Road, i64)> {
  let now = now_ms();
  let mut roads: Vec<(Road, i64)> = Vec::new();
  for category in app.store.categories().unwrap_or_default() {
    if category.state == Running::Stopped {
      continue;
    }
    let items = app.store.due_count(&category.name, now).unwrap_or(0);
    let tries = match category.shape {
      CategoryShape::Brief if items > 0 && dive::gathering_due(app, &category, now) == Some(true) => 1,
      CategoryShape::Brief => 0,
      _ => items,
    };
    let road = dive::road(app, &category);
    match roads.iter_mut().find(|(standing, _)| *standing == road) {
      Some((_, count)) => *count += tries,
      None => roads.push((road, tries)),
    }
  }
  roads
}

/// Whether a road still has a try due, so a place waiting on it is still wanted.
pub fn wanted(app: &App, road: &Road) -> bool {
  due(app).iter().any(|(standing, tries)| standing == road && *tries > 0)
}

/// One pull: the search line's role's cap read from agency, then a place asked for each try due on each road
/// that no place already waiting covers, while the places held and waited for stay under the cap. Answers the
/// places started, each carrying the try it takes to its end.
pub async fn pull(app: &Arc<App>) -> Vec<JoinHandle<()>> {
  let line = runner::line_of(&app.settings.road.key);
  let role = app.settings.runner.role(line).to_owned();
  let cap = match app.peers.role(&role).await {
    Ok(held) => {
      app.places.reached();
      held["maxConcurrent"].as_i64().unwrap_or(0)
    }
    Err(fault) => {
      app.places.failed(format!("agency's role {role} was not read: {fault}"));
      return Vec::new();
    }
  };
  let mut started = Vec::new();
  let mut waiting = app.places.waiting.lock().unwrap_or_else(|e| e.into_inner());
  let running = app.store.running_count().unwrap_or(i64::MAX);
  let mut held = running.saturating_add(waiting.values().sum::<i64>());
  for (road, tries) in due(app) {
    let asked = waiting.entry(road.clone()).or_default();
    while held < cap && *asked < tries {
      *asked += 1;
      held += 1;
      let (app, road) = (app.clone(), road.clone());
      started.push(tokio::spawn(async move { place(app, road).await }));
    }
  }
  started
}

/// The pull, made each time it is woken, however long that takes.
pub async fn wakes(app: Arc<App>) {
  loop {
    app.places.wake.notified().await;
    drop(pull(&app).await);
  }
}

/// One place to its end: its card and its slot had, the next try on its road taken and carried on them, or
/// both given back where its road has nothing left to take.
async fn place(app: Arc<App>, road: Road) {
  let handed = acquired(&app, &road).await;
  let taken = {
    let mut waiting = app.places.waiting.lock().unwrap_or_else(|e| e.into_inner());
    let taken = handed.as_ref().and_then(|_| {
      let (category, items) = dive::next(&app, &road)?;
      let ids: Vec<i64> = items.iter().map(|item| item.id).collect();
      app.store.start_try(&category.name, &ids).ok().flatten().map(|(run, _)| run)
    });
    if let Some(count) = waiting.get_mut(&road) {
      *count -= 1;
      if *count <= 0 {
        waiting.remove(&road);
      }
    }
    taken
  };
  let Some(handed) = handed else {
    return;
  };
  match taken {
    Some(run) => {
      if let Some(demand) = handed.demand {
        let _ = app.store.demand_tried(demand, run);
      }
      dive::carry(app, run, None, Some(handed)).await;
    }
    None => {
      runner::give_back_unrun(&app, &handed.slot.token, "the place was let in with no try left to take")
        .await;
      if let Some(demand) = handed.demand {
        engine::give_back(&app, demand).await;
      }
    }
  }
}

/// A place's card, where its road is watched, and then its slot, let in; none where its road had no try due
/// any longer, or agency gave no slot, what it holds then given back.
async fn acquired(app: &App, road: &Road) -> Option<Handed> {
  let mut drawn = [0u8; 8];
  if let Err(why) = getrandom::fill(&mut drawn) {
    app.places.failed(format!("no key could be drawn: {why}"));
    return None;
  }
  let key: String = drawn.iter().map(|byte| format!("{byte:02x}")).collect();
  let demand = match engine::watched(app, &road.0, &road.1) {
    Some(model) => Some(engine::card(app, &key, road, &model).await?),
    None => None,
  };
  let slot = match admitted(app, road, &key, demand).await {
    Ok(slot) => Some(slot),
    Err(why) => {
      if let Some(why) = why {
        app.places.failed(why);
      }
      None
    }
  };
  match slot {
    Some(slot) => Some(Handed { slot, demand }),
    None => {
      if let Some(demand) = demand {
        engine::give_back(app, demand).await;
      }
      None
    }
  }
}

/// A place's slot asked and asked for its place every half second until it is let in, however long the
/// role's line takes; or why not — none where its road had no try due any longer and it was given back.
async fn admitted(app: &App, road: &Road, key: &str, demand: Option<i64>) -> Result<Slot, Option<String>> {
  let line = runner::line_of(&app.settings.road.key);
  let asked = json!({
    "requestKey": format!("research:place:{key}"),
    "owner": OWNER,
    "role": app.settings.runner.role(line),
    "source": line,
    "provider": road.0,
    "model": road.1,
  });
  let said = app
    .peers
    .ask_slot(&asked)
    .await
    .map_err(|fault| Some(format!("agency gave no slot: {fault}")))?;
  app.places.reached();
  let mut slot =
    Slot::of(&said).ok_or_else(|| Some("agency answered the place's slot with no token".to_string()))?;
  loop {
    if slot.admitted() {
      return Ok(slot);
    }
    if slot.ended() {
      return Err(Some(slot.given_back("the place's slot request")));
    }
    if !wanted(app, road) {
      runner::give_back_unrun(app, &slot.token, "the place's road had no try due any longer").await;
      return Err(None);
    }
    tokio::time::sleep(runner::LOOK_EVERY).await;
    match app.peers.move_slot(&slot.token, &runner::admission(demand)).await {
      Ok(said) => slot = slot.admission(&said),
      Err(fault) => {
        runner::give_back_unrun(app, &slot.token, "the place's slot could not be let in").await;
        return Err(Some(format!("the place's slot could not be let in: {fault}")));
      }
    }
  }
}
