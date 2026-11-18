//! The repository's layout rules: no directory is named after its parent, and the root holds no build
//! tool's manifest beside Cargo's.

use std::fs;
use std::path::{Path, PathBuf};

/// Directories that are built, fetched or kept by tools rather than written.
const NOT_WRITTEN: &[&str] = &[".git", "target", "node_modules", ".placed"];

/// Manifests and lockfiles of build tools other than Cargo.
const OTHER_MANIFESTS: &[&str] = &[
  "package.json",
  "bun.lock",
  "bun.lockb",
  "bunfig.toml",
  "tsconfig.json",
  "package-lock.json",
  "yarn.lock",
  "pnpm-lock.yaml",
  "deno.json",
  "pyproject.toml",
  "requirements.txt",
  "go.mod",
];

fn root() -> PathBuf {
  PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

fn nested(dir: &Path, found: &mut Vec<PathBuf>) {
  let Ok(entries) = fs::read_dir(dir) else {
    return;
  };
  for entry in entries.flatten() {
    let path = entry.path();
    let name = entry.file_name();
    if !path.is_dir() || NOT_WRITTEN.iter().any(|skip| name == *skip) {
      continue;
    }
    if dir.file_name() == Some(name.as_os_str()) {
      found.push(path.strip_prefix(root()).unwrap_or(&path).to_path_buf());
    }
    nested(&path, found);
  }
}

#[test]
fn no_directory_is_named_after_its_parent() {
  let mut found = Vec::new();
  nested(&root(), &mut found);
  assert!(found.is_empty(), "directories named after their parent: {found:?}");
}

#[test]
fn the_root_holds_only_cargo_manifests() {
  let found: Vec<&str> =
    OTHER_MANIFESTS.iter().copied().filter(|name| root().join(name).exists()).collect();
  assert!(found.is_empty(), "manifests of other build tools at the root: {found:?}");
}
