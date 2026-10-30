//! A deep dive's page (`docs/runner.md`, 「The page」): every language its document holds drawn into one HTML
//! page on the archive's design, Korean first, a switch between them where it holds two, and the listen band
//! playing the narration of the language shown. The markdown is written as HTML by `pulldown-cmark`, any raw
//! HTML a result wrote stands as text, and a fenced `mermaid` block is a diagram the page draws with Mermaid.

use base64::Engine;
use pulldown_cmark::{CodeBlockKind, Event, Options, Parser, Tag, TagEnd, html};

use crate::words;

/// One language's side of the page: its code, its title, its description and its markdown body.
#[derive(Debug, Clone)]
pub struct Side<'a> {
  pub code: &'a str,
  pub title: &'a str,
  pub description: Option<&'a str>,
  pub body: &'a str,
}

/// A narration's sound: the language it speaks, its media type and its bytes.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Sound {
  pub code: String,
  pub media_type: String,
  pub bytes: Vec<u8>,
}

// The switch's look, an addition to the archive's design drawn in its colour variables, so it follows the
// reader's scheme.
const SWITCH_STYLE: &str = ".switch{display:flex;flex-wrap:wrap;align-items:center;gap:4px 8px;margin:0 0 12px;\
font-size:14px;color:var(--doc-muted)}.switch button{font:inherit;padding:2px 12px;border:1px solid \
var(--doc-rule);border-radius:999px;background:var(--doc-surface);color:var(--doc-ink-soft);cursor:pointer}\
.switch button[aria-pressed=\"true\"]{border-color:var(--doc-accent);background:var(--doc-accent-soft);\
color:var(--doc-accent);font-weight:600}";

// The switch's behaviour: the language pressed shown, the other hidden, and a sound playing there paused.
const SWITCH_SCRIPT: &str = "(function(){var root=document.documentElement;\
var buttons=document.querySelectorAll(\".switch button\");\
function show(code){root.setAttribute(\"data-shown\",code);root.setAttribute(\"lang\",code);\
for(var i=0;i<buttons.length;i++){buttons[i].setAttribute(\"aria-pressed\",\
String(buttons[i].getAttribute(\"data-show\")===code));}\
var sounds=document.querySelectorAll(\"audio[data-language]\");\
for(var j=0;j<sounds.length;j++){if(sounds[j].getAttribute(\"data-language\")!==code){sounds[j].pause();}}}\
for(var k=0;k<buttons.length;k++){buttons[k].addEventListener(\"click\",function(event){\
show(event.currentTarget.getAttribute(\"data-show\"));});}})();";

// Mermaid, the exact version the archive's own reader draws with, loaded over https by a page holding a
// diagram.
const MERMAID: &str = "https://cdn.jsdelivr.net/npm/mermaid@12.0.0/dist/mermaid.min.js";

// Mermaid started on the page's load: strict, in the theme of the reader's scheme, and a diagram that does not
// parse left as its text.
const MERMAID_SCRIPT: &str = "if(window.mermaid){var dark=window.matchMedia&&\
window.matchMedia(\"(prefers-color-scheme: dark)\").matches;mermaid.initialize({startOnLoad:true,\
securityLevel:\"strict\",suppressErrorRendering:true,theme:dark?\"dark\":\"neutral\"});}";

// The class a diagram's block carries, which Mermaid draws.
const DIAGRAM: &str = "<pre class=\"mermaid\">";

/// A text made safe to stand in HTML, as an element's text or an attribute's value.
pub fn escaped(text: &str) -> String {
  let mut out = String::with_capacity(text.len());
  let _ = pulldown_cmark_escape::escape_html(&mut out, text);
  out
}

/// Whether a fenced block's info string names Mermaid.
fn is_diagram(kind: &CodeBlockKind<'_>) -> bool {
  matches!(kind, CodeBlockKind::Fenced(info) if info.split_whitespace().next() == Some("mermaid"))
}

/// A markdown body written as HTML, any raw HTML it holds shown as text and each fenced `mermaid` block a
/// diagram's block, its text escaped.
pub fn body_html(markdown: &str) -> String {
  let options = Options::ENABLE_TABLES | Options::ENABLE_STRIKETHROUGH | Options::ENABLE_TASKLISTS;
  let mut events = Vec::new();
  let mut diagram: Option<String> = None;
  for event in Parser::new_ext(markdown, options) {
    match (event, diagram.as_mut()) {
      (Event::Start(Tag::CodeBlock(kind)), None) if is_diagram(&kind) => diagram = Some(String::new()),
      (Event::Text(text), Some(open)) => open.push_str(&text),
      (Event::End(TagEnd::CodeBlock), Some(_)) => {
        let text = diagram.take().unwrap_or_default();
        events.push(Event::Html(format!("{DIAGRAM}{}</pre>\n", escaped(&text)).into()));
      }
      (_, Some(_)) => {}
      (Event::Html(text) | Event::InlineHtml(text), None) => events.push(Event::Text(text)),
      (other, None) => events.push(other),
    }
  }
  let mut out = String::new();
  html::push_html(&mut out, events.into_iter());
  out
}

/// The sounds a page plays and whether each is tied to its language: where the page holds two languages and
/// both have a sound, each plays under its own; otherwise the one of the page's first language, else any one,
/// plays under every language.
fn playing<'a>(sides: &[Side<'_>], sounds: &'a [Sound]) -> Vec<(&'a Sound, bool)> {
  let held: Vec<&Sound> =
    sounds.iter().filter(|sound| sides.iter().any(|side| side.code == sound.code)).collect();
  if sides.len() > 1 && held.len() == sides.len() {
    return held.into_iter().map(|sound| (sound, true)).collect();
  }
  let first = sides.first().map(|side| side.code).unwrap_or_default();
  held
    .iter()
    .find(|sound| sound.code == first)
    .or_else(|| held.first())
    .copied()
    .or_else(|| sounds.first())
    .map(|sound| vec![(sound, false)])
    .unwrap_or_default()
}

/// The page: every side in the order given, the first shown first, and the narration's sounds.
pub fn compose(sides: &[Side<'_>], sounds: &[Sound]) -> String {
  let words = words::page();
  let Some(first) = sides.first() else {
    return String::new();
  };
  let two = sides.len() > 1;
  let mark = |code: &str| {
    if two {
      format!(" data-language=\"{}\" lang=\"{}\"", escaped(code), escaped(code))
    } else {
      String::new()
    }
  };
  let mut page = String::new();
  page.push_str(&format!(
    "<!doctype html>\n<html lang=\"{code}\" data-shown=\"{code}\">\n<head>\n<meta charset=\"utf-8\">\n\
     <meta name=\"viewport\" content=\"width=device-width, initial-scale=1\">\n<title>{title}</title>\n",
    code = escaped(first.code),
    title = escaped(first.title),
  ));
  if let Some(description) = first.description.filter(|d| !d.trim().is_empty()) {
    page.push_str(&format!("<meta name=\"description\" content=\"{}\">\n", escaped(description.trim())));
  }
  if two {
    let hiding: Vec<String> = sides
      .iter()
      .map(|side| {
        let code = escaped(side.code);
        format!(
          "html[data-shown=\"{code}\"] [data-language]:not([data-language=\"{code}\"]){{display:none}}"
        )
      })
      .collect();
    page.push_str(&format!("<style data-archive>{SWITCH_STYLE}{}</style>\n", hiding.concat()));
  }
  page.push_str("</head>\n<body>\n<header>\n");
  if two {
    let label = words.language(first.code).map(|l| l.switch.as_str()).unwrap_or(first.code);
    let buttons: Vec<String> = sides
      .iter()
      .enumerate()
      .map(|(at, side)| {
        let name = words.language(side.code).map(|l| l.name.as_str()).unwrap_or(side.code);
        format!(
          "<button type=\"button\" data-show=\"{}\" lang=\"{}\" aria-pressed=\"{}\">{}</button>",
          escaped(side.code),
          escaped(side.code),
          at == 0,
          escaped(name)
        )
      })
      .collect();
    page.push_str(&format!(
      "<nav class=\"switch\" aria-label=\"{}\">{}</nav>\n",
      escaped(label),
      buttons.join("<span aria-hidden=\"true\">|</span>")
    ));
  }
  for side in sides {
    page.push_str(&format!("<h1{}>{}</h1>\n", mark(side.code), escaped(side.title)));
  }
  for side in sides {
    if let Some(description) = side.description.filter(|d| !d.trim().is_empty()) {
      page.push_str(&format!("<p{}>{}</p>\n", mark(side.code), escaped(description.trim())));
    }
  }
  page.push_str("</header>\n");
  let played = playing(sides, sounds);
  if !played.is_empty() {
    page.push_str("<div class=\"listen\">\n");
    for side in sides {
      if let Some(language) = words.language(side.code) {
        page.push_str(&format!("<p{}>{}</p>\n", mark(side.code), escaped(&language.listen)));
      }
    }
    for (sound, tied) in played {
      let tie = if tied {
        format!(" data-language=\"{}\"", escaped(&sound.code))
      } else {
        String::new()
      };
      page.push_str(&format!(
        "<audio controls preload=\"metadata\"{tie} src=\"data:{};base64,{}\"></audio>\n",
        escaped(&sound.media_type),
        base64::engine::general_purpose::STANDARD.encode(&sound.bytes)
      ));
    }
    page.push_str("</div>\n");
  }
  let mut diagrams = false;
  for side in sides {
    let body = body_html(side.body);
    diagrams |= body.contains(DIAGRAM);
    page.push_str(&format!("<article{}>\n{body}</article>\n", mark(side.code)));
  }
  if two {
    page.push_str(&format!("<script>{SWITCH_SCRIPT}</script>\n"));
  }
  if diagrams {
    page.push_str(&format!("<script src=\"{MERMAID}\"></script>\n<script>{MERMAID_SCRIPT}</script>\n"));
  }
  page.push_str("</body>\n</html>\n");
  page
}
