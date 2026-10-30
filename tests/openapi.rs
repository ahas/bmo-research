//! The OpenAPI document (`docs/api.md`): the committed document is what the code generates, and its operations
//! are the routes the router answers.

use research_server::openapi::{OPERATIONS, openapi_json};

#[test]
fn the_committed_document_is_what_the_code_generates() {
  let committed = std::fs::read_to_string(concat!(env!("CARGO_MANIFEST_DIR"), "/docs/openapi.json"))
    .expect("the document is committed");
  assert!(
    committed == openapi_json(),
    "docs/openapi.json differs: run `bmo-research openapi > docs/openapi.json`"
  );
}

#[test]
fn every_operation_is_stated_once_and_every_route_of_the_api_document_is_one() {
  let mut seen: Vec<String> = OPERATIONS.iter().map(|o| format!("{} {}", o.method, o.path)).collect();
  let count = seen.len();
  seen.sort();
  seen.dedup();
  assert_eq!(seen.len(), count, "an operation is stated twice");

  let api = std::fs::read_to_string(concat!(env!("CARGO_MANIFEST_DIR"), "/docs/api.md"))
    .expect("docs/api.md reads");
  let mut documented: Vec<String> = api
    .lines()
    .filter_map(|line| {
      let cells: Vec<&str> = line.split('|').map(str::trim).collect();
      match cells.as_slice() {
        ["", method, path, ..] if ["GET", "POST", "PATCH", "DELETE", "PUT"].contains(method) => {
          Some(format!("{method} {}", path.trim_matches('`')))
        }
        _ => None,
      }
    })
    .collect();
  documented.sort();
  assert_eq!(documented, seen, "docs/api.md and the OpenAPI document state different routes");
}
