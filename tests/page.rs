//! A deep dive's page drawn (`docs/runner.md`, 「The page」): its diagrams written for Mermaid, which the page
//! loads only where it holds one.

use research_server::page::{Side, compose};

const MERMAID: &str = "https://cdn.jsdelivr.net/npm/mermaid@12.0.0/dist/mermaid.min.js";

fn page(body: &str) -> String {
  compose(&[Side {
    code: "en",
    title: "Thing-7B",
    description: None,
    body,
  }])
}

#[test]
fn a_mermaid_block_is_a_diagram_the_page_draws_with_mermaid_loaded_once() {
  let body = "## How it runs\n\n```mermaid\nflowchart TD\n  A[\"Ask <b>\"] --> B\n```\n\n```mermaid\ngraph LR\n  C --> D\n```\n\n```rust\nfn main() {}\n```\n";
  let html = page(body);
  assert!(
    html.contains("<pre class=\"mermaid\">flowchart TD\n  A[&quot;Ask &lt;b&gt;&quot;] --&gt; B\n</pre>"),
    "{html}"
  );
  assert!(!html.contains("language-mermaid"), "no code block stands for a diagram: {html}");
  assert!(html.contains("class=\"language-rust\""), "another fenced block stays code");
  assert_eq!(html.matches(&format!("<script src=\"{MERMAID}\"></script>")).count(), 1);
  for said in [
    "startOnLoad:true",
    "securityLevel:\"strict\"",
    "\"dark\"",
    "\"neutral\"",
    "prefers-color-scheme: dark",
  ] {
    assert!(html.contains(said), "{said}");
  }
}

#[test]
fn a_page_with_no_diagram_loads_nothing() {
  let html = page("## How it runs\n\n```text\nflowchart TD\n```\n\nIt runs.\n");
  assert!(!html.contains("class=\"mermaid\""));
  assert!(!html.contains("<script"), "{html}");
}
