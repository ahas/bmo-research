//! The daemon's settings (`docs/settings.md`): where it listens, the loop's numbers, the road its runs go out
//! on where a category names none, and the files its peers' addresses and tokens are read from.

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
  pub poll_seconds: u64,
  pub turn_wait_seconds: u64,
  pub model_nodes: Vec<String>,
  pub user_agent: String,
  pub road: Road,
  pub agency: Agency,
  pub receiver: Receiver,
  pub artifact: Artifact,
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
      concurrency: 1,
      retries: 3,
      retry_minutes: 30,
      poll_seconds: 30,
      turn_wait_seconds: 20,
      model_nodes: ["n3", "n4", "n5", "n6"].map(String::from).to_vec(),
      user_agent: "bmo-research".into(),
      road: Road::default(),
      agency: Agency::default(),
      receiver: Receiver::default(),
      artifact: Artifact::default(),
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

  /// The token the daemon's routes take.
  pub fn read_token(&self) -> Result<String, String> {
    let path = self.token_path();
    std::fs::read_to_string(&path)
      .map(|t| t.trim().to_string())
      .map_err(|e| format!("{}: {e}", path.display()))
  }
}
