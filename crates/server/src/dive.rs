//! The deep dive and its notices (`docs/api.md`, 「The deep dive」 and 「The notices」): the next run picked
//! among the running categories, asked of agency, read until it closes, its document read and its items
//! filed, and every recipient told.

use std::sync::Arc;
use std::time::Duration;

use bmo_research_client::{Category, CategoryShape, Item, RecipientKind, Running, Shape};
use tokio::task::JoinHandle;

use crate::App;
use crate::reading::{self, Exposure};
use crate::store::{Filed, now_ms};
use crate::words::{self, fill};

/// The daemon's loop: the tries it left running taken up, the owner's room given to every category holding
/// no recipient, then a scan and a start of what may run every `tick_seconds`.
pub async fn daemon(app: Arc<App>) {
  take_up(&app);
  give_owner_room(&app).await;
  loop {
    let _ = tick(&app).await;
    tokio::time::sleep(Duration::from_secs(app.settings.tick_seconds.max(1))).await;
  }
}

/// The tries a stopped daemon left running: one asked is read again, one never asked waits again.
pub fn take_up(app: &Arc<App>) {
  let runs = match app.store.open_runs() {
    Ok(r) => r,
    Err(why) => {
      eprintln!("bmo-research: the store: {why}");
      return;
    }
  };
  for run in runs {
    match run.run_id {
      None => {
        let _ = app.store.requeue(run.id);
      }
      Some(run_id) => {
        let app = app.clone();
        tokio::spawn(async move { carry(app, run.id, Some(run_id)).await });
      }
    }
  }
}

/// The owner's private room added to every category holding no recipient.
pub async fn give_owner_room(app: &App) {
  let bare = match app.store.categories_without_recipients() {
    Ok(b) if !b.is_empty() => b,
    _ => return,
  };
  match app.peers.owner_room().await {
    Ok(room) => {
      for category in bare {
        let _ = app.store.add_recipient(&category, RecipientKind::Room, Some(room), None, "owner");
      }
    }
    Err(why) => eprintln!("bmo-research: the owner's room could not be read: {why}"),
  }
}

/// One pass of the loop: a scan, then as many tries started as `concurrency` leaves room for. Answers the
/// tries started, each carried to its end on a task of its own.
pub async fn tick(app: &Arc<App>) -> Vec<JoinHandle<()>> {
  if let Err(why) = crate::scan::scan(app).await {
    eprintln!("bmo-research: the scan: {why}");
  }
  let mut started = Vec::new();
  loop {
    let running = app.store.running_count().unwrap_or(i64::MAX);
    if running >= app.settings.concurrency.max(1) {
      break;
    }
    let Some((category, items)) = next(app) else {
      break;
    };
    let ids: Vec<i64> = items.iter().map(|i| i.id).collect();
    let Ok(Some((run, _))) = app.store.start_try(&category.name, &ids) else {
      break;
    };
    let app = app.clone();
    started.push(tokio::spawn(async move { carry(app, run, None).await }));
  }
  started
}

/// The next try to ask: in each running category, its next waiting item, or a brief's whole gathering once
/// it is due; among them, the highest priority, then the category whose last try was asked longest ago.
fn next(app: &App) -> Option<(Category, Vec<Item>)> {
  let now = now_ms();
  let mut best: Option<(i64, Option<i64>, Category, Vec<Item>)> = None;
  for category in app.store.categories().ok()? {
    if category.state == Running::Stopped {
      continue;
    }
    let last = app.store.last_try_at(&category.name).ok()?;
    let items = if category.shape == CategoryShape::Brief {
      let gather = category.gather_minutes.unwrap_or(1440).max(1) * 60_000;
      let due =
        last.is_none_or(|at| now - at >= gather) || app.store.retry_due(&category.name, now).ok()?;
      if !due {
        continue;
      }
      app.store.waiting(&category.name, now, category.cap.max(1)).ok()?
    } else {
      app.store.waiting(&category.name, now, 1).ok()?
    };
    let Some(first) = items.first() else {
      continue;
    };
    let better = match &best {
      None => true,
      Some((priority, at, _, _)) => {
        first.priority > *priority
          || (first.priority == *priority && last.unwrap_or(i64::MIN) < at.unwrap_or(i64::MIN))
      }
    };
    if better {
      best = Some((first.priority, last, category, items));
    }
  }
  best.map(|(_, _, category, items)| (category, items))
}

/// The question a try is asked: one item's shape with its name and address, an advisory's with the stack,
/// a brief's with every item it gathers.
fn question(app: &App, items: &[Item]) -> String {
  let lead = &items[0];
  let address = lead.address.clone().unwrap_or_default();
  match lead.shape {
    Shape::Brief => {
      let lines: Vec<String> = items
        .iter()
        .map(|i| format!("- {} — {}", i.name, i.address.clone().unwrap_or_default()))
        .collect();
      words::question(Shape::Brief, &[("items", &lines.join("\n"))])
    }
    Shape::Advisory => {
      let stack: Vec<String> = app
        .store
        .stack()
        .unwrap_or_default()
        .into_iter()
        .map(|row| {
          let at: Vec<String> = [row.place, row.repository].into_iter().flatten().collect();
          format!("- {} {} ({})", row.component, row.version.unwrap_or_default(), at.join(", "))
        })
        .collect();
      words::question(
        Shape::Advisory,
        &[("name", &lead.name), ("address", &address), ("stack", &stack.join("\n"))],
      )
    }
    shape => words::question(shape, &[("name", &lead.name), ("address", &address)]),
  }
}

/// One try carried to its end: asked where it was not, read until its run closes, filed and told.
pub async fn carry(app: Arc<App>, run: i64, asked: Option<i64>) {
  let Ok(row) = app.store.run(run) else {
    return;
  };
  let items: Vec<Item> = row.item_ids.iter().filter_map(|id| app.store.item(*id).ok()).collect();
  let Some(lead) = items.first().cloned() else {
    let _ = app.store.end_run(run, "failed", None, Some("the try carries no item"));
    return;
  };
  let Ok(category) = app.store.category(&row.category_name) else {
    let _ = app.store.end_run(run, "failed", None, Some("the try's category no longer stands"));
    return;
  };
  let run_id = match asked {
    Some(id) => id,
    None => {
      let provider = category.provider.clone().unwrap_or_else(|| app.settings.road.provider.clone());
      let model = category.model.clone().unwrap_or_else(|| app.settings.road.model.clone());
      match app.peers.ask_run(&question(&app, &items), &provider, &model).await {
        Ok(id) => {
          let _ = app.store.keep_run_id(run, id);
          id
        }
        Err(why) => return fail(&app, run, &lead, "refused", None, &why).await,
      }
    }
  };
  let outcome = loop {
    match app.peers.run_state(run_id).await {
      Ok(state) if state.state == "closed" => break state.outcome,
      Ok(_) => {}
      Err(why) => eprintln!("bmo-research: run {run_id} could not be read: {why}"),
    }
    tokio::time::sleep(Duration::from_millis((app.settings.poll_seconds * 1000).max(50))).await;
  };
  if outcome.as_deref() == Some("cut") {
    return fail(&app, run, &lead, "cut", outcome.as_deref(), "the run was cut").await;
  }
  let document = match app.peers.filed_document(run_id).await {
    Ok(Some(d)) => d,
    Ok(None) => {
      return fail(&app, run, &lead, "failed", outcome.as_deref(), "the run closed with no document filed")
        .await;
    }
    Err(why) => return fail(&app, run, &lead, "failed", outcome.as_deref(), &why).await,
  };
  let (content, link) = match (app.peers.content(document.id).await, app.peers.share(document.id).await) {
    (Ok(c), Ok(l)) => (c, l),
    (Err(why), _) | (_, Err(why)) => {
      return fail(&app, run, &lead, "failed", outcome.as_deref(), &why).await;
    }
  };
  let _ = app.store.end_run(run, "closed", outcome.as_deref(), None);
  let mut filed = Filed {
    document_id: document.id,
    title: document.title.clone(),
    description: document.description.clone(),
    share_url: link.clone(),
    ..Filed::default()
  };
  let mut exposure = Exposure::default();
  match lead.shape {
    Shape::Manual => {
      let read = reading::manual_fit(&content, &words::document().manual, &app.settings.model_nodes);
      filed.fit = read.fit;
      filed.fit_nodes = read.nodes;
      filed.plan = read.plan;
    }
    Shape::Advisory => {
      exposure = reading::exposure(&content, &words::document().advisory);
      filed.fit = !exposure.touches.is_empty();
      filed.touches = exposure.summary();
      filed.plan = exposure.plan.clone();
    }
    Shape::Brief => {
      for figure in reading::figures(&content, &words::document().brief) {
        let _ = app.store.keep_fact(
          &figure.name,
          &figure.value,
          figure.unit.as_deref(),
          figure.as_of.as_deref(),
          figure.source.as_deref(),
          Some(run_id),
          Some(document.id),
        );
      }
    }
    Shape::Investigation => {}
  }
  let mut told = false;
  for item in &items {
    if app.store.file(item.id, &filed).unwrap_or(false) && item.id == lead.id {
      told = true;
    }
  }
  if told {
    tell(&app, &category, &lead, &filed, &exposure).await;
  }
}

/// A failed try: its items wait again after `retry_minutes`, or after the last retry are failed and every
/// recipient is told once.
async fn fail(app: &App, run: i64, lead: &Item, state: &str, outcome: Option<&str>, why: &str) {
  let _ = app.store.end_run(run, state, outcome, Some(why));
  let tries = app.store.item(lead.id).map(|i| i.tries).unwrap_or(lead.tries);
  let last = tries > app.settings.retries;
  let retry_at = (!last).then(|| now_ms() + app.settings.retry_minutes * 60_000);
  let moved = app.store.fail_try(run, why, retry_at).unwrap_or_default();
  if last && moved.contains(&lead.id) {
    eprintln!("bmo-research: item {} failed after {tries} tries: {why}", lead.id);
    if let Ok(category) = app.store.category(&lead.category_name) {
      let words = words::report();
      let notice = fill(&words.failed, &[("name", &lead.name), ("tries", &tries.to_string())]);
      notify(app, &category, lead, &notice, "failed").await;
    }
  }
}

/// The notice a filed item's recipients hear: the line, and for a `fit` item what its recipient needs to act.
pub fn notice(lead: &Item, filed: &Filed) -> String {
  let words = words::report();
  let summary = reading::first_sentence(filed.description.as_deref().unwrap_or_default());
  let mut lines = vec![fill(
    &words.line,
    &[("title", &filed.title), ("summary", &summary), ("link", &filed.share_url)],
  )];
  if filed.fit {
    match lead.shape {
      Shape::Advisory => {
        lines.push(fill(&words.touches, &[("touches", filed.touches.as_deref().unwrap_or_default())]))
      }
      _ => lines.push(words.fit.clone()),
    }
    lines.push(fill(&words.name, &[("name", &lead.name)]));
    if let Some(plan) = &filed.plan {
      lines.push(fill(&words.plan, &[("plan", plan)]));
    }
    if !filed.fit_nodes.is_empty() {
      lines.push(fill(&words.bench, &[("nodes", &filed.fit_nodes.join(", "))]));
    }
  }
  lines.join("\n")
}

/// Every recipient told of a filed item, and for an advisory each touched repository's worker its part.
async fn tell(app: &App, category: &Category, lead: &Item, filed: &Filed, exposure: &Exposure) {
  notify(app, category, lead, &notice(lead, filed), "report").await;
  if lead.shape != Shape::Advisory {
    return;
  }
  let stack = app.store.stack().unwrap_or_default();
  let words = words::report();
  for repository in exposure.repositories() {
    let mut workers: Vec<String> = stack
      .iter()
      .filter(|row| row.repository.as_deref() == Some(repository.as_str()))
      .filter_map(|row| row.worker.clone())
      .filter(|w| !w.is_empty())
      .collect();
    workers.sort();
    workers.dedup();
    if workers.is_empty() {
      continue;
    }
    let rows: Vec<String> = exposure
      .touches
      .iter()
      .filter(|t| t.repository.as_deref() == Some(repository.as_str()))
      .map(|t| {
        format!("- {} {} {}", t.component, t.version, t.host.clone().unwrap_or_default())
          .trim_end()
          .to_string()
      })
      .collect();
    let part = fill(
      &words.worker,
      &[
        ("name", &lead.name),
        ("repository", &repository),
        ("rows", &rows.join("\n")),
        ("link", &filed.share_url),
        ("plan", filed.plan.as_deref().unwrap_or_default()),
      ],
    );
    for worker in workers {
      let said = app.peers.turn(&worker, &part).await;
      if let Err(why) = &said {
        eprintln!("bmo-research: worker {worker} could not be told item {}: {why}", lead.id);
      }
      let _ =
        app
          .store
          .keep_report(lead.id, &format!("agent:{worker}"), "part", None, said.err().as_deref());
    }
  }
}

/// One notice to every recipient of the category, each kept as a report row.
async fn notify(app: &App, category: &Category, lead: &Item, notice: &str, kind: &str) {
  let recipients = app.store.recipients(&category.name).unwrap_or_default();
  let summary = format!("research {kind}: {}", lead.name);
  for recipient in recipients {
    let (who, said) = match (recipient.kind, recipient.room_id, recipient.agent_name.as_deref()) {
      (RecipientKind::Room, Some(room), _) => {
        (format!("room:{room}"), app.peers.instruct(room, lead.id, notice, &summary).await)
      }
      (RecipientKind::Agent, _, Some(agent)) => {
        (format!("agent:{agent}"), app.peers.turn(agent, notice).await.map(|()| None))
      }
      _ => continue,
    };
    if let Err(why) = &said {
      eprintln!("bmo-research: {who} refused item {}: {why}", lead.id);
    }
    let (instruction, error) = match said {
      Ok(id) => (id, None),
      Err(why) => (None, Some(why)),
    };
    let _ = app.store.keep_report(lead.id, &who, kind, instruction, error.as_deref());
  }
}
