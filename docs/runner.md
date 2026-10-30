# The research runner

How the research unit runs a research: a search at one of three depths, or a browse, asked of it by the chat
agent through the tool `research/run`, by the daemon's own deep dive, or by any caller of `POST /research-runs`;
what it is composed of, what it is held to, the seat and the sessions it goes out in, its rounds, its branches,
and how its result is filed and told. Agency supplies the seats and the thrown-away sessions; everything else
of a run is this unit's.

## The keys

- **A run is asked at a key:** `search:basic`, `search:thorough` and `search:exhaustive` go out on agency's
  search line, `browse` on its browse line. The key is also the work key the run's seat is asked under, so
  the seat's model and effort are the tier agency's assignment names for it.
- **Each key has its settings** (「What a run is held to」) and its rounds (`prompts/runner.toml`).

## What a run is held to: the research settings

- **One set, the owner's,** kept in the store and changed through `PUT /research-settings`; where nothing was
  changed, the settings file's `[research_settings]` stand (`docs/settings.md`):
  - **the branch role:** the co-worker role a search's branches are seated under, one agency's `GET /roles`
    lists;
  - **each key's branches and clock:** how many branches a run of it may open, and how many minutes its web
    tools stay open from the moment it is asked;
  - **each key's budget:** how many pages its first answer may fetch, and how many each branch may. The request
    states it as a ceiling; the record carries it as `pages` and `branchPages` for the web tools that hold a
    run to it;
  - **the fetch strategy** (`direct-then-browser`, `browser-first` or `direct-only`) and **the gaps** between
    two direct requests to one host and between two requests to one search engine, each null where the web
    tools' own default stands.
- **The settings travel on the run's session record** under `labels.research`, written by agency from the
  `labels` this unit hands at the session's start: `closesAt`, `run` (this unit's run id), `owner` (who asked),
  `source` (the line), `role` (the branch role), `branches`, `pages`, `branchPages`, `chat` (the room, 0 where
  none), `strategy`, `hostIntervalMs`, `engineIntervalMs`, `rulebooks` (the rulebooks the run is handed for the
  reader), `provider` and `model` (the run's road), and, on a run allowed branches, `branchesAt` (this unit's
  base address) and `branchesToken` (its token), which the web tools' branch tool asks a branch at.
- **The capabilities a record keeps** are the settings' `runner.capabilities`, handed as `toolsAllowed`, less
  `web.branches` where the run may open no branch and less `web.rulebooks` where it is handed no rulebook.
- **The search engines' state** is the web tools' own, read through the tool host's call of
  `explorer/search_engines` and answered beside the settings; where the tool host cannot be asked, the answer
  says why instead.

## The line's instructions

- **A search's system prompt** is composed here as the run is asked: the search task
  (`prompts/search/task.md`) with the web namespace's listing set in its `{tools}` mark, then the search
  rulebooks in the order `prompts/runner.toml` names — those binding only some searches given by the one
  section deciding whether each applies, the others whole — then the plain-writing rulebook and the rulebook
  against destroying the system. The listing is the tool host's: the `listing` line of every tool under the
  namespace `web`, by its order. A search whose listing cannot be had is refused 503 and nothing is asked.
- **A rulebook binding only some searches** is read whole by the run, once its deciding section says it
  applies, with `research/rulebook`, which answers the rulebooks the record names under `rulebooks` alone.
- **A browse's system prompt** is the browse task (`prompts/browse/task.md`) and the rulebook against
  destroying the system.
- **A branch's system prompt** is the branch instructions (`prompts/search/branch.md`) with the same listing,
  and the rulebook against destroying the system. Agency puts the platform's harness opening before each.
- **The request** is the line's request template filled with the ask: a search's the question, who asked,
  when, the options, the depth, the branches it may open and its budget line; a browse's the task, its time in
  all, the time for researching and the time kept for writing, a fifth of the clock and at least a minute.

## One run, step by step

1. **The ask:** `POST /research-runs` with the key, the question and, where they are known, who asked, the
   options, the room the result is for, and the provider and model the run is to go out on. A key that is no
   research key or an empty question is refused 400.
2. **The composition:** the settings read, the system prompt and the request composed; a search with no web
   listing is refused 503.
3. **The seat:** agency's `POST /seat-requests` with `requestKey` and `owner` `research:<run>`, the role the
   settings' `runner.role` names, `selection` `auto`, the provider the ask names where it names one, `workKey`
   the key and `source` the line. The seat answers the provider, the model, the effort and the tier its work
   key resolved to.
4. **The session:** agency's `POST /sessions` with `agent` the line, `scope` `apart`, the provider and the model
   the ask names, else the seat's, `cwd` empty, `line` the line, `systemPrompt` the composed instructions,
   `labels` `{research: <the run's record>}`, `toolsAllowed` the run's capabilities, and `boundMs` the clock
   and a writing margin of `runner.writing_margin_minutes`. A seat or a session agency refuses is answered 503
   with agency's sentence; a seat already asked is cancelled and nothing is left standing.
5. **The answer at once:** 202 with the run's id, its key, its state (`waiting` while its seat waits for a
   place, `running` once admitted), its road and the moment its web tools close. The run's clock starts at the
   ask, so a seat that waits spends the run's own time.
6. **The admission:** a seat still waiting is asked for its place again every half second (`PATCH` naming
   `admitted`) until it is let in, then bound to the session (`PATCH` naming `bound` with the session's id). A
   seat the pool ends, or one still waiting when the clock runs out, ends the run `failed`.
7. **The first turn:** the request, as one turn of the session (`POST /sessions/{id}/turns`) carrying the work
   parts — the provider, the model, the seat's mapping (model, effort, tier), a fresh resume, an empty folder,
   the name `research-<run>`, the title, the room or 0, the key as the work key, no words heard and no
   variables — waited on until it ends. The session is then ended (`DELETE /sessions/{id}`).
8. **The rounds:** the key's rounds are asked of a second session of their own, started the same way with the
   round's record (the run's with `branches` 0 and no `branchesAt` or `branchesToken`) and the capabilities
   less `web.branches` and `web.rulebooks`, one turn each, named `research-<run>-<round>`, each prompt the round's
   question, the task as the run was given it and the answer written last (「The rounds」). The last answer is the
   run's result. The rounds' session is ended after the last.
9. **The end:** the seat is given back (`finished`, its outcome `answered` or `failed: <why>`), the result is
   drawn into its document and filed (「The document」), the run is closed in the ledger with its outcome, the
   document and its link, and its room is told (「The room」). Every step is written into the run's log.

## The rounds

- **Each key's list** stands in `prompts/runner.toml`: at the basic depth only the final question, at the
  deeper ones and for a browse every question in turn — what the task must establish, the sources it needs,
  the facts checked against a second source, the coverage of the question, the facts written into the brain,
  and the final answer. A list ends in `final` and holds it nowhere else.
- **A round's prompt** is its question, then the task as the run was given it under a heading of its own, then
  the answer written last under another. Each round is a fresh turn: the session carries nothing between them.
- **The ceiling:** `ceiling_seconds` bounds the rounds from the first answer. Past it, the key's `past_ceiling`
  says what follows: `final` asks the final question at once, `last` keeps the answer already written.
- **A round that does not answer** leaves the answer written last standing as the result.

## Branches

- **A run allowed branches opens them with the web tools' branch tool,** which asks this unit's
  `POST /branches` with the token the record names (`docs/api.md`, 「research runs」). The allowance is counted
  here, per run: an ask past the checks of the run's state and its allowance counts against it, whatever then
  becomes of it.
- **A branch** is a seat of the common co-worker pool under the settings' branch role and a thrown-away session
  on the run's provider and model, on the branch instructions, its record the run's with `branchOf` the run's
  id, `pages` the run's `branchPages`, `branches` 0 and no `branchPages`, `branchesAt` or `branchesToken`. Its
  one turn carries the angle, framed as data, and how long is left before research closes; its session is
  bounded to two minutes past the close, which cuts a turn still working then, and the turn is waited on to
  that bound and half a minute more.
- **A branch ends** `completed`, `stopped` (cut at the close, its notes as far as they got) or `failed`, its
  session ended and its seat given back with the same word; a caller that lets go of the ask ends it the same
  way.

## The document

- **Every result leaves a document,** filed with the artifact service's `POST /documents` and given a share
  link with `POST /shares` that ends a day after it is made; the document stays in the archive. The result is
  the searcher's structure lines (`prompts/search/task.md`); the document is made of them and carries none of
  them:
  - **its head:** the title the `TITLE:` line, the description the `TOPIC:` line, the kind `learning` where
    `KIND: learning` says so and `search` otherwise, `actorName` `research`, `scope` the room, and the labels
    `research` (the run's id), `key`, `query`, and `keywords`, `rule` and `topic` where the result writes them.
    The `ANSWERED:` line is the run's own and stands nowhere.
  - **its body:** each block a section under a heading in the question's language, the words of
    `prompts/search/document.toml` (Korean where the question holds Hangul, English otherwise): the verdict
    under the answer, the summary under the summary (under the answer on a brief result), each `SECTION:` under
    its own name, the unconfirmed list, and the sources last, one link a line, titled by what each supports. A
    set's entries stand as sections of their own; a menu as its own list.
  - **its pictures:** every `IMAGE:` line's picture fetched by a guarded direct fetch — every hop's host a
    public address, at most three redirects, an image type, at most 15 MB — at most twelve within forty seconds,
    attached under a name of its own (`POST /documents/{id}/attachments`), and the body set again with
    `PATCH /documents/{id}`; a picture not kept is drawn from its address.
  - **the narration:** a deep dive's `LISTEN:` script is lifted out and never filed.

## The room

- **The room is told through the receiver's `POST /instructions`,** as a `notice` under the kind
  `research-ended:<run>`; a run that names no room is sent with none, and the receiver hands it the owner's
  private room.
- **A filed result** is told as the answer in brief — the verdict, else the summary, else the title — and the
  one link, with the shape the chat agent answers in: one or two short lines in the owner's language and its
  own voice, then the link, and no grounds, sources or document text in the chat.
- **A result that could not be filed** is told whole; **a run with no answer** is told what it could not do;
  **a cut run** hands what it had written as unfinished, or one line saying the work was cut off.
- **A run the daemon asks for its own deep dive tells no room:** the dive's notices are its report
  (`docs/api.md`, 「The deep dive」).

## Ending a session

- **A session is ended once its turn has answered.** Agency answers the end 409 while a turn of the session
  runs and does not interrupt it; the session's bound cuts such a turn. An end answered 409 is asked again
  every two seconds until the turn is over, at most for what is left of the session's bound.

## A restart

- **A run is this unit's process's own.** A daemon that starts closes every run its store holds open as `cut`
  before its routes stand: each session the run held is ended, each seat it held given back, and its room told
  what it had written, as a cut run is.
