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
   research key or an empty question is refused 400. The daemon's deep dive asks its runs here too, each
   with the arbiter demand its try was granted where its road is watched (`docs/design.md`, 「The engine a
   run needs」); such a run's seat and sessions stand on that demand.
2. **The composition:** the settings read, the system prompt and the request composed; a search with no web
   listing is refused 503.
3. **The seat:** agency's `POST /seat-requests` with `requestKey` and `owner` `research:<run>`, the role the
   settings' `runner.role` names, `selection` `auto`, the provider the ask names where it names one, `workKey`
   the key and `source` the line. The seat answers the provider, the model, the effort and the tier its work
   key resolved to.
4. **The session:** agency's `POST /sessions` with `agent` the line, `scope` `apart`, the provider and the model
   the ask names, else the seat's, `cwd` empty, `line` the line, `systemPrompt` the composed instructions,
   `labels` `{research: <the run's record>}`, `toolsAllowed` the run's capabilities, and `boundMs` the clock
   and a writing margin of `runner.writing_margin_minutes`, and `demandId` the run's demand where it holds
   one. A seat or a session agency refuses is answered 503
   with agency's sentence; a seat already asked is cancelled and nothing is left standing.
5. **The answer at once:** 202 with the run's id, its key, its state (`waiting` while its seat waits for a
   place, `running` once admitted), its road and the moment its web tools close. The run's clock starts at the
   ask, so a seat that waits spends the run's own time.
6. **The admission:** a seat still waiting is asked for its place again every half second (`PATCH` naming
   `admitted`, and `within` the run's demand where it holds one) until it is let in, then bound to the session (`PATCH` naming `bound` with the session's id). A
   seat the pool ends, or one still waiting when the clock runs out, ends the run `failed`.
7. **The first turn:** the request, as one turn of the session (`POST /sessions/{id}/turns`) carrying the work
   parts — the provider, the model, the seat's mapping (model, effort, tier), a fresh resume, an empty folder,
   the name `research-<run>`, the title, the room or 0, the key as the work key, no words heard and no
   variables — waited on until it ends; what it wrote is its whole text (「What a turn wrote」). The session is
   then ended (`DELETE /sessions/{id}`).
8. **The rounds:** the key's rounds are asked of a second session of their own, started the same way with the
   round's record (the run's with `branches` 0 and no `branchesAt` or `branchesToken`) and the capabilities
   less `web.branches` and `web.rulebooks`, one turn each, named `research-<run>-<round>`, each prompt the round's
   question, the task as the run was given it and the answer written last (「The rounds」). The last answer is the
   run's result. The rounds' session is ended after the last, and after the whole document and the narration
   where they are asked.
9. **The whole document and the narration, for a deep dive:** before the session that wrote it is ended — the
   rounds' session, else the first where the key has no rounds — a deep dive's result short of its shape is
   asked once more for the whole document (「The deep dive's page」), and a result that carries no `LISTEN:`
   script is asked for one (「The narration」).
10. **The end:** the seat is given back (`finished`, its outcome `answered` or `failed: <why>`), the result is
   drawn into its document and filed (「The document」; a deep dive's held to its shape first, 「The deep dive's
   page」), the run is closed in the ledger with its outcome, the document and its link, and its room is told
   (「The room」). Every step is written into the run's log.

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

## What a turn wrote

- **A turn's text is the whole of what it wrote.** Agency answers a turn with its `answer` and its `text`,
  every text block the turn wrote, in order; the `answer` may be the last block alone, so a result written in
  more than one block is never read from it. Where the blocks hold more than the `answer`, what the turn wrote
  is the blocks joined in order, from the last block that opens a result's head — one holding an `ANSWERED:` or
  a `TITLE:` line — to the end, or every block where none opens one; no block after the head is ever left
  out. Where the blocks hold no more than the `answer`, the `answer` is what it wrote, and the blocks joined
  where it names none.

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
- **A branch of a run holding a demand** asks the arbiter's master its own first, for the service the engine
  watch maps the branch's model to, `within` the run's demand, and waits for the grant until research
  closes; its seat's admission names `within` that demand and its session stands on it as `demandId`, and
  it is given back as the branch ends (`docs/api.md`, 「The engine」).
- **A branch ends** `completed`, `stopped` (cut at the close, its notes as far as they got) or `failed`, its
  session ended and its seat given back with the same word; a caller that lets go of the ask ends it the same
  way.

## The document

- **Every result that may be filed leaves a document,** filed with the artifact service's `POST /documents`
  and given a share link with `POST /shares` that ends a day after it is made; the document stays in the
  archive. The result is the searcher's structure lines (`prompts/search/task.md`); the document is made of
  them and carries none of them:
  - **its head:** the title the `TITLE:` line, the description the `TOPIC:` line, the kind `learning` where
    `KIND: learning` says so and `search` otherwise, `actorName` `research`, `scope` the room, and the labels
    `research` (the run's id), `key`, `query`, and `keywords`, `rule` and `topic` where the result writes them.
    The `ANSWERED:` line is the run's own and stands nowhere.
  - **its body:** each block a section under a heading in the question's language, the words of
    `prompts/search/document.toml` (Korean where the question holds Hangul, English otherwise): the verdict
    under the answer, a `SUMMARY:` block a result still writes folded into the answer's text (the answer itself
    where it writes no verdict), each `SECTION:` under its own name, the unconfirmed list, and the sources
    last, one link a line, titled by what each supports. A set's entries stand as sections of their own; a menu
    as its own list.
  - **its pictures:** every `IMAGE:` line's picture fetched by a guarded direct fetch — every hop's host a
    public address, at most three redirects, an image type, at most 15 MB — at most twelve within forty seconds,
    attached under a name of its own (`POST /documents/{id}/attachments`), and the body set again with
    `PATCH /documents/{id}`; a picture not kept is drawn from its address.
  - **the narration:** a `LISTEN:` script is lifted out of the body. A run told to its room files it nowhere;
    the deep dive's document is given it as sound, attached beside its page (「The deep dive's page」).

## The narration

- **A deep dive's result always carries a script for the ear.** Where the result its last turn wrote has no
  `LISTEN:` block, or an empty one, the same session is asked one more turn, named `research-<run>-narration`,
  for the narration alone: the narration question (`prompts/runner.toml`, `[narration]`), the deep-dive
  rulebook's section on the narration, the task as the run was given it and the answer written last. The
  narration says what the thing is, the verdict and the one thing to know, in plain spoken sentences in the
  question's language, with no heading, list or structure line.
- **That turn's text is the script,** a `LISTEN:` line it wrote itself dropped, set after the result under a
  `LISTEN:` line of its own; the result with it is the run's result and what the ledger keeps as `written`.
  It is asked after the whole document (「The deep dive's page」), so the script is set after the result that is
  filed; only then is the result held to its shape and filed.
- **A narration turn that writes nothing,** or fails, leaves the result as it stands, and the document has no
  sound. Where the rounds' session never started, the first answer stands without one. A run told to a room
  is asked no narration.

## The deep dive's page

A run the daemon asks for its own deep dive is filed differently from a run told to a room: as one page the
daemon finishes, held first to the shape the deep dive asked for.

- **The shape is checked before anything is filed.** The deep dive names the section headings its item's shape
  holds, in every language a result may be written in (`prompts/document.toml`, `[sections]`). A result is
  whole, and may be filed, only where it carries all of these:
  - the head lines the task requires of every result: the `ANSWERED:` line and the `TITLE:` line;
  - a body that opens with the answer: its first heading the answer's, in the words of the question's language
    (`prompts/search/document.toml`), so a body opening with a section's continuation is never whole;
  - at least half of the shape's section headings in one of the languages they are listed in, a heading
    written in Hangul being Korean and any other English, each held by a heading of the body (matched without
    regard to case, a heading's numbering aside).
- **A result short of its shape is asked for once more,** before its session is ended: one more turn of the
  session that wrote it, named `research-<run>-whole`, its prompt the whole-document question
  (`prompts/runner.toml`, `[whole]`) with what was missing set in, the task as the run was given it and the
  answer written last. That turn's text is the result, and it is the one judged; a turn that writes nothing
  leaves the result as it stood. Where the rounds' session never started, the first answer is judged as it is.
- **A result still short of its shape is not filed:** the run closes `failed`, its error naming each thing
  missing — an empty body, a head line, the answer at the body's open, or the headings looked for — and the deep
  dive's try fails with that error, so the item waits for its retry and the queue shows why. No document
  stands for it and no notice goes out.
- **The title** is the `TITLE:` line. A body that opens with a heading saying the title loses that heading,
  since the page's head says it.
- **It is filed as a page,** `kind` `page`, its `html` the page drawn in English alone (「The page」), its head
  and labels as a room's document's are, with the label `languages` `en`; its pictures are kept beside it as a
  room's document's are, and the page draws every picture from its address.
- **The daemon finishes it** (`docs/api.md`, 「The page」): the Korean side, with the page drawn again with
  both languages and set with `PATCH /documents/{id}`, and the narration's sounds attached beside it, before
  its report is told. The result stays in the ledger as `written`, which the daemon draws the page from
  again.

## The page

- **One HTML page holds every language the document has,** Korean first where it has it, then English, each
  side its title, its description and its body, the body's markdown written as HTML by a maintained library
  with any raw HTML the result wrote shown as text.
- **The head** is the archive design's masthead: a `header` opening the body with, where the page holds two
  languages, a small switch above the title naming each language in its own words (`prompts/page.toml`),
  Korean pressed first. Pressing a language shows its side and hides the other. The switch is plain inline
  script and style, the style marked `data-archive` and drawn in the design's colour variables, so the page
  reads in the light and the dark scheme alike.
- **The listen band stands under the title,** after the `header`, once the run has attached a narration
  (`docs/api.md`, 「The page」): a band element holding its heading in each language side's own words
  (`prompts/page.toml`, `listen`), each marked as its side, and one `audio` element for each narration
  attached, written as plain markup — its address `attachment:` and the sound's name, percent-encoded where
  the name needs it, and `data-language` its language — with no `controls`, no `data:` address and no
  script of the page's own. The archive draws the player: as the page is read it stands each `audio`
  element in its place as its own player element, keeping the attributes, and plays the sound kept under the
  name. The page writes the band and the references alone. Where it holds two languages the switch hides
  the other side's band heading and player by their `data-language`, selected by that attribute alone since
  the element is the archive's player by then, and pauses each hidden player.
- **Diagrams** are a body's fenced `mermaid` blocks, each written as a `pre` of the class `mermaid` with its
  text escaped, and drawn by the page itself: Mermaid, at the exact version the archive's reader draws with,
  loaded once over https and started on the page's load, strict, in the dark theme under a dark colour scheme
  and the neutral one otherwise. Both sides' diagrams stand in the page, so the switch draws nothing again. A
  diagram that does not parse stays as its text. A page with no diagram loads nothing.
- **A page with one language** has no switch and no hidden side.
## The room

- **The room is told through the receiver's `POST /instructions`,** as a `notice` under the kind
  `research-ended:<run>`; a run that names no room is sent with none, and the receiver hands it the owner's
  private room.
- **A filed result** is told as the answer in brief — the verdict, else a `SUMMARY:` block it still writes,
  else the title — and the one link, with the shape the chat agent answers in: one or two short lines in the
  owner's language and its own voice, then the link, and no grounds, sources or document text in the chat.
- **A result that could not be filed** is told whole; **a run with no answer** is told what it could not do;
  **a cut run** hands what it had written as unfinished, or one line saying the work was cut off.
- **A run the daemon asks for its own deep dive tells no room:** the dive's notices are its report
  (`docs/api.md`, 「The deep dive」), and its document is the page the daemon finishes (「The deep dive's
  page」).

## Ending a session

- **A session is ended once its turn has answered.** Agency answers the end 409 while a turn of the session
  runs and does not interrupt it; the session's bound cuts such a turn. An end answered 409 is asked again
  every two seconds until the turn is over, at most for what is left of the session's bound.

## A restart

- **A run is this unit's process's own.** A daemon that starts closes every run its store holds open as `cut`
  before its routes stand: each session the run held is ended, each seat it held given back, and its room told
  what it had written, as a cut run is.
