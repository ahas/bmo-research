//! The daemon's settings (`docs/settings.md`): where it listens, the loop's numbers, the road its runs go out
//! on where a category names none, how a document is cut for the translator, the models the engine watch
//! asks the arbiter's cards for, how the runner seats a run and what it is held to where the owner changed nothing, where the
//! tool host answers, the voice a narration is spoken in, where a stuck item's repair is filed, and the files
//! its peers' addresses and tokens are read from.

use std::collections::BTreeMap;

use std::path::{Path, PathBuf};

use serde::Deserialize;

#[derive(Debug, Clone, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct Settings {
  pub listen: String,
  pub token_file: String,
  pub store: String,
  pub tick_seconds: u64,
  pub concurrency: i64,
  pub retries: i64,
  pub retry_minutes: i64,
  pub restore_wait_minutes: i64,
  pub poll_seconds: u64,
  pub turn_wait_seconds: u64,
  pub user_agent: String,
  pub road: Road,
  pub translation: Translation,
  pub engine: Engine,
  pub runner: Runner,
  pub research_settings: ResearchDefaults,
  pub tool_host: ToolHost,
  pub agency: Agency,
  pub receiver: Receiver,
  pub artifact: Artifact,
  pub voice: Voice,
  pub job: Job,
  /// The data root, read from the environment as the settings are loaded; never a key of the file.
  #[serde(skip)]
  pub data_root: PathBuf,
}

impl Default for Settings {
  fn default() -> Self {
    Settings {
      listen: "127.0.0.1:3238".into(),
      token_file: "research.token".into(),
      store: "research".into(),
      tick_seconds: 30,
      concurrency: 4,
      retries: 3,
      retry_minutes: 5,
      restore_wait_minutes: 20,
      poll_seconds: 30,
      turn_wait_seconds: 20,
      user_agent: "bmo-research".into(),
      road: Road::default(),
      translation: Translation::default(),
      engine: Engine::default(),
      runner: Runner::default(),
      research_settings: ResearchDefaults::default(),
      tool_host: ToolHost::default(),
      agency: Agency::default(),
      receiver: Receiver::default(),
      artifact: Artifact::default(),
      voice: Voice::default(),
      job: Job::default(),
      data_root: data_root(),
    }
  }
}

/// The road a research run is asked on.
#[derive(Debug, Clone, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct Road {
  pub key: String,
  pub provider: String,
  pub model: String,
}

impl Default for Road {
  fn default() -> Self {
    Road {
      key: "search:exhaustive".into(),
      provider: "registered".into(),
      model: "flash-next".into(),
    }
  }
}

/// How a filed document's body is cut for the translator, and how many translations are out with it at
/// once: of one page, and across the unit, the translator's slots.
#[derive(Debug, Clone, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct Translation {
  pub part_chars: usize,
  pub parts_out: usize,
  pub parts_out_total: usize,
}

impl Default for Translation {
  fn default() -> Self {
    Translation {
      part_chars: 2500,
      parts_out: 2,
      parts_out_total: 4,
    }
  }
}

/// The registered models the runs stand on, the arbiter service each one's engine is, and the arbiter's
/// master that grants their cards; no master address asks no demand.
#[derive(Debug, Clone, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct Engine {
  pub address: String,
  pub token: String,
  pub provider: String,
  pub models: Vec<Watched>,
}

impl Default for Engine {
  fn default() -> Self {
    Engine {
      address: String::new(),
      token: String::new(),
      provider: "registered".into(),
      models: vec![Watched {
        registration: "flash-next".into(),
        service: "lab-arena".into(),
      }],
    }
  }
}

/// One watched model: its registration's name and the arbiter service its engine is.
#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Watched {
  pub registration: String,
  pub service: String,
}

impl Engine {
  /// The watched model a road goes out on, or none where the road is not watched or no master is named.
  pub fn model(&self, provider: &str, model: &str) -> Option<&Watched> {
    if self.address.trim().is_empty() || provider != self.provider {
      return None;
    }
    self.models.iter().find(|w| w.registration == model)
  }
}

/// How the runner seats a run and what its session record keeps.
#[derive(Debug, Clone, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct Runner {
  pub role: String,
  pub capabilities: Vec<String>,
}

impl Default for Runner {
  fn default() -> Self {
    Runner {
      role: "scout".into(),
      capabilities: ["room.chat", "room.tools", "web.read", "web.sites", "web.branches", "web.rulebooks"]
        .map(String::from)
        .to_vec(),
    }
  }
}

/// One research key's branches, clock and budget.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, serde::Serialize)]
#[serde(deny_unknown_fields)]
pub struct KeyDefaults {
  pub branches: i64,
  pub minutes: i64,
  #[serde(default)]
  pub pages: Option<i64>,
  #[serde(default)]
  pub branch_pages: Option<i64>,
}

/// What every research run is held to where the owner changed nothing.
#[derive(Debug, Clone, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct ResearchDefaults {
  pub branch_role: String,
  pub strategy: String,
  pub host_interval_ms: Option<i64>,
  pub engine_interval_ms: Option<i64>,
  pub keys: BTreeMap<String, KeyDefaults>,
}

impl Default for ResearchDefaults {
  fn default() -> Self {
    let key = |branches, minutes, pages, branch_pages| KeyDefaults {
      branches,
      minutes,
      pages,
      branch_pages,
    };
    ResearchDefaults {
      branch_role: "scout".into(),
      strategy: "direct-then-browser".into(),
      host_interval_ms: None,
      engine_interval_ms: None,
      keys: BTreeMap::from([
        ("search:basic".to_string(), key(0, 5, Some(3), None)),
        ("search:thorough".to_string(), key(2, 15, Some(8), Some(4))),
        ("search:exhaustive".to_string(), key(5, 30, Some(20), Some(6))),
        ("browse".to_string(), key(0, 15, None, None)),
      ]),
    }
  }
}

/// Where the tool host's control socket answers, and the file its issuer token stands in.
#[derive(Debug, Clone, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct ToolHost {
  pub socket: String,
  pub issuer_token: String,
}

impl Default for ToolHost {
  fn default() -> Self {
    ToolHost {
      socket: String::new(),
      issuer_token: "tool-host/issuer-token".into(),
    }
  }
}

#[derive(Debug, Clone, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct Agency {
  pub env: String,
}

impl Default for Agency {
  fn default() -> Self {
    Agency {
      env: "agency.env".into(),
    }
  }
}

#[derive(Debug, Clone, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct Receiver {
  pub env: String,
}

impl Default for Receiver {
  fn default() -> Self {
    Receiver {
      env: "receiver.env".into(),
    }
  }
}

#[derive(Debug, Clone, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct Artifact {
  pub address: String,
  pub token: String,
}

impl Default for Artifact {
  fn default() -> Self {
    Artifact {
      address: "http://127.0.0.1:3230".into(),
      token: "artifact.token".into(),
    }
  }
}

/// The voice unit a deep dive's narration is spoken by: its own settings file, which names its address and its
/// token's file, and the voice the narration is spoken in, empty for the first the unit lists.
#[derive(Debug, Clone, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct Voice {
  pub settings: String,
  pub name: String,
}

impl Default for Voice {
  fn default() -> Self {
    Voice {
      settings: "voice.toml".into(),
      name: String::new(),
    }
  }
}

/// The job service a stuck item's repair is filed in: its own settings file, which names where it listens and
/// its token's file, and the queue, the worker and the labels a repair is filed with.
#[derive(Debug, Clone, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct Job {
  pub settings: String,
  pub queue: String,
  pub assignee: String,
  pub labels: Vec<String>,
}

impl Default for Job {
  fn default() -> Self {
    Job {
      settings: "bmo-job.toml".into(),
      queue: "todo".into(),
      assignee: "helper".into(),
      labels: vec!["lane=repair".into()],
    }
  }
}

/// `~` at the front of a path stands for the home directory.
pub fn expand(path: &str) -> PathBuf {
  if let Some(rest) = path.strip_prefix("~/")
    && let Some(home) = std::env::var_os("HOME")
  {
    return Path::new(&home).join(rest);
  }
  PathBuf::from(path)
}

/// The data root `BMO_DATA_DIR` names, else the platform's own under the home folder.
pub fn data_root() -> PathBuf {
  match std::env::var_os("BMO_DATA_DIR") {
    Some(dir) if !dir.is_empty() => PathBuf::from(dir),
    _ => expand("~/.local/share/bmo/data"),
  }
}

impl Settings {
  /// The settings file `BMO_RESEARCH_SETTINGS` names, else `research/settings.toml` under the data root, else
  /// the defaults.
  pub fn load() -> Result<Settings, String> {
    let root = data_root();
    let path = std::env::var_os("BMO_RESEARCH_SETTINGS")
      .filter(|p| !p.is_empty())
      .map(PathBuf::from)
      .unwrap_or_else(|| root.join("research/settings.toml"));
    let mut settings = match std::fs::read_to_string(&path) {
      Ok(text) => Settings::read(&text).map_err(|e| format!("{}: {e}", path.display()))?,
      Err(e) if e.kind() == std::io::ErrorKind::NotFound => Settings::default(),
      Err(e) => return Err(format!("{}: {e}", path.display())),
    };
    settings.data_root = root;
    Ok(settings)
  }

  /// Settings read from a file's text, every key it leaves out at its default.
  pub fn read(text: &str) -> Result<Settings, String> {
    toml::from_str(text).map_err(|e| e.to_string())
  }

  /// A file under the data root's `env/`, by its name; a name that is a path of its own stands as it is.
  pub fn env_file(&self, name: &str) -> PathBuf {
    let named = expand(name);
    if named.is_absolute() { named } else { self.data_root.join("env").join(named) }
  }

  pub fn token_path(&self) -> PathBuf {
    self.env_file(&self.token_file)
  }

  /// The folder the store stands in.
  pub fn store_dir(&self) -> PathBuf {
    let named = expand(&self.store);
    if named.is_absolute() { named } else { self.data_root.join(named) }
  }

  /// The tool host's control socket: the path the settings name, else `bmo/mcp-host.sock` under the runtime
  /// folder.
  pub fn tool_host_socket(&self) -> PathBuf {
    if !self.tool_host.socket.trim().is_empty() {
      return expand(self.tool_host.socket.trim());
    }
    let runtime = match std::env::var_os("XDG_RUNTIME_DIR") {
      Some(dir) if !dir.is_empty() => PathBuf::from(dir),
      // SAFETY: getuid only reads the process's own user id.
      _ => PathBuf::from(format!("/run/user/{}", unsafe { libc::getuid() })),
    };
    runtime.join("bmo/mcp-host.sock")
  }

  /// The file the tool host's issuer token stands in, under the data root where it names no path of its own.
  pub fn tool_host_token_path(&self) -> PathBuf {
    let named = expand(&self.tool_host.issuer_token);
    if named.is_absolute() { named } else { self.data_root.join(named) }
  }

  /// The token the daemon's routes take.
  pub fn read_token(&self) -> Result<String, String> {
    let path = self.token_path();
    std::fs::read_to_string(&path)
      .map(|t| t.trim().to_string())
      .map_err(|e| format!("{}: {e}", path.display()))
  }
}
