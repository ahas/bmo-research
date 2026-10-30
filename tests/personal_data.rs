//! No tracked file holds personal data: a mail address outside the neutral examples, a home path that names
//! a person, a telephone number, a token or a private network address (`CLAUDE.md`, 「Rules」). A finding
//! names the file, the line and the kind, never the value.

use std::collections::BTreeSet;
use std::path::PathBuf;
use std::process::{Command, Stdio};
use std::sync::LazyLock;

use regex::Regex;

/// The home folders a neutral example writes, so a person's own is not among them.
const NEUTRAL_HOMES: [&str; 6] = ["user", "tester", "someone", "qa-admin", "placeholder", "example"];

/// One kind of personal data, the form it takes and what the repository admits of it.
struct Shape {
  kind: &'static str,
  pattern: Regex,
  neutral: fn(&str) -> bool,
}

fn root() -> PathBuf {
  PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

/// Every file git tracks or would track: the committed ones and the new ones it does not ignore.
fn tracked() -> Vec<String> {
  let output = Command::new("git")
    .args(["ls-files", "--cached", "--others", "--exclude-standard", "-z"])
    .current_dir(root())
    .stdin(Stdio::null())
    .output()
    .expect("git lists the files");

  assert!(
    output.status.success(),
    "git ls-files failed: {}",
    String::from_utf8_lossy(&output.stderr)
  );

  String::from_utf8(output.stdout)
    .expect("the paths are text")
    .split('\0')
    .filter(|path| !path.is_empty() && root().join(path).is_file())
    .map(str::to_owned)
    .collect()
}

/// The tracked files that read as text, with their text.
fn tracked_texts() -> Vec<(String, String)> {
  tracked()
    .into_iter()
    .filter_map(|path| std::fs::read_to_string(root().join(&path)).ok().map(|text| (path, text)))
    .collect()
}

/// Whether a mail address stands at a reserved example domain, where a neutral example is written.
fn example_mail(address: &str) -> bool {
  let domain = address.rsplit_once('@').map(|(_, domain)| domain.to_lowercase()).unwrap_or_default();

  ["example.com", "example.net", "example.org"]
    .iter()
    .any(|example| domain == *example || domain.ends_with(&format!(".{example}")))
    || [".example", ".test", ".invalid", ".localhost"]
      .iter()
      .any(|suffix| domain.ends_with(suffix))
}

/// Whether a home path's own folder is a neutral example rather than a person's.
fn neutral_home(found: &str) -> bool {
  let folder = found.rsplit(['/', '\\']).next().unwrap_or(found);

  NEUTRAL_HOMES.contains(&folder) || folder.starts_with("agent-")
}

/// Whether a token is a written placeholder: a generated secret is mixed case, a placeholder is not.
fn placeholder_token(found: &str) -> bool {
  !found.chars().any(|one| one.is_ascii_uppercase())
}

fn nothing(_: &str) -> bool {
  false
}

/// A shape, with its form read once.
fn shape(kind: &'static str, pattern: &str, neutral: fn(&str) -> bool) -> Shape {
  Shape {
    kind,
    pattern: Regex::new(pattern).expect("a personal-data form reads"),
    neutral,
  }
}

/// The forms each kind takes, kept as shapes and as no list of the values to look for.
fn shapes() -> &'static [Shape] {
  static SHAPES: LazyLock<Vec<Shape>> = LazyLock::new(|| {
    vec![
      shape(
        "a mail address",
        r"\b[A-Za-z0-9._%+-]+@[A-Za-z][A-Za-z0-9.-]*\.[A-Za-z]{2,}\b",
        example_mail,
      ),
      shape("a home path", r"(?:/home/|/Users/)[A-Za-z0-9_][A-Za-z0-9._-]*", neutral_home),
      shape("a home path", r"[A-Za-z]:\\Users\\[A-Za-z0-9_][A-Za-z0-9._-]*", neutral_home),
      shape(
        "a telephone number",
        concat!(
          r"\b01[016789][-. ]?\d{3,4}[-. ]?\d{4}\b|",
          r"\+\d{1,3}[-. ]?\d{2,4}[-. ]?\d{3,4}[-. ]?\d{4}\b|",
          r"\+\d{9,15}\b"
        ),
        nothing,
      ),
      shape(
        "a token",
        concat!(
          r"gh[pousr]_[A-Za-z0-9]{20,}|",
          r"github_pat_[A-Za-z0-9_]{20,}|",
          r"sk-[A-Za-z0-9_-]{20,}|",
          r"xox[baprs]-[A-Za-z0-9-]{10,}|",
          r"AKIA[0-9A-Z]{16}|",
          r"AIza[0-9A-Za-z_-]{35}"
        ),
        placeholder_token,
      ),
      shape(
        "a private network address",
        concat!(
          r"(?i)\b(?:10\.\d{1,3}\.\d{1,3}\.\d{1,3}|",
          r"192\.168\.\d{1,3}\.\d{1,3}|",
          r"172\.(?:1[6-9]|2\d|3[01])\.\d{1,3}\.\d{1,3})\b|",
          r"\b(?:f[cd][0-9a-f]{2}|fe80):"
        ),
        nothing,
      ),
    ]
  });

  SHAPES.as_slice()
}

/// The kinds of personal data `line` holds, each found by its form and not admitted as neutral.
fn personal_in(line: &str) -> BTreeSet<&'static str> {
  shapes()
    .iter()
    .filter(|shape| shape.pattern.find_iter(line).any(|found| !(shape.neutral)(found.as_str())))
    .map(|shape| shape.kind)
    .collect()
}

#[test]
fn no_tracked_file_holds_personal_data() {
  let mut found = Vec::new();

  for (path, text) in tracked_texts() {
    for (number, line) in text.lines().enumerate() {
      for kind in personal_in(line) {
        found.push(format!("{path}:{}: {kind}", number + 1));
      }
    }
  }

  assert!(found.is_empty(), "personal data in tracked files: {found:#?}");
}

#[test]
fn the_check_finds_each_form_and_admits_a_neutral_example() {
  let (at, local) = ("@", "tester");
  let (slash, folder, stranger) = ("/", "user", "nobody");
  let (dot, network, unit) = (".", "10", "0");
  let (phone, token_prefix) = ("010", "ghp_");
  let body = "A".repeat(36);

  // Each form is put together here, so this file does not hold one itself.
  let examples = [
    ("a mail address", format!("{local}{at}{stranger}{dot}com")),
    ("a home path", format!("{slash}home{slash}{stranger}")),
    ("a telephone number", format!("{phone}-1234-5678")),
    ("a token", format!("{token_prefix}{body}")),
    ("a private network address", format!("{network}.{unit}.{unit}.1")),
  ];

  for (kind, text) in examples {
    assert!(personal_in(&text).contains(kind), "{kind} is not found in its own form");
  }

  let (domain, placeholder) = ("example.com", "never-in-a-report-at-all");
  let admitted = [
    format!("{local}{at}{domain}"),
    format!("{slash}home{slash}{folder}"),
    format!("sk-{placeholder}"),
  ];

  for text in admitted {
    assert!(personal_in(&text).is_empty(), "a neutral example is taken for personal data");
  }
}
