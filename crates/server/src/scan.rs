//! The scan (`docs/sources.md`): every source due read, each candidate judged once against the seen rows and
//! the archive, and each category's free slots filled from what waits unqueued.

use bmo_research_client::{Category, Running, Source};

use crate::App;
use crate::store::{Yield, now_ms};

/// One pass: the sources due of every running category read, then every running category's slots filled.
pub async fn scan(app: &App) -> Result<(), String> {
  let now = now_ms();
  for category in app.store.categories()? {
    if category.state == Running::Stopped {
      continue;
    }
    for source in app.store.category_sources(&category.name)? {
      let every = source.every_minutes.unwrap_or(category.cadence_minutes).max(1);
      if source.read_at.is_some_and(|read| now - read < every * 60_000) {
        continue;
      }
      read_source(app, &category, &source).await;
    }
    app.store.fill(&category)?;
  }
  Ok(())
}

/// One source read: a failure said once in the log and counted, each new candidate kept as seen, and what the
/// reading left written on the source.
pub async fn read_source(app: &App, category: &Category, source: &Source) {
  let candidates = match crate::sources::fetch(&app.http, source).await {
    Ok(c) => c,
    Err(why) => {
      if source.failure_count == 0 {
        eprintln!("bmo-research: source {} ({}) failed: {why}", source.id, source.kind);
      }
      let _ = app.store.source_read(source.id, Err(&why));
      return;
    }
  };
  let mut found = Yield {
    candidates: candidates.len() as i64,
    taken: 0,
  };
  let mut archive_said = false;
  for candidate in candidates {
    match app.store.is_seen(&candidate.key()) {
      Ok(false) => {}
      Ok(true) => continue,
      Err(why) => {
        eprintln!("bmo-research: the store: {why}");
        return;
      }
    }
    match app.peers.archive_holds(&candidate.name).await {
      Ok(held) => match app.store.keep_seen(&candidate, &category.name, Some(source.id), held) {
        Ok(made) => found.taken += i64::from(made && !held),
        Err(why) => eprintln!("bmo-research: the store: {why}"),
      },
      Err(why) => {
        // Left unseen, so the next reading judges it again.
        if !archive_said {
          eprintln!("bmo-research: the archive could not be asked: {why}");
          archive_said = true;
        }
      }
    }
  }
  let _ = app.store.source_read(source.id, Ok(found));
}
