//! The research unit's OpenAPI document, `docs/openapi.json`, generated from the client crate's types.

use bmo_research_client::*;
use schemars::Schema;
use schemars::generate::{SchemaGenerator, SchemaSettings};
use serde_json::{Map, Value, json};

/// One parameter of an operation.
pub struct Param {
  pub name: &'static str,
  pub place: &'static str,
  pub kind: &'static str,
  pub description: &'static str,
}

/// One operation of the document.
pub struct Operation {
  pub method: &'static str,
  pub path: &'static str,
  pub id: &'static str,
  pub tag: &'static str,
  pub summary: &'static str,
  pub params: &'static [Param],
  /// The request body's type, where the route takes one.
  pub body: Option<&'static str>,
  /// The success status and its JSON type, where it answers one.
  pub answer: (u16, Option<&'static str>),
  /// The codes beside `unauthorized` the route can answer.
  pub codes: &'static [&'static str],
}

const fn path(name: &'static str, description: &'static str) -> Param {
  Param {
    name,
    place: "path",
    kind: "string",
    description,
  }
}

const fn query(name: &'static str, kind: &'static str, description: &'static str) -> Param {
  Param {
    name,
    place: "query",
    kind,
    description,
  }
}

const CATEGORY: Param = path("categoryName", "the category's name");
const RUN: Param = path("runId", "the research run's id");
const LIMIT: Param = query("limit", "integer", "50 where none is named, at most 200");

/// Every operation, in the order the document lists them.
pub const OPERATIONS: &[Operation] = &[
  Operation {
    method: "GET",
    path: "/health",
    id: "getHealth",
    tag: "health",
    summary: "Readiness and version.",
    params: &[],
    body: None,
    answer: (200, Some("Health")),
    codes: &[],
  },
  Operation {
    method: "GET",
    path: "/state",
    id: "getState",
    tag: "state",
    summary: "Whether the loop runs, each category's state and its part of the queue.",
    params: &[],
    body: None,
    answer: (200, Some("DaemonState")),
    codes: &["failed"],
  },
  Operation {
    method: "PATCH",
    path: "/state",
    id: "patchState",
    tag: "state",
    summary: "Stop or resume every category, or the one named.",
    params: &[],
    body: Some("StateChange"),
    answer: (200, Some("DaemonState")),
    codes: &["bad-request", "not-found", "failed"],
  },
  Operation {
    method: "GET",
    path: "/categories",
    id: "listCategories",
    tag: "categories",
    summary: "The categories by name.",
    params: &[],
    body: None,
    answer: (200, Some("CategoryPage")),
    codes: &["failed"],
  },
  Operation {
    method: "POST",
    path: "/categories",
    id: "createCategory",
    tag: "categories",
    summary: "Register a category, its first recipient the owner's private room.",
    params: &[],
    body: Some("NewCategory"),
    answer: (201, Some("Category")),
    codes: &["bad-request", "conflict", "failed"],
  },
  Operation {
    method: "GET",
    path: "/categories/{categoryName}",
    id: "getCategory",
    tag: "categories",
    summary: "One category.",
    params: &[CATEGORY],
    body: None,
    answer: (200, Some("Category")),
    codes: &["not-found", "failed"],
  },
  Operation {
    method: "PATCH",
    path: "/categories/{categoryName}",
    id: "patchCategory",
    tag: "categories",
    summary: "Change a category's purpose, cap, cadence, gathering or road.",
    params: &[CATEGORY],
    body: Some("CategoryChange"),
    answer: (200, Some("Category")),
    codes: &["bad-request", "not-found", "failed"],
  },
  Operation {
    method: "DELETE",
    path: "/categories/{categoryName}",
    id: "deleteCategory",
    tag: "categories",
    summary: "Take a category away with its recipients.",
    params: &[CATEGORY],
    body: None,
    answer: (204, None),
    codes: &["not-found", "conflict", "failed"],
  },
  Operation {
    method: "GET",
    path: "/categories/{categoryName}/recipients",
    id: "listRecipients",
    tag: "recipients",
    summary: "A category's recipients, the owner's room first.",
    params: &[CATEGORY],
    body: None,
    answer: (200, Some("RecipientPage")),
    codes: &["not-found", "failed"],
  },
  Operation {
    method: "POST",
    path: "/categories/{categoryName}/recipients",
    id: "createRecipient",
    tag: "recipients",
    summary: "Add a room or an agent to a category.",
    params: &[CATEGORY],
    body: Some("NewRecipient"),
    answer: (201, Some("Recipient")),
    codes: &["bad-request", "not-found", "conflict", "failed"],
  },
  Operation {
    method: "DELETE",
    path: "/categories/{categoryName}/recipients/{recipientId}",
    id: "deleteRecipient",
    tag: "recipients",
    summary: "Take a recipient away.",
    params: &[CATEGORY, path("recipientId", "the recipient's id")],
    body: None,
    answer: (204, None),
    codes: &["bad-request", "not-found", "failed"],
  },
  Operation {
    method: "GET",
    path: "/sources",
    id: "listSources",
    tag: "sources",
    summary: "The registered sources by id.",
    params: &[
      query("categoryName", "string", "the category the sources belong to"),
      query("after", "integer", "a source's id"),
      LIMIT,
    ],
    body: None,
    answer: (200, Some("SourcePage")),
    codes: &["bad-request", "failed"],
  },
  Operation {
    method: "POST",
    path: "/sources",
    id: "createSource",
    tag: "sources",
    summary: "Register a source.",
    params: &[],
    body: Some("NewSource"),
    answer: (201, Some("Source")),
    codes: &["bad-request", "not-found", "failed"],
  },
  Operation {
    method: "GET",
    path: "/sources/{sourceId}",
    id: "getSource",
    tag: "sources",
    summary: "One source.",
    params: &[path("sourceId", "the source's id")],
    body: None,
    answer: (200, Some("Source")),
    codes: &["bad-request", "not-found", "failed"],
  },
  Operation {
    method: "PATCH",
    path: "/sources/{sourceId}",
    id: "patchSource",
    tag: "sources",
    summary: "Change a source's address, query, filter, cadence or limit.",
    params: &[path("sourceId", "the source's id")],
    body: Some("SourceChange"),
    answer: (200, Some("Source")),
    codes: &["bad-request", "not-found", "failed"],
  },
  Operation {
    method: "DELETE",
    path: "/sources/{sourceId}",
    id: "deleteSource",
    tag: "sources",
    summary: "Take a source away; what it turned up stays.",
    params: &[path("sourceId", "the source's id")],
    body: None,
    answer: (204, None),
    codes: &["bad-request", "not-found", "failed"],
  },
  Operation {
    method: "GET",
    path: "/queue",
    id: "listQueue",
    tag: "queue",
    summary: "The items in the queue's order.",
    params: &[
      query("categoryName", "string", "the category the items belong to"),
      query("state", "string", "a comma-separated list of item states"),
      query("after", "integer", "an item's id: the page continues past its place"),
      LIMIT,
    ],
    body: None,
    answer: (200, Some("ItemPage")),
    codes: &["bad-request", "failed"],
  },
  Operation {
    method: "POST",
    path: "/queue",
    id: "createAsk",
    tag: "queue",
    summary: "Ask for a thing by its name and its maker's address.",
    params: &[],
    body: Some("Ask"),
    answer: (201, Some("Item")),
    codes: &["bad-request", "not-found", "conflict", "failed"],
  },
  Operation {
    method: "GET",
    path: "/queue/{itemId}",
    id: "getItem",
    tag: "queue",
    summary: "One item.",
    params: &[path("itemId", "the item's id")],
    body: None,
    answer: (200, Some("Item")),
    codes: &["bad-request", "not-found", "failed"],
  },
  Operation {
    method: "PATCH",
    path: "/queue/{itemId}",
    id: "patchItem",
    tag: "queue",
    summary: "Dismiss an item, mark it taken, or move it to the front.",
    params: &[path("itemId", "the item's id")],
    body: Some("ItemChange"),
    answer: (200, Some("Item")),
    codes: &["bad-request", "not-found", "conflict", "failed"],
  },
  Operation {
    method: "GET",
    path: "/runs",
    id: "listRuns",
    tag: "runs",
    summary: "The tries newest first.",
    params: &[
      query("itemId", "integer", "an item the tries carry"),
      query("categoryName", "string", "the category the tries ran for"),
      query("before", "integer", "a try's id"),
      LIMIT,
    ],
    body: None,
    answer: (200, Some("RunPage")),
    codes: &["bad-request", "failed"],
  },
  Operation {
    method: "GET",
    path: "/runs/{id}",
    id: "getRun",
    tag: "runs",
    summary: "One try.",
    params: &[path("id", "the try's id")],
    body: None,
    answer: (200, Some("Run")),
    codes: &["bad-request", "not-found", "failed"],
  },
  Operation {
    method: "GET",
    path: "/stack",
    id: "listStack",
    tag: "stack",
    summary: "The platform's stack inventory, by component.",
    params: &[],
    body: None,
    answer: (200, Some("StackPage")),
    codes: &["failed"],
  },
  Operation {
    method: "POST",
    path: "/stack",
    id: "createStackRow",
    tag: "stack",
    summary: "Add a row to the stack inventory.",
    params: &[],
    body: Some("NewStackRow"),
    answer: (201, Some("StackRow")),
    codes: &["bad-request", "failed"],
  },
  Operation {
    method: "DELETE",
    path: "/stack/{id}",
    id: "deleteStackRow",
    tag: "stack",
    summary: "Take a row of the stack inventory away.",
    params: &[path("id", "the row's id")],
    body: None,
    answer: (204, None),
    codes: &["bad-request", "not-found", "failed"],
  },
  Operation {
    method: "GET",
    path: "/facts",
    id: "listFacts",
    tag: "facts",
    summary: "The figures the briefs kept, newest first.",
    params: &[
      query("name", "string", "the figure's name"),
      query("before", "integer", "a fact's id"),
      LIMIT,
    ],
    body: None,
    answer: (200, Some("FactPage")),
    codes: &["bad-request", "failed"],
  },
  Operation {
    method: "POST",
    path: "/research-runs",
    id: "createResearchRun",
    tag: "research runs",
    summary: "Ask a research: a search at a depth or a browse, answered at once.",
    params: &[],
    body: Some("ResearchRunAsk"),
    answer: (202, Some("ResearchRun")),
    codes: &["bad-request", "unavailable", "failed"],
  },
  Operation {
    method: "GET",
    path: "/research-runs/{runId}",
    id: "getResearchRun",
    tag: "research runs",
    summary: "One research run.",
    params: &[RUN],
    body: None,
    answer: (200, Some("ResearchRunView")),
    codes: &["bad-request", "not-found", "failed"],
  },
  Operation {
    method: "GET",
    path: "/research-runs/{runId}/log-lines",
    id: "getResearchLog",
    tag: "research runs",
    summary: "A research run's log, oldest first.",
    params: &[RUN],
    body: None,
    answer: (200, Some("ResearchLog")),
    codes: &["bad-request", "not-found", "failed"],
  },
  Operation {
    method: "POST",
    path: "/branches",
    id: "createBranch",
    tag: "research runs",
    summary: "Open a branch of a research run, answered once it ends.",
    params: &[],
    body: Some("BranchAsk"),
    answer: (200, Some("BranchEnd")),
    codes: &["bad-request", "not-found", "branch-refused", "failed"],
  },
  Operation {
    method: "GET",
    path: "/research-settings",
    id: "getResearchSettings",
    tag: "research settings",
    summary: "The settings every research run is held to, with the search engines' state.",
    params: &[],
    body: None,
    answer: (200, Some("ResearchSettings")),
    codes: &["failed"],
  },
  Operation {
    method: "PUT",
    path: "/research-settings",
    id: "putResearchSettings",
    tag: "research settings",
    summary: "Change the research settings; a field left out keeps what it holds.",
    params: &[],
    body: Some("ResearchSettingsChange"),
    answer: (200, Some("ResearchSettings")),
    codes: &["bad-request", "upstream-failed", "failed"],
  },
];

fn schema(g: &mut SchemaGenerator, name: &str) -> Schema {
  match name {
    "Health" => g.subschema_for::<Health>(),
    "DaemonState" => g.subschema_for::<DaemonState>(),
    "StateChange" => g.subschema_for::<StateChange>(),
    "Category" => g.subschema_for::<Category>(),
    "CategoryPage" => g.subschema_for::<Page<Category>>(),
    "NewCategory" => g.subschema_for::<NewCategory>(),
    "CategoryChange" => g.subschema_for::<CategoryChange>(),
    "Recipient" => g.subschema_for::<Recipient>(),
    "RecipientPage" => g.subschema_for::<Page<Recipient>>(),
    "NewRecipient" => g.subschema_for::<NewRecipient>(),
    "Source" => g.subschema_for::<Source>(),
    "SourcePage" => g.subschema_for::<Page<Source>>(),
    "NewSource" => g.subschema_for::<NewSource>(),
    "SourceChange" => g.subschema_for::<SourceChange>(),
    "Item" => g.subschema_for::<Item>(),
    "ItemPage" => g.subschema_for::<Page<Item>>(),
    "Ask" => g.subschema_for::<Ask>(),
    "ItemChange" => g.subschema_for::<ItemChange>(),
    "Run" => g.subschema_for::<Run>(),
    "RunPage" => g.subschema_for::<Page<Run>>(),
    "StackRow" => g.subschema_for::<StackRow>(),
    "StackPage" => g.subschema_for::<Page<StackRow>>(),
    "NewStackRow" => g.subschema_for::<NewStackRow>(),
    "FactPage" => g.subschema_for::<Page<Fact>>(),
    "ResearchRunAsk" => g.subschema_for::<ResearchRunAsk>(),
    "ResearchRun" => g.subschema_for::<ResearchRun>(),
    "ResearchRunView" => g.subschema_for::<ResearchRunView>(),
    "ResearchLog" => g.subschema_for::<ResearchLog>(),
    "BranchAsk" => g.subschema_for::<BranchAsk>(),
    "BranchEnd" => g.subschema_for::<BranchEnd>(),
    "ResearchSettings" => g.subschema_for::<ResearchSettings>(),
    "ResearchSettingsChange" => g.subschema_for::<ResearchSettingsChange>(),
    other => panic!("the document names no type {other}"),
  }
}

/// What status a kebab-case code comes to.
fn status_of(code: &str) -> u16 {
  match code {
    "bad-request" => 400,
    "unauthorized" => 401,
    "no-route" | "not-found" => 404,
    "conflict" | "branch-refused" => 409,
    "failed" => 500,
    "upstream-failed" => 502,
    "unavailable" => 503,
    _ => 422,
  }
}

/// What the response of one code is called.
fn response_name(code: &str) -> String {
  code
    .split('-')
    .map(|part| {
      let mut letters = part.chars();
      letters
        .next()
        .map(|first| first.to_uppercase().collect::<String>() + letters.as_str())
        .unwrap_or_default()
    })
    .collect()
}

const DESCRIPTION: &str = "The research unit: a daemon that keeps a queue of things to research by category, reads \
  the sources the owner registers, has each item researched by its own runner in its category's shape, and tells \
  the category's recipients. It files no job and launches nothing.";

/// The calls the unit makes, by unit and route (docs/api.md, 「Calls this unit makes」).
const CALLS: &[(&str, &str)] = &[
  ("agency", "POST /seat-requests"),
  ("agency", "PATCH /seat-requests/{token}"),
  ("agency", "POST /sessions"),
  ("agency", "POST /sessions/{sessionId}/turns"),
  ("agency", "DELETE /sessions/{sessionId}"),
  ("agency", "GET /roles"),
  ("agency", "GET /models/{registration}"),
  ("agency", "POST /translation-runs"),
  ("artifact", "GET /documents"),
  ("artifact", "POST /documents"),
  ("artifact", "PATCH /documents/{documentId}"),
  ("artifact", "POST /documents/{documentId}/attachments"),
  ("artifact", "GET /documents/{documentId}/content"),
  ("artifact", "GET /shares"),
  ("artifact", "POST /shares"),
  ("receiver", "GET /owner"),
  ("receiver", "POST /instructions"),
  ("tool-host", "GET /namespaces/{namespace}/tools"),
  ("tool-host", "POST /tool-sessions"),
  ("tool-host", "POST /tool-calls"),
  ("tool-host", "DELETE /tool-sessions/{token}"),
];

/// The unit's OpenAPI document.
pub fn openapi() -> Value {
  let mut generator = SchemaSettings::draft2020_12()
    .for_serialize()
    .with(|settings| {
      settings.definitions_path = "/components/schemas".into();
      settings.meta_schema = None;
    })
    .into_generator();
  let error = generator.subschema_for::<Failure>();

  let mut tags: Vec<&str> = Vec::new();
  let mut paths: Map<String, Value> = Map::new();
  for operation in OPERATIONS {
    if !tags.contains(&operation.tag) {
      tags.push(operation.tag);
    }
    let mut answers: Map<String, Value> = Map::new();
    let (status, answer) = operation.answer;
    let mut stated = json!({ "description": "Answered." });
    if let Some(name) = answer {
      stated["content"] = json!({ "application/json": { "schema": schema(&mut generator, name) } });
    }
    answers.insert(status.to_string(), stated);
    let mut codes = vec!["unauthorized"];
    codes.extend(operation.codes.iter().copied());
    for code in &codes {
      answers
        .entry(status_of(code).to_string())
        .or_insert_with(|| json!({ "$ref": format!("#/components/responses/{}", response_name(code)) }));
    }
    let mut member = json!({
      "operationId": operation.id,
      "tags": [operation.tag],
      "summary": operation.summary,
      "responses": answers,
      "x-codes": codes,
    });
    let parameters: Vec<Value> = operation
      .params
      .iter()
      .map(|p| {
        json!({
          "name": p.name,
          "in": p.place,
          "required": p.place == "path",
          "schema": { "type": p.kind, "description": p.description },
        })
      })
      .collect();
    if !parameters.is_empty() {
      member["parameters"] = Value::Array(parameters);
    }
    if let Some(name) = operation.body {
      member["requestBody"] = json!({
        "required": true,
        "content": { "application/json": { "schema": schema(&mut generator, name) } },
      });
    }
    paths.entry(operation.path.to_owned()).or_insert_with(|| json!({}))[operation.method.to_lowercase()] =
      member;
  }

  let mut codes: Vec<&str> = OPERATIONS.iter().flat_map(|o| o.codes.iter().copied()).collect();
  codes.push("unauthorized");
  codes.sort_unstable();
  codes.dedup();
  let mut responses = Map::new();
  for code in codes {
    responses.insert(
      response_name(code),
      json!({ "description": code, "content": { "application/json": { "schema": error } } }),
    );
  }

  json!({
    "openapi": "3.1.0",
    "info": { "title": "bmo-research", "version": env!("CARGO_PKG_VERSION"), "description": DESCRIPTION },
    "servers": [{ "url": "/", "description": "the research daemon" }],
    "security": [{ "bearer": [] }],
    "x-layer": "downstream",
    "x-calls": CALLS.iter().map(|(unit, route)| json!({ "unit": unit, "route": route })).collect::<Vec<_>>(),
    "x-events": [],
    "tags": tags.iter().map(|tag| json!({ "name": tag })).collect::<Vec<_>>(),
    "paths": paths,
    "components": {
      "schemas": generator.take_definitions(true),
      "responses": responses,
      "securitySchemes": {
        "bearer": {
          "type": "http",
          "scheme": "bearer",
          "description": "The daemon's token, from the file its settings name under the data root's env/.",
        },
      },
    },
  })
}

/// The document as the repository keeps it: two-space indented JSON with a closing newline.
pub fn openapi_json() -> String {
  let mut text = serde_json::to_string_pretty(&openapi()).expect("a JSON value always renders");
  text.push('\n');
  text
}
