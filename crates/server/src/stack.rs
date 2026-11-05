//! The platform's recorded stack (`docs/design.md`, 「The age floor」): the components the owner's registrations
//! and the unit deployments record, the age floor an advisory is judged against, and the block an advisory's
//! question hands its run.
//!
//! The floor is the oldest recorded component, by the moment its build or version was recorded. An advisory
//! whose fixed builds appeared before it cannot reach any recorded build, so it is skipped before it joins
//! the queue. A version the inventory does not record is never read as old or new: one row with no version
//! leaves the floor unknown, and an unknown floor skips nothing.

use std::path::{Path, PathBuf};

use serde::Deserialize;

use crate::App;
use crate::config::Settings;

/// One component the platform records: a row the owner or a job registered, or a unit's deployment record.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Recorded {
  pub component: String,
  pub version: Option<String>,
  pub place: Option<String>,
  pub repository: Option<String>,
  pub worker: Option<String>,
  /// When the build or version was recorded, epoch milliseconds.
  pub at: Option<i64>,
}

/// The oldest recorded component, the floor an advisory's age is judged against.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AgeFloor {
  pub component: String,
  pub version: String,
  pub at: i64,
}

impl AgeFloor {
  /// What the floor is, in one English line: the component, its version and when it was recorded.
  pub fn line(&self) -> String {
    format!("{} {} (recorded {})", self.component, self.version, date(self.at))
  }
}

/// The platform's whole recorded stack: the registered rows and the deployments' records.
pub fn inventory(app: &App) -> Vec<Recorded> {
  let mut rows: Vec<Recorded> = app
    .store
    .stack()
    .unwrap_or_default()
    .into_iter()
    .map(|row| Recorded {
      component: row.component,
      version: row.version,
      place: row.place,
      repository: row.repository,
      worker: row.worker,
      at: Some(row.added_at),
    })
    .collect();
  rows.extend(deployed(&app.settings));
  rows.sort_by(|a, b| {
    a.component
      .to_lowercase()
      .cmp(&b.component.to_lowercase())
      .then_with(|| a.version.cmp(&b.version))
      .then_with(|| a.at.cmp(&b.at))
  });
  rows.dedup_by(|a, b| {
    a.component.eq_ignore_ascii_case(&b.component)
      && a.version == b.version
      && a.place == b.place
      && a.repository == b.repository
  });
  rows
}

/// The floor an advisory is judged against: the oldest recorded component with a version, or none where the
/// inventory is empty or any row records no version. One unknown version leaves the whole floor unknown, so
/// an unrecorded component is never treated as old or new.
pub fn age_floor(rows: &[Recorded]) -> Option<AgeFloor> {
  let mut oldest: Option<AgeFloor> = None;
  for row in rows {
    let version = recorded_version(row.version.as_deref())?;
    let at = row.at?;
    if oldest.as_ref().is_none_or(|floor| at < floor.at) {
      oldest = Some(AgeFloor {
        component: row.component.clone(),
        version: version.to_string(),
        at,
      });
    }
  }
  oldest
}

/// Whether an advisory published at `published_at` predates the floor: its fixed builds, shipped with the
/// advisory, cannot reach a build recorded after it. A reading with no date, or no floor, skips nothing.
pub fn stale(published_at: Option<i64>, floor: Option<&AgeFloor>) -> bool {
  matches!((published_at, floor), (Some(at), Some(floor)) if at < floor.at)
}

/// The inventory as the advisory's question reads it, a row a line, an unrecorded version written `unknown`.
pub fn block(rows: &[Recorded]) -> String {
  if rows.is_empty() {
    return "The inventory records no component.".to_string();
  }
  rows
    .iter()
    .map(|row| {
      let version = recorded_version(row.version.as_deref()).unwrap_or("unknown");
      let at: Vec<String> = [row.place.clone(), row.repository.clone()]
        .into_iter()
        .flatten()
        .filter(|place| !place.trim().is_empty())
        .collect();
      if at.is_empty() {
        format!("- {} {version}", row.component)
      } else {
        format!("- {} {version} ({})", row.component, at.join(", "))
      }
    })
    .collect::<Vec<_>>()
    .join("\n")
}

/// The floor the run is held to, and how an advisory older than it is answered; where no floor stands, the
/// run is told so and skips nothing on age.
pub fn floor_text(floor: Option<&AgeFloor>) -> String {
  match floor {
    Some(floor) => format!(
      "The oldest recorded component is {}. An advisory whose fixed builds appeared before that is out of scope: say so in one line, name this floor, and stop. An advisory whose fixed builds appeared at or after it is judged as usual.",
      floor.line()
    ),
    None => "No age floor stands: the inventory is empty, or it records a component with no version, and an unrecorded version is neither old nor new. No advisory is skipped on age.".to_string(),
  }
}

/// The platform's deploy records, one row each: a unit's live commit, its release version where the record
/// names one, and the moment it was recorded. The folders are the settings' roots; empty reads the folders
/// beside the data root's own, where the unit deployments stand.
pub fn deployed(settings: &Settings) -> Vec<Recorded> {
  let mut rows = Vec::new();
  for root in roots(settings) {
    let direct = root.join("deployed.json");
    if direct.is_file() {
      rows.extend(read_deploy(&root, &direct));
      continue;
    }
    let Ok(entries) = std::fs::read_dir(&root) else {
      continue;
    };
    for entry in entries.flatten() {
      let path = entry.path();
      let Some(name) = path.file_name().and_then(|name| name.to_str()) else {
        continue;
      };
      if !name.starts_with("bmo-") || !path.is_dir() {
        continue;
      }
      let record = path.join("deployed.json");
      if record.is_file() {
        rows.extend(read_deploy(&path, &record));
      }
    }
  }
  rows
}

/// One deploy record: its component is the unit the root is named for, and its version the release version
/// where the record names one, else the commit, which is the build that went live. A record that does not
/// read, or names neither, is left out.
#[derive(Deserialize)]
struct Deployed {
  #[serde(default)]
  commit: String,
  #[serde(default)]
  version: Option<String>,
  #[serde(default)]
  at: Option<String>,
}

fn read_deploy(root: &Path, record: &Path) -> Option<Recorded> {
  let text = std::fs::read_to_string(record).ok()?;
  let deployed: Deployed = serde_json::from_str(&text).ok()?;
  let component = root.file_name()?.to_str()?.strip_prefix("bmo-")?.to_string();
  if component.is_empty() {
    return None;
  }
  let version = deployed
    .version
    .as_deref()
    .map(str::trim)
    .filter(|version| !version.is_empty())
    .map(str::to_string)
    .or_else(|| {
      let commit = deployed.commit.trim();
      (!commit.is_empty()).then(|| commit.to_string())
    })?;
  let at = deployed.at.as_deref().and_then(epoch_ms)?;
  Some(Recorded {
    component: component.clone(),
    version: Some(version),
    place: None,
    repository: Some(component),
    worker: None,
    at: Some(at),
  })
}

/// The folders the deploy records are read from: the settings' roots where they name any, else the folders
/// beside the data root's own, a deployment's root named `bmo-<unit>` and the data under `bmo/data`.
fn roots(settings: &Settings) -> Vec<PathBuf> {
  let named: Vec<PathBuf> = settings
    .stack
    .roots
    .iter()
    .map(|root| crate::config::expand(root))
    .filter(|root| !root.as_os_str().is_empty())
    .collect();
  if !named.is_empty() {
    return named;
  }
  let data = &settings.data_root;
  let under = data.file_name().and_then(|name| name.to_str()) == Some("data")
    && data.parent().and_then(Path::file_name).and_then(|name| name.to_str()) == Some("bmo");
  if under {
    data
      .parent()
      .and_then(Path::parent)
      .map(|share| vec![share.to_path_buf()])
      .unwrap_or_default()
  } else {
    Vec::new()
  }
}

/// A version the inventory records, none where it records none: a blank or the word `unknown` is not a
/// version, and is never read as old or new.
fn recorded_version(version: Option<&str>) -> Option<&str> {
  version
    .map(str::trim)
    .filter(|version| !version.is_empty() && !version.eq_ignore_ascii_case("unknown"))
}

fn epoch_ms(text: &str) -> Option<i64> {
  chrono::DateTime::parse_from_rfc3339(text.trim()).ok().map(|at| at.timestamp_millis())
}

fn date(at: i64) -> String {
  chrono::DateTime::<chrono::Utc>::from_timestamp_millis(at)
    .map(|at| at.to_rfc3339())
    .unwrap_or_else(|| at.to_string())
}
