//! The text of the repository holds no Hangul outside the prompts and no personal literal anywhere. The
//! prompts are the one place Korean stands, since it is what the agents are told and answer in.
//!
//! A personal literal is read off the machine the test runs on rather than written here, since a list of
//! them would itself be one: the account's name where its home is a person's, the machine's host name and
//! the e-mail address git commits under.

use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

/// Directories that are built, fetched or kept by tools rather than written.
const NOT_WRITTEN: &[&str] = &[".git", "target", "node_modules"];

/// The folders of code, tests and documents, which hold no Hangul.
const ENGLISH: &[&str] = &["crates", "tools", "tests", "deploy", "docs", "src"];

/// The folder of prompts, read for personal literals alone.
const PROMPTS: &str = "prompts";

fn root() -> PathBuf {
  PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

/// Every text file under `dir`, a file that is not UTF-8 passed over.
fn texts(dir: &Path, found: &mut Vec<(PathBuf, String)>) {
  let Ok(entries) = std::fs::read_dir(dir) else {
    return;
  };

  for entry in entries.flatten() {
    let path = entry.path();

    if path.is_dir() {
      if !NOT_WRITTEN.iter().any(|skip| entry.file_name() == *skip) {
        texts(&path, found);
      }
    } else if let Ok(text) = std::fs::read_to_string(&path) {
      found.push((path, text));
    }
  }
}

/// The files at the root, and those of the folders named.
fn files(folders: &[&str]) -> Vec<(PathBuf, String)> {
  let mut found = Vec::new();

  for entry in std::fs::read_dir(root()).expect("the root reads").flatten() {
    let path = entry.path();

    // A linked worktree's `.git` is a file naming its git directory by absolute path, kept by git as the folder is.
    if path.is_file()
      && !NOT_WRITTEN.iter().any(|skip| entry.file_name() == *skip)
      && let Ok(text) = std::fs::read_to_string(&path)
    {
      found.push((path, text));
    }
  }

  for folder in folders {
    texts(&root().join(folder), &mut found);
  }

  found
}

/// A README translated into another language, `README.<language>.md` at the root, which is written in that language.
fn translated(path: &Path) -> bool {
  path.parent() == Some(root().as_path())
    && path
      .file_name()
      .and_then(|name| name.to_str())
      .is_some_and(|name| name.starts_with("README.") && name != "README.md")
}

/// The README's line of links to its translations, which names each language in its own script.
fn language_bar(line: &str) -> bool {
  line.contains("](README.ko.md)")
}

fn hangul(one: char) -> bool {
  matches!(one, '\u{1100}'..='\u{11FF}' | '\u{3130}'..='\u{318F}' | '\u{A960}'..='\u{A97F}' | '\u{AC00}'..='\u{D7FF}')
}

fn shown(path: &Path, line: usize) -> String {
  format!("{}:{line}", path.strip_prefix(root()).unwrap_or(path).display())
}

/// The account's name, where its home stands under `/home`: a person's account rather than a service's.
fn account() -> Option<String> {
  // SAFETY: getpwuid reads the password database for this process's own user; the record it returns is
  // copied out before any other call could overwrite it.
  let record = unsafe { libc::getpwuid(libc::getuid()) };

  if record.is_null() {
    return None;
  }

  // SAFETY: a non-null record carries NUL-terminated name and home strings.
  let (name, home) = unsafe {
    (
      std::ffi::CStr::from_ptr((*record).pw_name).to_string_lossy().into_owned(),
      std::ffi::CStr::from_ptr((*record).pw_dir).to_string_lossy().into_owned(),
    )
  };

  Path::new(&home).starts_with("/home").then_some(name)
}

fn host() -> Option<String> {
  let name = std::fs::read_to_string("/proc/sys/kernel/hostname").ok()?.trim().to_owned();

  (name != "localhost").then_some(name)
}

fn committer() -> Option<String> {
  let output = Command::new("git")
    .args(["config", "user.email"])
    .current_dir(root())
    .stdin(Stdio::null())
    .output()
    .ok()?;
  let address = String::from_utf8_lossy(&output.stdout).trim().to_owned();

  (output.status.success() && !address.is_empty()).then_some(address)
}

/// The personal literals of this machine, each long enough to be told from a word by itself.
fn personal() -> Vec<String> {
  [account(), host(), committer()]
    .into_iter()
    .flatten()
    .map(|literal| literal.to_lowercase())
    .filter(|literal| literal.chars().count() >= 4)
    .collect()
}

/// Whether `literal` stands in `line` as a word of its own, not inside a longer one.
fn holds(line: &str, literal: &str) -> bool {
  let line = line.to_lowercase();
  let word = |at: Option<char>| at.is_some_and(|one| one.is_alphanumeric() || one == '_' || one == '-');

  line.match_indices(literal).any(|(at, _)| {
    !word(line[..at].chars().next_back()) && !word(line[at + literal.len()..].chars().next())
  })
}

#[test]
fn no_code_test_or_document_holds_hangul() {
  let mut found = Vec::new();

  for (path, text) in files(ENGLISH) {
    if translated(&path) {
      continue;
    }

    for (index, line) in text.lines().enumerate() {
      if line.chars().any(hangul) && !language_bar(line) {
        found.push(shown(&path, index + 1));
      }
    }
  }

  assert!(found.is_empty(), "Hangul outside the prompts: {found:#?}");
}

/// The root license names its copyright holder by the owner's public handle, so its copyright line is the
/// one place that handle stands.
fn license_holder(path: &Path, line: &str) -> bool {
  path == root().join("LICENSE") && line.starts_with("Copyright (c) ")
}

#[test]
fn no_file_holds_a_personal_literal() {
  let literals = personal();
  let mut folders = ENGLISH.to_vec();
  folders.push(PROMPTS);
  let mut found = Vec::new();

  for (path, text) in files(&folders) {
    for (index, line) in text.lines().enumerate() {
      if license_holder(&path, line) {
        continue;
      }
      for literal in &literals {
        if holds(line, literal) {
          found.push(format!("{}: {literal}", shown(&path, index + 1)));
        }
      }
    }
  }

  assert!(found.is_empty(), "personal literals: {found:#?}");
}

#[test]
fn a_literal_is_found_as_a_word_alone() {
  assert!(holds("the path /home/someone/.local", "someone"));
  assert!(holds("Someone wrote it", "someone"));
  assert!(!holds("everyone", "someone"));
  assert!(!holds("someones_file", "someone"));
  assert!(hangul('\u{D55C}'));
  assert!(!hangul('\u{300C}'));
}
