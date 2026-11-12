//! A stop (`docs/runner.md`, 「A stop」): the signals that ask it, the word every part of the daemon that starts
//! work reads, and the routes served until it is asked, then drained for `stop_seconds` before what is still in
//! flight is handed back.

use std::future::IntoFuture;
use std::sync::Arc;
use std::time::Duration;

use tokio::net::TcpListener;
use tokio::sync::watch;

use crate::App;
use crate::engine;
use crate::routes;
use crate::runner;

/// Whether a stop was asked.
pub struct Stop {
  asked: watch::Sender<bool>,
}

impl Default for Stop {
  fn default() -> Self {
    Stop {
      asked: watch::Sender::new(false),
    }
  }
}

impl Stop {
  /// The stop asked: nothing new starts from now on.
  pub fn ask(&self) {
    self.asked.send_replace(true);
  }

  /// Whether the stop was asked.
  pub fn asked(&self) -> bool {
    *self.asked.borrow()
  }

  /// Ends once the stop is asked, at once where it already was.
  pub async fn wait(&self) {
    let mut asked = self.asked.subscribe();
    let _ = asked.wait_for(|asked| *asked).await;
  }
}

/// The signals that ask a stop, taken from the moment this stands so none arriving later ends the process
/// unhandled: SIGTERM, which a deployment's stop sends, and an interrupt.
pub struct Signals {
  #[cfg(unix)]
  terminate: tokio::signal::unix::Signal,
  #[cfg(unix)]
  interrupt: tokio::signal::unix::Signal,
}

impl Signals {
  #[cfg(unix)]
  pub fn take() -> Result<Signals, String> {
    use tokio::signal::unix::{SignalKind, signal};
    let take =
      |kind: SignalKind| signal(kind).map_err(|e| format!("a stop's signal could not be taken: {e}"));
    Ok(Signals {
      terminate: take(SignalKind::terminate())?,
      interrupt: take(SignalKind::interrupt())?,
    })
  }

  #[cfg(not(unix))]
  pub fn take() -> Result<Signals, String> {
    Ok(Signals {})
  }

  /// The next signal asking a stop, by its name.
  #[cfg(unix)]
  pub async fn next(&mut self) -> &'static str {
    tokio::select! {
      _ = self.terminate.recv() => "SIGTERM",
      _ = self.interrupt.recv() => "an interrupt",
    }
  }

  /// The next signal asking a stop, by its name.
  #[cfg(not(unix))]
  pub async fn next(&mut self) -> &'static str {
    let _ = tokio::signal::ctrl_c().await;
    "an interrupt"
  }
}

/// The routes answered on `listener` until the stop is asked; then no connection is accepted, the calls being
/// answered and the runs under way are given `stop_seconds` to end, and what has not ended is handed back.
pub async fn serve(app: Arc<App>, listener: TcpListener) -> Result<(), String> {
  let asked = {
    let app = app.clone();
    async move { app.stop.wait().await }
  };
  let serving = axum::serve(listener, routes::router(app.clone()))
    .with_graceful_shutdown(asked)
    .into_future();
  tokio::pin!(serving);
  let ended = tokio::select! {
    biased;
    () = app.stop.wait() => None,
    served = &mut serving => Some(served),
  };
  // The routes end on their own only for a fault; ended with the stop asked, they are drained already.
  if !app.stop.asked() {
    return ended.unwrap_or(Ok(())).map_err(|e| e.to_string());
  }
  let drained = async {
    let served = match ended {
      Some(served) => served,
      None => serving.await,
    };
    if let Err(why) = served {
      eprintln!("bmo-research: the routes ended with a fault while stopping: {why}");
    }
    runs_closed(&app).await;
  };
  let bound = Duration::from_secs(app.settings.stop_seconds);
  if tokio::time::timeout(bound, drained).await.is_err() {
    eprintln!("bmo-research: what was in flight did not end within {}s of the stop", bound.as_secs());
  }
  hand_back(&app).await;
  Ok(())
}

/// Ends once the store holds no research run open.
async fn runs_closed(app: &App) {
  loop {
    if app.store.open_research_runs().is_ok_and(|open| open.is_empty()) {
      return;
    }
    tokio::time::sleep(runner::LOOK_EVERY).await;
  }
}

/// What is still in flight once the drain ends handed back: every open run cut and what it held given back,
/// every place given back, then every arbiter demand the store holds.
async fn hand_back(app: &Arc<App>) {
  let cut = runner::hand_back(app).await;
  if cut > 0 {
    eprintln!("bmo-research: {cut} research runs under way at the stop were closed as cut");
  }
  let given = engine::give_back_all(app).await;
  if given > 0 {
    eprintln!("bmo-research: {given} arbiter demands held at the stop were given back");
  }
}
