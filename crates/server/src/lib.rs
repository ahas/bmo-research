//! The research unit's daemon: its settings, its store, the source readers and the scan, the deep dive and
//! its notices, the engine a run needs, the readings of a filed document and its Korean rendering, the research
//! runner with its branches, its documents and the tool host it reads, the routes and the OpenAPI document
//! (`docs/api.md`).

use std::sync::Arc;

pub mod branches;
pub mod config;
pub mod dive;
pub mod document;
pub mod engine;
pub mod openapi;
pub mod page;
pub mod peers;
pub mod reading;
pub mod review;
pub mod routes;
pub mod runner;
pub mod scan;
pub mod sources;
pub mod store;
pub mod tool_host;
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
  /// The tries waiting for their engine's card.
  pub waits: engine::Waits,
  /// The pages being finished now.
  pub pages: dive::Pages,
  /// The last failure to file a stuck item's repair.
  pub repairs: dive::Repairs,
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
      waits: engine::Waits::default(),
      pages: dive::Pages::default(),
      repairs: dive::Repairs::default(),
    })
  }
}
