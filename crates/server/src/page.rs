//! A deep dive's page (`docs/runner.md`, 「The page」): every language its document holds drawn into one HTML
//! page on the archive's design, Korean first in the markup, opening on the reader's language side with
//! Korean as its fallback, a switch between them where it holds two, and the listen band under the title
//! referring to each narration attached by its name, which the archive stands as its player; under the title
//! too, the line saying where the research came from.
//! The markdown is written as HTML by `pulldown-cmark`, a table written without the row under its head stands
//! as a table, any raw HTML a result wrote stands as text, a fenced `mermaid` block is a diagram the page
//! draws with Mermaid, and a citation mark is a footnote linked to its entry of the sources list and back.

use std::collections::{HashMap, HashSet};

use percent_encoding::{AsciiSet, NON_ALPHANUMERIC, utf8_percent_encode};
use pulldown_cmark::{CodeBlockKind, CowStr, Event, Parser, Tag, TagEnd, html};

use bmo_research_client::{Item, OriginKind};

use crate::structure::{self, marks};
use crate::words;

/// One language's side of the page: its code, its title, its description, the line saying where the research
/// came from in markdown, and its markdown body.
#[derive(Debug, Clone)]
pub struct Side<'a> {
  pub code: &'a str,
  pub title: &'a str,
  pub description: Option<&'a str>,
  pub origin: Option<&'a str>,
  pub body: &'a str,
}

/// The side a page opens on: the first of the reader's languages whose primary subtag names a held side,
/// else the language the artifact reader names in `named` where it names a held side and is not the
/// fallback, else Korean where the page holds it, else the page's first side. A tag is matched by its
/// primary subtag without regard to case. The page's own script applies this same order as it opens.
pub fn opening<'a>(codes: &[&'a str], reader: &[&str], named: Option<&str>) -> Option<&'a str> {
  let fallback = codes.iter().copied().find(|code| *code == "ko").or_else(|| codes.first().copied());
  let find = |tag: &str| {
    let primary = tag.split('-').next().unwrap_or_default();
    codes.iter().copied().find(|code| code.eq_ignore_ascii_case(primary))
  };
  named
    .and_then(find)
    .filter(|code| Some(*code) != fallback)
    .or_else(|| reader.iter().find_map(|tag| find(tag)))
    .or_else(|| named.and_then(find))
    .or(fallback)
}

/// Text set into markdown as it stands, its marks taken as text.
fn plain(text: &str) -> String {
  let mut out = String::with_capacity(text.len());
  for c in text.chars() {
    if matches!(c, '\\' | '`' | '*' | '_' | '[' | ']' | '<' | '>' | '#') {
      out.push('\\');
    }
    out.push(c);
  }
  out
}

/// Where an item's research came from, in one language's words (`prompts/page.toml`) as markdown: the source
/// that found it, the owner's ask, or the document a follow-up follows from by its number and its title,
/// linked where its live link is known, `title` standing for the title the item keeps where it is named; then
/// the words the ask was made with. None where the language has no words or the origin names nothing.
pub fn origin_line(item: &Item, code: &str, title: Option<&str>) -> Option<String> {
  let words = words::page().language(code)?;
  let origin = &item.origin;
  let said = match origin.kind {
    OriginKind::Source => {
      let source = origin.source.clone().or_else(|| item.source_id.map(|id| format!("#{id}")))?;
      words::fill(&words.source, &[("source", &plain(&source))])
    }
    OriginKind::Asked => words.asked.clone(),
    OriginKind::FollowUp => {
      let document = origin.document_id?.to_string();
      match title.or(origin.title.as_deref()).filter(|t| !t.trim().is_empty()) {
        Some(title) => {
          let title = match origin.share_url.as_deref().filter(|url| !url.trim().is_empty()) {
            Some(url) => format!("[{}](<{}>)", plain(title.trim()), url.trim()),
            None => plain(title.trim()),
          };
          words::fill(&words.follow_up, &[("document", &document), ("title", &title)])
        }
        None => words::fill(&words.follow_up_untitled, &[("document", &document)]),
      }
    }
  };
  let because = origin
    .because
    .as_deref()
    .map(str::trim)
    .filter(|b| !b.is_empty())
    .map(|b| words::fill(&words.because, &[("because", &plain(b))]));
  Some(match because {
    Some(because) => format!("{said} {because}"),
    None => said,
  })
}

// The switch's look, an addition to the archive's design drawn in its colour variables, so it follows the
// reader's scheme.
const SWITCH_STYLE: &str = ".switch{display:flex;flex-wrap:wrap;align-items:center;gap:4px 8px;margin:0 0 12px;\
font-size:14px;color:var(--doc-muted)}.switch button{font:inherit;padding:2px 12px;border:1px solid \
var(--doc-rule);border-radius:999px;background:var(--doc-surface);color:var(--doc-ink-soft);cursor:pointer}\
.switch button[aria-pressed=\"true\"]{border-color:var(--doc-accent);background:var(--doc-accent-soft);\
color:var(--doc-accent);font-weight:600}";

// The page's opening side, chosen before the body is drawn: the reader's own language where the page holds
// it, the language the artifact reader names where that language is not Korean, Korean as the fallback, and
// the only side otherwise. The page's markup names the held languages and the fallback.
const READER_SCRIPT: &str = "(function(){var root=document.documentElement;\
var held=(root.getAttribute(\"data-languages\")||\"\").split(\",\");\
var fallback=root.getAttribute(\"data-fallback\")||\"\";\
function held_code(tag){var primary=String(tag||\"\").toLowerCase().split(\"-\")[0];\
for(var i=0;i<held.length;i++){if(held[i].toLowerCase()===primary){return held[i];}}return \"\";}\
var chosen=\"\";\
try{var from_reader=held_code(new URLSearchParams(location.search).get(\"language\"));\
if(from_reader&&from_reader!==fallback){chosen=from_reader;}}catch(e){}\
if(!chosen&&navigator.languages){for(var j=0;j<navigator.languages.length&&!chosen;j++){\
chosen=held_code(navigator.languages[j]);}}\
if(!chosen){chosen=held_code(navigator.language);}\
if(!chosen){try{chosen=held_code(new URLSearchParams(location.search).get(\"language\"));}catch(e){}}\
if(!chosen){chosen=held_code(fallback)||held[0]||\"\";}\
if(chosen){root.setAttribute(\"data-shown\",chosen);root.setAttribute(\"lang\",chosen);}})();";

// The switch's behaviour: the language pressed shown, the other hidden, and a player playing there paused. The
// players are found by their language alone, since the archive stands each `audio` element as its own player.
// It starts on the side the page opened on, so the narration heard is that side's.
const SWITCH_SCRIPT: &str = "(function(){var root=document.documentElement;\
var buttons=document.querySelectorAll(\".switch button\");\
function show(code){root.setAttribute(\"data-shown\",code);root.setAttribute(\"lang\",code);\
for(var i=0;i<buttons.length;i++){buttons[i].setAttribute(\"aria-pressed\",\
String(buttons[i].getAttribute(\"data-show\")===code));}\
var sounds=document.querySelectorAll(\"[data-language]\");\
for(var j=0;j<sounds.length;j++){if(sounds[j].getAttribute(\"data-language\")!==code\
&&typeof sounds[j].pause===\"function\"){sounds[j].pause();}}}\
var current=root.getAttribute(\"data-shown\");if(current){show(current);}\
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

// What a sound's name keeps as it is in an `attachment:` address: letters, digits and the unreserved marks.
const NAME_KEPT: &AsciiSet = &NON_ALPHANUMERIC.remove(b'-').remove(b'.').remove(b'_').remove(b'~');

/// The address a page refers to a sound attached beside it by: `attachment:` and its name, percent-encoded
/// where the name needs it.
pub fn attachment_address(name: &str) -> String {
  format!("attachment:{}", utf8_percent_encode(name, NAME_KEPT))
}

/// One narration attached to the page's document: the language it speaks and the name it is kept under.
#[derive(Debug, Clone, Copy)]
pub struct Sound<'a> {
  pub code: &'a str,
  pub name: &'a str,
}

/// One picture the body names, drawn where it stands: the target the markdown names it by, the address the
/// page draws it from — its own kept copy or the address the result gave — and the name it is kept under
/// beside the document, which the page writes on the image.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Figure {
  pub target: String,
  pub address: String,
  pub name: String,
}

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

/// A markdown body written as HTML for the side of the language `code`, a table written without the row under
/// its head given that row, any raw HTML it holds shown as text,
/// each fenced `mermaid` block a diagram's block, its text escaped, each citation mark naming an entry of
/// its sources list a footnote (`cited`), and each picture `figures` names drawn where the body names it.
pub fn body_html(markdown: &str, code: &str) -> String {
  body_html_with(markdown, code, &[])
}

/// A body as `body_html` writes it, each picture `figures` names drawn where the body names it with its
/// caption under it, and a picture no figure names drawn from its own address.
pub fn body_html_with(markdown: &str, code: &str, figures: &[Figure]) -> String {
  let markdown = structure::mended(markdown);
  let mut events = Vec::new();
  let mut diagram: Option<String> = None;
  let mut image: Option<(String, String)> = None;
  for event in Parser::new_ext(&markdown, structure::options()) {
    match (event, diagram.as_mut(), image.as_mut()) {
      (Event::Start(Tag::CodeBlock(kind)), None, None) if is_diagram(&kind) => {
        diagram = Some(String::new())
      }
      (Event::Text(text), Some(open), _) => open.push_str(&text),
      (Event::End(TagEnd::CodeBlock), Some(_), _) => {
        let text = diagram.take().unwrap_or_default();
        events.push(Event::Html(format!("{DIAGRAM}{}</pre>\n", escaped(&text)).into()));
      }
      (Event::Start(Tag::Image { dest_url, .. }), None, None) => {
        image = Some((dest_url.to_string(), String::new()))
      }
      (Event::Text(text), None, Some((_, alt))) | (Event::Code(text), None, Some((_, alt))) => {
        alt.push_str(&text)
      }
      (Event::End(TagEnd::Image), None, Some(_)) => {
        let (target, alt) = image.take().unwrap_or_default();
        let alt = alt.trim();
        let drawn = match figures.iter().find(|figure| figure.target == target) {
          Some(figure) => picture_html(figure, alt),
          None => format!("<img src=\"{}\" alt=\"{}\">", escaped(&target), escaped(alt)),
        };
        events.push(Event::InlineHtml(drawn.into()));
      }
      (_, Some(_), _) | (_, _, Some(_)) => {}
      (Event::Html(text) | Event::InlineHtml(text), None, None) => events.push(Event::Text(text)),
      (other, None, None) => events.push(other),
    }
  }
  let mut out = String::new();
  html::push_html(&mut out, cited(merged(events), code).into_iter());
  out
}

// One picture drawn where the body names it, its caption under it: the result's caption, which names the
// source entry the picture came from. The name it is kept under stands on the image for a reader of the
// markup.
fn picture_html(figure: &Figure, alt: &str) -> String {
  let caption = match alt.is_empty() {
    true => String::new(),
    false => format!("<span class=\"caption\">{}</span>", escaped(alt)),
  };
  format!(
    "<span class=\"picture\"><img src=\"{}\" alt=\"{}\" data-attachment=\"{}\">{caption}</span>",
    escaped(&figure.address),
    escaped(alt),
    escaped(&figure.name)
  )
}

// Adjacent texts joined into one: the parser hands a mark's brackets apart, and raw HTML shown as text stands
// beside the text around it.
fn merged(events: Vec<Event<'_>>) -> Vec<Event<'_>> {
  let mut out: Vec<Event<'_>> = Vec::with_capacity(events.len());
  for event in events {
    match (out.last_mut(), event) {
      (Some(Event::Text(before)), Event::Text(text)) => *before = format!("{before}{text}").into(),
      (_, event) => out.push(event),
    }
  }
  out
}

// The class a citation mark's link carries, which the footnotes' style draws.
const CITE: &str = "class=\"cite\"";

// The pictures' look, an addition to the archive's design drawn in its colour variables: the kept picture
// centred in the reading column under the text that names it, its caption under it.
const PICTURE_STYLE: &str = ".picture{display:block;margin:12px 0;text-align:center}.picture \
img{max-width:100%;height:auto;border:1px solid var(--doc-rule);border-radius:8px}.picture \
.caption{display:block;margin-top:6px;font-size:14px;color:var(--doc-muted)}";

// The footnotes' look, an addition to the archive's design drawn in its colour variables: a mark raised, the
// box saying what its source supports and its address under the line as wide as the reading column while it
// is hovered or focused, and the entry or the mark jumped to highlighted.
const CITE_STYLE: &str = "article{position:relative}a.cite{font-size:.75em;line-height:0;vertical-align:super;\
text-decoration:none;white-space:nowrap}a.cite:hover::after,a.cite:focus-visible::after{content:attr(data-note) \
\"\\A\" attr(data-url);position:absolute;left:0;right:0;z-index:10;margin-top:1.75em;padding:8px 12px;\
border:1px solid var(--doc-rule);border-radius:8px;background:var(--doc-surface);color:var(--doc-ink);\
font-size:14px;font-weight:400;line-height:1.5;vertical-align:baseline;white-space:pre-line;\
overflow-wrap:anywhere;box-shadow:0 4px 16px rgba(0,0,0,.25);pointer-events:none}li:target,a.cite:target{\
background:var(--doc-accent-soft);border-radius:4px}li[id],a.cite[id]{scroll-margin-top:24px}";

/// One entry of a sources list: what its source supports and its address, where the entry links one.
struct Entry {
  note: String,
  url: Option<String>,
}

/// The fragment a mark's key is named by in an identifier: its number, and `-snippet` for a mark with `†`.
fn fragment(key: &str) -> String {
  match key.strip_suffix('\u{2020}') {
    Some(number) => format!("{number}-snippet"),
    None => key.to_owned(),
  }
}

/// The sources list's entries, by the place of each entry's item among the events: an item whose first text
/// opens with a mark and a space, or is the mark alone, its note the text it links, else the rest of its text,
/// and its address the first link it holds. A key a list holds twice is the first item's.
fn entries(events: &[Event<'_>]) -> HashMap<usize, (String, Entry)> {
  let mut found = HashMap::new();
  let mut keys = HashSet::new();
  for (at, event) in events.iter().enumerate() {
    if !matches!(event, Event::Start(Tag::Item)) {
      continue;
    }
    let Some(opening) = events[at + 1..].iter().position(|e| !matches!(e, Event::Start(Tag::Paragraph)))
    else {
      continue;
    };
    let opening = at + 1 + opening;
    let Event::Text(first) = &events[opening] else {
      continue;
    };
    let Some((place, key)) = marks(first).into_iter().next().filter(|(place, _)| place.start == 0) else {
      continue;
    };
    if !first[place.end..].is_empty() && !first[place.end..].starts_with(char::is_whitespace) {
      continue;
    }
    if !keys.insert(key.to_owned()) {
      continue;
    }
    let (mut linked, mut rest, mut url) = (String::new(), first[place.end..].to_owned(), None);
    let (mut depth, mut links) = (0usize, 0usize);
    for event in &events[opening + 1..] {
      match event {
        Event::Start(Tag::Item) => depth += 1,
        Event::End(TagEnd::Item) if depth == 0 => break,
        Event::End(TagEnd::Item) => depth -= 1,
        Event::Start(Tag::Link { dest_url, .. }) => {
          links += 1;
          url.get_or_insert_with(|| dest_url.to_string());
        }
        Event::End(TagEnd::Link) => links = links.saturating_sub(1),
        Event::Text(text) | Event::Code(text) if links > 0 => linked.push_str(text),
        Event::Text(text) | Event::Code(text) => rest.push_str(text),
        _ => {}
      }
    }
    let note = if linked.trim().is_empty() { rest } else { linked };
    found.insert(
      at,
      (
        key.to_owned(),
        Entry {
          note: note.trim().to_owned(),
          url,
        },
      ),
    );
  }
  found
}

/// A body's citation marks made footnotes for the side of the language `code`: each entry of its sources list
/// a target, `<code>-source-<n>`, its own mark linking back to the first place the text cites it,
/// `<code>-cite-<n>`; each mark naming an entry a raised link to it, carrying what the entry's source supports
/// and its address for the footnotes' style to show. A mark inside a link, a picture or code, or among the
/// entries' own words stays as written, as does a mark naming no entry.
fn cited<'a>(events: Vec<Event<'a>>, code: &str) -> Vec<Event<'a>> {
  let listed = entries(&events);
  if listed.is_empty() {
    return events;
  }
  let by_key: HashMap<&str, &Entry> = listed.values().map(|(key, entry)| (key.as_str(), entry)).collect();
  let back = words::page().language(code).map(|l| l.back.clone()).unwrap_or_default();
  let code = escaped(code);
  let mut out: Vec<Event<'a>> = Vec::with_capacity(events.len());
  let mut first_cited: HashSet<String> = HashSet::new();
  let mut backs: Vec<(usize, String)> = Vec::new();
  let (mut links, mut code_blocks, mut entry_depth) = (0usize, 0usize, None::<usize>);
  let mut item_depth = 0usize;
  let mut lead: Option<String> = None;
  for (at, event) in events.into_iter().enumerate() {
    match event {
      Event::Start(Tag::Item) => {
        item_depth += 1;
        match listed.get(&at) {
          Some((key, _)) if entry_depth.is_none() => {
            entry_depth = Some(item_depth);
            lead = Some(key.clone());
            out.push(Event::Html(format!("<li id=\"{code}-source-{}\">", fragment(key)).into()));
          }
          _ => out.push(Event::Start(Tag::Item)),
        }
      }
      Event::End(TagEnd::Item) => {
        if entry_depth == Some(item_depth) {
          entry_depth = None;
          lead = None;
        }
        item_depth = item_depth.saturating_sub(1);
        out.push(Event::End(TagEnd::Item));
      }
      Event::Text(text) if lead.is_some() => {
        let key = lead.take().unwrap_or_default();
        let mark_end = 1 + key.len() + 1;
        backs.push((out.len(), key));
        out.push(Event::InlineHtml(CowStr::Borrowed("")));
        if text.len() > mark_end {
          out.push(Event::Text(text[mark_end..].to_owned().into()));
        }
      }
      Event::Text(text) if entry_depth.is_none() && links == 0 && code_blocks == 0 => {
        let found: Vec<_> = marks(&text).into_iter().filter(|(_, key)| by_key.contains_key(key)).collect();
        if found.is_empty() {
          out.push(Event::Text(text));
          continue;
        }
        let mut from = 0;
        for (place, key) in found {
          if place.start > from {
            out.push(Event::Text(text[from..place.start].to_owned().into()));
          }
          let entry = by_key[key];
          let name = fragment(key);
          let id = if first_cited.insert(key.to_owned()) {
            format!(" id=\"{code}-cite-{name}\"")
          } else {
            String::new()
          };
          out.push(Event::InlineHtml(
            format!(
              "<a {CITE}{id} href=\"#{code}-source-{name}\" aria-describedby=\"{code}-source-{name}\" \
               data-note=\"{}\" data-url=\"{}\">{}</a>",
              escaped(&entry.note),
              escaped(entry.url.as_deref().unwrap_or_default()),
              escaped(&text[place.clone()])
            )
            .into(),
          ));
          from = place.end;
        }
        if from < text.len() {
          out.push(Event::Text(text[from..].to_owned().into()));
        }
      }
      Event::Start(Tag::Link { .. } | Tag::Image { .. }) => {
        links += 1;
        out.push(event);
      }
      Event::End(TagEnd::Link | TagEnd::Image) => {
        links = links.saturating_sub(1);
        out.push(event);
      }
      Event::Start(Tag::CodeBlock(_)) => {
        code_blocks += 1;
        out.push(event);
      }
      Event::End(TagEnd::CodeBlock) => {
        code_blocks = code_blocks.saturating_sub(1);
        out.push(event);
      }
      other => out.push(other),
    }
  }
  for (place, key) in backs {
    let name = fragment(&key);
    let mark = escaped(&format!("[{key}]"));
    out[place] = Event::InlineHtml(if first_cited.contains(&key) {
      format!("<a href=\"#{code}-cite-{name}\" title=\"{}\">{mark}</a>", escaped(&back)).into()
    } else {
      mark.into()
    });
  }
  out
}

/// The page: every side in the order given, the first shown first, the listen band under the title where a
/// narration is attached, and the footnotes' style where a body cites an entry of its sources list.
pub fn compose(sides: &[Side<'_>], sounds: &[Sound<'_>]) -> String {
  compose_with(sides, sounds, &[])
}

/// The page as `compose` draws it, each picture `figures` names drawn where a body names it.
pub fn compose_with(sides: &[Side<'_>], sounds: &[Sound<'_>], figures: &[Figure]) -> String {
  let words = words::page();
  let Some(first) = sides.first() else {
    return String::new();
  };
  let two = sides.len() > 1;
  let codes: Vec<&str> = sides.iter().map(|side| side.code).collect();
  let fallback = opening(&codes, &[], None).unwrap_or(first.code);
  let held = match two {
    true => format!(
      " data-languages=\"{}\" data-fallback=\"{}\"",
      escaped(&codes.join(",")),
      escaped(fallback)
    ),
    false => String::new(),
  };
  let mark = |code: &str| {
    if two {
      format!(" data-language=\"{}\" lang=\"{}\"", escaped(code), escaped(code))
    } else {
      String::new()
    }
  };
  let mut page = String::new();
  page.push_str(&format!(
    "<!doctype html>\n<html lang=\"{code}\" data-shown=\"{code}\"{held}>\n<head>\n<meta charset=\"utf-8\">\n\
     <meta name=\"viewport\" content=\"width=device-width, initial-scale=1\">\n<title>{title}</title>\n",
    code = escaped(first.code),
    title = escaped(first.title),
  ));
  if let Some(description) = first.description.filter(|d| !d.trim().is_empty()) {
    page.push_str(&format!("<meta name=\"description\" content=\"{}\">\n", escaped(description.trim())));
  }
  let bodies: Vec<String> =
    sides.iter().map(|side| body_html_with(side.body, side.code, figures)).collect();
  let mut style = String::new();
  if two {
    style.push_str(SWITCH_STYLE);
    for side in sides {
      let code = escaped(side.code);
      style.push_str(&format!(
        "html[data-shown=\"{code}\"] [data-language]:not([data-language=\"{code}\"]){{display:none}}"
      ));
    }
  }
  if bodies.iter().any(|body| body.contains(CITE)) {
    style.push_str(CITE_STYLE);
  }
  if bodies.iter().any(|body| body.contains("class=\"picture\"")) {
    style.push_str(PICTURE_STYLE);
  }
  if !style.is_empty() {
    page.push_str(&format!("<style data-archive>{style}</style>\n"));
  }
  if two {
    page.push_str(&format!("<script>{READER_SCRIPT}</script>\n"));
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
  for side in sides {
    if let Some(origin) = side.origin.filter(|o| !o.trim().is_empty()) {
      let label = words.language(side.code).map(|l| plain(&l.origin)).unwrap_or_default();
      let line = body_html(&format!("**{label}:** {}", origin.trim()), side.code);
      page.push_str(&format!("<div class=\"origin\"{}>\n{line}</div>\n", mark(side.code)));
    }
  }
  page.push_str("</header>\n");
  if !sounds.is_empty() {
    page.push_str("<section class=\"listen\">\n");
    for side in sides {
      if let Some(language) = words.language(side.code) {
        page.push_str(&format!("<p{}>{}</p>\n", mark(side.code), escaped(&language.listen)));
      }
    }
    for sound in sounds {
      page.push_str(&format!(
        "<audio src=\"{}\" data-language=\"{}\"></audio>\n",
        escaped(&attachment_address(sound.name)),
        escaped(sound.code)
      ));
    }
    page.push_str("</section>\n");
  }
  let mut diagrams = false;
  for (side, body) in sides.iter().zip(&bodies) {
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
