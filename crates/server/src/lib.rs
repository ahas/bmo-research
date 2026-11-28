//! The research unit's daemon: its settings, its store, the source readers and the scan, the places the nodes
//! pull the queue by, the deep dive and its notices, the engine a run needs, the readings of a filed document and its Korean rendering, the research
//! runner with its branches, its documents and the tool host it reads, the routes and the OpenAPI document
//! (`docs/api.md`).

use std::sync::Arc;

pub mod branches;
pub mod config;
pub mod daemon;
pub mod dive;
pub mod document;
pub mod engine;
pub mod fill;
pub mod openapi;
pub mod page;
pub mod peers;
pub mod pull;
pub mod reading;
pub mod references;
pub mod review;
pub mod routes;
pub mod runner;
pub mod scan;
pub mod sources;
pub mod stack;
pub mod stop;
pub mod store;
pub mod structure;
pub mod tool_host;
pub mod translation;
pub mod words;

/// The version this build answers with: the set's, which the testbed hands the build as `BMO_VERSION`, else
/// the manifest's placeholder (bmo-chat's `docs/conventions.md`, 「Versions」).
pub const VERSION: &str = match option_env!("BMO_VERSION") {
  Some(version) => version,
  None => env!("CARGO_PKG_VERSION"),
};

/// Everything the routes and the loop share.
pub struct App {
  pub settings: Arc<config::Settings>,
  pub store: store::Store,
  pub peers: peers::Peers,
  /// The client the sources are read with, under the settings' `User-Agent`.
  pub http: reqwest::Client,
  pub token: String,
  /// The places waiting for their engine's card.
  pub waits: engine::Waits,
  /// The places asked and not yet holding a try, and what wakes the pull.
  pub places: pull::Places,
  /// The pages being finished now.
  pub pages: dive::Pages,
  /// The last failure to file a stuck item's repair.
  pub repairs: dive::Repairs,
  /// Whether a stop was asked, which every part that starts work reads.
  pub stop: stop::Stop,
}

impl App {
  pub fn new(settings: config::Settings, store: store::Store, token: String) -> Arc<App> {
    if !settings.runner.skills.trim().is_empty() {
      words::hold_skills(config::expand(settings.runner.skills.trim()));
    }
    let settings = Arc::new(settings);
    let http = reqwest::Client::builder()
      .user_agent(format!("{}/{}", settings.user_agent, VERSION))
      .build()
      .unwrap_or_default();
    let stop = stop::Stop::default();
    Arc::new(App {
      peers: peers::Peers::new(settings.clone(), stop.watch()),
      settings,
      store,
      http,
      token,
      waits: engine::Waits::default(),
      places: pull::Places::default(),
      pages: dive::Pages::default(),
      repairs: dive::Repairs::default(),
      stop,
    })
  }
}
