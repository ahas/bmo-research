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
| 503 | `unavailable` | a research run cannot go out now: agency could not be reached or failed on its side for its seat or its session, or the tool host gave no web listing; the sentence is theirs |

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
  each `categoryName`, `state`, `waitingCount`, `runningCount`, `finishingCount` (its items whose page is
  finished before they are told), `stuckCount` (its items stuck, 「The repair」) and `cap`, and `holds`, the
  runs asked that
  wait for their engine now (「The engine」), each `reason` (`waiting-card`), `runId` (the try), its
  `categoryName`, `registration`, `serviceName` (the arbiter service whose card it waits for), `demandId`
  (null until the master has answered the ask), `cause` and `causeText` (the master's reason the card
  waits — `starting`, `in-use`, `held` or `start-failed` — and its sentence, null where it gave none),
  `since` (when the demand was asked) and `reported` (whether the owner's room was told the engine has not
  come); empty where nothing waits.
- **A run waiting for its card counts as running** against `concurrency`; its category's sources are read
  and its part of the queue fills as ever.
- **A stopped category starts no run** and its sources are not read; a run under way is waited on to its
  end and reported as any other, and a run still waiting for its card gives its demand back and its items
  go back as they stood, the try uncounted. `running` starts it again. A category's state outlives a restart; a
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
  `lastTaken` how many of them it kept as new, each waiting for a slot in the queue, both null before a
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
  English), `shareUrl` (the document's live link), `languages` (the languages the document's page holds,
  Korean first: `["en"]` until the Korean side is had, `["ko", "en"]` after, empty before a document is
  filed; 「The page」), `narrations` (the languages whose narration the page plays, Korean first), `pending`
  (what the page still lacks before its report is told: `korean` for its Korean side, `narration-ko` and
  `narration-en` for each narration its script allows; empty where it lacks nothing), `pendingRetryAt` (when
  that is asked again, null where nothing is pending), `fit`, `touches` (the hosts and repositories an
  advisory names), `plan` (the heading of the document's plan section, as the document holds it),
  `jobItemId` (the job item the chat agent filed for it), `repairItemId` (the job item filed for the helper
  while it is stuck, 「The repair」; null where none was), `error` (why its latest try failed or was
  interrupted, null after one that filed its document), `foundAt`, `createdAt` and `changedAt`.
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
  tries pass `retries`, still asked again on every retry wait while the helper is asked to fix the cause
  (「The repair」); `taken` once the chat agent filed the work it calls for; `dismissed` on the owner's
  word. No count of tries gives an item up: dismissal is the one way an item leaves the queue unreported. A
  dismissed item's key stays seen, so nothing turns it up again.
- **The queue's order** is the order runs are taken in, within a category: `priority` from highest, then
  `foundAt` from oldest, then `id`. Between categories, the next run goes to the highest priority waiting,
  and among equals to the category whose last run was asked longest ago, so no category's flood starves
  another. `GET /queue` answers the running items, then the waiting and stuck ones in their category's
  order, then the rest newest first; `after` continues past the named item's place in it.
- **The cap.** A category's part of the queue holds at most its `cap` items `waiting` or `running` that the
  scan took in, a stuck item not among them; an ask is never refused for the cap. A slot that frees is
  filled from the category's seen
  candidates no item stands for, oldest first (`docs/sources.md`, 「From a candidate to an item」).
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
  figure stands for, as the brief writes it), `sourceAddress`, `runId` (the research run whose brief held it),
  `documentId` and `createdAt`. A brief's figures are read from its figures table (「The brief's facts」).

### research runs

| Method | Path | Does |
|---|---|---|
| POST | `/research-runs` | Ask a research: a search at a depth or a browse; 202 with the run. |
| GET | `/research-runs/{runId}` | One research run. |
| GET | `/research-runs/{runId}/log-lines` | A research run's log, oldest first. |
| POST | `/branches` | Open a branch of a research run and answer once it ends. |

- **An ask** (`docs/runner.md`) names `key` (`search:basic`, `search:thorough`, `search:exhaustive` or
  `browse`) and `query`, and may name `asker`, `options`, `roomId` (the room the result is told to; the
  owner's private room where none is named), `provider` and `model` (the road it goes out on; the seat's
  where none is named). A key that is no research key or an empty query is 400 `bad-request`. Where agency
  cannot be reached for the seat or the session or answers it with a failure of its own (5xx), or a search's
  web listing cannot be had, it is 503 `unavailable`; where agency refuses the seat or the session for what
  it asks, or gives a seat with no token or no model, it is 502 `upstream-failed`. Either carries the
  sentence, and nothing is left standing.
- **The ask is answered at once,** 202 with `runId`, `key`, `state` (`waiting` while its seat waits for a
  place, `running` once let in), `provider`, `model` and `closesAt` (when its web tools close; the clock runs
  from the ask). The run goes on in the background to its end.
- **A research run** is `runId`, `key`, `query`, `asker`, `roomId`, `provider`, `model`, `state` (`waiting`,
  `running` or `closed`), `outcome` (`answered`, `failed`, or `cut` where what it stood on went away,
  `docs/runner.md`, 「A cut run」; null while open), `startedAt`, `closesAt`,
  `finishedAt`, `documentId` and `link` (the filed document and its share link, null where none was filed),
  `error` (why it failed), `branches` (how many it opened) and `turns` (the names of the agency turns it went
  out under, its first answer's and each round's, by which its tool calls are read in agency's log).
- **Its log** answers `items`, each `id`, `runId`, `at`, `kind` (`asked`, `seated`, `answer`, `round`,
  `branch`, `whole`, `narration`, `filed`, `told` or `ended`) and `text`, the run's own notes from the ask to
  its end, and for a deep dive's run how its page was finished (「The page」): every narration sound the page
  was given or not, and why, under `narration`; why its Korean side is pending, and what its report waits on
  each time that changes, under `filed`; and when its report was told, under `told`.
- **A branch** is asked with the token the run's record names, `run` (the run's id) and `angle`, and answered
  200 once it ends with `index` (counted from 1 per run), `state` (`completed`, `stopped` at research close,
  or `failed`), `notes` (what the branch wrote, empty where nothing) and `reason` (why, on `failed`; null
  otherwise). A run that does not stand is 404 `not-found`, an empty angle 400 `bad-request`. It is 409
  `branch-refused`, the sentence saying why, where the run is no longer running or its research has closed,
  the run is allowed no branches, it has opened every branch it is allowed (the sentence names how many), or
  the pool stayed full until research closed; every ask past the first two checks counts against the
  allowance. A caller that lets go ends its branch: its session ended once its turn is over, its seat
  given back and its demand with them.
- **A branch of a run holding a card** (「The engine」) asks its own demand first, for the service the
  engine watch maps the branch's model to, within the run's demand; it waits for the grant until research
  closes, when it is 409 `branch-refused` with nothing left standing, and its seat and session stand on the
  grant.

### research settings

| Method | Path | Does |
|---|---|---|
| GET | `/research-settings` | The settings every research run is held to, with the search engines' state. |
| PUT | `/research-settings` | Change them; a field left out keeps what it holds. |

- **The settings** (`docs/runner.md`, 「What a run is held to」) are `branchRole`, `strategy`,
  `hostIntervalMs` and `engineIntervalMs` (null where the web tools' default stands), `keys` (each `key`,
  `branches`, `minutes`, `pages` and `branchPages`, the last two null for no ceiling), `source` (`default`
  where the settings file's stand, `set` once changed), `engines` (each `name`, `category`, `state`, `reason`,
  `observedAt` and `suspendedUntil`, as the web tools answer them; null where they could not be read) and
  `enginesUnread` (why not, or null).
- **A change** names any of `branchRole`, `strategy`, `hostIntervalMs`, `engineIntervalMs` (null returns to
  the web tools' default) and `keys`; a key's `pages` or `branchPages` left out keeps what it holds. A role
  agency's `GET /roles` does not list, a strategy outside the three, a gap below 0, a key that is no research
  key, branches below 0, a clock under a minute or a budget under a page is 400 `bad-request`; it is answered
  with the settings as they now stand.

## The deep dive

- **`concurrency` tries at once at most,** across the running categories, each waiting for its engine first
  where its road is watched (「The engine」). A try asks the runner
  (「research runs」, `docs/runner.md`) for a research run at the settings' `road.key`, on the category's
  provider and model (the settings' `road` where it names none), with the question its shape takes (「The
  questions」) and no room, keeps the run's id on its items and its run row, and reads the run's row every
  `poll_seconds` until its `state` is `closed`. A run the deep dive asks tells no room itself.
- **The run is held to its shape:** the try hands the runner the section headings its item's shape holds
  (`prompts/document.toml`, `[sections]`), and the runner files the result as a page only where it is whole:
  its head lines, its body opening with the answer, the opening section saying what the subject is where
  its shape holds one (`prompts/document.toml`, `[opening]`), and at least half of those headings in one
  language, the run's session asked once more for the whole document where they are short
  (`docs/runner.md`, 「The deep dive's page」). The try hands the runner the item's origin too, which the
  page prints under its title.
- **A try ends filed, interrupted or failed.** It is filed once the run's row names the document it filed.
- **An interrupted try** is one that ended because something the run needs could not be reached or was
  taken away, not because of what it asked or wrote: the runner refusing the ask 503 (`docs/runner.md`,
  「One run, step by step」), a run that closed `cut` (this unit's restart, or agency failing or no longer
  holding the run's seat or session, `docs/runner.md`, 「A cut run」), or a try of a watched road whose card
  was taken or no longer stands (「The engine」). It is not counted toward `retries`, and its items go back
  as they stood — `waiting`, or `stuck` — with no retry wait (`retryAt` null), so the next pass of the loop
  asks them again; a watched road's try waits there for its card however long that takes. Its error stays
  on the items and on its try's row, saying what went away and that the try is not counted.
- **A failed try** is one that ended on its own content: the runner refusing the ask 400, 500 or 502 (an ask
  of the wrong shape, the runner's own failure, agency refusing what was asked), or a closed run with no
  document, its error the run's — the result still short of its shape once asked again whole, or a turn
  that wrote nothing while its seat stood. It counts toward `retries`: its items wait again with `retryAt`
  `retry_minutes` ahead. A failed try that takes the lead's tries past `retries` puts its items `stuck`
  (「The repair」), and every recipient of the category is told so once.
- **A brief** gathers, once its category's `gatherMinutes` have passed since its last run was asked, every
  waiting or stuck item of the category whose retry is due, at most its `cap`, into one run; a gathering
  whose retry is due is asked at once.
- **A daemon that starts** closes every research run it left open as `cut` (`docs/runner.md`, 「A
  restart」), then takes up every try it left `running`: one with a run id is read again, and ends as a
  cut run's try, interrupted; one without puts its items back as they stood, uncounted. A page it left
  `finishing`, or with a part pending, is asked again on its retry wait as any pending page is (「The page」).
- **A filed document's page is finished before it is told** (「The page」): its items are filed `finishing`
  first, the page read from the run's result as the ledger keeps it (`written`, drawn as the runner drew
  it), then its Korean side and its narrations are asked and the page drawn again with what was had. Once
  the page is whole its items are `reported` or `fit` and the recipients are told; until then its pending
  parts are asked again on every retry wait.

### The repair

- **A stuck item is still asked:** on every retry wait, as a waiting item is, its tries counted on. It
  leaves `stuck` once a try files its document, once it is asked again (「queue」, `state` `waiting`), or
  once the owner dismisses it.
- **The helper is asked to fix the cause.** Each pass of the loop files, before it starts its tries, one
  repair item in the job service for the stuck items of each research run that hold none — one for the
  items a stuck brief gathered, one for each item stuck with no run — and keeps the job item's number on
  each as `repairItemId`. The item is `POST /items` with the settings' `[job]` `queue`, `assignee` and
  `labels` (`docs/settings.md`, 「Peers」): the helper as its worker, under a label the deployment's job
  service opens unattended, so it needs no approval. Its title and brief are `prompts/repair.toml`'s, with
  the evidence set in: each item's number, name, address, category, shape, tries and last error; the
  research runs of its tries, each with its outcome and its error; and the last lines of its latest run's
  log. The brief tells the helper to find the cause — the question template, a rulebook, the runner, the
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
  empty watches nothing: no demand is asked and every try goes out as soon as a slot frees.
- **A try of a watched road asks its card first:** the master's `POST /demands` with `serviceName` the
  service its road's registration maps to, `count` 1, `workKey` `research:run:<runId>` and `ref` naming
  the try and its item, kept in the store (`docs/store.md`, `demands`). The try is asked of the runner
  only once the demand is `granted`, read with `GET /demands/{demandId}` and its answer held open; it waits
  however long that takes, the hold in `GET /state` saying why. A demand the master no longer holds is
  asked again; a master that cannot be reached is asked again on the next pass, and said in the log once
  while the failure stays the same.
- **The demand is held for the whole run** and handed to it: the run's seat and its sessions stand on it,
  so agency asks no card of its own for them (「Calls this unit makes」). It is given back with
  `DELETE /demands/{demandId}` once the run has closed with its document, before the page is finished, or
  as the try ends where it ends sooner, however it ends; a pass of the loop gives back any demand whose try
  has ended or whose branch's run has closed. A daemon that starts gives back every
  demand its store holds, since no run it asked is still under way.
- **A try whose category is stopped** while it waits for its card gives its demand back, ends `failed`
  with its `error` saying so, and its items go back as they stood with the try not counted.
- **A failed try of a watched road is interrupted** where its demand, read as the try fails, names a
  claimant or a lower tier that took its card (`taken`), no longer stands granted, is no longer held by the
  master (404, as after the master's restart), or cannot be read because the master cannot be reached.
  Such a try ends in its state with its `error` saying which, and its items go back as they stood at once,
  the try not counted (「The deep dive」). The words of the failure are not read.
- **An engine that does not come:** a try still waiting for its card `restore_wait_minutes` after its demand
  was asked is told to the owner's private room once (「The notices」, `restore`); it goes on waiting.

### The questions

The templates under `prompts/` are in English, one per item shape, each naming its shape's rulebook so the
search runner applies it, and the run writes its document in English, the question's language:

- `manual.md` and `investigation.md`, a `manual-or-investigation` category's: a `model` item is asked as a
  manual (the rulebook `research.md`, 「The manual」), and a `paper`, `release` or `entry` item as an
  investigation (`research.md`, 「The investigation」); an ask names its own. The item's `{name}`,
  `{address}` and `{origin}` are set in. The manual's question names the headings and the label the fit is
  read by; both name the opening section saying what the subject is and what it is for.
- `advisory.md`, an `advisory` category's (`security-advisory.md`): the item's `{name}`, `{address}` and
  `{origin}`, and `{stack}`, the stack inventory a row a line. It asks for the exposure as a table naming
  each component's repository.
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
  `PATCH /documents/{id}`: `title` the Korean title where the Korean side is had, `html` the page, and
  `labels` the run's document's labels with `languages` naming the languages the page holds, comma-separated.
  Every item the document was filed for keeps the languages as `languages` and the narrations as `narrations`.
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
  until each half fits. At most `translation.parts_out` parts are out at once, and the translations are
  joined in the body's order, a blank line between two.
- **Each part carries the `note`,** the item's name set in: the translator's context, that the text is
  part of a research report about the thing for the owner, that model, product and dataset names, figures,
  units, code, links and the fixed section names stay as written, and the register of the owner's
  documents.
- **A note the translator answered as text is taken out:** where a translation holds more paragraphs than its
  part, and its first paragraph is prose saying the item's name while the part opens with a heading or with
  a paragraph not saying it, that paragraph restates the note and is dropped, as often as the count allows.
- **A part refused 413 `too-large`** is halved the same way and each half asked again; a part that cannot
  be halved any further, a 503 `no-login-open`, or any other failure leaves the Korean side pending.
- **A part is waited on until agency answers it,** with no clock of this unit's: a translation waits as
  long as its card's grant and the translation take, so the parts out keep the card they share and its
  place in the master's line (「Calls this unit makes」).
- **A part translated short is halved the same way:** a part of more than 200 characters whose text ends a
  sentence — its last character, closing quotes, brackets and emphasis marks aside, a full stop, a question
  mark, an exclamation mark or an ellipsis — while its translation does not, was cut off by the translator,
  so the translation is set aside and each half asked again. A part of 200 characters or fewer is taken as
  it comes. The
  Korean side had is kept in the store (`docs/store.md`, `renderings`), so the page can be drawn again.
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
  synthesis kept, to be read again on the next try. Each is attached to the document with `POST /documents/{id}/attachments`: the Korean sound as `narration-ko.ogg`
  with `role` `head`, the English as `narration-en.ogg` with no role. The page is then drawn again with its
  listen band referring to every sound attached by its name (`docs/runner.md`, 「The page」), and the
  archive plays them there. A sound already attached is not made again.
- **A page is whole** once it holds its Korean side and every narration its result's script allows: both
  sounds where the result has a `LISTEN:` script, none where it has none, so a page whose result has no
  script is whole with its Korean side.
- **A narration that cannot be made** — no voice, a translation refused, a synthesis refused (the voice unit's
  422 `unconverted-run` for spoken words it could not clear among them) or failed — leaves the
  document without that sound for now, and the sound is pending; why is written into the run's log under
  `narration`, the synthesis named with the length of the text it was asked to speak.
- **What is pending is asked again on every retry wait,** `retry_minutes` after its last try, until it is
  had, while its item stands `finishing`, `reported`, `fit` or `taken`: the Korean side and each missing
  sound alike, and no count of tries ends it. A try that fails waits for the next; nothing a page lacks is
  given up on. A page whose try is still under way when its wait ends — a narration still waiting for its
  synthesis — is not asked a second time beside it, and its next wait runs from that try's end. A dismissed
  item's page is no longer asked. The tries are counted apart from the run's tries.

### The notices

- **Every recipient of the category is told every notice:** a report, once the item's page is whole (「The
  page」), with the words `prompts/report.toml` holds as `line`, the Korean title, the first sentence of the
  Korean description and the document's one share link set in, and, for a follow-up, `follows` with the
  title of the document it follows from set in (its Korean title where its page has one, else the title
  the item keeps), or `follows_document` with its number where no title is known; or, as an item first
  becomes stuck, `stuck`, with the name and the tries set in. The narration is never named, and the parent's link stands
  on the page, not in the notice.
- **A report is told once, whole.** While its page lacks a part, its recipients hear nothing of the item:
  it stands `finishing`, its `pending` naming what it waits on, and its run's log says the same under
  `filed`. The report goes out on the try that makes the page whole, the run's log saying so under `told`.
  A page whose report was already told is not told again when a part it lacked is had later.
- **A room** is told with the receiver's `POST /instructions`: `room`, `sort` `notice`, `kind`
  `research-report:<itemId>` (a brief's first item), `said` `{"order": "<the notice>"}` and `summary`.
- **An agent** is told with agency's `POST /sessions/<agentName>@worker/turns`, the notice as `prompt`.
  The daemon waits at most `turn_wait_seconds` for the answer; a turn still working then is counted as
  handed, since agency carries a sent turn to its end.
- **A `fit` item's notice carries what its recipient needs to act,** after the line: what its shape says
  (`fit` for a manual, the question for the owner's word; `touches` for an advisory, what it touches set
  in), the thing's name (`name`), and the plan's place (`plan`, the heading set in). The chat agent files
  what the owner's word calls for, and marks the item `taken` or `dismissed`.
- **The share link** is the document's live link from the artifact service's `GET /shares`, or one made
  with `POST /shares` where none stands.
- **An advisory's touched repositories are told their own part.** For each repository the exposure table
  touches, every worker the stack names for that repository is told, by a turn as an agent recipient is,
  the words `worker` holds: the advisory's name, the repository, its touched rows, the English document's
  share link and the plan's place. Workers read English, which the page's switch shows them. The
  category's recipients are told the whole; a repository no stack row names a worker for is heard by them
  alone.
- **An engine that does not come** is told to the owner's private room alone, once per waiting try, with
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
- **`research/run` sends a research out** and is answered at once with the run's id, its state, its road and
  when its web tools close, or why it cannot go out now. What the chat agent is told of it, in the entry's
  description: the depth is the key — `search:basic` for a question after one value or one fact,
  `search:thorough` for a question with a few angles, `search:exhaustive` for a deep dive, `browse` for a
  task that must open and click through pages — and the question is written whole, as the owner means it;
  `roomId` is the room the turn answers, so the result comes back there; a `model`, with its `provider` where
  one is meant, is named only where the owner named one for this research, and none otherwise, so the seat's
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
version the binary holds and says which; `nouns` prints `research`, the noun `bmo research …` hands it.
Under `research` stand `state`, `stop`, `resume`, `categories`, `queue`, `ask`, `taken`, `dismiss`,
`front`, `sources`, `recipients`, `runs`, `stack` and `facts`, each calling the route of the same name through the client; `--json` prints
the member or the page exactly as the API answers it.

## Calls this unit makes

Each is the documented route of the unit that holds what must reach it; the daemon keeps no client crate
of these units, and reads each one's address and token from that unit's own files under the data root's
`env/`, named by the settings (`docs/settings.md`, 「Peers」).

- **agency:** `POST /seat-requests` with `requestKey`, `owner`, `role`, `selection` `auto`, `provider` where
  one is named, `workKey` and `source`, a run's or a branch's seat, answered with its `token`, `state`,
  `provider`, `model`, `effort` and `tier`; `PATCH /seat-requests/{token}` naming `admitted` (its place asked
  for), `bound` with `childId` (bound to its session) or `finished` with `outcome`, and `cancel` where it never
  ran; an admission of a seat whose run or branch holds a card names `demandId`, that card's demand id, the
  seat standing on it with no card of its own asked.
  `POST /sessions` with `agent`, `scope` `apart`, `provider`, `model`, `cwd`, `line`, `systemPrompt`,
  `labels`, `toolsAllowed` and `boundMs`, and `demandId` where its run or branch holds a card, answered
  with the session's `id`, or 503 `line-not-carried`.
  `POST /sessions/{id}/turns` with `prompt` and the work parts (`provider`, `model`, `mapping`, `resume`,
  `cwd`, `name`, `title`, `chat`, `workKey`, `hearsWords`, `variables`), answered once the turn ends with its
  `answer`. `DELETE /sessions/{id}`, the session ended, answered 409 while its turn runs. `GET /roles`, the
  co-worker roles a branch role is checked against.
  No call of the seat and session routes carries a clock of this unit's: each is waited on until agency
  answers, however long a card it waits on takes; a turn's send alone is bounded, by its session's bound
  and a margin past it.
  `POST /sessions/{agentName}@worker/turns` with `prompt`, a notice to an agent. `POST /translation-runs`
  with `purpose`, `language`, `text` and `note`, one part of a page's Korean side or its narration;
  answered 200 with `text`, 413 `too-large` or 503 `no-login-open`. A translation's call carries no clock
  of this unit's either: it is waited on until agency answers, however long the card the translations
  share takes to be granted and the part to be translated.
- **the artifact service:** `GET /documents?q=<name>&limit=1`, the archive check a candidate passes
  (`docs/sources.md`); `POST /documents`, a run's document, `html` for a deep dive's page;
  `POST /documents/{documentId}/attachments` with `name`, `mediaType`, `content` (base64) and, for the Korean
  narration, `role` `head`, a picture of a run's document or a narration's sound;
  `GET /documents/{documentId}/attachments`, whether a narration's sound is already attached;
  `PATCH /documents/{documentId}` with `body` and `labels` once a room's document's pictures are kept, or with
  `title`, `html` and `labels` as a page is drawn again;
  `GET /shares?documentId=<documentId>` and `POST /shares` with `documentId` and, for a run's document,
  `expiresAt` a day ahead, its live link.
- **the voice unit:** at the address and with the token its own settings file under `env/` names
  (`docs/settings.md`, 「Peers」): `GET /voices`, the first voice where the settings name none;
  `POST /syntheses` with `voiceName`, `text`, the narration's written words, `language`, the narration's own
  (`ko` or `en`), and no `spoken`, so the voice unit derives the spoken words in that language, a narration
  spoken; `GET /syntheses/{synthesisId}` read every
  second until its `state` is `done` or `failed`, however long that takes, its `waitingOn` read as it
  stands, a 404 the synthesis gone, an unanswered read or a 5xx read again; and
  `GET /syntheses/{synthesisId}/sound`, the sound, OGG/Opus.
- **the job service:** at the address and with the token its own settings file under `env/` names
  (`docs/settings.md`, 「Peers」): `POST /items` with `title`, `brief`, `queueName`, `assigneeName` and
  `labels` — the settings' and `research-item`, the lead item's number — a stuck item's repair for the
  helper (「The repair」), answered 201 with the item's `id`.
- **the receiver:** `GET /owner`, the owner's private room; `POST /instructions`, a report line, the notice
  that an engine has not come, or a research run's end (`room`, null where the run named none, `sort` `notice`,
  `kind` `research-ended:<run>`, `said`, `summary` and `events`).
- **the tool host:** on its control socket, under the issuer token it keeps (`docs/settings.md`,
  「The tool host」): `GET /namespaces/web/tools` page by page, read for each tool's `listing`;
  `POST /tool-sessions` with a record naming the namespace `explorer` and `BMO_DATA_DIR`,
  `POST /tool-calls` under that session's token calling `explorer/search_engines`, and
  `DELETE /tool-sessions/{token}`, the search engines' state. A `page` source's listing the same way under
  a record naming the namespace `web`, the capability `web.read`, `BMO_DATA_DIR` and the label `research`
  with its `strategy` (explorer's `direct-then-browser` for `direct`, `browser-first` for `browser`), whose
  calls of `web/web_fetch` hand `url`, `raw` true, `max_chars` and `offset` and read the page's markup on
  slice by slice, waiting at most two minutes for each (`docs/sources.md`, 「A page」).
- **the arbiter:** at the engine watch's master address, with its token where one is named (「The engine」):
  `POST /demands` with `serviceName`, `count` 1, `workKey`, `ref` and, for a branch, `within`, answered with
  the demand (`id`, `state`, `taken`, `reason`); `GET /demands/{demandId}?waitMs=<ms>`, read until it is
  `granted`, a 404 asked again; `DELETE /demands/{demandId}`, given back, a 404 counted as given back. The
  bench lock of a verification is the job's; the daemon asks no ticket.
- **The job service is never called:** the work a report calls for is the chat agent's to file.
- **the sources:** each source's own address, read with the daemon's HTTP client, a `page` source's
  through the tool host as above (`docs/sources.md`).
