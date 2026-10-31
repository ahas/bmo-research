//! A deep dive's page drawn (`docs/runner.md`, 「The page」): its diagrams written for Mermaid, which the page
//! loads only where it holds one, its listen band referring to each narration attached by its name, and the
//! line under its title saying where the research came from.

use research_server::page::{Side, Sound, attachment_address, compose, origin_line};

const MERMAID: &str = "https://cdn.jsdelivr.net/npm/mermaid@12.0.0/dist/mermaid.min.js";

fn page(body: &str) -> String {
  compose(
    &[Side {
      code: "en",
      title: "Thing-7B",
      description: None,
      origin: None,
      body,
    }],
    &[],
  )
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

#[test]
fn the_listen_band_stands_under_the_title_with_a_reference_to_each_narration_attached() {
  let sides = [
    Side {
      code: "ko",
      title: "Thing-7B (ko)",
      description: None,
      origin: None,
      body: "The body (ko).",
    },
    Side {
      code: "en",
      title: "Thing-7B",
      description: None,
      origin: None,
      body: "The body.",
    },
  ];
  let sounds = [
    Sound {
      code: "ko",
      name: "narration-ko.ogg",
    },
    Sound {
      code: "en",
      name: "narration-en.ogg",
    },
  ];
  let html = compose(&sides, &sounds);
  let band = html.find("<section class=\"listen\">").expect("the band stands");
  assert!(
    html.find("</header>").unwrap() < band && band < html.find("<article").unwrap(),
    "{html}"
  );
  let words = research_server::words::page();
  for side in ["ko", "en"] {
    let heading = &words.language(side).unwrap().listen;
    let marked = format!("<p data-language=\"{side}\" lang=\"{side}\">{heading}</p>");
    assert!(html.contains(&marked), "{marked} missing from {html}");
  }
  assert!(
    html.contains("<audio src=\"attachment:narration-ko.ogg\" data-language=\"ko\"></audio>"),
    "{html}"
  );
  assert!(
    html.contains("<audio src=\"attachment:narration-en.ogg\" data-language=\"en\"></audio>"),
    "{html}"
  );
  assert!(!html.contains("data:"), "no sound is embedded: {html}");
  assert!(!html.contains("controls"), "the archive draws the player: {html}");
  assert!(
    html.contains("querySelectorAll(\"[data-language]\")"),
    "the switch finds the players by language"
  );
  assert!(!html.contains("audio[data-language]"));
}

#[test]
fn a_page_with_no_narration_attached_has_no_band_and_a_name_is_percent_encoded() {
  assert!(!page("The body.").contains("class=\"listen\""));
  assert_eq!(attachment_address("narration-ko.ogg"), "attachment:narration-ko.ogg");
  assert_eq!(attachment_address("a sound #1.ogg"), "attachment:a%20sound%20%231.ogg");
}

#[test]
fn the_line_saying_where_the_research_came_from_stands_under_the_title_its_marks_as_text() {
  use bmo_research_client::{Item, ItemState, Origin, OriginKind, Shape};
  let item = |origin: Origin, source_id: Option<i64>| Item {
    id: 3,
    category_name: "it".into(),
    key: "example.org/thing".into(),
    name: "Thing-7B".into(),
    maker: None,
    address: None,
    kind: "asked".into(),
    shape: Shape::Manual,
    source_id,
    asked: true,
    origin,
    priority: 1,
    state: ItemState::Waiting,
    tries: 0,
    retry_at: None,
    run_id: None,
    document_id: None,
    title: None,
    description: None,
    share_url: None,
    languages: vec![],
    narrations: vec![],
    pending: vec![],
    pending_retry_at: None,
    fit: false,
    touches: None,
    plan: None,
    job_item_id: None,
    error: None,
    found_at: 0,
    created_at: 0,
    changed_at: 0,
  };
  let none = Origin {
    kind: OriginKind::Asked,
    source: None,
    because: None,
    item_id: None,
    document_id: None,
    title: None,
    share_url: None,
  };
  let words = research_server::words::page().language("en").unwrap();
  let asked = origin_line(&item(none.clone(), None), "en", None).unwrap();
  assert_eq!(asked, words.asked);
  let source = Origin {
    kind: OriginKind::Source,
    ..none.clone()
  };
  let found = origin_line(&item(source.clone(), Some(4)), "en", None).unwrap();
  assert!(found.contains("#4"), "a source that no longer stands is named by its number: {found}");
  let follow = Origin {
    kind: OriginKind::FollowUp,
    because: Some("Is *this* the [lighter] one?".into()),
    document_id: Some(812),
    title: Some("Parent_7B".into()),
    share_url: Some("https://share.example/d812".into()),
    ..none
  };
  let line = origin_line(&item(follow.clone(), None), "en", None).unwrap();
  let untitled = Origin {
    title: None,
    ..follow.clone()
  };
  assert!(
    origin_line(&item(untitled, None), "en", None)
      .unwrap()
      .starts_with(&research_server::words::fill(&words.follow_up_untitled, &[("document", "812")]))
  );
  let html = compose(
    &[Side {
      code: "en",
      title: "Thing-7B",
      description: Some("A thing."),
      origin: Some(&line),
      body: "The body.",
    }],
    &[],
  );
  let at = html.find("<div class=\"origin\">").expect("the line stands");
  assert!(html.find("A thing.").unwrap() < at && at < html.find("</header>").unwrap(), "{html}");
  assert!(html.contains(&format!("<strong>{}:</strong>", words.origin)));
  assert!(html.contains("<a href=\"https://share.example/d812\">Parent_7B</a>"), "{html}");
  assert!(html.contains("Is *this* the [lighter] one?"), "the marks stand as text: {html}");
  assert!(!page("The body.").contains("class=\"origin\""), "a page handed no origin prints none");
}
