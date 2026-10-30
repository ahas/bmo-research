//! The source readers on fixture bodies (`docs/sources.md`): each kind's answer read into candidates, the
//! filter, and the key a candidate is judged by.

mod support;

use bmo_research_client::{Filter, Shape};
use research_server::sources::{self, Candidate, ItemKind};

fn names(candidates: &[Candidate]) -> Vec<&str> {
  candidates.iter().map(|c| c.name.as_str()).collect()
}

#[test]
fn the_hub_list_reads_into_model_candidates() {
  let found = sources::read_huggingface(support::fixture("huggingface.json").as_bytes()).unwrap();
  assert_eq!(
    names(&found),
    [
      "example-lab/Example-Model-7B",
      "someone/Example-Model-7B-chat-roleplay",
      "other-lab/Tiny-Vision-2B"
    ]
  );
  let first = &found[0];
  assert_eq!(first.maker.as_deref(), Some("example-lab"));
  assert_eq!(first.address.as_deref(), Some("https://huggingface.co/example-lab/Example-Model-7B"));
  assert_eq!(first.kind, ItemKind::Model);
  assert!(first.published_at.is_some());
  assert!(
    found[2].published_at.is_some(),
    "the last modified time stands where no creation time does"
  );
  assert!(first.words.contains("text-generation"));
}

#[test]
fn the_arxiv_feed_reads_into_paper_candidates_without_their_version() {
  let found = sources::read_arxiv(support::fixture("arxiv.xml").as_bytes()).unwrap();
  assert_eq!(names(&found), ["Two-Bit Weights Without Loss", "A Survey of Things"]);
  assert_eq!(found[0].address.as_deref(), Some("https://arxiv.org/abs/2610.00001"));
  assert_eq!(found[0].maker.as_deref(), Some("Example Author"));
  assert_eq!(found[0].kind, ItemKind::Paper);
  assert!(found[0].words.contains("quantization"));
  assert!(found[0].words.contains("cs.LG"));
}

#[test]
fn a_release_list_reads_into_release_candidates_passing_drafts_over() {
  let found = sources::read_github_releases(
    support::fixture("releases.json").as_bytes(),
    "example-org/example-engine",
  )
  .unwrap();
  assert_eq!(names(&found), ["example-engine v2.1.0"]);
  assert_eq!(found[0].maker.as_deref(), Some("example-org"));
  assert_eq!(
    found[0].address.as_deref(),
    Some("https://github.com/example-org/example-engine/releases/tag/v2.1.0")
  );
  assert!(found[0].words.contains("faster"));
}

#[test]
fn a_feed_reads_into_entry_candidates_made_by_the_feed() {
  let found = sources::read_feed(support::fixture("feed.xml").as_bytes()).unwrap();
  assert_eq!(found.len(), 2);
  assert_eq!(found[0].maker.as_deref(), Some("Example Security List"));
  assert_eq!(found[0].address.as_deref(), Some("https://security.example.org/advisories/2026-01"));
  assert_eq!(found[0].kind, ItemKind::Entry);
  assert!(found[0].published_at.is_some());
}

#[test]
fn a_body_its_kind_does_not_read_is_a_failure() {
  assert!(sources::read_huggingface(b"<html>").is_err());
  assert!(sources::read_feed(b"{\"not\": \"a feed\"}").is_err());
}

#[test]
fn the_filter_requires_and_excludes_without_regard_to_case() {
  let found = sources::read_huggingface(support::fixture("huggingface.json").as_bytes()).unwrap();
  let filter = Filter {
    require: vec!["GGUF".into()],
    exclude: vec!["roleplay".into()],
  };
  let passing: Vec<&str> = found.iter().filter(|c| c.passes(&filter)).map(|c| c.name.as_str()).collect();
  assert_eq!(passing, ["other-lab/Tiny-Vision-2B"]);
  assert!(found.iter().all(|c| c.passes(&Filter::default())));
}

#[test]
fn a_key_is_the_address_bare_else_the_name_lower_cased() {
  let mut candidate = Candidate {
    name: "Some Thing".into(),
    maker: None,
    address: Some("HTTPS://Example.org/Thing/".into()),
    published_at: None,
    kind: ItemKind::Entry,
    words: String::new(),
  };
  assert_eq!(candidate.key(), "example.org/thing");
  candidate.address = None;
  assert_eq!(candidate.key(), "some thing");
}

#[test]
fn a_made_thing_is_a_manual_and_a_claim_an_investigation() {
  assert_eq!(ItemKind::Model.shape(), Shape::Manual);
  for kind in [ItemKind::Paper, ItemKind::Release, ItemKind::Entry] {
    assert_eq!(kind.shape(), Shape::Investigation);
  }
}
