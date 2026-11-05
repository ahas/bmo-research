//! A deep dive's page drawn (`docs/runner.md`, 「The page」): its diagrams written for Mermaid, which the page
//! loads only where it holds one, its listen band referring to each narration attached by its name, and the
//! line under its title saying where the research came from.

use research_server::page::{Side, Sound, attachment_address, compose, opening, origin_line};

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
fn a_page_opens_on_the_reader_s_language_with_korean_as_its_fallback() {
  let codes = ["ko", "en"];
  assert_eq!(opening(&codes, &["en-US", "en"], None), Some("en"), "an English reader opens English");
  assert_eq!(opening(&codes, &["ko-KR", "ko"], None), Some("ko"), "a Korean reader opens Korean");
  assert_eq!(
    opening(&codes, &["fr-FR"], None),
    Some("ko"),
    "Korean stands where the language is not held"
  );
  assert_eq!(opening(&codes, &["ko-KR"], Some("en")), Some("en"), "the reader's named language wins");
  assert_eq!(opening(&codes, &[], None), Some("ko"), "the fallback is Korean");
  assert_eq!(opening(&["en"], &["ko-KR"], None), Some("en"), "the only side shows");
  assert_eq!(opening(&["ko", "en"], &["fr-FR"], Some("en")), Some("en"), "a named held side is taken");
  assert_eq!(opening(&[], &["en"], None), None, "no side opens");
}

#[test]
fn the_page_chooses_its_side_before_the_body_and_starts_the_switch_and_narration_on_it() {
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
  assert!(
    html.contains("<html lang=\"ko\" data-shown=\"ko\" data-languages=\"ko,en\" data-fallback=\"ko\">"),
    "{html}"
  );
  let reader = html.find("navigator.languages").expect("the reader's languages are read");
  assert!(reader < html.find("<body>").unwrap(), "the side is chosen before the body is drawn");
  assert!(
    html.contains("from_reader!==fallback"),
    "the reader's named language is not the fallback"
  );
  assert!(html.contains("navigator.language"), "the browser's language is read");
  assert!(
    html.contains("html[data-shown=\"en\"] [data-language]:not([data-language=\"en\"]){display:none}")
  );
  assert!(
    html.contains("var current=root.getAttribute(\"data-shown\");if(current){show(current);}"),
    "the switch starts on the side the page opened on: {html}"
  );
  for code in ["ko", "en"] {
    assert!(
      html.contains(&format!("data-language=\"{code}\"")),
      "the {code} side and its narration are marked"
    );
  }
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
    repair_item_id: None,
    error: None,
    error_at: None,
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

const CITED: &str = "## The answer\n\nIt runs on 8 GB. [1][2]\n\nIt was out in May [3\u{2020}], not June [9], and [1] again.\n\n```text\nThe [1] in code.\n```\n\nSee [the card [1]](https://example.org/card).\n\n## Sources\n\n- [1] [the card: memory and licence](https://example.org/card)\n- [2] [the release notes](https://example.org/notes)\n- [3\u{2020}] [a snippet <of> the date](https://example.org/date?a=1&b=2)\n- [4] [never cited](https://example.org/never)\n";

#[test]
fn a_citation_mark_links_to_its_source_entry_which_links_back_to_the_first_mark() {
  let html = page(CITED);
  assert!(
    html.contains(
      "<a class=\"cite\" id=\"en-cite-1\" href=\"#en-source-1\" aria-describedby=\"en-source-1\" \
       data-note=\"the card: memory and licence\" data-url=\"https://example.org/card\">[1]</a>"
    ),
    "{html}"
  );
  assert!(html.contains("id=\"en-cite-2\" href=\"#en-source-2\""), "{html}");
  assert!(
    html.contains("id=\"en-cite-3-snippet\" href=\"#en-source-3-snippet\""),
    "a snippet's mark is a footnote of its own: {html}"
  );
  assert!(
    html.contains(
      "data-note=\"a snippet &lt;of&gt; the date\" data-url=\"https://example.org/date?a=1&amp;b=2\""
    ),
    "{html}"
  );
  assert_eq!(html.matches("id=\"en-cite-1\"").count(), 1, "only the first mark is a target");
  assert_eq!(html.matches("href=\"#en-source-1\"").count(), 2, "both marks in the text link: {html}");
  assert!(html.contains("June [9]"), "a mark naming no entry stays as written: {html}");
  assert!(html.contains("The [1] in code."), "a mark in code stays as written");
  assert!(html.contains(">the card [1]</a>"), "a mark inside a link stays as written: {html}");
  assert!(
    html.contains(
      "<li id=\"en-source-1\"><a href=\"#en-cite-1\" title=\"Back to where the text cites it\">[1]</a> \
       <a href=\"https://example.org/card\">the card: memory and licence</a></li>"
    ),
    "{html}"
  );
  assert!(
    html.contains("<li id=\"en-source-4\">[4] <a href=\"https://example.org/never\">"),
    "an entry never cited links back to nothing: {html}"
  );
  assert!(html.contains("<style data-archive>"), "the footnotes' style is set: {html}");
  assert!(html.contains("a.cite:hover::after"), "the box shows on hover");
  assert!(html.contains("li:target"), "the entry jumped to is highlighted");
  assert!(!html.contains("<script"), "the footnotes run no script: {html}");
}

#[test]
fn each_side_names_its_footnotes_by_its_language() {
  let sides = [
    Side {
      code: "ko",
      title: "Thing-7B (ko)",
      description: None,
      origin: None,
      body: CITED,
    },
    Side {
      code: "en",
      title: "Thing-7B",
      description: None,
      origin: None,
      body: CITED,
    },
  ];
  let html = compose(&sides, &[]);
  for id in ["ko-cite-1", "ko-source-1", "en-cite-1", "en-source-1"] {
    assert_eq!(html.matches(&format!("id=\"{id}\"")).count(), 1, "{id}: {html}");
  }
  assert_eq!(
    html.matches("<style data-archive>").count(),
    1,
    "one style holds the switch and the footnotes"
  );
}

#[test]
fn a_body_citing_no_entry_has_no_footnotes() {
  let html = page("## The answer\n\nIt runs [1].\n\n## Sources\n\n- the card\n");
  assert!(html.contains("It runs [1]."), "{html}");
  assert!(!html.contains("class=\"cite\""));
  assert!(!html.contains("<style"), "{html}");
}

#[test]
fn an_entry_whose_link_the_translation_broke_is_still_a_target() {
  let html = page("It runs [13].\n\n- [13] the card's words](https://example.org/card)\n");
  assert!(html.contains("<li id=\"en-source-13\">"), "{html}");
  assert!(
    html.contains("data-note=\"the card&#39;s words](https://example.org/card)\" data-url=\"\""),
    "{html}"
  );
}
