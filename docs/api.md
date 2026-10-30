# The boundary API

This document is bmo-research's boundary: every route the daemon answers, the tools the tool host places
for it, and every call it makes into another unit. A caller reaches the daemon only across this boundary.
`docs/design.md` states what the daemon is for; this document states the contract.

`docs/openapi.json` is the machine contract: every route's parameters, bodies, answers and codes. It is
generated from the client crate's own types by `bmo-research openapi`, and a test fails where the committed
document differs from what the code generates. Where a field's exact shape matters, the OpenAPI document is
the one to read.

## The shapes the boundary is made of

- **A route** is an operation the daemon answers: a verb and a path, a request body and a response body,
  and the status codes and kebab-case outcome codes it answers with.
- **An event** is something a unit publishes as it happens. The daemon publishes none and serves no
  `/events` stream; a reader polls the collections.
- **A tool** is an entry the tool host places under the namespace `research`, one executable answering
  every entry by calling the routes below (「The tools」).
- **A client** is the Rust crate `bmo-research-client` this repository publishes, carrying the types of
  the boundary and a client calling every route.

## Versioning

- The contract carries the repository's one version; the workspace manifest is its source. It moves only
  at a release the owner declares by name (bmo-chat's `docs/conventions.md`, 「Versions」); a deployment is
  told apart by its commit.
- A change to a route or a body repairs this document and `docs/openapi.json` in the same commit.

## The wire format

- Every route is answered on the loopback address `listen` names (`docs/settings.md`), under
  `Authorization: Bearer <token>` with the token the file `token_file` names holds; the daemon writes
  that file at its first start where it does not stand, with a fresh random token, readable by its
  account alone. A request without the token is answered 401.
- A route takes and answers JSON, keys in camelCase, times in epoch milliseconds, ids as signed 64-bit
  integers. A list is a page, `{"items": [...], "more": boolean}`, asked with `after` or `before` and
  `limit` (50 where it names none, at most 200).
- A failure is a non-2xx status with `code` and `error`:

| Status | `code` | When |
|---|---|---|
| 400 | `bad-request` | the body or the query is not the shape the route takes; the sentence names what is wrong |
| 401 | `unauthorized` | the token is missing or wrong |
| 404 | `no-route` | no route answers this method and path |
| 404 | `not-found` | nothing stands at the key the path names |
| 409 | `conflict` | the member's state does not allow the change, or the key is taken |
| 500 | `failed` | the daemon failed; the sentence is the failure |
| 502 | `upstream-failed` | a unit the daemon called failed; the sentence names it |

## Routes by resource

### health

| Method | Path | Does |
|---|---|---|
| GET | `/health` | `ready`, `reason` (null when ready) and `version`. |

### state

| Method | Path | Does |
|---|---|---|
| GET | `/state` | Whether the loop runs, each category's state and its part of the queue. |
| PATCH | `/state` | `state`: `stopped` or `running`, for every category or for the one `categoryName` names. |

- **The state** answers `state` (`stopped` where every category is stopped, `running` otherwise),
  `changedAt` (the latest change of any category's state, null where none was changed) and `categories`,
  each `categoryName`, `state`, `waitingCount`, `runningCount` and `cap`.
- **A stopped category starts no run** and its sources are not read; a run under way is waited on to its
  end and reported as any other. `running` starts it again. A category's state outlives a restart; a
  category starts running. A `categoryName` no category holds is 404 `not-found`.

### categories

| Method | Path | Does |
|---|---|---|
| GET | `/categories` | The categories by name. |
| POST | `/categories` | Register a category; 201 with the category, its first recipient the owner's private room. |
| GET | `/categories/{categoryName}` | One category. |
| PATCH | `/categories/{categoryName}` | Change `purpose`, `cap`, `cadenceMinutes`, `gatherMinutes`, `provider` or `model`. |
| DELETE | `/categories/{categoryName}` | Take a category away with its recipients; 204. |

- **A category** (`docs/design.md`, 「Categories」) is `name`, `purpose` (a line), `shape`, `cap`,
  `cadenceMinutes` (how often its sources are read where a source names no cadence of its own),
  `gatherMinutes` (for a `brief` category, how often its items are gathered into one run; null for the
  others), `provider` and `model` (its road, null where the settings' `road` stands), `state`
  (`running` or `stopped`), `stateAt` and `createdAt`.
- **Its shape** says what a run of its items is asked for (「The questions」): `manual-or-investigation`,
  each item as a manual or an investigation by its kind; `advisory`, each item as a security advisory
  judged against the stack (「stack」); `brief`, every waiting item of the category gathered into one brief
  once `gatherMinutes` have passed since the last.
- **Three stand from the store's first start:** `it` (`manual-or-investigation`, cap 5), `security`
  (`advisory`, cap 3) and `economy` (`brief`, cap 30, gathered every 1440 minutes), each read every 60
  minutes, on the settings' road, with no source. A category taken away is not brought back.
- **A new category** names `name` (a lower-case letter, then lower-case letters, digits and `-`, at most 32),
  `purpose` and `shape`, and may name `cap` (5 where it names none), `cadenceMinutes` (60), `gatherMinutes`
  (1440 for a `brief` category, refused for another), `provider` and `model`. A name taken is 409
  `conflict`; a shape outside the three, or a number below 1, is 400 `bad-request`.
- **A change** leaves out what it keeps; an empty `provider` or `model` gives the settings' road back.
- **A category a source or an item waiting or running still names** is not taken away: 409 `conflict`.

### sources

| Method | Path | Does |
|---|---|---|
| GET | `/sources` | The registered sources by id, narrowed by `categoryName`, paged by `after`. |
| POST | `/sources` | Register a source; 201 with the source. |
| GET | `/sources/{sourceId}` | One source. |
| PATCH | `/sources/{sourceId}` | Change `address`, `query`, `filter`, `everyMinutes` or `limit`. |
| DELETE | `/sources/{sourceId}` | Take the source away; 204. What it turned up stays. |

- **A source** is `id`, `categoryName`, `kind`, `address`, `query` (null where the kind takes none),
  `filter` (`require` and `exclude`, lists of words), `everyMinutes` (null where its category's cadence
  stands), `limit` (how many items one reading asks for), `failureCount`, `lastError`, `readAt`,
  `createdAt` and `changedAt`.
- **A new source** names `categoryName` and `kind` and, where the kind takes them, `address`, `query`,
  `filter`, `everyMinutes` and `limit`. A category that does not stand is 404 `not-found`; a kind outside
  `docs/sources.md`, a kind that needs a query given none, an `everyMinutes` or a `limit` below 1 is 400
  `bad-request`. An address left out is the kind's own.
- **`failureCount`** counts the readings that failed since the last that did; `lastError` is the last
  failure's sentence, null once a reading succeeds.

### queue

| Method | Path | Does |
|---|---|---|
| GET | `/queue` | The items in the queue's order, narrowed by `categoryName` and `state` (a comma-separated list), paged by `after`. |
| POST | `/queue` | Ask for a thing by its name and its maker's address; 201 with the item. |
| GET | `/queue/{itemId}` | One item. |
| PATCH | `/queue/{itemId}` | Dismiss it, mark it taken, or move it to the front. |

- **An item** is `id`, `categoryName`, `key` (the canonical address, else the name lower-cased), `name`,
  `maker`, `address` (the maker's page for the thing), `kind` (`model`, `paper`, `release`, `entry`, or
  `asked` for an ask), `shape` (`manual`, `investigation`, `advisory` or `brief`), `sourceId`, `asked`,
  `priority`, `state`, `tries`, `retryAt`, `runId` (agency's ledger run of the latest try), `documentId`,
  `title` and `description` (the document's), `shareUrl`, `koDocumentId` and `koShareUrl` (the Korean
  rendering's document and its live link, null until it is filed, 「The Korean rendering」), `fit`, `fitNodes` (the nodes of the settings'
  `model_nodes` a manual's fit names), `touches` (the hosts and repositories an advisory names), `plan`
  (the heading of the document's plan section, as the document holds it), `jobItemId` (the job item the
  chat agent filed for it), `error`, `foundAt`, `createdAt` and `changedAt`.
- **Its states:** `waiting` until a run is asked for it; `running` while one is under way; `reported` once
  its document is told to its category's recipients, or `fit` where the document asks the owner's word (「The fit」);
  `failed` after its last try; `taken` once the chat agent filed the work it calls for; `dismissed` on the
  owner's word. A dismissed item's key stays seen, so nothing turns it up again.
- **The queue's order** is the order runs are taken in, within a category: `priority` from highest, then
  `foundAt` from oldest, then `id`. Between categories, the next run goes to the highest priority waiting,
  and among equals to the category whose last run was asked longest ago, so no category's flood starves
  another. `GET /queue` answers the running items, then the waiting ones in their category's order, then
  the rest newest first; `after` continues past the named item's place in it.
- **The cap.** A category's part of the queue holds at most its `cap` items `waiting` or `running` that the
  scan took in; an ask is never refused for the cap. A slot that frees is filled from the category's seen
  candidates no item stands for, oldest first (`docs/sources.md`, 「From a candidate to an item」).
- **An ask** names `name` and `address`, and may name `categoryName` (`it` where it names none) and `shape`
  (`manual` or `investigation`, for a `manual-or-investigation` category alone; `manual` where it names
  none); it is queued at once with `asked` true and a `priority` of 1, above every item a scan took in (0).
  An empty name or address, or a shape the category does not take, is 400 `bad-request`; a category that
  does not stand is 404 `not-found`; a key an item still `waiting` or `running` holds is 409 `conflict`.
  The archive is not asked: the owner's word stands over it.
- **A change** is a merge patch of `state` or `front`; the daemon files nothing and launches nothing, so a
  change only records what was decided elsewhere:
  - `state` `dismissed` dismisses an item in any state but `taken` and `dismissed`; a run under way goes
    on in agency, and its end is no longer told.
  - `state` `taken` with `jobItemId`, the number of the job item the chat agent filed on the owner's word,
    keeps that number on the item. Only a `reported` or `fit` item is taken; a `taken` naming no
    `jobItemId` is 400 `bad-request`.
  - `front` true gives a waiting item a `priority` one above the highest any item holds, so it is its
    category's next.
  A move the state does not allow is 409 `conflict`; a body naming neither is 400 `bad-request`.

### runs

| Method | Path | Does |
|---|---|---|
| GET | `/runs` | The tries newest first, narrowed by `itemId` or `categoryName`, paged by `before`. |
| GET | `/runs/{id}` | One try. |

- **A run here is the daemon's record of one try:** `id`, `categoryName`, `itemId` (the item it was
  asked for; a brief's first item), `itemIds` (every item it carries: one, or a brief's whole gathering),
  `try` (counted from 1), `runId` (agency's ledger run, null where the ask was refused), `state`
  (`running`, `closed`, `refused`, `cut` or `failed`), `outcome` (agency's, as its ledger closed the run),
  `error`, `startedAt` and `endedAt`.

### recipients

| Method | Path | Does |
|---|---|---|
| GET | `/categories/{categoryName}/recipients` | The category's recipients in order, the owner's room first. |
| POST | `/categories/{categoryName}/recipients` | Add a room or an agent; 201 with the recipient. |
| DELETE | `/categories/{categoryName}/recipients/{recipientId}` | Take a recipient away; 204. |

- **A recipient** (`docs/design.md`, 「The notifier」) is who hears a category's notices: `id`,
  `categoryName`, `kind` (`room` or `agent`), `roomId` (the receiver's room, for a room), `agentName` (an
  agent that answers no room, a worker, for an agent), `origin` (`owner` for the owner's private room,
  `added` otherwise) and `addedAt`.
- **A new recipient** names `kind` and the `roomId` or the `agentName` it takes. A room or an agent the
  category already holds is 409 `conflict`; a kind outside the two, or a recipient naming neither, is 400
  `bad-request`; a category that does not stand is 404 `not-found`.
- **A room is told** through the receiver's instruction route, which hands the notice to the room's
  answerer; **an agent is told** by a turn into its worker session through agency, the notice as the
  turn's prompt (「The notices」). What a recipient does with a notice is its own.
- **The owner's private room is every category's first.** As the daemon starts, it reads the receiver's
  `GET /owner` and adds that room to every category holding no recipient, and a category registered later
  is given it as it is made; a receiver that cannot be reached is said in the log and asked again at the
  next start.

### stack

| Method | Path | Does |
|---|---|---|
| GET | `/stack` | The platform's stack inventory, by component. |
| POST | `/stack` | Add a row; 201 with the row. |
| DELETE | `/stack/{id}` | Take a row away; 204. |

- **A row** is `id`, `component` (a distribution, a runtime, a library or a program), `version`, `place`
  (the host it runs on), `repository` (the repository that runs it, null where none), `worker` (the agent
  that keeps that repository, null where none) and `addedAt`. An empty component, or a row naming neither
  a place nor a repository, is 400 `bad-request`. The owner and the jobs keep it; the daemon reads it into
  every advisory's question, and a touched repository's worker is told its part (「The notices」).

### facts

| Method | Path | Does |
|---|---|---|
| GET | `/facts` | The figures the briefs kept, newest first, narrowed by `name`, paged by `before`. |

- **A fact** is `id`, `name` (an index, a rate, a price, a release), `value`, `unit`, `asOf` (the date the
  figure stands for, as the brief writes it), `sourceAddress`, `runId` (agency's ledger run whose brief held it),
  `documentId` and `createdAt`. A brief's figures are read from its figures table (「The brief's facts」).

## The deep dive

- **One run at a time,** `concurrency` at most, while the run's category runs. A try asks agency for a
  research run at the settings' `road.key`, on the category's provider and model (the settings' `road`
  where it names none), with the question its shape takes (「The questions」) and no room, keeps the run's
  id on its items and its run row, and reads agency's `GET /runs/{runId}` every `poll_seconds` until the
  run's `state` is `closed`.
- **A run that closed** with the outcome `cut` is a failed try. Any other outcome ends the try once the
  artifact service answers the document filed under the label `research` with the run's id; a closed run
  with no such document is a failed try.
- **A failed try** — an ask agency refused (503 or any failure), a cut run, a missing document — puts its
  items back `waiting` with `retryAt` `retry_minutes` ahead. After `retries` retries they are `failed` and
  every recipient of the category is told so once.
- **A brief** gathers, once its category's `gatherMinutes` have passed since its last run was asked, every
  waiting item of the category, at most its `cap`, into one run; a gathering whose retry is due is asked
  at once.
- **A daemon that starts** takes up every run it left `running`: one with a run id is polled again, one
  without puts its items back `waiting`.
- **A filed document is rendered in Korean** before it is told (「The Korean rendering」): its items are
  filed `reported` or `fit` first, then the rendering is asked, then the recipients are told.

### The questions

The templates under `prompts/` are in English, one per item shape, each naming its shape's rulebook so the
search runner applies it, and the run writes its document in English, the question's language:

- `manual.md` and `investigation.md`, a `manual-or-investigation` category's: a `model` item is asked as a
  manual (the rulebook `research.md`, 「The manual」), and a `paper`, `release` or `entry` item as an
  investigation (`research.md`, 「The investigation」); an ask names its own. The item's `{name}` and
  `{address}` are set in. The manual's question names the headings and the label the fit is read by.
- `advisory.md`, an `advisory` category's (`security-advisory.md`): the item's `{name}` and `{address}`,
  and `{stack}`, the stack inventory a row a line. It asks for the exposure as a table naming each
  component's repository.
- `brief.md`, a `brief` category's (`economy-brief.md`): `{items}`, every gathered item's name and address
  a line.

### The fit

- **The words a document is read by** are those `prompts/document.toml` holds, one set per language for
  each shape: English, the language every run is asked in, and Korean, the language the rulebooks name
  their sections in. A reading looks for either language's heading or table and reads by the set that
  finds it, English first; a heading, a column, a label and a cell's opening word are matched without
  regard to case.
- **A manual is read for whether it asks the owner's word,** by the words under `[[manual]]`, those of the
  research rulebook's section on running the thing in the lab and its line of the cards it fits: under
  the heading `heading` names, the line opening with `label` that names anything but the `none` word
  marks the item `fit`.
- **An advisory is read for what it touches,** by the words under `[[advisory]]`: the exposure table, the
  table whose header holds its column words for the component, the version, the repository, the host and
  whether it is touched. Every row whose touched cell opens with the `yes` word is a touch; the item keeps
  the repositories and hosts touched, joined, as `touches`, and is `fit` where any row is touched.
- **An investigation and a brief are never marked `fit`.**
- **The reading is a hint, and the owner decides.** It reads the line's words and judges nothing. Each node
  of the settings' `model_nodes` standing in a manual's line as a word of its own is kept in `fitNodes`.
- **The plan's place** is the heading of the section `plan_heading` names for the shape (the reproduction
  plan of a manual, the patch steps of an advisory), as the document holds it, kept as `plan`.

### The brief's facts

- **A brief's figures table** is the table whose header holds the column words `prompts/document.toml`
  names under `[[brief]]`: the figure's name, its value, its unit, the date it stands for and its source.
  Each row naming a name and a value is kept as a fact; a source cell holding a link keeps the link's
  address. A brief with no such table keeps none.

### The Korean rendering

- **The English document is rendered in Korean by agency's translator** (`POST /translation-runs`) with
  `purpose` `document` and `language` the one `prompts/translation.toml` names, `Korean`, one text at a
  time: the document's title as a part of its own, with the `title_note`; its description, where it has
  one, with the `note`; then its body.
- **The body is cut at its headings** into parts no longer than `translation.part_chars` characters
  (`docs/settings.md`): whole sections are gathered into a part while they fit, and a section longer than
  that is halved, at a heading, else a blank line, else a line's end, else a space, nearest its middle,
  until each half fits. At most `translation.parts_out` parts are out at once, and the translations are
  joined in the body's order, a blank line between two.
- **Each part carries the `note`,** the item's name set in: the translator's context, that the text is
  part of a research report about the thing for the owner, that model, product and dataset names, figures,
  units, code, links and the fixed section names stay as written, and the register of the owner's
  documents.
- **A part refused 413 `too-large`** is halved the same way and each half asked again; a part that cannot
  be halved any further, a 503 `no-login-open`, or any other failure leaves the whole rendering pending.
- **The rendering is filed as a document of its own** with the artifact service's `POST /documents`:
  `kind` `record`, `title` the translated title, `body` the joined parts, `description` the translated
  description, `actorName` `bmo-research`, and `labels` `research` (the item's id), `translationOf` (the
  English document's id) and `language` `ko`; its share link is made as the English one's is. Every item
  the English document was filed for keeps the Korean document as `koDocumentId` and its link as
  `koShareUrl`.
- **A pending rendering is asked again** on the retry wait, `retry_minutes` after its last try, while its
  item stands `reported`, `fit` or `taken`. Its tries are counted apart from the run's tries; after
  `retries` retries it is no longer asked, and the English link stands alone.

### The notices

- **Every recipient of the category is told every notice:** a report, with the words
  `prompts/report.toml` holds as `line`, the Korean document's title, the first sentence of its
  description and its share link set in, then `english`, the English document's share link set in; or a
  failed try's last, `failed`, with the name and the tries set in.
- **A report whose rendering is pending** is told with `line` naming the English document's title,
  description and link, then `pending`. Once the rendering is filed on a later try, every recipient is told
  a follow-up under the same kind, so the receiver replaces the report where it still waits: `translated`,
  the Korean title and link set in, then `english`, and for a `fit` item the lines that act on it below.
- **A room** is told with the receiver's `POST /instructions`: `room`, `sort` `notice`, `kind`
  `research-report:<itemId>` (a brief's first item), `said` `{"order": "<the notice>"}` and `summary`.
- **An agent** is told with agency's `POST /sessions/<agentName>@worker/turns`, the notice as `prompt`.
  The daemon waits at most `turn_wait_seconds` for the answer; a turn still working then is counted as
  handed, since agency carries a sent turn to its end.
- **A `fit` item's notice carries what its recipient needs to act,** after the line: what its shape says
  (`fit` for a manual, the question for the owner's word; `touches` for an advisory, what it touches set
  in), the thing's name (`name`), and the plan's place (`plan`, the heading set in); and for a manual whose
  `fitNodes` is not empty, `bench`, those nodes set in: the bench must be carried by a worker that does not
  stand on them, since the bench evicts the model the worker would run on. The chat agent files what the
  owner's word calls for, and marks the item `taken` or `dismissed`.
- **The share link** is the document's live link from the artifact service's `GET /shares`, or one made
  with `POST /shares` where none stands.
- **An advisory's touched repositories are told their own part.** For each repository the exposure table
  touches, every worker the stack names for that repository is told, by a turn as an agent recipient is,
  the words `worker` holds: the advisory's name, the repository, its touched rows, the English document's
  share link and the plan's place. Workers read English, so the part names no Korean rendering. The
  category's recipients are told the whole; a repository no stack row names a worker for is heard by them
  alone.
- **A recipient that refuses** is said in the log and kept as a report row in the state `refused`; the
  others are told all the same.

## The tools

The tool host places `tools/research` under the namespace `research`: one executable, speaking the host's
call protocol 1, answering each entry by calling the routes above at the address and with the token the
daemon's settings name, read under the data root the session record's `BMO_DATA_DIR` names.

| Entry | Arguments | Calls | Capability |
|---|---|---|---|
| `research/ask` | `name`, `address`, `category?`, `shape?` | `POST /queue` | `research.ask` |
| `research/queue` | `category?`, `state?` | `GET /queue` | `research.ask` |
| `research/taken` | `item`, `job` | `PATCH /queue/{itemId}` | `research.owner` |
| `research/dismiss` | `item` | `PATCH /queue/{itemId}` | `research.owner` |
| `research/categories` | `action` (`list`, `add`, `remove`, `change`) and the category's fields | `GET`, `POST`, `PATCH`, `DELETE /categories` | `research.ask`; `add`, `remove` and `change` also `research.owner` |
| `research/sources` | `action` (`list`, `add`, `remove`) and the source's fields | `GET`, `POST`, `DELETE /sources` | `research.ask`; `add` and `remove` also `research.owner` |
| `research/recipients` | `action` (`list`, `add`, `remove`), `category`, `room?` or `agent?` | `GET`, `POST`, `DELETE /categories/{categoryName}/recipients` | `research.ask`; `add` and `remove` also `research.owner` |
| `research/rooms` | `action` (`list`, `add`, `remove`), `category`, `room?` | the same, for the recipients that are rooms | `research.ask`; `add` and `remove` also `research.owner` |
| `research/stop` | `category?` | `PATCH /state` | `research.owner` |
| `research/resume` | `category?` | `PATCH /state` | `research.owner` |

- **An entry that both reads and changes** is shown to a session holding `research.ask`, and the tool
  refuses its changing actions to a session whose record does not also hold `research.owner`.
- **Each answers a short text** for the model to read, and a refusal as a result marked as an error.

## The command

`bmo-research` serves and answers the deployment: `serve` runs the daemon; `health` answers 0 once the
daemon answers ready; `openapi` prints the OpenAPI document; `migrate` brings the store's schema to the
version the binary holds and says which; `nouns` prints `research`, the noun `bmo research …` hands it.
Under `research` stand `state`, `stop`, `resume`, `categories`, `queue`, `ask`, `taken`, `dismiss`,
`front`, `sources`, `recipients`, `runs`, `stack` and `facts`, each calling the route of the same name through the client; `--json` prints
the member or the page exactly as the API answers it.

## Calls this unit makes

Each is the documented route of the unit that holds what must reach it; the daemon keeps no client crate
of these units, and reads each one's address and token from that unit's own files under the data root's
`env/`, named by the settings (`docs/settings.md`, 「Peers」).

- **agency:** `POST /research-runs` with `key`, `query`, `provider` and `model`, and no `roomId`; answered
  202 with `runId`, or 503 where no road carries it. `GET /runs/{runId}`, read for `state` and `outcome`.
  `POST /sessions/{agentName}@worker/turns` with `prompt`, a notice to an agent. `POST /translation-runs`
  with `purpose`, `language`, `text` and `note`, one part of a Korean rendering; answered 200 with `text`,
  413 `too-large` or 503 `no-login-open`.
- **the artifact service:** `GET /documents?q=<name>&limit=1`, the archive check a candidate passes
  (`docs/sources.md`); `GET /documents?label=research:<runId>&limit=20`, the document a run filed, the
  first carrying no `translationOf` label; `GET /documents/{documentId}/content`, its text;
  `GET /shares?documentId=<documentId>` and `POST /shares` with `documentId`, its live link;
  `POST /documents`, a Korean rendering.
- **the receiver:** `GET /owner`, the owner's private room; `POST /instructions`, a report line.
- **the arbiter:** no route of it is called. The bench lock of a verification is the job's, and the
  daemon's pause while a bench holds a node its model stands on is not built (`docs/design.md`, 「Phases」).
- **The job service is never called:** the work a report calls for is the chat agent's to file.
- **the sources:** each source's own address, read with the daemon's HTTP client (`docs/sources.md`).
