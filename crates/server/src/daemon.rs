//! The daemon's life as `bmo-research serve` runs it: its token, its store, the sweep of what an earlier
//! process left (`docs/runner.md`, 「A restart」), the routes and the loop, until a signal asks the stop
//! (`docs/runner.md`, 「A stop」).

use crate::App;
use crate::config::Settings;
use crate::store::Store;

/// The daemon's token, written at the first start where its file does not stand, readable by its account.
fn token(settings: &Settings) -> Result<String, String> {
  let path = settings.token_path();
  if !path.exists() {
    let mut bytes = [0u8; 32];
    getrandom::fill(&mut bytes).map_err(|e| format!("no random token could be drawn: {e}"))?;
    let token: String = bytes.iter().map(|b| format!("{b:02x}")).collect();
    if let Some(dir) = path.parent() {
      std::fs::create_dir_all(dir).map_err(|e| format!("{}: {e}", dir.display()))?;
    }
    write_private(&path, &token)?;
  }
  settings.read_token()
}

#[cfg(unix)]
fn write_private(path: &std::path::Path, text: &str) -> Result<(), String> {
  use std::io::Write as _;
  use std::os::unix::fs::OpenOptionsExt as _;
  let mut file = std::fs::OpenOptions::new()
    .write(true)
    .create_new(true)
    .mode(0o600)
    .open(path)
    .map_err(|e| format!("{}: {e}", path.display()))?;
  file.write_all(text.as_bytes()).map_err(|e| format!("{}: {e}", path.display()))
}

#[cfg(not(unix))]
fn write_private(path: &std::path::Path, text: &str) -> Result<(), String> {
  std::fs::write(path, text).map_err(|e| format!("{}: {e}", path.display()))
}

/// The daemon from its start to its stop.
pub async fn run() -> Result<(), String> {
  let mut signals = crate::stop::Signals::take()?;
  let settings = Settings::load()?;
  let token = token(&settings)?;
  let store = Store::open(&settings.store_dir())?;
  let listen = settings.listen.clone();
  let app = App::new(settings, store, token);
  let cut = crate::runner::sweep(&app).await;
  if cut > 0 {
    eprintln!("bmo-research: {cut} research runs left open were closed as cut");
  }
  let given = crate::engine::give_back_all(&app).await;
  if given > 0 {
    eprintln!("bmo-research: {given} arbiter demands left held were given back");
  }
  let listener = tokio::net::TcpListener::bind(&listen).await.map_err(|e| format!("{listen}: {e}"))?;
  eprintln!("bmo-research: serving on http://{listen}");
  tokio::spawn(crate::dive::daemon(app.clone()));
  let stopping = app.clone();
  tokio::spawn(async move {
    let signal = signals.next().await;
    eprintln!("bmo-research: {signal} asked a stop; draining");
    stopping.stop.ask();
  });
  crate::stop::serve(app, listener).await
}
