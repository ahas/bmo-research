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
| 409 | `branch-refused` | a branch is not opened; the sentence says why (「research runs」) |
| 500 | `failed` | the daemon failed; the sentence is the failure |
| 502 | `upstream-failed` | a unit the daemon called failed, or refused what it was asked; the sentence names it |
| 503 | `unavailable` | a research run cannot go out now: agency could not be reached or failed on its side for its slot, or the tool host gave no web listing; the sentence is theirs |

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
  each `categoryName`, `state`, `waitingCount` (its items `waiting`, in the queue for a place),
  `runningCount` (its items `running`, carried by a try whose run holds a place), `finishingCount` (its
  items whose page is finished before they are told) and `stuckCount` (its items stuck, 「The repair」), and
  `holds`, the places asked that wait for their engine now (「The engine」), each `reason`
  (`waiting-card`), `place` (the place's request key), `categoryNames` (the running categories on the
  place's road), `registration`, `serviceName` (the arbiter service whose card it waits for), `demandId`
  (null until the master has answered the ask), `cause` and `causeText` (the master's reason the card
  waits — `starting`, `in-use`, `held` or `start-failed` — and its sentence, null where it gave none),
  `since` (when the demand was asked) and `reported` (whether the owner's room was told the engine has not
  come); empty where nothing waits. The queue has no cap, and none is answered.
- **A place waiting for its card or its slot holds no item:** the items it may take stand `waiting` and
  are counted so, and its category's sources are read and its part of the queue grows as ever.
- **A stopped category starts no run** and its sources are not read; no place takes its items, which stay
  `waiting`, and a place waiting on a road with no running category's try due gives itself back. A run
  under way is waited on to its end and reported as any other. `running` starts it again, its items pulled
  at once. A category's state outlives a restart; a category starts running. A `categoryName` no category
  holds is 404 `not-found`.

### categories

| Method | Path | Does |
|---|---|---|
| GET | `/categories` | The categories by name. |
| POST | `/categories` | Register a category; 201 with the category, its first recipient the owner's private room. |
| GET | `/categories/{categoryName}` | One category. |
| PATCH | `/categories/{categoryName}` | Change `purpose`, `cadenceMinutes`, `gatherMinutes`, `provider`, `model`, `instruction` or `calendar`. |
| DELETE | `/categories/{categoryName}` | Take a category away with its recipients; 204. |

- **A category** (`docs/design.md`, 「Categories」) is `name`, `purpose` (a line), `shape`,
  `cadenceMinutes` (how often its sources are read where a source names no cadence of its own),
  `gatherMinutes` (for a `brief` or a `topic` category, how often it is gathered into one run; null for the
  others), `provider` and `model` (its road, null where the settings' `road` stands), `instruction` (a
  `topic` category's own question, null for another shape), `calendar` (the calendar a `topic` category's
  dated findings are written to, `realm` — `mine` or `organization` — and `calendarId`; null where it
  writes none), `state` (`running` or `stopped`), `stateAt` and `createdAt`.
- **Its shape** says what a run of its items is asked for (「The questions」): `manual-or-investigation`,
  each item as a manual or an investigation by its kind; `advisory`, each item as a security advisory
  judged against the stack (「stack」); `brief`, every waiting item of the category gathered into one brief
  once `gatherMinutes` have passed since the last; `topic`, one run asked the category's own `instruction`
  once `gatherMinutes` have passed since the last, with every waiting item of the category under it, and on
  the instruction alone where none waits (`docs/design.md`, 「Categories」).
- **Three stand from the store's first start:** `it` (`manual-or-investigation`), `security`
  (`advisory`) and `economy` (`brief`, gathered every 1440 minutes), each read every 60
  minutes, on the settings' road, with no source. A category taken away is not brought back.
- **A new category** names `name` (a lower-case letter, then lower-case letters, digits and `-`, at most 32),
  `purpose` and `shape`, and may name `cadenceMinutes` (60), `gatherMinutes`
  (1440 for a `brief` or a `topic` category, refused for another), `provider` and `model`. A `topic`
  category names its `instruction` and may name its `calendar`; either named for another shape, a `topic`
  with no instruction, or a `calendar` whose realm is neither `mine` nor `organization` or whose
  `calendarId` is empty, is 400 `bad-request`. A name taken is 409
  `conflict`; a shape outside the four, or a number below 1, is 400 `bad-request`.
- **A change** leaves out what it keeps; an empty `provider` or `model` gives the settings' road back, and
  a `calendar` given as null stops a topic writing to one. A topic's instruction is never emptied.
- **A category a source or an item waiting or running still names** is not taken away: 409 `conflict`.

### sources

| Method | Path | Does |
|---|---|---|
| GET | `/sources` | The registered sources by id, narrowed by `categoryName`, paged by `after`. |
| POST | `/sources` | Register a source; 201 with the source. |
| GET | `/sources/{sourceId}` | One source. |
| PATCH | `/sources/{sourceId}` | Change `address`, `query`, `pattern`, `strategy`, `filter`, `everyMinutes` or `limit`. |
| DELETE | `/sources/{sourceId}` | Take the source away; 204. What it turned up stays. |

- **A source** is `id`, `categoryName`, `kind`, `address`, `query` (null where the kind takes none),
  `pattern` and `strategy` (a `page` source's link pattern and how its listing is read, `direct` or
  `browser`; null for another kind), `filter` (`require` and `exclude`, lists of words), `everyMinutes`
  (null where its category's cadence stands), `limit` (how many items one reading asks for),
  `failureCount`, `lastError`, `readAt`, `scanCount`, `lastYield`, `lastTaken`, `createdAt` and `changedAt`.
- **A new source** names `categoryName` and `kind` and, where the kind takes them, `address`, `query`,
  `pattern`, `strategy`, `filter`, `everyMinutes` and `limit`. A category that does not stand is 404
  `not-found`; a kind outside `docs/sources.md`, a kind that needs a query given none, an `everyMinutes` or
  a `limit` below 1 is 400 `bad-request`. An address left out is the kind's own.
- **A `page` source** names its `address` and a `pattern` (`docs/sources.md`, 「A page」), and may name its
  `strategy`, `direct` where it names none. A `page` source with no pattern, a pattern that is no path
  prefix and does not read as a regular expression, a strategy other than `direct` or `browser`, or a
  `pattern` or a `strategy` named for another kind, is 400 `bad-request`, on a change as on a new source.
- **What its readings left** (`docs/sources.md`, 「What a reading leaves on its source」): `readAt` is the
  last reading's time, null before the first; `scanCount` counts every reading, failed or not;
  `lastYield` is how many candidates the last reading that succeeded turned up past the filter, and
  `lastTaken` how many of them it kept as new, each queued, both null before a
  reading first succeeds; `failureCount` counts the readings that failed since the last that did not, and
  `lastError` is the last reading's failure sentence, null when the last reading succeeded.

### queue

| Method | Path | Does |
|---|---|---|
| GET | `/queue` | The items in the queue's order, narrowed by `categoryName` and `state` (a comma-separated list), paged by `after`. |
| POST | `/queue` | Ask for a thing by its name and its maker's address; 201 with the item. |
| GET | `/queue/{itemId}` | One item. |
| PATCH | `/queue/{itemId}` | Dismiss it, mark it taken, ask a stuck one again, or move it to the front. |

- **An item** is `id`, `categoryName`, `key` (the canonical address, else the name lower-cased), `name`,
  `maker`, `address` (the maker's page for the thing), `kind` (`model`, `paper`, `release`, `entry`, or
  `asked` for an ask), `shape` (`manual`, `investigation`, `advisory` or `brief`), `sourceId`, `asked`,
  `origin` (where it came from, below), `priority`, `state`, `tries`, `retryAt`, `runId` (the research run
  of the latest try), `documentId`, `title` and `description` (the head the run gave its document, in
  English, the title a noun phrase naming the subject, never the verdict; `docs/design.md`, 「Reports and
  rooms」), `shareUrl` (the document's live link), `languages` (the languages the document's page holds,
  Korean first: `["en"]` until the Korean side is had, `["ko", "en"]` after, empty before a document is
  filed; 「The page」), `narrations` (the languages whose narration the page plays, Korean first), `pending`
  (what the page still lacks before its report is told: `korean` for its Korean side, `narration-ko` and
  `narration-en` for each narration its script allows, `review` for its review and `review-fixes` for the
  review's fixes not yet applied; empty where it lacks nothing), `pendingRetryAt` (when
  that is asked again, null where nothing is pending), `fit`, `touches` (the hosts and repositories an
  advisory names), `plan` (the heading of the document's plan section, as the document holds it),
  `jobItemId` (the job item the chat agent filed for it), `repairItemId` (the job item filed for the repair worker
  while it is stuck, 「The repair」; null where none was), `error` (why its latest try failed or was
  interrupted, or the floor a skipped advisory was judged against: not how the item stands now; null while a
  try is under way and after one that filed its document, until that document is gone, which it then names;
  「The page」), `errorAt` (when that try ended, null where `error` is), `foundAt`, `createdAt` and
  `changedAt`.
- **Where an item came from** is its `origin`, an object whose `kind` is one of three:
  - `source`, an item a scan took in: `source` names the source that found it, by its kind and what it
    reads (its query, else its address), null where that source no longer stands;
  - `asked`, the owner's ask: `because` holds the words the ask was made with, null where it named none;
  - `follow-up`, an ask made from an earlier research: `itemId` and `documentId` name the item and the
    document it follows from, `title` that document's title and `shareUrl` its live link, as they stood when
    the ask was made, and `because` the sentence saying why this one was asked.
  The fields a kind does not use are null.
- **Its states:** `waiting` until a run is asked for it; `running` while one is under way; `finishing` once
  its run has filed its document and while its page is not yet whole, `pending` naming what it waits on and
  nobody told of it yet (「The page」); `reported` once its whole page is told to its category's
  recipients, or `fit` where the document asks the owner's word (「The fit」); `stuck` once its failed
  tries pass `retries`, still asked again on every retry wait while the repair worker is asked to fix the cause
  (「The repair」); `taken` once the chat agent filed the work it calls for; `dismissed` on the owner's
  word; `skipped` once an advisory's run judged its fixed builds older than the platform's age floor before
  researching it, with no document and no notice, `error` the one line naming the floor
  (「The deep dive」). No count of tries gives an item up: dismissal and a skip are the ways an item leaves the
  queue unreported. A dismissed item's key stays seen, so nothing turns it up again; a skipped one stays seen
  too.
- **The queue's order** is the order runs are taken in, within a category: `priority` from highest, then
  `foundAt` from oldest, then `id`. Between categories, the categories take turns: of those running with a
  try due (an item whose retry is due, or a brief's gathering that is due, 「The deep dive」), the next try
  goes to the one whose last try was asked longest ago, one never tried first, and the try it starts makes
  it the latest. A place let in (「The deep dive」) takes the next try of the categories on its road in
  this order. `priority` orders a category's own items and never crosses into another's turn, so no
  category's flood, nor its asks, starves another of the places. `GET /queue` answers the running items,
  then the waiting and stuck ones in their category's order, then the rest newest first; `after` continues
  past the named item's place in it.
- **No cap.** The queue holds every item the scan takes in and every ask, however many, whole and per
  category: a seen candidate no item stands for is queued `waiting` in the pass that keeps it, oldest first
  (`docs/sources.md`, 「From a candidate to an item」). An item is `waiting` until a place takes it and
  `running` only while its try's run holds that place (「The deep dive」).
- **An ask** names `name` and `address`, and may name `categoryName` (`it` where it names none) and `shape`
  (`manual` or `investigation`, for a `manual-or-investigation` category alone; `manual` where it names
  none); it is queued at once with `asked` true and a `priority` of 1, above every item a scan took in (0).
  An empty name or address, or a shape the category does not take, is 400 `bad-request`; a category that
  does not stand is 404 `not-found`; a key an item still `waiting`, `running` or `stuck` holds is 409
  `conflict`.
  The archive is not asked: the owner's word stands over it.
- **An ask may say why and from what:** `because`, the words it is made with, kept as its origin's; and,
  for a follow-up of an earlier research, `followsItemId`, the item it follows from, or
  `followsDocumentId`, the document. An ask naming either is a `follow-up`, any other an `asked`. The parent
  is read as the ask is made: a named item gives its document, its title (its name where it has none) and its
  live link; a named document gives those of the item that filed it, else those of the research run that
  filed it, else its number alone. An item that does not stand is 404 `not-found`, an item that has filed
  no document yet is 409 `conflict`, and an item and a document that are not each other's are 400
  `bad-request`.
- **A change** is a merge patch of `state` or `front`, recording what was decided elsewhere; it files
  nothing and launches nothing:
  - `state` `dismissed` dismisses an item in any state but `taken` and `dismissed`; a run under way goes
    on to its end, which is no longer told.
  - `state` `taken` with `jobItemId`, the number of the job item the chat agent filed on the owner's word,
    keeps that number on the item. Only a `reported` or `fit` item is taken; a `taken` naming no
    `jobItemId` is 400 `bad-request`.
  - `state` `waiting` asks a `stuck` item again once its cause is fixed: it waits with its tries counted
    from none, no retry wait, and no repair item. Only a stuck item is moved so.
  - `front` true gives a waiting or stuck item a `priority` one above the highest any item holds, so it is
    its category's next; a stuck one's retry wait is cleared with it.
  A move the state does not allow is 409 `conflict`; a body naming neither is 400 `bad-request`.

### runs

| Method | Path | Does |
|---|---|---|
| GET | `/runs` | The tries newest first, narrowed by `itemId` or `categoryName`, paged by `before`. |
| GET | `/runs/{id}` | One try. |

- **A run here is the daemon's record of one try:** `id`, `categoryName`, `itemId` (the item it was
  asked for; a brief's first item), `itemIds` (every item it carries: one, or a brief's whole gathering),
  `try` (counted from 1), `runId` (the research run, null where the ask was refused), `state`
  (`running`, `closed`, `refused`, `cut` or `failed`), `outcome` (the research run's, as it closed),
  `error`, `startedAt` and `endedAt`. A try is opened only once a place is let in for it (「The deep
  dive」), so a `running` try is one whose run holds or is about to take its place's slot.

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
  answerer; **an agent is told** by a run of agency's `worker` role at its repository's checkout, the
  notice as the turn's prompt (「The notices」). What a recipient does with a notice is its own.
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
  every advisory's question, together with the platform's deploy records (`docs/settings.md`, 「The stack」),
  and a touched repository's worker is told its part (「The notices」). The oldest recorded component is the
  age floor every advisory is judged against (「The questions」): at the scan and again in the run's scope turn
  before its research, so an advisory whose fixed builds predate the floor is skipped with no document and no
  notice.

### facts

| Method | Path | Does |
|---|---|---|
| GET | `/facts` | The figures the briefs kept, newest first, narrowed by `name`, paged by `before`. |

- **A fact** is `id`, `name` (an index, a rate, a price, a release), `value`, `unit`, `asOf` (the date the
  figure stands for, as the brief writes it), `sourceAddress`, `runId` (the research run whose brief held it),
  `documentId` and `createdAt`. A brief's figures are read from its figures table (「The brief's facts」).

### research runs

| Method | Path | Does |
|---|---|---|
| POST | `/research-runs` | Ask a research: a search at a depth or a browse; 202 with the run. |
| GET | `/research-runs/{runId}` | One research run. |
| GET | `/research-runs/{runId}/log-lines` | A research run's log, oldest first. |
| POST | `/branches` | Open a branch of a research run and answer once it ends. |

- **An ask** (`docs/runner.md`) names `key` (`search:basic`, `search:thorough`, `search:exhaustive`,
  `browse` or `references`) and `query`, and may name `asker`, `options` (text for a search, an object for a
  `references` run; 「The references dossier」), `roomId` (the room the result is told to; the owner's private
  room where none is named), `provider` and `model` (the road it goes out on: a `model` alone goes out on
  the provider an agent of its role holds it under, and where neither is named the run goes out on its slot's
  road, that of the agent of its role it is let in on; `docs/runner.md`, 「One run, step by step」). A key that is no research key, an
  empty query, a `provider` named without a `model` or a `references` ask whose options do not stand is 400
  `bad-request`. Where agency cannot be reached for the slot or the role's agents, or answers either with a
  failure of its own (5xx), or a search's or a references run's web listing cannot be had, it is 503
  `unavailable`; where agency refuses the slot or the role for what it asks, or answers the slot with no
  token, it is 502 `upstream-failed`. Either carries the sentence, and nothing is left standing.
- **A `references` ask** is the drawing service's, and its `options` object holds `subjectKey` (the
  subject's key in the brain), `kind` (`character`, `person`, `place`, `object`, `style`, `work` or
  `other`), `language` (`ko` or `en`, the language of the person who asked for the draw), `candidateLimit`
  (1 to 24, 12 where none) and `imageEngines` (a subset of `openverse`, `wikicommons.images` and `unsplash`,
  all where none); an option the key does not name is not read. The same JSON as a string is taken. No
  candidate is left out for its licence: the licence line is recorded as metadata.
  The run is filed as a `search` document whose body's last fenced block is the candidate dossier
  (`docs/runner.md`, 「The references dossier」); its labels are `research`, `key` and `category`
  (`references`), `query`, `languages`, `paint.subject-key` and `paint.schema` `references-v1`, and no room
  is told.
- **The ask is answered at once,** 202 with `runId`, `key`, `state` (`waiting` while its slot waits for its
  place, `running` once let in), `provider` and `model`, the road its slot stands on as it is answered — the
  one the ask named, else the one it was let in on where it is let in at once — each empty while a slot
  whose ask named none waits: the run's road stands on the run once its slot is let in. The run goes on in
  the background to its end, with no clock (`docs/runner.md`, 「Nothing of a run is clocked」); a session
  agency does not start for it ends it `cut` or `failed` (`docs/runner.md`, 「One run, step by step」).
- **A research run** is `runId`, `key`, `query`, `asker`, `roomId`, `provider`, `model`, `state` (`waiting`,
  `running` or `closed`), `outcome` (`answered`, `failed`, `skipped` where an advisory's fixed builds predate
  the age floor, or `cut` where what it stood on went away, `docs/runner.md`, 「A cut run」; null while open),
  `startedAt`, `finishedAt`, `documentId` and `link` (the filed document and its share link, null where none
  was filed), `error` (why it failed, or the floor a skipped advisory was judged against), `branches` (how
  many it opened), `turns` (the names of the agency turns it went out under, its first answer's, each
  round's and its page's review's, by which its tool calls are read in agency's log), `narrations` (the
  languages whose narration its document plays) and `pending` (what its document still lacks, as a page's
  `pending` names it: for a run told to its room, `narration-<code>` until its sound is attached, its room
  told once it lacks nothing, `docs/runner.md`, 「The narration's sound」; empty where it lacks nothing or no
  document was filed).
- **Its log** answers `items`, each `id`, `runId`, `at`, `kind` (`asked`, `admitted`, `answer`, `round`,
  `branch`, `whole`, `narration`, `filed`, `review`, `told` or `ended`) and `text`, the run's own notes from
  the ask to its end, and how its document was finished after it (「The page」): every narration sound the
  document was given or not, and why, under `narration`; why a deep dive's Korean side is pending, and what
  its report or its room's telling waits on each time that changes, under `filed`; what a deep dive's review
  answered and each fix applied, left out or still pending, under `review`; and when its report was told, or
  its room, under `told`.
- **A branch** is asked with the token the run's record names, `run` (the run's id) and `angle`, and
  answered 200 once it ends with `index` (counted from 1 per run), `state` (`completed` or `failed`),
  `notes` (what the branch wrote, empty where nothing) and `reason` (why, on `failed`; null otherwise). A
  run that does not stand is 404 `not-found`, an empty angle 400 `bad-request`. It is 409 `branch-refused`,
  the sentence saying why, where the run is no longer running, the run is allowed no branches, or it has
  opened every branch it is allowed (the sentence names how many); every ask past the first two checks
  counts against the allowance. Its slot is waited for however long its role's line takes, and a run that
  stops running meanwhile has the branch refused the same way, with nothing left standing. A caller that lets
  go ends its branch: its session ended once its turn is over, its slot given back and its demand with them.
- **A branch of a run holding a card** (「The engine」) asks its own demand first, for the service the
  engine watch maps the branch's model to, within the run's demand; it waits for the grant however long it
  takes, and a run that stops running meanwhile has it refused 409 `branch-refused` with nothing left
  standing; its slot and session stand on the grant.

### research settings

| Method | Path | Does |
|---|---|---|
| GET | `/research-settings` | The settings every research run is held to, with the search engines' state. |
| PUT | `/research-settings` | Change them; a field left out keeps what it holds. |

- **The settings** (`docs/runner.md`, 「What a run is held to」) are `branchRole`, `strategy`,
  `hostIntervalMs` and `engineIntervalMs` (null where the web tools' default stands), `keys` (each `key`,
  `branches`, `pages` and `branchPages`, the last two null for no ceiling), `source` (`default`
  where the settings file's stand, `set` once changed), `engines` (each `name`, `category`, `state`, `reason`,
  `observedAt` and `suspendedUntil`, as the web tools answer them; null where they could not be read) and
  `enginesUnread` (why not, or null).
- **A change** names any of `branchRole`, `strategy`, `hostIntervalMs`, `engineIntervalMs` (null returns to
  the web tools' default) and `keys`; a key's `pages` or `branchPages` left out keeps what it holds. A role
  agency's `GET /roles` does not list, a strategy outside the three, a gap below 0, a key that is no research
  key, branches below 0 or a budget under a page is 400 `bad-request`; it is answered
  with the settings as they now stand.

## The deep dive

- **A node pulls** (`docs/design.md`, 「A node pulls」). Each pull counts the tries due on each road the
  running categories go out on — each waiting or stuck item whose retry is due, a brief's gathering once it
  is due — and asks one place for each that no place already waiting covers, never more across every road
  than the cap agency's `GET /roles/{role}` answers for the search line's role (`maxConcurrent`) less the
  places the daemon holds or waits for; a role that cannot be read asks none, said in the log once while
  the failure stays the same. A place is a slot asked with `requestKey` `research:place:<key>`, a key drawn
  for it, `owner` `research:places`, `role` the search line's role, `source` the line, and the road's
  `provider` and `model` (「Calls this unit makes」), its card asked first where the road is watched
  (「The engine」), then asked for its place however long the line takes. Once it is let in it takes the
  next try on its road in the queue's order (「queue」) and opens the try with it; one let in with nothing
  left to take is given back, cancelled, with its card. One that waits while its road has no try due gives
  itself back the same way. A place agency gives no slot — agency not reached, failing on its side, or
  refusing what was asked — takes no item and opens no try, its card given back; it is said in the log once
  while the failure stays the same, and the next pull asks again. A pull is made on an event: a place let in, a scan's pass, an ask, an item asked
  again or moved to the front, a category resumed, and a try's run ending.
- **A try runs on its place.** It asks the runner
  (「research runs」, `docs/runner.md`) for a research run at the settings' `road.key`, on the category's
  provider and model (the settings' `road` where it names none), with the question its shape takes (「The
  questions」), no room and the place's slot and demand, which the run stands on and asks none of its own
  for (`docs/runner.md`, 「One run, step by step」), keeps the run's id on its items and its run row, and
  reads the run's row every `poll_seconds` until its `state` is `closed`. A run the deep dive asks tells no
  room itself. A try that asks no run — one that takes up a document an earlier run filed — gives its
  place's slot back, cancelled, as it starts.
- **An advisory's run is judged against the age floor before its research** (`docs/runner.md`, 「The deep
  dive's page」): the try hands the runner the floor its question names, the run asks one scope turn of the
  advisory's fixed builds, and a run older than the floor closes `skipped` with no shape asked and no
  document filed.
- **The run is held to its shape:** the try hands the runner the section headings its item's shape holds
  (`prompts/document.toml`, `[sections]`), and the runner files the result as a page only where it is whole:
  its head lines, its body opening with the answer, the opening section saying what the subject is where
  its shape holds one (`prompts/document.toml`, `[opening]`), and at least half of those headings in one
  language, the run's session asked once more for the whole document where they are short
  (`docs/runner.md`, 「The deep dive's page」). The try hands the runner the item's origin too, which the
  page prints under its title.
- **A try ends filed, skipped, interrupted or failed.** It is filed once the run's row names the document it
  filed; a run closed `skipped` ends the try `skipped`, its items `skipped` with the floor in their error and
  no document or notice standing, and no retry due.
- **An interrupted try** is one that ended because something the run needs could not be reached or was
  taken away, not because of what it asked or wrote: the runner refusing the ask 503 (`docs/runner.md`,
  「One run, step by step」), a run that closed `cut` (this unit's stop or restart, or agency failing or no longer
  holding the run's slot or session, `docs/runner.md`, 「A cut run」), or a try of a watched road whose card
  was taken or no longer stands (「The engine」). It is not counted toward `retries`, and its items go back
  as they stood — `waiting`, or `stuck` — with no retry wait (`retryAt` null), and a pull is made, so the
  next place let in takes them again, however long that takes. Its error stays
  on the items and on its try's row, saying what went away and that the try is not counted.
- **A failed try** is one that ended on its own content: the runner refusing the ask 400, 500 or 502 (an ask
  of the wrong shape, the runner's own failure, agency refusing what was asked), or a closed run with no
  document whose outcome is not `skipped`, its error the run's — the result still short of its shape once
  asked again whole, or a turn that wrote nothing while its slot stood. It counts toward `retries`: its items wait again with `retryAt`
  `retry_minutes` ahead. A failed try that takes the lead's tries past `retries` puts its items `stuck`
  (「The repair」), and every recipient of the category is told so once.
- **A brief** gathers every waiting or stuck item of its category whose retry is due into one run, once its category's `gatherMinutes` have passed since the last of its tries that filed its
  document was asked (at once where none has). A try of it that was interrupted or failed has filed nothing,
  so the gathering stays due: an interrupted one is taken again by the next place let in, as any
  interrupted try is, and a failed one once its retry wait ends, the gathering waiting while any item of it still waits on a
  retry so the items are not split.
- **A daemon that starts** closes every research run it left open as `cut` (`docs/runner.md`, 「A
  restart」) and gives back everything `research:places` holds of agency, every place it had asked, let in
  or waiting, then takes up every try it left `running`: one with a run id is read again, and ends as a
  cut run's try, interrupted; one without puts its items back as they stood, uncounted. Each item cut so
  stands `waiting` again, or `stuck` where its tries already passed `retries`, for the places its first
  pull asks. A page it left
  `finishing`, or with a part pending, is asked again on its retry wait as any pending page is (「The page」).
- **A filed document's page is finished before it is told** (「The page」): its items are filed `finishing`
  first, the page read from the run's result as the ledger keeps it (`written`, drawn as the runner drew
  it), then its Korean side and its narrations are asked, then its review and the review's fixes
  (`docs/runner.md`, 「The review」), and the page drawn again with what was had. Once the page is whole and
  reviewed its items are `reported` or `fit` and the recipients are told; until then its pending parts are
  asked again on every retry wait, each alone.
- **A try asked again continues from the run before it** where that run wrote a result for the same
  question (`docs/runner.md`, 「A run that continues」): the try before it that asked a run names it, and
  the try hands the runner that run, whatever the try before ended as. Where that run filed its document
  and the try failed after it — its share link not made, its result not read — the try asks no run and
  takes that document up as its own, so no document is filed twice.

### The repair

- **A stuck item is still asked:** on every retry wait, as a waiting item is, its tries counted on. It
  leaves `stuck` once a try files its document, once it is asked again (「queue」, `state` `waiting`), or
  once the owner dismisses it.
- **The repair worker is asked to fix the cause.** Each pass of the loop files, before it starts its tries, one
  repair item in the job service for the stuck items of each research run that hold none — one for the
  items a stuck brief gathered, one for each item stuck with no run — and keeps the job item's number on
  each as `repairItemId`. The item is `POST /items` with the settings' `[job]` `queue`, `assignee` and
  `labels` (`docs/settings.md`, 「Peers」): the settings' assignee as its worker, the research unit's own by default, under a label the deployment's job
  service opens unattended, so it needs no approval. Its title and brief are `prompts/repair.toml`'s, with
  the evidence set in: each item's number, name, address, category, shape, tries and last error; the
  research runs of its tries, each with its outcome and its error; and the last lines of its latest run's
  log. The brief tells the worker to find the cause — the question template, a rulebook, the runner, the
  page's shape or the engine's road — fix it where it lies, then ask the item again with `PATCH
  /queue/{itemId}` naming `state` `waiting`, or `front` true, and to check first with `GET /queue/{itemId}`
  that it is still stuck, since its next retry may have filed it.
- **A filing that fails is asked again on the next pass,** said in the log once while the failure stays
  the same.
- **One repair for each stuck stretch:** an item asked again holds no repair item, and is stuck anew only
  after `retries` more failed tries; one whose try files its document keeps its repair item's number.

### The engine

- **What is watched** is the settings' `[engine]` (`docs/settings.md`): the arbiter master's address, the
  provider, and each registration on it with the arbiter service its engine is. A master address left
  empty watches nothing: no demand is asked and every place asks its slot at once.
- **Every demand this unit's work asks stands in the engine watch's pool, at its rank:** `pool` and `rank` on the
  master's `POST /demands`, `cardPool` and `cardRank` on agency's slot admissions and session starts.
- **A place on a watched road asks its card first:** the master's `POST /demands` with `serviceName` the
  service its road's registration maps to, `count` 1, `workKey` `research:place:<key>` and `ref` naming the
  place, kept in the store (`docs/store.md`, `demands`). Its slot is asked only once the demand is
  `granted`, read with `GET /demands/{demandId}` and its answer held open; it waits however long that
  takes, the hold in `GET /state` saying why. The try the place takes keeps the demand as its own. A demand the master no longer holds is
  asked again; a master that cannot be reached is asked again on the next pass, and said in the log once
  while the failure stays the same.
- **The demand is held for the whole run** and handed to it: the run's slot and its sessions stand on it,
  so agency asks no card of its own for them (「Calls this unit makes」). It is given back with
  `DELETE /demands/{demandId}` once the run has closed with its document, before the page is finished, or
  as the try ends where it ends sooner, however it ends; a pass of the loop gives back any demand whose try
  has ended or whose branch's run has closed. A daemon that starts gives back every
  demand its store holds, since no run it asked is still under way.
- **A place whose road has no try due** while it waits for its card gives its demand back and asks no
  slot; no item was taken for it.
- **A failed try of a watched road is interrupted** where its demand, read as the try fails, names a
  claimant or a lower tier that took its card (`taken`), no longer stands granted, is no longer held by the
  master (404, as after the master's restart), or cannot be read because the master cannot be reached.
  Such a try ends in its state with its `error` saying which, and its items go back as they stood at once,
  the try not counted (「The deep dive」). The words of the failure are not read.
- **An engine that does not come:** a place still waiting for its card `restore_wait_minutes` after its
  demand was asked is told to the owner's private room once (「The notices」, `restore`); it goes on
  waiting.

### The questions

The templates under `prompts/` are in English, one per item shape, each naming its shape's rulebook so the
search runner applies it, and the run writes its document in English, the question's language:

- `manual.md` and `investigation.md`, a `manual-or-investigation` category's: a `model` item is asked as a
  manual (the rulebook `research.md`, 「The manual」), and a `paper`, `release` or `entry` item as an
  investigation (`research.md`, 「The investigation」); an ask names its own. The item's `{name}`,
  `{address}` and `{origin}` are set in. The manual's question names the headings and the label the fit is
  read by; both name the opening section saying what the subject is and what it is for.
- **An investigation's `{prior}`** is its category's nearest earlier reports, at most a handful, read from
  the store's own queue rather than the queue listing: the reported items whose titles and subjects share
  the item's subject terms — the same model family, task, benchmark or technique — each by its document
  number, title, subject and live link, the latest first where they stand equally near. A category holding
  no reported item sets the one line saying none stands, so the run compares with nothing and invents no
  comparison.
- `advisory.md`, an `advisory` category's (`security-advisory.md`): the item's `{name}`, `{address}` and
  `{origin}`, `{stack}`, the stack inventory a row a line, and `{floor}`, the age floor the run is judged
  against. It asks for the exposure as a table naming each component's repository.
- **The age floor** is the oldest recorded component, by the moment its build or version was recorded, and
  the floor is read afresh on every pass: a deployment or a registration moves it. An advisory whose fixed
  builds appeared before it never joins the queue; the run told the floor names it in its document, and an
  inventory that is empty or records a component with no version gives no floor, so no advisory is skipped
  on age.
- **`{origin}`** is the item's origin in one English line, the words the page prints it in
  (`prompts/page.toml`): the source that found it by its kind and what it reads; the owner's ask, with
  the words it was made with quoted; or, for a follow-up, the document it follows from by its number and
  title, linked where its live link is known, with the sentence saying why it was asked quoted. A brief,
  gathered from its category's sources, is handed none.
- `brief.md`, a `brief` category's (`economy-brief.md`): `{items}`, every gathered item's name and address
  a line.

### The fit

- **The words a document is read by** are those `prompts/document.toml` holds, one set per language for
  each shape: English, the language every run is asked in, and Korean, the language the rulebooks name
  their sections in. A reading looks for either language's heading or table and reads by the set that
  finds it, English first; a heading, a column, a label and a cell's opening word are matched without
  regard to case. A table written without the row under its head is read as a table, given that row as a
  page is drawn (`docs/runner.md`, 「The page」).
- **A manual is read for whether it asks the owner's word,** by the words under `[[manual]]`, those of the
  research rulebook's section on running the thing in the lab and its line of the cards it fits: under
  the heading `heading` names, the line opening with `label` that names anything but the `none` word
  marks the item `fit`.
- **An advisory is read for what it touches,** by the words under `[[advisory]]`: the exposure table, the
  table whose header holds its column words for the component, the version, the repository, the host and
  whether it is touched. Every row whose touched cell opens with the `yes` word is a touch; the item keeps
  the repositories and hosts touched, joined, as `touches`, and is `fit` where any row is touched.
- **An investigation and a brief are never marked `fit`.**
- **The reading is a hint, and the owner decides.** It reads the line's words and judges nothing.
- **The plan's place** is the heading of the section `plan_heading` names for the shape (the reproduction
  plan of a manual, the patch steps of an advisory), as the document holds it, kept as `plan`.

### The brief's facts

- **A brief's figures table** is the table whose header holds the column words `prompts/document.toml`
  names under `[[brief]]`: the figure's name, its value, its unit, the date it stands for and its source.
  Each row naming a name and a value is kept as a fact; a source cell holding a link keeps the link's
  address. A brief with no such table keeps none.

### The page

- **One document holds both languages:** the page the runner filed in English (`docs/runner.md`, 「The page」)
  is drawn again with the Korean side first once that side is had, and set with the artifact service's
  `PATCH /documents/{id}`: `title` the Korean title where the Korean side is had, `html` the page opening on
  the reader's language side, Korean where that language is not held, and
  `labels` the run's document's labels with `languages` naming the languages the page holds, comma-separated,
  `category` the lead item's shape (`manual`, `investigation`, `advisory` or `brief`), and `pending` what the
  page still lacks, the item's `pending` comma-separated, left out once it lacks nothing — the archive's
  category and in-progress marks (bmo-artifact's `docs/artifact.md`, 「Where a person reads it」).
- **What a page holds and lacks is kept by its document** (`docs/store.md`, `pages`): the languages it holds,
  the narrations it plays, what it still lacks and when that is asked again, and how many Korean sides and
  narrations were asked for it. Every item the document was filed for answers them as its `languages`,
  `narrations`, `pending` and `pendingRetryAt`; the page is finished for the item its document's run was
  asked for.
- **The labels follow what the page lacks:** each time a try leaves the page lacking something other than it
  lacked before, the document's labels are set again with `PATCH /documents/{id}` and `labels` alone, so the
  archive marks a page in progress while it lacks a part and done once it lacks none.
- **Each side prints where the research came from** under its title, from the item's `origin` in the side's
  own words (`prompts/page.toml`; `docs/runner.md`, 「The page」): the Korean side names a parent document
  by the Korean title its page holds where it has one, else by the title the item keeps. The line is drawn
  from the item, not translated.
- **The Korean side is rendered by agency's translator** (`POST /translation-runs`) with `purpose` `document`
  and `language` the one `prompts/translation.toml` names, `Korean`, one text at a time: the description,
  where it has one, then the body with the title set over it as its top-level heading, so the Korean title is
  the translated heading, else the item's name with its shape's Korean word (`prompts/page.toml`).
- **A name is never translated, only prose:** a description that is the item's name, case aside, or that
  holds no whitespace, a name or an identifier, stands on the Korean side as written.
- **The body is cut at its headings** into parts no longer than `translation.part_chars` characters
  (`docs/settings.md`): whole sections are gathered into a part while they fit, and a section longer than
  that is halved, at a heading, else a blank line, else a line's end, else a space, nearest its middle,
  until each half fits. At most `translation.parts_out` parts of one page are out at once, and the
  translations are joined in the body's order, a blank line between two.
- **The translator is asked no more at once than it has slots:** across every page being drawn, and the
  titles, descriptions, sections and narrations beside them, at most `translation.parts_out_total`
  translations are out with agency at once (`docs/settings.md`). Every other one waits its turn in this
  unit, with no clock, in the order it was asked, so a redraw of many pages queues here and not at the
  engine, which takes no more at once than its slots.
- **A part's structure is the daemon's, and only its prose is sent.** The part is read into its blocks by a
  maintained library, a table written without the row under its head given that row first, as the page
  draws it (`docs/runner.md`, 「The page」). Its pieces of prose are each heading's words, each paragraph,
  each list item's words and each table cell, in the part's order; everything around them — the heading
  marks, list markers and indentation, a quote's marks, a table's pipes and the row under its head, blank
  lines, every code block and diagram, raw HTML — is kept by the daemon as the English part writes it and
  never sent. A citation mark opening or closing a piece stands outside it, and a piece whose whole text is
  one link, as a sources list's entry is past its mark, sends the link's words alone, its address kept. A
  piece holding no letter is kept as written. The pieces are sent as one text, a blank line between two,
  each on one line.
- **The translation is set back in the English part's exact structure:** each piece's translation, on one
  line, in its piece's place, a pipe in a cell's translation escaped, and a mark opening a block — a
  heading's, a quote's, a list item's or a rule's — that a translation opens with and its English piece
  does not escaped. A piece's citation marks are its English piece's: a mark its translation lost is set at
  its end, and one it added is taken out.
- **An answer is read piece by piece,** split at its blank lines. Where it holds as many pieces as were sent,
  each stands in its place; where it holds another number, the pieces sent are halved and each half asked
  again, in order, down to a piece asked alone, whose answer is taken whole as that piece.
- **A text translated short is asked again:** a text of more than 200 characters whose text ends a sentence
  — its last character, closing quotes, brackets and emphasis marks aside, a full stop, a question mark, an
  exclamation mark or an ellipsis — while its translation does not was cut off by the translator. The
  pieces of a text cut off are halved and each half asked again; a piece of more than 200 characters cut
  off within an answer otherwise whole is asked again alone; and a piece asked alone and cut off is halved
  as text, at a blank line, else a line's end, else a space, nearest its middle, each half asked again. A
  text of 200 characters or fewer is taken as it comes.
- **Each text sent carries the `note`,** the item's name set in: the translator's context, that the text is
  part of a research report about the thing for the owner, in the register of the owner's documents, and
  that a top-level heading opening it is the report's title, kept a noun phrase of at most 15 Korean
  syllables (`docs/design.md`, 「Reports and rooms」).
- **Each text sent carries its `terms`,** what it keeps exactly as written, each a term whose target is
  itself, which agency hands the translation model as its own reference translations (agency's
  「translations」): the item's name; every inline code span, link address and bare web address; every fixed
  section name (`prompts/document.toml`, `[sections]` and `[opening]`) a heading's words hold, as the
  heading writes it; every word of its prose holding both a letter and a digit, or a capital after its first
  letter — a model, product or dataset name; and every figure with its unit, a number and the word after it
  where that word holds a slash or is an abbreviation of at most four letters holding a capital. Each stands
  once, in the order the text holds them, at most forty, read from the pieces sent by a maintained library,
  a heading's words read as a heading. A text halved is given the terms of each half.
- **A note the translator answered as text is taken out:** where an answer holds more paragraphs than the
  text sent, and its first paragraph is prose saying the item's name while the text opens with a heading's
  words or with a paragraph not saying it, that paragraph restates the note and is dropped, as often as the
  count allows.
- **A text refused 413 `too-large`** is halved and each half asked again: its pieces where it holds more than
  one, else a piece as text, as one cut off is; a piece that cannot be halved any further, a 503
  `no-login-open`, or any other failure leaves the Korean side pending.
- **Every part is checked against its English part before it is kept:** both are read by the library, a
  table without the row under its head given it, and they agree where they hold the same blocks in the same
  order and nesting — each heading at its level, each paragraph, each list and its items (how loosely a
  list is written aside), each table with its columns, its head and its rows, each code block, quote and
  rule — and the same citation marks, each as often. A part that does not agree is never kept or drawn: it leaves the Korean side pending, and is asked
  again alone on its retry wait. The description is checked the same way.
- **Every part translated is kept as it is had** (`docs/store.md`, `translated_parts`), by the document and
  the part's English text, so a Korean side asked again translates only the parts it does not hold yet: one
  part failing never has the others asked again. A part kept is taken only while it agrees with its English
  part; one that does not is asked again alone.
- **A part is waited on until agency answers it,** with no clock of this unit's: a translation waits as
  long as its card's grant and the translation take, so the parts out keep the card they share and its
  place in the master's line (「Calls this unit makes」). The Korean side had is kept in the store
  (`docs/store.md`, `renderings`), so the page can be drawn again.
- **The narration** is the result's `LISTEN:` script. The English sound speaks it as written; the Korean sound
  speaks it translated as one text with the `narration_note`, the item's name set in: that it is a spoken
  narration kept as its transcript, to be translated in short sentences, its names, acronyms, figures and
  units kept as written. Each text handed to the voice is the narration's written words, and only those: the
  voice unit derives the spoken words from it and checks them before it speaks. Each is spoken by the voice
  unit (「Calls this unit makes」) in the settings' voice, a
  text of at most the 4,000 characters the voice unit speaks: a longer one is spoken up to the last sentence
  that ends within them, and the run's log says so. Each synthesis is followed until it is spoken or has
  failed, however long its card takes to be granted: nothing ends that wait, a read the voice unit does not
  answer, or answers with a 5xx, is read again a second later, and while it waits, each time the voice
  unit's `waitingOn` sentence for it changes, the run's log says so once under `narration`, naming the
  language and that sentence.
- **A narration's synthesis is kept until its sound is attached** (`docs/store.md`, `syntheses`), so a
  narration has one synthesis living at a time, through a restart of either unit. A try that finds one kept
  follows it and asks none, its text neither translated nor handed to the voice again; one kept that has
  failed, or that the voice unit answers 404 for, is set aside as the try reads it first, and a new one is
  asked in its place. A synthesis the try itself followed to its failure is set aside too, and the narration
  stays pending until the next retry wait. A read refused otherwise leaves the narration pending with its
  synthesis kept, to be read again on the next try.
- **Each synthesis is asked under a key,** `research:<documentId>:<language>`, and `:<n>` after it once the
  narration was asked anew `n` times (`docs/store.md`, `narration_asks`): the count goes up when a synthesis
  ends without its sound, failed or gone, and when the review rewrites the narration, so the voice unit
  answers a repeat of the same ask with the synthesis it holds for it, and a narration asked anew on purpose
  is never answered with the sound made before.
- **Each sound is attached** to the document with `POST /documents/{id}/attachments`: the Korean sound as `narration-ko.ogg`
  with `role` `head`, the English as `narration-en.ogg` with no role. The page is then drawn again with its
  listen band referring to every sound attached by its name (`docs/runner.md`, 「The page」), and the
  archive plays the sound of the side shown there, so the narration's language follows the language the
  page opened on and the switch rather than a fixed side. A sound already attached is not made again.
- **A page's pictures are drawn from the copies kept beside it:** each picture the run's result named, the
  block it stands in closing with a source entry, is kept as `picture-<n>.<ext>` and drawn where the body
  names it, with the result's caption and the source entry's title under it. The page carries the picture's
  bytes itself, so a reader needs no outside host; a picture the report could not keep is drawn from its
  address. A page drawn again, from the daemon's start or `bmo-research redraw`, draws the same pictures from
  the attachments the document keeps.
- **The Korean narration's words are kept** as they are translated (`docs/store.md`, `page_texts`), so the
  review reads what was spoken and a sound made again speaks them without translating them again.
- **Every filed page is drawn as this binary draws it.** As the daemon starts, and on `bmo-research
  redraw`, the page of every item holding a document, for the item its document's run was asked for, is
  drawn again from what is had of it — the run's result, or the English body its review rewrote
  (`docs/store.md`, `page_texts`), the Korean side kept (`renderings`) where its page holds one, and the
  narrations it plays, each by the name its sound is kept under — and compared with the page as the
  archive keeps it, read with `GET /documents/{id}/content?download=true`, its labels with the ones the
  archive answers on `GET /documents/{id}`. A page whose HTML or labels differ is set as any page drawn
  again, and one that matches is left alone, so a change in how a page is drawn reaches every
  filed page and nothing else is written. A page being finished at that moment is left to its finishing,
  which draws it the same way; a page whose result is not kept, or that cannot be read or set, is said in
  the log and left as it stands; a page whose document the archive answers 404 for is closed as failed
  (`docs/runner.md`, 「The page」). Nothing a page lacks is asked here, and no one is told. The count of pages
  drawn again is said in the log, and printed by the step.
- **A filed Korean side that does not agree with its English is set aside** as its page is drawn again: its
  body, the Korean title set over it as its heading, checked against the English body with the title set
  over it the way a part is (above), and its description against the English one. Where they do not agree,
  each English part the body is cut into now takes as its translation the Korean that stands in its place —
  where both bodies hold as many headings, the same top-level blocks of the same section, each agreeing
  with its English block, the part beginning and ending between blocks — where that Korean agrees with the
  part and none is kept for it already (`docs/store.md`, `translated_parts`); the description likewise.
  The page is then drawn and set without its Korean side, every item of its document holding `languages`
  `en`, `korean` among what it lacks and its retry wait due at once, and the document's labels set with
  them, `languages` `en` and `pending` naming `korean`, so the daemon asks the Korean side as it asks any
  pending part: only the parts that took no Korean are translated again, and the page is drawn again with
  its Korean side once it is had. A report told before is not told again. How many Korean sides were set
  aside is said in the log.
- **A page is whole** once it holds its Korean side and every narration its result's script allows: both
  sounds where the result has a `LISTEN:` script, none where it has none, so a page whose result has no
  script is whole with its Korean side. A whole page whose items stand `finishing` is then reviewed once
  (`docs/runner.md`, 「The review」): the review is pending until it answers, and its fixes until each is
  applied. A fix's English section and narration, and the Korean they bring, are kept (`page_texts`,
  `renderings`), and the page is drawn from what is kept: the English side from the reviewed English
  body where the review rewrote one, else from the run's result. A fix is counted applied once the page
  holding it is set in the archive.
- **A narration that cannot be made** — no voice, a translation refused, a synthesis refused (the voice unit's
  422 `unconverted-run` for spoken words it could not clear among them) or failed — leaves the
  document without that sound for now, and the sound is pending; why is written into the run's log under
  `narration`, the synthesis named with the length of the text it was asked to speak.
- **A run told to its room has its document finished the same way** (`docs/runner.md`, 「The narration's
  sound」): its document's page kept as a deep dive's (`docs/store.md`, `pages`), holding the question's
  language alone, its one narration spoken as the script is written, with no translation, under the same
  key, kept and followed the same way, and attached as `narration-<code>.ogg` with `role` `head`, the
  document's labels `languages` and `pending` set as a page's are. It has no Korean side and no review, and
  its room is told, once, as the sound is attached. Its run answers its `narrations` and `pending`
  (「research runs」).
- **What is pending is asked again on every retry wait,** `retry_minutes` after its last try, until it is
  had, while its item stands `finishing`, `reported`, `fit` or `taken`, or for a run told to its room's
  document whatever the run's outcome: the Korean side and each missing sound alike, and no count of tries
  ends it. A try that fails waits for the next; nothing a page lacks is
  given up on. A page whose try is still under way when its wait ends — a narration still waiting for its
  synthesis — is not asked a second time beside it, and its next wait runs from that try's end. A dismissed
  item's page is no longer asked, nor a page whose document is gone: the archive answering
  `GET /documents/{id}` with 404 closes it as failed, its `finishing` items waiting for a try of their own
  (`docs/runner.md`, 「The page」). The tries are counted apart from the run's tries.

### The notices

- **Every recipient of the category is told every notice:** a report, once the item's page is whole (「The
  page」), with the words `prompts/report.toml` holds as `line`, the Korean title, which names the thing,
  the first sentence of the Korean description and the document's one share link set in, and, for a
  follow-up, `follows` with the
  title of the document it follows from set in (its Korean title where its page has one, else the title
  the item keeps), or `follows_document` with its number where no title is known; or, as an item first
  becomes stuck, `stuck`, with the name and the tries set in. The narration is never named, and the parent's link stands
  on the page, not in the notice.
- **A report is told once, whole.** While its page lacks a part, its recipients hear nothing of the item:
  it stands `finishing`, its `pending` naming what it waits on, and its run's log says the same under
  `filed`. The report goes out on the try that makes the page whole, the run's log saying so under `told`.
  A page whose report was already told is not told again when a part it lacked is had later.
- **An advisory skipped on the age floor is told to no one:** it files no document and no page stands, so no
  report, no worker's part and no room's notice goes out for it; its item keeps the one line naming the floor.
- **A room** is told with the receiver's `POST /instructions`: `room`, `sort` `notice`, `kind`
  `research-report:<itemId>` (a brief's first item), `said` `{"order": "<the notice>"}` and `summary`.
- **An agent** is told by one run of agency's `worker` role at its repository's checkout, sent on a task of
  its own so the deep dive goes on while the run works. An agent recipient is a repository's worker, named
  as its repository is, and its checkout is the `path` agency's `GET /repositories/{agentName}` answers.
  - **The slot:** `POST /slot-requests` with `role` `worker`, `source` `notice`, `owner`
    `research:notice:<itemId>`, `path` the checkout, `placeKey` the agent's name, so the notice's run takes
    the checkout as a job's does, `title` the notice's kind and the agent, and `requestKey`
    `research:notice:<itemId>:<agentName>:<n>`, `n` a number drawn at random for the ask, so no key a
    given-back request spent is asked again. The slot is asked for its place every half second however long
    the role's line takes.
  - **The session:** once let in, `POST /sessions` with `scope` `role` and `slot` the slot's token, and
    nothing of a road or a folder: the run goes out as the `worker` role on the slot's road at the checkout,
    on the role's own prompt. Agency binds the slot to that session as it starts it, so the slot ends with
    the session.
  - **The turn:** the notice as `prompt`, under the name `research-notice-<itemId>-<agentName>` and the
    title, waited on until it ends however long that takes (`docs/runner.md`, 「A turn's send」). The
    session is then ended, which gives the slot back with it, and the slot is finished with the turn's
    outcome where agency still holds it; both are kept in the store while they stand, so a restart gives
    them back (`docs/runner.md`, 「A restart」).
  - **Its report row** is kept as the run ends, `sent` once the turn is answered and `refused` with the
    sentence of the step that failed: no repository of the name, no slot, no session, or no answer.
- **A `fit` item's notice carries the reading and the data its recipient acts on,** after the line: what
  its shape says (`fit` for a manual, what it touches for an advisory, `touches` set in), the thing's name
  (`name`), and the plan's place (`plan`, the heading set in). The notice asks nothing of its reader: the
  chat agent's own rules have it put the question to the owner, file what his word calls for, and mark
  the item `taken` or `dismissed`.
- **The share link** is the document's live link from the artifact service's `GET /shares`, or one made
  with `POST /shares` where none stands.
- **An advisory's touched repositories are told their own part.** For each repository the exposure table
  touches, every worker the stack names for that repository is told, by a turn as an agent recipient is,
  the words `worker` holds: a marker naming the notice, the advisory's name, the repository, its touched
  rows, the English document's share link and the plan's place. Workers read English, which the page's
  switch shows them. What a worker does with its part is its own rules'; the notice decides nothing for
  it. The category's recipients are told the whole; a repository no stack row names a worker for is heard
  by them alone.
- **An engine that does not come** is told to the owner's private room alone, once per waiting place, with
  `restore`, the registration, its service and the minutes waited set in, under the kind
  `research-restore:<registration>`.
- **A recipient that refuses** is said in the log and kept as a report row in the state `refused`; the
  others are told all the same.

## The tools

The tool host places `tools/research` under the namespace `research`: one executable, speaking the host's
call protocol 1, answering each entry by calling the routes above at the address and with the token the
daemon's settings name, read under the data root the session record's `BMO_DATA_DIR` names.

| Entry | Arguments | Calls | Capability |
|---|---|---|---|
| `research/ask` | `name`, `address`, `category?`, `shape?`, `because?`, `follows?`, `document?` | `POST /queue` | `research.ask` |
| `research/queue` | `category?`, `state?` | `GET /queue`, and `GET /state` for its `holds`, said first | `research.ask` |
| `research/taken` | `item`, `job` | `PATCH /queue/{itemId}` | `research.owner` |
| `research/dismiss` | `item` | `PATCH /queue/{itemId}` | `research.owner` |
| `research/categories` | `action` (`list`, `add`, `remove`, `change`) and the category's fields | `GET`, `POST`, `PATCH`, `DELETE /categories` | `research.ask`; `add`, `remove` and `change` also `research.owner` |
| `research/sources` | `action` (`list`, `add`, `remove`) and the source's fields | `GET`, `POST`, `DELETE /sources` | `research.ask`; `add` and `remove` also `research.owner` |
| `research/recipients` | `action` (`list`, `add`, `remove`), `category`, `room?` or `agent?` | `GET`, `POST`, `DELETE /categories/{categoryName}/recipients` | `research.ask`; `add` and `remove` also `research.owner` |
| `research/rooms` | `action` (`list`, `add`, `remove`), `category`, `room?` | the same, for the recipients that are rooms | `research.ask`; `add` and `remove` also `research.owner` |
| `research/stop` | `category?` | `PATCH /state` | `research.owner` |
| `research/resume` | `category?` | `PATCH /state` | `research.owner` |
| `research/run` | `key`, `query`, `options?`, `roomId?`, `provider?`, `model?`, `asker?` | `POST /research-runs` | `research.ask` |
| `research/rulebook` | `name` | none: it reads the rulebook from the deploy tree | `web.rulebooks` |

- **An entry that both reads and changes** is shown to a session holding `research.ask`, and the tool
  refuses its changing actions to a session whose record does not also hold `research.owner`.
- **Each answers a short text** for the model to read, and a refusal as a result marked as an error.
- **No entry is held to a clock:** the manifest names no `timeout`, and a call ends with the daemon's answer,
  its connection breaking, or its caller going away.
- **`research/run` sends a research out** and is answered at once with the run's id, its state and its road,
  or why it cannot go out now. What the chat agent is told of it, in the entry's
  description: the depth is the key — `search:basic` for a question after one value or one fact,
  `search:thorough` for a question with a few angles, `search:exhaustive` for a deep dive, `browse` for a
  task that must open and click through pages — and the question is written whole, as the owner means it;
  `roomId` is the room the turn answers, so the result comes back there; a `model`, with its `provider` where
  one is meant, is named only where the owner named one for this research, and none otherwise, so the role's
  own road carries it; the result comes back to the room as an instruction once the run ends, the answer in a
  line or two with its document's link.
- **`research/ask` says why and from what:** `because` is the ask's words, and `follows` the item or
  `document` the document an ask made from an earlier research follows from (「queue」).
- **`research/rulebook` answers one rulebook whole,** read from `prompts/rules/<name>.md` under the first
  folder, from the tool's working folder upward, that holds `prompts/rules`: the deployment places the
  rulebooks beside the tool's executable. Only a name the session record's `labels.research.rulebooks` holds is
  read; any other is refused with the names that are. It is shown to a search run, whose record holds
  `web.rulebooks`.

## The command

`bmo-research` serves and answers the deployment: `serve` runs the daemon; `health` answers 0 once the
daemon answers ready; `openapi` prints the OpenAPI document; `migrate` brings the store's schema to the
version the binary holds and says which; `redraw` draws every filed page again where it differs from what
this binary draws, and prints how many it set (「The page」); `nouns` prints `research`, the noun `bmo research …` hands it.
Under `research` stand `state`, `stop`, `resume`, `categories`, `queue`, `ask`, `taken`, `dismiss`,
`front`, `sources`, `recipients`, `runs`, `stack` and `facts`, each calling the route of the same name through the client; `--json` prints
the member or the page exactly as the API answers it.

## Calls this unit makes

Each is the documented route of the unit that holds what must reach it; the daemon keeps no client crate
of these units, and reads each one's address and token from that unit's own files under the data root's
`env/`, named by the settings (`docs/settings.md`, 「Peers」).

- **agency:** `POST /slot-requests` with `requestKey`, `owner`, `role` and `source`, and `provider` and
  `model` together where the slot's road is named — a run's where its ask names a model, a place's its
  road's, a branch's and a review's the run's road — a run's, a place's, a branch's or a review's slot,
  answered with its `token` and `state` and,
  once let in, the `provider`, `model` and `effort` it goes out on, or 422 once its key was spent; a notice's
  slot names `path` and `placeKey` beside them (「The notices」). `PATCH /slot-requests/{token}` naming no state
  (its place asked for, answered with the request's `state`: asked again while it is `waiting` or `reserved`,
  let in with `request` once it is `active`, and never asked again once it is `released` or `cancelled`, its
  `endedBy` and `outcome` saying who gave it back and why) or `finished` with `outcome`, and `cancel` where it
  never ran, a finish answered `held` false where agency gave the slot back first; this unit binds no slot
  itself (`docs/runner.md`, 「The slots' children」); an admission of a slot whose place, run or branch holds
  a card names `demandId`, that card's demand id, the slot standing on it with no card of its own asked. `POST
  /sessions` with `scope` `role` and `slot`, the token of the active slot the session runs on, and beside them
  `line`, `systemPrompt`, `labels` and `toolsAllowed` for a run's, a branch's or a review's, and `demandId`
  where its run or branch holds a card; never a role, a provider, a model, a folder or `boundMs`, the session
  running as the slot's role on its road at its path, and agency binding the slot to it where the slot is
  bound to no session yet. Answered with the session's `id`, `<role>@run:<n>`, or 422 `slot-not-active` or
  `contradicts-slot`, or 503 `line-not-carried`. `DELETE /slot-requests?owner=`, at the daemon's start, for
  the daemon's places, `research:places`, and for the owner of each run it closes and of each run or notice
  its store still holds something under (`research:<run>`, `research:notice:<itemId>`), answered with the counts given back and `sessionsEnded`, the
  sessions agency ended (`docs/runner.md`, 「A restart」). `POST /sessions/{id}/turns` with `prompt` and the
  turn's own parts (`resume`, `name`, `title`, `chat`, `hearsWords`, `variables`) and `key`, the turn's own,
  under which a send agency did not answer goes again and attaches to the turn standing under it, its road, effort and folder
  the session's, answered once the turn ends with its `answer`, or 409 `session-busy` while an earlier turn of
  the session still runs: `GET /sessions/{id}` is then read every two seconds until its `run` is null, and the
  turn sent again (`docs/runner.md`, 「A turn's send」). `DELETE /sessions/{id}`, the session ended, answered
  409 while its turn runs. `GET /roles`, the roles a branch role is checked against; `GET /roles/{roleName}`,
  the `agents` a run naming a model alone reads its provider off, and the `maxConcurrent` a pull reads the
  search line's role's cap from, the count of its agents that are switched on; `GET /repositories/{repositoryName}`, the
  checkout a notice to an agent runs at. No call of the slot and session routes carries a clock of this
  unit's, a turn's send among them: each is waited on until agency answers, however long a card it waits on
  takes and however long the turn writes, and each is asked again, unchanged, while agency does not answer or
  answers 503 `stopping` (`docs/runner.md`, 「Agency away」). The calendar unit, for a topic that names a calendar
  (`docs/design.md`, 「Categories」): `POST /{realm}/events?holderName=bmo-research` with `id`, `calendarId`,
  `title`, `description`, `start`, `end` and `allDay`, one call an event, the `id` a digest of the topic, the
  title and the start, so the same event written again is answered as it stands. `POST /translation-runs` with `purpose`, `language`, `text`, `note`
  and `terms` (each a `source` alone, kept as written), one part of a page's Korean side or its narration;
  answered 200 with `text`, 413 `too-large` or 503 `no-login-open`. A translation's call carries no clock of
  this unit's either: it is waited on until agency answers, however long the card the translations share takes
  to be granted and the part to be translated.
- **the artifact service:** `GET /documents?q=<name>&limit=1`, the archive check a candidate passes
  (`docs/sources.md`); `POST /documents`, a run's document, `html` for a deep dive's page;
  `POST /documents/{documentId}/attachments` with `name`, `mediaType`, `content` (base64) and, for a deep
  dive's Korean narration and a room's document's narration, `role` `head`, a picture of a run's document as
  `picture-<n>.<ext>` or a narration's sound;
  `GET /documents/{documentId}/attachments`, the files a document keeps — whether a narration's sound is
  already attached, and the pictures a page draws again — and
  `GET /documents/{documentId}/attachments/{attachmentId}`, one file's bytes;
  `PATCH /documents/{documentId}` with `body` and `labels` where a picture the archive refused is drawn from
  its address instead, or with `title`, `html` and `labels` as a page is drawn again, or with `labels` alone
  as what a page or a room's document lacks changes;
  `GET /documents/{documentId}/content?download=true` and `GET /documents/{documentId}`, a page and its labels
  as the archive keeps them, compared with the page drawn again (「The page」);
  `GET /shares?documentId=<documentId>` and `POST /shares` with `documentId` and, for a run's document as
  it is filed, `expiresAt` a day ahead, its live link, read again as a room's document is told, one made
  with no `expiresAt` where none stands then.
- **the voice unit:** at the address and with the token its own settings file under `env/` names
  (`docs/settings.md`, 「Peers」): `GET /voices`, the first voice where the settings name none;
  `POST /syntheses` with `voiceName`, `text`, the narration's written words, `language`, the narration's own
  (`ko` or `en`), `key`, the narration's (「The page」), so a repeat while that synthesis is waiting, speaking
  or done is answered with it and one after it failed makes a new one, and no `spoken`, so the voice unit
  derives the spoken words in that language, a narration spoken; `GET /syntheses/{synthesisId}` read every
  second until its `state` is `done` or `failed`, however long that takes, its `waitingOn` read as it
  stands, a 404 the synthesis gone, an unanswered read or a 5xx read again; and
  `GET /syntheses/{synthesisId}/sound`, the sound, OGG/Opus.
- **the job service:** at the address and with the token its own settings file under `env/` names
  (`docs/settings.md`, 「Peers」): `POST /items` with `title`, `brief`, `queueName`, `assigneeName` and
  `labels` — the settings' and `research-item`, the lead item's number — a stuck item's repair for the
  repair worker (「The repair」), answered 201 with the item's `id`.
- **the receiver:** `GET /owner`, the owner's private room; `POST /instructions`, a report line, the notice
  that an engine has not come, or a research run's end (`room`, null where the run named none, `sort` `notice`,
  `kind` `research-ended:<run>`, `said`, `summary` and `events`).
- **the tool host:** on its control socket, under the issuer token it keeps (`docs/settings.md`,
  「The tool host」): `GET /namespaces/web/tools` page by page, read for each tool's `listing`;
  `POST /tool-sessions` with a record naming the namespace `explorer` and `BMO_DATA_DIR`,
  `POST /tool-calls` under that session's token calling `explorer/search_engines`, the search engines' state,
  and calling `explorer/fetch_raster` with `address` and `pacing`, the picture a document keeps, answered as
  one image block with its base64 `data` and `mimeType`, and `DELETE /tool-sessions/{token}`. A record names
  no `ttlMs`: it stands until the call it was made for ends, and is then taken back. A `page` source's listing the same way under
  a record naming the namespace `web`, the capability `web.read`, `BMO_DATA_DIR` and the label `research`
  with its `strategy` (explorer's `direct-then-browser` for `direct`, `browser-first` for `browser`), whose
  calls of `web/web_fetch` hand `url`, `raw` true, `max_chars` and `offset` and read the page's markup on
  slice by slice, each slice waited on until the tool host answers it (`docs/sources.md`, 「A page」). No
  call of the control socket carries a clock of this unit's.
- **the arbiter:** at the engine watch's master address, with its token where one is named (「The engine」):
  `POST /demands` with `serviceName`, `count` 1, `workKey`, `ref`, the watch's `pool` and `rank` and, for a branch, `within`, answered with
  the demand (`id`, `state`, `taken`, `reason`); `GET /demands/{demandId}?waitMs=<ms>`, read until it is
  `granted`, a 404 asked again; `DELETE /demands/{demandId}`, given back, a 404 counted as given back. The
  bench lock of a verification is the job's; the daemon asks no ticket.
- **The job service is never called:** the work a report calls for is the chat agent's to file.
- **the sources:** each source's own address, read with the daemon's HTTP client and waited on until it
  answers or its connection breaks, a `page` source's through the tool host as above (`docs/sources.md`).
