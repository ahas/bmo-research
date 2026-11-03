# The research runner

How the research unit runs a research: a search at one of three depths, or a browse, asked of it by the chat
agent through the tool `research/run`, by the daemon's own deep dive, or by any caller of `POST /research-runs`;
what it is composed of, what it is held to, the slot and the sessions it goes out in, its rounds, its branches,
and how its result is filed and told. Agency supplies the slots and the role's sessions; everything else of a
run is this unit's.

## The keys

- **A run is asked at a key:** `search:basic`, `search:thorough` and `search:exhaustive` go out on agency's
  search line, `browse` on its browse line. A run holds the role the settings name for its line — `voyager`
  for the search line, `seeker` for the browse line (`docs/settings.md`, `[runner]`) — and goes out on the
  provider, the model and the effort that role's order gives its slot as agency lets it in, unless the ask
  names a road of its own (「One run, step by step」). Its turns go out on the road its slot was let in on,
  which agency reads off the session, and carry no work parts of their own.
- **Each key has its settings** (「What a run is held to」) and its rounds (`prompts/runner.toml`).

## What a run is held to: the research settings

- **One set, the owner's,** kept in the store and changed through `PUT /research-settings`; where nothing was
  changed, the settings file's `[research_settings]` stand (`docs/settings.md`):
  - **the branch role:** the role a search's branches hold, one agency's `GET /roles` lists;
  - **each key's branches:** how many branches a run of it may open;
  - **each key's budget:** how many pages its first answer may fetch, and how many each branch may. The request
    states it as a ceiling; the record carries it as `pages` and `branchPages` for the web tools that hold a
    run to it;
  - **the fetch strategy** (`direct-then-browser`, `browser-first` or `direct-only`) and **the gaps** between
    two direct requests to one host and between two requests to one search engine, each null where the web
    tools' own default stands.
- **The settings travel on the run's session record** under `labels.research`, written by agency from the
  `labels` this unit hands at the session's start: `run` (this unit's run id), `owner` (who asked),
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
  when, the options, the depth, the branches it may open and its budget line; a browse's the task. Neither
  names a span of time: nothing of a run is bounded by a clock (「Nothing of a run is clocked」).

## One run, step by step

1. **The ask:** `POST /research-runs` with the key, the question and, where they are known, who asked, the
   options, the room the result is for, and the provider and model the run is to go out on. A key that is no
   research key, an empty question or a provider named without its model is refused 400. The daemon's deep
   dive asks its runs here too, each with the arbiter demand its try was granted where its road is watched
   (`docs/design.md`, 「The engine a run needs」); such a run's slot and sessions stand on that demand.
2. **The composition:** the settings read, the system prompt and the request composed; a search with no web
   listing is refused 503.
3. **The slot:** agency's `POST /slot-requests` with `requestKey` and `owner` `research:<run>`, `role` the role
   of the run's line, `source` the line, and `provider` and `model` where the ask names a model: the road the
   slot is let in on in place of its role's order. An ask naming both names them; an ask naming a model alone
   names that model on the provider the role's order holds it under — the first member of the order naming
   that model, else the provider the role stands on now, else the order's first — read off agency's
   `GET /roles/{role}`, since a slot's road names its provider and its model together. It is answered with its
   token and its state; the road of a slot whose ask names none is its role's order's, chosen as the slot is
   let in. A request key names one request for good, and once that request is given back agency refuses any
   ask under it, so every slot this unit asks goes under a key none of its slots was asked under before: a
   run's under its run, asked once; a branch's under its run and its index, which the allowance counts once
   each (「Branches」); and a review's under its run and the count of its asks (「The review」). A slot or a
   role agency cannot be asked for — agency not reached, or answering with a failure of its own (5xx) — is
   answered 503 with agency's sentence; one agency refuses for what it asks, a role it does not hold among
   them, or a slot answered with no token, is answered 502. Either way nothing is left standing.
4. **The answer at once:** 202 with the run's id, its key, its state (`waiting` while its slot waits for its
   place, `running` once let in) and the road its slot stands on as it is answered — the one the ask named,
   else the one it was let in on where it is let in at once — empty while a slot whose ask named none waits:
   the run's road stands on the run once its slot is let in.
5. **The admission:** a slot still waiting is asked for its place again every half second (`PATCH` naming no
   state, and `demandId` the run's demand where it holds one, which the slot stands on with no card of its own
   asked) until it is let in, however long its role's line takes. Each admission is answered with the
   request's `state`: `waiting` or `reserved` is asked again, `active` lets the run in, and `released` or
   `cancelled`, a request given back before it got a place, is never asked for again: it ends the run with no
   answer, the run cut (「A cut run」), its log naming who gave it back and why, agency's `endedBy` and
   `outcome`. The run's road is then the slot's provider, model and effort, the road its ask named
   where it named one; a slot let in with no model ends the run failed.
6. **The session:** agency's `POST /sessions` with `scope` `role`, `slot` the slot's token, `line` the line,
   `systemPrompt` the composed instructions, `labels` `{research: <the run's record>}`, `toolsAllowed` the
   run's capabilities, and `demandId` the run's demand where it holds one. It names no role, provider, model
   or folder: the session runs as the slot's role on the provider, model and effort the slot was let in on,
   at the line's own directory, and agency binds the run's slot to it as it starts it (「The slots'
   children」). It names no `boundMs` either, since no turn of a run is cut (「Nothing of a
   run is clocked」). Agency answers the session's id, `<role>@run:<n>`, which every later call on it names.
   A session agency cannot start ends the run with no answer: cut where agency was not reached or failed on
   its own side (「A cut run」), failed where it refused what was asked. A run that continues from an earlier
   run's result asks no first turn, so it starts no session here: its first is the rounds' (「A run that
   continues」).
7. **The first turn:** the request, as one turn of the session (`POST /sessions/{id}/turns`) carrying the
   turn's own parts — a fresh resume, the name `research-<run>`, the title, the room or 0, no words heard and
   no variables — its road, effort and folder the session's, waited on until it ends (「A turn's send」); what
   it wrote is its whole text (「What a turn wrote」). A run that continues from an earlier run's result asks no
   first turn: that result is its first answer (「A run that continues」).
8. **The rounds:** the key's rounds, or those a continuing run has left (「A run that continues」), are
   asked of a session of their own, started the same way on the run's slot, with the round's record (the
   run's with `branches` 0 and no `branchesAt` or `branchesToken`) and the capabilities less `web.branches`
   and `web.rulebooks`; its start moves the slot onto it, and the first answer's session is then ended
   (「The slots' children」). One turn each, named `research-<run>-<round>`, each prompt the round's question,
   the task as the run was given it and the answer written last (「The rounds」). The last answer is the run's
   result. A rounds' session that does not start leaves the first answer standing as the result, on the
   session the slot is still bound to.
9. **The whole document and the narration:** in the session that wrote the result — the rounds' session,
   else the first answer's where the key has no rounds, started on the run's slot for them where the run asked
   no first turn — a deep dive's result short of its shape is asked
   once more for the whole document (「The deep dive's page」), and any result that carries no `LISTEN:`
   script is asked for one (「The narration」).
10. **The end:** the slot is given back (`finished`, its outcome `answered` or `failed: <why>`) and the
   session it is bound to then ended (`DELETE /sessions/{id}`), every session before it ended already (「The
   slots' children」); the result is drawn into its document and filed (「The document」; a deep dive's held to
   its shape first, 「The deep dive's page」), and the run is closed in the ledger with its outcome, the document and its link. A run told to its
   room then has its document's narration spoken and attached, and its room is told once the sound is
   (「The narration's sound」, 「The room」). Every step is written into the run's log.

## Nothing of a run is clocked

- **A run ends when its work ends:** its last turn answered, a step of it failing on its own, or a loss
  (「A cut run」). No span of time ends, refuses or shortens any of it: the record names no closing time, so
  the web tools stay open to the run for as long as it works, held to its page budget alone; the rounds are
  asked in turn to the last; no session this unit starts names `boundMs`, so agency cuts none of its turns;
  no turn's send carries a clock of this unit's; and a slot, the run's or a branch's, and a branch's card are
  waited for however long the role's line and the arbiter take.
- **A turn whose agent never ends is stopped, never timed out:** the owner interrupts it through agency, by
  the run name it goes out under (「One run, step by step」), and the turn answers with what it wrote by then,
  the run going on from there as from any turn that answered; the daemon's restart cuts the whole run
  (「A restart」).

## The slots' children

- **Agency binds a slot to each role session started on it,** as it starts that session: the start moves the
  slot's lease onto the session it opens, the session the lease was bound to before is bound to it no longer,
  and ending the session the lease is bound to gives the slot back (agency's `docs/role-slots.md`, 「The
  line」). This unit binds no slot itself.
- **A branch's, a review's and a notice's slot carries one session,** started once the slot is let in and
  ended once its one turn is over, which gives the slot back with it.
- **The run's slot carries the run's sessions one after another:** the first answer's, then the rounds',
  which also asks the whole document and the narration. Each is started on the slot before the session the
  slot is bound to is ended, so the slot is bound to the session the run works in and to no other, and no
  session stands open for the slot's sake alone. The run's turns go out each under a run name of its own.
- **Each slot is still given back by this unit,** with the word its work came to. The run's is given back
  before the session it is bound to is ended, so its finish answers whether agency still held it then (「A cut
  run」). A branch's, a review's and a notice's is given back after its session is ended, so the slot stands
  until the turn is over; agency answers that finish as no longer held, the session's end having given it
  back, and it changes nothing.

## A turn's send

- **A turn is waited on until agency answers it,** however long the card its model needs takes to be granted
  — a voice, a bench or another run holding it — and however long the turn then writes.
- **A send answered 409 `session-busy`** finds its session still in a turn that an earlier send of this unit
  lost on the way, which agency carries to its end all the same. It waits for that turn's end: the session's
  state (`GET /sessions/{id}`) is read every two seconds until it names no run, and the turn is then sent
  again, as many times as it is answered busy. A session agency no longer holds (404) is a loss (「A cut
  run」).

## The rounds

- **Each key's list** stands in `prompts/runner.toml`: at the basic depth only the final question, at the
  deeper ones and for a browse every question in turn — what the task must establish, the sources it needs,
  the facts checked against a second source, the coverage of the question, the facts written into the brain,
  and the final answer. A list ends in `final` and holds it nowhere else.
- **A round's prompt** is its question, then the task as the run was given it under a heading of its own, then
  the answer written last under another. Each round is a fresh turn: the session carries nothing between them.
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
- **A branch** is a slot of the settings' branch role and a session of that role, both on the run's road — the
  slot asked with the run's provider and model, the session started on that slot once it is let in — on the
  search line and the branch instructions, its record the run's with `branchOf` the run's id, `pages` the
  run's `branchPages`, `branches` 0 and no `branchPages`, `branchesAt` or `branchesToken`. Its slot is waited
  for however long its role's line takes, while its run is running, and agency binds it to the branch's
  session as it starts it (「The slots' children」). Its one turn carries the angle, framed as data; the turn is
  waited on until it ends (「A turn's send」).
- **A branch of a run holding a demand** asks the arbiter's master its own first, for the service the engine
  watch maps the branch's model to, `within` the run's demand, and waits for the grant however long it takes,
  while its run is running; its slot's admission and its session both stand on that demand, naming it as
  `demandId`, and it is given back as the branch ends (`docs/api.md`, 「The engine」).
- **A branch ends** `completed` or `failed`, its session ended and its slot given back with the same word; a
  caller that lets go of the ask ends it the same way, its slot given back as stopped.

## The document

- **Every result that may be filed leaves a document,** filed with the artifact service's `POST /documents`
  and given a share link with `POST /shares` that ends a day after it is made; the document stays in the
  archive. The result is the voyager's structure lines (`prompts/search/task.md`); the document is made of
  them and carries none of them:
  - **its head:** the title the `TITLE:` line — a noun phrase naming the subject, never a sentence or a
    verdict, at most 15 Korean syllables or 30 characters in English (`docs/design.md`, 「Reports and
    rooms」) — the description the `TOPIC:` line, the kind `learning` where
    `KIND: learning` says so and `search` otherwise, `actorName` `research`, `scope` the room, and the labels
    `research` (the run's id), `key`, `query`, `category` (the key, the archive's category for the document;
    bmo-artifact's `docs/artifact.md`, 「Where a person reads it」), and `keywords`, `rule` and `topic` where the
    result writes them.
    The `ANSWERED:` line is the run's own and stands nowhere.
  - **its body:** each block a section under a heading in the question's language, the words of
    `prompts/search/document.toml` (Korean where the question holds Hangul, English otherwise): the verdict
    under the answer, a `SUMMARY:` block a result still writes folded into the answer's text (the answer itself
    where it writes no verdict), each `SECTION:` under its own name, the unconfirmed list, and the sources
    last, one link a line, titled by what each supports. A set's entries stand as sections of their own; a menu
    as its own list.
  - **its pictures:** every `IMAGE:` line's picture fetched by a guarded direct fetch — every hop's host a
    public address, at most three redirects, an image type, at most 15 MB — at most twelve, each fetch waited
    on until its host answers or its connection breaks, attached under a name of its own
    (`POST /documents/{id}/attachments`), and the body set again with `PATCH /documents/{id}`; a picture not
    kept is drawn from its address.
  - **the narration:** a `LISTEN:` script is lifted out of the body and given to the document as sound: a
    run told to its room has it attached at the document's head (「The narration's sound」), a deep dive's
    page beside its page (「The deep dive's page」).

## The narration

- **Every run's result carries a script for the ear,** a run told to its room's and a deep dive's alike.
  Where the result its last turn wrote has no `LISTEN:` block, or an empty one, the same session is asked one
  more turn, named `research-<run>-narration`, for the narration alone: the narration question
  (`prompts/runner.toml`, `[narration]`), the search-kinds rulebook's section on the narration, the task as
  the run was given it and the answer written last. The narration says what the thing is, the verdict and
  the one thing to know, in plain spoken sentences in the question's language, with no heading, list or
  structure line.
- **That turn's text is the script,** a `LISTEN:` line it wrote itself dropped, set after the result under a
  `LISTEN:` line of its own; the result with it is the run's result and what the ledger keeps as `written`.
  A deep dive's is asked after its whole document (「The deep dive's page」), so the script is set after the
  result that is filed; only then is the result held to its shape and filed.
- **The script is written words.** It keeps names, acronyms and figures as written, since it is the
  narration's transcript as well as what is heard: the voice unit derives the spoken words from it
  (`docs/api.md`, 「The page」).
- **A narration turn that writes nothing,** or fails, leaves the result as it stands, and the document has no
  sound. Where the rounds' session never started, the first answer stands without one.

## The narration's sound

A run told to its room has its document heard as well as read, as a deep dive's page is, and its room hears
of it only once the sound is attached.

- **The script is spoken as it is written,** in the question's language: Korean where the question holds
  Hangul, English otherwise, the language the result is written in, so the owner asking in Korean hears
  Korean. It is not translated. The voice unit speaks it in the settings' voice (`docs/settings.md`,
  `[voice]`), the voice a deep dive's narration is spoken in, a script longer than the voice speaks cut at
  its last sentence within it, the run's log saying so.
- **The sound is made the way a page's is** (`docs/api.md`, 「The page」): its synthesis asked under the key
  `research:<documentId>:<language>`, counted anew after one that ended without its sound, kept until its
  sound is attached so the narration has one synthesis living at a time, and followed until it is spoken or
  has failed, however long its card takes. A synthesis that failed, or that the voice unit no longer holds,
  is set aside and the narration asked again on the next retry wait, `retry_minutes` after its try; no count
  of tries ends it.
- **It is attached at the document's head:** `narration-<code>.ogg`, `code` the question's language, with
  `role` `head`, which the archive's listen band plays at the head of the document (bmo-artifact's
  `docs/artifact.md`, 「How a document is drawn」).
- **The document's page is kept as a deep dive's is** (`docs/store.md`, `pages`): the language it holds, the
  narration it plays, what it still lacks — `narration-<code>` — and when that is asked again. The document is
  filed with the labels `languages`, its language, and, where its result has a script, `pending`,
  `narration-<code>`, so the archive marks it in progress; the labels are set again without `pending` once
  the sound is attached.
- **The room is told once the sound is attached,** once: the answer in brief and the document's one live
  link (「The room」), its link the artifact service's `GET /shares` answers, else one made with
  `POST /shares`, else the link the document was filed with. A result with no script — its narration turn
  wrote none — is whole as it is filed, and its room is told at once. Until then the room hears nothing of
  it; the run's log says what the document waits on under `filed`, each sound made or not, and why, under
  `narration`, and the telling under `told`.
- **The run is closed before its sound is made:** its slot is given back and its sessions ended, so nothing
  of agency stands for it while the voice speaks. What the document lacks is asked again on every retry
  wait, by the daemon's loop as a deep dive's pending page is, and a restart takes it up the same way.

## A run that continues

- **A deep dive's try continues from the run before it** where that run wrote a result — what the ledger
  keeps as `written` — and was asked the same question: the try hands the runner that run, and the new run
  takes that result as its first answer instead of asking the first turn, so nothing a run wrote is
  researched again because a later step of it failed or was cut (`docs/design.md`, 「Nothing is given up」).
- **The rounds go on after the last one the runs before it answered,** in turn; that round and those before
  it are not asked again, and a round that went out without answering is asked again. Where `final`
  answered, no round is asked; where none answered, every round is. The runs before it are the run it
  continues, the run that one continued, and so on, each run's own answered rounds read before those of the
  run it continued; the ledger keeps each run's answered rounds (`docs/store.md`, `research_runs`).
- **Then as any run:** the whole document where the result is short of its shape, the narration where it
  carries none, and the filing, on the new run's own slot and sessions. The run's log says which run
  it continues and from which round.
- **A run before it that wrote nothing,** or that was asked another question, leaves nothing to continue
  from: the try is researched from its start.

## The deep dive's page

A run the daemon asks for its own deep dive is filed differently from a run told to a room: as one page the
daemon finishes, held first to the shape the deep dive asked for.

- **The shape is checked before anything is filed.** The deep dive names the section headings its item's shape
  holds and its opening's headings, in every language a result may be written in (`prompts/document.toml`,
  `[sections]` and `[opening]`), and the item's origin line the page prints. A result is whole, and may be
  filed, only where it carries all of these:
  - the head lines the task requires of every result: the `ANSWERED:` line and the `TITLE:` line;
  - a body that opens with the answer: its first heading the answer's, in the words of the question's language
    (`prompts/search/document.toml`), so a body opening with a section's continuation is never whole;
  - where the shape holds an opening (a manual's and an investigation's), the section saying what the
    subject is and what it is for: a heading of the body holding one of the opening's headings in either
    language, matched without regard to case;
  - at least half of the shape's section headings in one of the languages they are listed in, a heading
    written in Hangul being Korean and any other English, each held by a heading of the body (matched without
    regard to case, a heading's numbering aside).
- **A result short of its shape is asked for once more,** before its session is ended: one more turn of the
  session that wrote it, named `research-<run>-whole`, its prompt the whole-document question
  (`prompts/runner.toml`, `[whole]`) with what was missing set in, the task as the run was given it and the
  answer written last. That turn's text is the result, and it is the one judged; a turn that writes nothing
  leaves the result as it stood. Where the rounds' session never started, the first answer is judged as it is.
- **A result still short of its shape is not filed:** the run closes `failed`, its error naming each thing
  missing — an empty body, a head line, the answer at the body's open, the opening section, or the headings
  looked for — and the deep dive's try fails with that error, so the item waits for its retry and the queue
  shows why; a run that met a loss on the way closes `cut` instead (「A cut run」). No document stands for it
  and no notice goes out.
- **The title** is the `TITLE:` line, a noun phrase naming the subject and its shape, as every run's is
  (「The document」). A body that opens with a heading saying the title loses that heading,
  since the page's head says it.
- **The opening section opens the page's body:** the first section whose heading holds one of the shape's
  opening headings is set before the answer, so the page says what the subject is before its verdict and its
  figures; the result keeps the order it was written in. The Korean side is translated from the body in that
  order.
- **It is filed as a page,** `kind` `page`, its `html` the page drawn in English alone (「The page」), its head
  and labels as a room's document's are, with the label `languages` `en`, `category` its item's shape in place
  of the key, and `pending` `korean`, since its Korean side is still to come; its pictures are kept beside it as a
  room's document's are, and the page draws every picture from its address.
- **The daemon finishes it** (`docs/api.md`, 「The page」): the Korean side, with the page drawn again with
  both languages and set with `PATCH /documents/{id}`, the narration's sounds attached beside it, and then
  the review (「The review」) and its fixes, before its report is told. The result stays in the ledger as
  `written`, which the daemon draws the page from again.

## The review

- **A deep dive's page is reviewed once, whole, before its report is told,** by the run's own model: once
  its Korean side is had and every narration its script allows is attached, and only while its items stand
  `finishing` and its document stands; a page whose report was told before, or whose document is gone
  (「The page」), is never reviewed.
- **Each ask of the review first gives back what its run still holds of agency,** by its owner
  (`DELETE /slot-requests?owner=research:<run>`): the run is closed, so all that can stand under its owner is
  an earlier review ask whose admission or give-back never reached agency, which agency keeps in the role's
  line, passed over but never dropped. A page therefore holds one review request at most. A release that
  does not go through leaves the review pending, asked again on the page's retry wait.
- **The review is one turn on a slot and a session of its own,** asked the way a run's are (「One run, step by
  step」): a slot with `requestKey` `research:<run>:review:<n>`, `n` the count of the review's asks the ledger
  keeps, counted before each ask so a review asked again is never asked under a request agency gave back,
  `owner` `research:<run>`, `role` the role of the run's line, `source` the line, and the run's provider and
  model; once it is let in, a session started on that slot on the run's line, its record the run's, no
  capabilities and no `boundMs`, its system prompt the deep-dive rulebook's section on the review
  (`prompts/runner.toml`, `[review]`) and the rulebook against destroying the system. The slot waits for its
  place however long that takes, with no clock of this unit's, and agency binds it to the review's session as
  it starts it (「The slots' children」); the one turn, named `research-<run>-review`, is waited on until it
  ends (「A turn's send」); the session is then ended and the slot given back. Its slot and its session are held
  under the run, so a restart gives them back. Agency asks the card its model needs for them, as for any slot
  that names no demand.
- **The turn is handed the finished page:** the review question, then the English side and the Korean side,
  each its title, its description and its body as the page holds them, then the English narration's script
  and the Korean narration's words as they were spoken.
- **It answers `PASS`, or its fixes,** each a block opened by a `FIX:` line naming the side — `en` or `ko`
  with the section's heading as the English side writes it, or `narration-en` or `narration-ko` — then a
  `WHY:` line, then the part rewritten whole: a section with its heading line first, a narration whole.
  An answer holding neither is taken as no fix, the run's log saying so; a turn that writes nothing, or a
  slot or session agency cannot give, leaves the review pending, asked again on the page's retry wait.
- **Each fix redraws the part it names alone,** every other part kept as it is:
  - **an English section** is set in place of the section under that heading on the English side, and the
    rewritten section alone is translated (`docs/api.md`, 「The page」) and set in place of the Korean
    side's section that stands at the same place among its headings;
  - **a Korean section** is set in place of the Korean side's section at that place, the English side left
    as it is; where the Korean side holds a different number of headings, the section is placed by the
    heading its rewrite opens with;
  - **the English narration** replaces the script, and both sounds are made again from it, the Korean
    translated again from the rewrite;
  - **the Korean narration** replaces the Korean words, and the Korean sound alone is made again.
  A sound made again goes through the narration's own path (`docs/api.md`, 「The page」): its synthesis kept
  until it is attached, under a key counted anew, so the voice unit makes a new one rather than answering
  with the first. It is attached under a name of its own, `narration-reviewed-<code>.ogg`, since the archive
  keeps one file under a name, and the listen band refers to it in place of the first.
- **A fix that cannot be placed** — no section under its heading — is left out, the run's log saying so; one
  whose translation or sound cannot be had yet stays pending, asked again on the page's retry wait. Once
  every fix is applied, or left out, the review is done: it is never asked again, and the page is not
  reviewed a second time.

## The page

- **One HTML page holds every language the document has,** Korean first where it has it, then English, each
  side its title, its description and its body, the body's markdown written as HTML by a maintained library
  with any raw HTML the result wrote shown as text.
- **A table stands as a table** where the result wrote it without the row under its head: lines that each
  open and close with a pipe, two or more running to the end of a paragraph the library reads, are given
  that row — a column under each cell of the first — and set apart from any prose the paragraph opens with,
  before the body is drawn. A document's tables are read the same way (`docs/api.md`, 「The fit」). The
  deep-dive rulebook asks for every table written whole, its separator row under its head; this mending
  holds where a result was written without it.
- **The head** is the archive design's masthead: a `header` opening the body with, where the page holds two
  languages, a small switch above the title naming each language in its own words (`prompts/page.toml`),
  Korean pressed first. Pressing a language shows its side and hides the other. The switch is plain inline
  script and style, the style marked `data-archive` and drawn in the design's colour variables, so the page
  reads in the light and the dark scheme alike.
- **Where the research came from stands under the title,** in the `header` after the description: one line
  for each language side, marked as its side, its label and its words those of `prompts/page.toml`
  (`[[language]]`: the label `origin`, then `source`, `asked`, or `follow_up` (`follow_up_untitled` where
  no title is known), then `because` where the ask said why), drawn from the item's origin as the deep dive
  hands it (`docs/api.md`, 「queue」): the source that found it; the owner's ask, the
  words it was made with quoted; or the document a follow-up follows from, by its number and its title,
  linked to its live link where that is known, with the sentence saying why it was asked quoted. Markdown
  marks in a title or a quoted sentence stand as text. A page drawn for no item, or for a brief, has no such
  line.
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
- **A citation mark is a footnote.** A body's sources list (「The document」) is a list whose items each open
  with a mark, `[n]`, or `[n†]` for a value read only in a search result's snippet, then the link to the page
  titled by what it supports. Each such item is a target named by its side's language and its mark, and a
  mark of the same side's text that names one links to it:
  - **the mark** stands raised, as a link to its entry; the first place a side cites an entry is a target of
    its own, which the entry's mark links back to, titled by the `back` words of its side's language
    (`prompts/page.toml`);
  - **hovering a mark, or focusing it from the keyboard,** shows what its source supports and its address in
    a box under the line, as wide as the reading column, drawn from the mark's attributes by the page's style
    alone; a tap follows the link, so on a phone the reader jumps to the entry and back;
  - **the entry jumped to and the mark jumped back to stand highlighted,** by the style's `:target` rule;
  - a mark naming no entry of its side, a mark inside a link or code, and the words of the entries
    themselves stay as written, and an entry whose mark the side never cites links back to nothing.
  The footnotes' look is a style marked `data-archive` and drawn in the design's colour variables, set only
  on a page whose text cites an entry. The page runs no script for them.
- **A page with one language** has no switch and no hidden side.
- **A page whose document is gone is closed as failed.** Before a page is drawn again or asked for what it
  lacks — a deep dive's or the document of a run told to its room — its document is read with the artifact
  service's `GET /documents/{id}`. Where the archive answers 404, the document no longer stands: the page is
  closed (`docs/store.md`, `pages`), lacking nothing, and is never drawn, asked for a part or reviewed again;
  its run's log says so under `filed`. Every item the document was filed for keeps the document's being gone
  as its `error`, and one still `finishing`, whose report nobody was told, waits for a try of its own after
  the retry wait, as after a failed try, so a fresh run researches it again. A run told to its room whose room
  was still waiting for the document is told once, its result whole, as a result that could not be filed is
  (「The room」). Any other answer to the read leaves the page to its work as it stands.

## The room

- **The room is told through the receiver's `POST /instructions`,** as a `notice` under the kind
  `research-ended:<run>`; a run that names no room is sent with none, and the receiver hands it the owner's
  private room.
- **A filed result** is told once its document's narration is attached (「The narration's sound」), as the
  answer in brief — the verdict, else a `SUMMARY:` block it still writes, else the title — and the one link,
  with the shape the chat agent answers in: one or two short lines in the owner's language and its own
  voice, then the link, and no grounds, sources or document text in the chat.
- **A result that could not be filed** is told whole; **a run with no answer** is told what it could not do;
  **a cut run** hands what it had written as unfinished, or one line saying the work was cut off.
- **A run the daemon asks for its own deep dive tells no room:** the dive's notices are its report
  (`docs/api.md`, 「The deep dive」), and its document is the page the daemon finishes (「The deep dive's
  page」).

## A cut run

- **A run is cut where what it stands on went away,** not failed. The run meets a loss where:
  - a call into agency for its slot, its session or a turn does not reach agency, is answered with a failure
    of agency's own (5xx), or is answered 404 for the slot or the session the call names, which agency no
    longer holds;
  - agency gives its slot back before it got a place, the admission answering `released` or `cancelled`;
  - its slot, given back at its end, is one agency no longer held (the finish answering `held` false), or the
    finish does not reach agency.
- **A run that met a loss and filed no document closes `cut`,** its error naming the first loss; one that
  filed its document all the same closes `answered`. A round, the whole document or the narration that did
  not come for a loss leaves the result as it stands, as any turn that does not answer does, and the result
  is then judged as ever.
- **Anything else that ends a run with no document closes it `failed`:** a turn that wrote nothing while
  the run's slot stood, agency refusing a call for what it asked (another 4xx), a result short of its shape,
  or a filing that did not go through. A deep dive's try reads the outcome: a cut run's try is not counted,
  a failed one's is (`docs/api.md`, 「The deep dive」).
- **A cut run asked for a room tells it as a cut run** (「The room」), and the restart's cut is the same
  outcome (「A restart」).

## Ending a session

- **A session is ended once its turn has answered.** Agency answers the end 409 while a turn of the session
  runs — one whose send was lost on the way — and does not interrupt it. An end answered 409 is asked again
  every two seconds until the turn is over, however long that takes.

## A restart

- **A run is this unit's process's own.** A daemon that starts closes every run its store holds open as `cut`
  before its routes stand, and gives back everything the run held of agency by its owner: one
  `DELETE /slot-requests?owner=research:<run>` gives back each slot the run, its branches and its review
  asked, waiting or let in, and ends each session agency bound one of them to; each session the run held that
  agency did not end is then ended once its turn is over. Its room is told what it had written, as a cut run's
  is. A closed run whose review held a slot or a session at the restart is given back the same way, under its
  run's owner, and the review stays pending, asked again on its page's retry wait. A notice to an agent under
  way at the restart is given back too: its session is ended once its turn is over, and then everything its
  owner `research:notice:<itemId>` holds is given back, so the checkout is never let go under a turn still
  going (`docs/api.md`, 「The notices」); it is not told again.
- **A closed run whose document still lacks its sound** is no open run: its synthesis stays kept, and its
  narration is followed or asked again on its retry wait, its room told once the sound is attached
  (「The narration's sound」).
