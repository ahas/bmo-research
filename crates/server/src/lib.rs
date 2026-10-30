//! The research unit's daemon: its settings, its store, the source readers and the scan, the deep dive and
//! its notices, the bench watch, the readings of a filed document and its Korean rendering, the routes and
//! the OpenAPI document (`docs/api.md`).

use std::sync::Arc;

pub mod bench;
pub mod config;
pub mod dive;
pub mod openapi;
pub mod peers;
pub mod reading;
pub mod routes;
pub mod scan;
pub mod sources;
pub mod store;
pub mod translation;
pub mod words;

/// Everything the routes and the loop share.
pub struct App {
  pub settings: Arc<config::Settings>,
  pub store: store::Store,
  pub peers: peers::Peers,
  /// The client the sources are read with, under the settings' `User-Agent`.
  pub http: reqwest::Client,
  pub token: String,
  /// How each watched model stands against the bench.
  pub watch: bench::Watch,
}

impl App {
  pub fn new(settings: config::Settings, store: store::Store, token: String) -> Arc<App> {
    let settings = Arc::new(settings);
    let http = reqwest::Client::builder()
      .user_agent(format!("{}/{}", settings.user_agent, env!("CARGO_PKG_VERSION")))
      .build()
      .unwrap_or_default();
    Arc::new(App {
      peers: peers::Peers::new(settings.clone()),
      settings,
      store,
      http,
      token,
      watch: bench::Watch::default(),
    })
  }
}
