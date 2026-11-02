# The research daemon

bmo-research is the platform's research unit: a daemon that keeps a queue of things to research, finds new
ones on the sites the owner registers, deep-dives each one in the lab's own routine, files the report in the
archive and tells the agents and rooms that should hear of it. It researches and measures, and notifies;
it launches nothing, and the one job item it files is the repair of its own stuck research, for the
helper: what a report calls for is the chat agent's to raise on the owner's word. It is the home of the
research feature; agency supplies it sessions and seats and nothing more.

## What it is for

- **Bongbos's routine, running on its own.** The owner's collaborator reads a maker's material, measures
  what it claims on the lab's own hardware and reports in a fixed shape (the rulebook
  `prompts/rules/research.md`: the investigation, the manual and the verification). The daemon does the
  first two shapes by itself, continuously, and the third on the owner's word.
- **Continuous, capped and cheap.** The queue has a cap; whenever a slot frees, the daemon fills it from
  what its sources turned up, unconditionally, and goes on until the owner stops it. The runs go out on
  kakia, the lab's own model, which costs nothing.
- **Semi-automatic.** A deep dive that finds a thing the maker's own documents show usable in the lab is
  reported to the owner first, as a notice the chat agent delivers. On his word the chat agent raises the
  verification — the maker's figures re-measured on our cards under a bench lock, by a job that follows the
  researching-and-benching skill — and what passes is registered with the arbiter as a service. The daemon
  is the researcher and the notifier in that exchange, never the one that files or launches.

## Principles

- **It calls other units through their documented APIs alone** and depends on no other unit's code:
  - agency, for a seat of the co-worker pool and a thrown-away session on its search or browse line, and
    the turns into it (「Where the runner lives」);
  - the tool host, for the web namespace's listing a search's instructions carry and for the web tools'
    search engines' state;
  - explorer, for the web, through the tool host's `web` namespace in the runs it asks for; the daemon's
    own scan reads a feed or an API with its own HTTP client, since a feed is not a page an agent reads, and
    a `page` source's listing through the tool host's `web/web_fetch`, so explorer's pacing and Chrome
    stand round it;
  - artifact, to file a run's document, draw a deep dive's page again and keep its narration beside it
    (`POST /documents`, `PATCH /documents/{id}`, `POST /documents/{id}/attachments`, `POST /shares`), and to
    read a filed page as it keeps it, so a page drawn otherwise now is set again
    (`GET /documents/{id}/content`);
  - the voice unit, to speak a deep dive's narration (`POST /syntheses`);
  - the receiver, to tell a room (`POST /instructions`) and to learn the owner's private room (`GET /owner`);
  - the arbiter's master, to ask for the engine each run needs and to read why it waits (「The engine a run
    needs」).
  - the job service, for one item alone: the repair of an item stuck past its retries, filed for the helper
    (「Nothing is given up」).
  Any other job is the chat agent's to file.
- **Every per-deployment choice is a setting or a registration,** never code: the sources, the cap, the
  cadence, the model's road, the report rooms, the token files (`docs/settings.md`).
- **The daemon decides nothing a person should and files nothing an agent should.** It never approves a
  verification, never raises a queue item for what a report calls for, never registers a service with the arbiter, and never spends a
  paid road unless a setting names one. It notifies: every report, every `fit` item and every item stuck is
  an instruction the receiver hands to the room's agent, which says it in its own voice and acts on the
  owner's word.
- **A base never knows its consumers** and this unit is a consumer of every unit it calls; nothing here is
  branched on by agency, explorer or the arbiter.

## The loop

```
sources ──scan──▶ candidates ──new?──▶ queue (cap) ──slot free──▶ deep dive ──▶ report ──▶ notice to the room's agent
                                                                                                  │
                                                     the owner's word ◀── the chat agent says it ◀┘
                                                              │
                                                              ▼
                              the chat agent files the verification job ── bench lock ── registration
```

1. **Scan.** On the cadence, each source is read and its items are turned into candidates: a name, the
   maker, the address of the maker's own page, the date it appeared, and the source it came from.
2. **New?** A candidate is new where no seen row holds its key (the canonical address, else the name
   lower-cased), no archive document carries its name in its title or labels (`GET /documents?q=`), and
   the owner has not dismissed it. A new candidate is kept as seen the moment it is read, so it is judged
   once.
3. **The queue.** The queue is the researcher's own, a table of its store that says what is researched
   next; it is no part of the job service's queue, which holds work for the workers and which the research
   unit never writes. New candidates wait in the queue by the order they were found; one the owner asked for
   by name stands first. Each category's part of the queue holds at most its `cap` items waiting or running; a scan that finds more
   keeps them as seen but unqueued, and the next scan that finds the queue short takes the oldest
   unqueued first. The queue never idles while something is unqueued.
4. **The deep dive.** As many runs at once as the arbiter grants engines for, up to `concurrency`
   (「The engine a run needs」): the item becomes a research run on kakia's road at the exhaustive depth, asked as a manual where the thing is a model, an engine, a
   dataset or a tool, and as an investigation where it is a claim or an event. The run's question is the
   item's name, its maker's address and where it came from — the source that found it, the owner's ask, or
   the earlier research a follow-up continues — with the shape named, so the runner's research rulebook
   applies. A try cut short by something it needs going away is asked again at once, uncounted; a try that
   fails on its own content is asked again after `retry_minutes`, and past its retries the item is stuck
   and the helper is asked to fix the cause (「Nothing is given up」).
5. **The report.** The document the run filed is the report: it opens by saying what the thing is and
   what it is for, and under its title says where the research came from. The daemon tells every report
   room: the share link and one line saying what the thing is, naming the earlier document a follow-up
   continues, nothing else — never the grounds, never the sources. The first room is the owner's private room, read from the receiver at start; rooms are added
   and removed through the routes and the tools.
6. **The word.** A report whose document says the thing fits the lab (the manual's section on running it in
   the lab, under the heading `prompts/document.toml` names, names a card it fits) is marked `fit`, and its notice says so and asks for the owner's word. The notice
   carries what the chat agent needs to act on that word: the report's link, the thing's name and the
   reproduction plan's place in the document. The owner answers in the room; the chat agent files the
   verification item as it files any work, with the report's link in the brief, and tells the daemon the
   item is taken (`PATCH /queue/{item}` with `taken` and the item's number) or dismissed. An item nobody
   answers stays `fit` and is listed, never nagged about.
7. **The verification** is a job of the worker's, under the researching-and-benching skill and the bench
   lock (「The bench lock」), reported by the job itself as every job reports. Registering a service with
   the arbiter is the job's last step, on the owner's word in the item. The daemon takes no part in it
   beyond its own runs waiting while the bench holds the engine they need (「The engine a run needs」).

## Categories

Research is done by category: each source belongs to one, and the category says why the daemon reads
it, what a deep dive of its items produces, where the report goes and how much of the queue it may
hold. A category is a registration (`docs/settings.md`): its name, its purpose in a line, its report
shape, its cap, its cadence, its road, and its report rooms. Three stand at the start:

| Category | Purpose | What a deep dive produces | What follows |
|---|---|---|---|
| `it` | new AI technology, as Bongbos's lab researches it | the manual or the investigation (the rulebook `research.md`): the maker's own material, the lab's fit, the reproduction plan | a `fit` item is reported for the owner's word; approved, a verification job under the bench lock, and a service registered with the arbiter |
| `security` | vulnerability news that bears on the platform's own stack | an advisory investigation: what the flaw is, the affected versions, whether and where the platform runs them (the stack inventory below), the vendor's fix and the patch steps | each repository's worker is notified of its own part; the patch is the worker's work |
| `economy` | the economic news and indicators the owner's later portfolio and trading system will read | a brief, not a deep dive: the items of the day in one document with the figures they carry, the sources, and the facts kept for the economy store | the brief is notified to the owner's room now and to the trading agent later; the facts stand in the store for it to read |

- **The shape is the category's, not the source's.** `it` asks the research run for a manual or an
  investigation; `security` asks for the advisory shape; `economy` gathers a day's items into one run that
  writes the brief. The question templates under `prompts/` are one per shape.
- **The security category reads advisories and fixes, never exploits.** Its sources are vendor advisories,
  CVE feeds and distribution security lists; its deep dive judges exposure from the platform's stack
  inventory — the distributions, runtimes, libraries and programs the platform runs, kept as a registration
  the owner and the jobs maintain — and names the patch; writeups of how a flaw is exploited are not read
  past what the exposure judgement needs. A reported item says which hosts and repositories it touches.
- **The economy category keeps facts.** Each brief's figures (an index, a rate, a price, a release) are
  written to the store with their source and date, so a later system reads a series, not documents.
- **Each category has its own cap, cadence, road and rooms,** and a category can be stopped alone. The
  owner's private room is every category's first room.
- **The categories take turns at `concurrency`.** Of the running categories with a try due, the next try
  goes to the one whose last try was asked longest ago, one never tried first; an item's priority orders its
  own category's part of the queue and never another's. So one category's flood, or a string of asks in it,
  never starves another (`docs/api.md`, 「queue」).
- **A category the owner adds later** needs a shape: one of the three above, or a new template under
  `prompts/` with its rulebook under `prompts/rules/`, named in `prompts/runner.toml`.

## Sources

A source is a registration: a category, a kind, an address and, where the kind takes them, a query and a filter.

| Kind | Reads | Items |
|---|---|---|
| `huggingface-trending` | the Hub's models API ordered by trending, a page | model repositories: id, author, last modified, tags |
| `huggingface-search` | the Hub's models API with a search and a sort | the same |
| `arxiv` | the arXiv API with a query, newest first | papers: id, title, authors, submitted, abstract |
| `github-releases` | a repository's releases API | releases: tag, name, published |
| `rss` | a feed | entries: title, link, published |
| `page` | a listing page read through explorer's `web_fetch` where a maker's blog has no feed, directly or in Chrome | the links the page lists that match the source's pattern: text, address |

- A source carries `every_minutes` where it is read more or less often than the daemon's cadence, and a
  `filter`: words an item must carry and words that rule it out, so a trending page's chat fine-tunes do
  not fill the queue.
- A source that fails is said once in the log and read again next time; its failures are counted on the row
  and shown.

## Where the runner lives

- **The research runner is this unit's** (`docs/runner.md`): the search and browse instructions, the
  rulebooks, the request, the research settings, the rounds, the branch allowance, the filing of the
  document and the room's instruction, and the run's ledger and log. Agency supplies a seat of its co-worker
  pool and thrown-away sessions on its search and browse lines, whose system prompt this unit hands in whole
  and whose record carries the run's settings under the labels this unit names; it runs nothing of research
  on its own.
- **Every research goes through the one runner:** the chat agent's `research/run`, any caller of
  `POST /research-runs`, and the daemon's own deep dive, which asks it on the road its settings name,
  kakia's: provider `registered`, model `flash-next`. The deep dive reads the run's end from the runner's
  ledger and the document from the run's row, and sends its own line to its rooms; a run it asks tells no
  room itself.

## The bench lock

- **A bench owns the lab while it measures.** The verification job takes the card it measures on through
  the arbiter, as the occupant `bench`, at tier 1: trainer stands at 0 and takes the card outright, every
  service stands at 2 and gives way to the bench, and the bench gives way to trainer. The tiers are the
  owner's rules on each node (`occupants`), which stand over a service's own declaration.
- **A bench runs on as many cards as the arbiter grants, at most four.** The job asks the arbiter's master
  for the cards its measurement can use as `bench`, never naming a machine, and measures once on each card
  it is granted, one measurement per card.
- **The lock is a ticket under `bench`** on each granted card, asked with `POST /tickets` naming the card
  and the memory the bench needs, and given back with `DELETE` when the measurement there is done. The job runs with
  `BMO_CARD_SERVICE=bench` so every process it starts on the node is the bench's. A ticket told to give
  way — trainer claimed the card — ends the measurement at its next boundary and the job waits for its turn
  again; a measurement cut in the middle is thrown away and run again whole.
- **What the bench stops comes back by the arbiter's own road:** an occupant that yielded is restored when
  the ticket is given back, its units loaded back or its process started again, as the arbiter's placement
  rules say. The job never stops a service by hand and never starts one by hand.
- **The release is proved before the report,** as the researching-and-benching skill says: the ticket
  given back, each yielded service answering its own health, and the arbiter's card showing no bench.

## The engine a run needs

- **Routing is automatic.** A run names the model it goes out on, a registration of agency's; where the
  model runs and on which card is decided by the arbiter's master and agency's registry at that moment.
  No setting of this unit names a node, lists the nodes of a model or pins a model to one.
- **kakia's two models are two engines sharing the lab's language-model cards:** `flash-next` on the
  arbiter service `lab-arena`, `flash-next-coder` on `lab-arena-coder`. Every such card can hold either, and
  which one stands on it is the arbiter's grant (bmo-lab-arena `docs/engines.md`). The daemon's runs go out
  on `flash-next`; their branches, and the verification job's co-workers, on `flash-next-coder`.
- **A run asks for its engine before it starts.** For a category whose road is one of the engine watch's
  registrations (`docs/settings.md`, `[engine]`), where the watch names a master, the daemon asks the
  arbiter's master for one card of that registration's service, as the run's own, with the run as the work
  it continues, and starts the run once a card is granted. The wait has no clock of the daemon's, and
  neither has the run (`docs/runner.md`, 「Nothing of a run is clocked」). A category stopped while its run waits
  takes the demand back and its items wait again, the try uncounted. The run's seat and sessions are asked with the demand's id, so agency asks none
  of its own for them and places them on the granted card. The demand is held for the whole run and given
  back when the run ends; the daemon gives back, at its start, every demand its store holds for a run no
  longer under way.
- **`concurrency` is how many deep dives the daemon asks for at once,** four to start with; how many run
  is how many cards the arbiter's one rule gives them. Each run's demand is an ordinary one, standing in the
  line in the order it was asked like every other caller's.
- **Nothing takes a run's engine from it but a bench or a claim.** A run holds its card until it ends; a
  demand for the other engine waits until then, however long the run takes. A bench or a claim stops the
  engine as the arbiter's lower tiers do; the run's turn in hand is cut, and its next turn waits for its
  card to be granted again.
- **A run's branch is asked within the run's demand.** A branch goes to any other card the arbiter can
  open for the Coder engine; only where every other card is in use is the run's own card lent to it, the
  run's engine standing down until the branch is finished, however long it takes, and coming back after
  two engine loads (bmo-lab-arena `docs/engines.md`, 「The self-swap」). The search task prompt states this
  as a rule (`prompts/search/task.md`).
- **A try the arbiter cut is retried, not counted against its tries.** A try of a watched road that
  fails after its demand's card was taken by a bench or a claim, or after the master stopped holding it, is
  the eviction's: its item waits again at once with the try uncounted. The words of the refusal are not
  read; the demand's answer is the evidence. A try's retries stand as they are.
- **The owner hears of an engine that stays away:** a run's demand still waiting after
  `restore_wait_minutes` is told to the owner's private room once; the demand goes on waiting.
- **Why nothing runs is shown:** the state route answers each hold — the demand and its reason (the
  engine starting, every card in use, a bench or a claim) — and the categories it holds, so the queue tool
  and the console can say it.
- **The verification is kakia's worker with its co-workers.** The notice for a `fit` item names that
  worker, and the chat agent files the item for it; its co-workers run on `flash-next-coder`, and its
  bench runs on the cards the arbiter grants it (「The bench lock」).
- **kakia's own turns (the chat companion and its worker) are agency's to hold** while their engine is
  away, as agency's rules for an engine the arbiter stands say; the daemon adds nothing to that.

## Nothing is given up

- **Nothing is parked on a clock while what it waits for is away.** A try that ends because something it
  needs could not be reached or was taken away — agency down or restarted under the run, the run's seat or
  session no longer held, the arbiter's card taken or the master restarted, this unit's own restart — is
  interrupted: uncounted, and asked again on the next pass, where it waits for its card or its seat however
  long that takes. The evidence is the source of the failure — a call that did not reach its unit, a status
  a unit answered, a demand's answer, the seat's finish — never the words of an error (`docs/api.md`, 「The
  deep dive」; `docs/runner.md`, 「A cut run」).
- **A try that fails on its own content waits a short while,** `retry_minutes`, and is counted.
- **Only what failed is made again.** A report is never thrown away and researched again from its start
  because one part of it failed or its review found a fault; the part that failed, or the part a fix names,
  is made again alone and everything already had is kept:
  - a translation part that failed is asked again alone, the parts already translated kept (`docs/api.md`,
    「The page」);
  - a narration missing is asked alone, the other kept;
  - a review's fix redraws only the section or the narration it names (`docs/runner.md`, 「The review」);
  - a try asked again after a try whose run wrote a result continues from that result: its first answer and
    the rounds already answered are not asked again (`docs/runner.md`, 「A run that continues」). A try is
    researched from its start only where no run before it wrote anything, or where its question is no
    longer the one that result answers.
- **No count of failures gives an item up.** Past its retries the item is `stuck`: still asked on every
  retry wait, while the helper is handed the evidence — the item, its runs, their errors and the last lines
  of their log — through a repair item in the job service, to fix the cause in the prompts, the rulebooks,
  the runner or the engine's road and then ask the item again (`docs/api.md`, 「The repair」). The owner's
  dismissal is the one way an item leaves the queue unreported.

## The notifier

The research unit's border is the notice. Each category names who hears its reports — the recipients —
and what each recipient does with them is that recipient's own: the supervisor adopts, a worker patches, a
trading agent reads. The unit never adopts, never patches, never trades, and never files the work those
take; so its own area stays the research and the measurement, however many categories it gains.

| Category | Recipients to start with | What the recipient does with the notice |
|---|---|---|
| `it` | the supervisor, the chat agent, in the owner's private room | reads the manual, puts the question to the owner, files the verification on his word, and carries the adoption |
| `security` | the worker of each repository the advisory touches, and the owner's private room | the worker is told the flaw, the exposure and the patch steps for its own repository, and raises or takes the patch as its own work; the room hears which hosts and repositories were touched |
| `economy` | the owner's private room; later the trading agent, added as a recipient when it exists | reads the brief and the facts; the trading agent folds them into its own state |

- **A recipient is a room or an agent,** a registration on the category (`/categories`, `research/rooms`
  and `research/recipients`): a room is told through the receiver's instruction route, which hands the
  notice to the room's answerer; an agent that answers no room — a worker — is told by a turn into its
  session through agency's session route, the notice as the turn's words. A worker's turn is a notice,
  never an order to deploy: what it does with it is the worker's rules'.
- **A security notice names its repository.** The stack inventory maps each component to the repository
  or host that runs it and the worker that keeps it; the advisory investigation's exposure table carries
  that column, and the daemon sends each touched repository's worker its own part. A component with no
  worker goes to the owner's room alone.
- **The owner's private room hears every category** unless its recipients are changed; the chat agent
  says the notice in its own voice, and the owner's word comes back through the chat agent.

## Language

- **Every research is asked and answered in English.** The question templates under `prompts/` are English, so
  the runner's search writes its document in English, as its rules say a result follows the question's
  language; the rulebooks, the brain rows a run writes and every record of the unit are English as the
  platform's are.
- **What reaches the owner is Korean, rendered by the platform's translator, in the same document.** A deep
  dive's document is one HTML page (`docs/runner.md`, 「The page」) holding every language it has: filed in
  English, then drawn again with its Korean side once the daemon has it translated through agency's
  translation route on the `translate` road — the registry's translation model first — part by part, each
  part's prose alone sent and set back by the daemon in the English part's own structure, checked against
  it before it is kept, each part with a note that gives the translator its context — that the text is a
  research report about the named thing, in the register of the owner's documents — and with the terms it
  keeps exactly as written, its model and product names, figures with their units, code, links and fixed
  section names, handed to the translation model as its own reference translations rather than as an
  instruction. A switch at the page's top shows Korean first and English beside it, each named in its own
  words. A notice's own line is Korean from the start (`prompts/report.toml`).
- **The page is heard as well as read.** The run's narration script is spoken by the platform's voice unit:
  in Korean, translated through the same route with a note that it is a spoken narration, as the sound the
  archive plays at the document's head, and in English as it was written, kept beside it; the page's listen
  band plays the sound of the language shown.
- **A narration handed to the voice is its written words.** Each is the transcript the voice unit keeps, with
  names, acronyms and figures as written; the voice unit derives the spoken words from it, the same words as a
  Korean reader says them aloud, and checks them before it speaks. Spoken words it refuses, or a synthesis that
  fails, leave that narration pending like any other part the page lacks.
- **A report is told only once its page is whole and reviewed:** its Korean side had, every narration its
  script allows attached, and then the page read once whole by the run's own model, which answers that it
  passes or names the fixes it needs, each applied to the part it names alone (`docs/runner.md`, 「The
  review」). The review is asked once; its fixes are not reviewed again. A translation that cannot be had
  (no road carries the translate line, a part refused as too long after cutting), a narration that cannot
  be made, a review that does not answer or a fix that cannot be applied yet leaves that part pending, and
  the daemon asks it again on every retry wait until it has it; no count of tries gives it up. Until then
  the recipients hear nothing of the item, and its state in the queue says what it waits on.
- **A narration has one synthesis living at a time.** The synthesis asked for it is kept in the store until
  its sound is attached, through a restart of the daemon or of the voice unit; while it stands waiting or
  speaking it is followed, however long its card takes, and nothing is asked beside it. Only a synthesis
  that failed, or one the voice unit no longer holds, is asked again: a narration is made anew after its
  synthesis ended without a sound, never while one is still on its way.

## Reports and rooms

- **A report line** is the document's one share link and one line in Korean saying what the thing is and
  the verdict in a few words, and for a `fit` item the question. Nothing else goes into the room: the
  document holds the grounds, in both languages, and the narration.
- **A document that is not the research asked for is never reported:** a deep dive's result whose body holds
  none of its shape's sections is not filed, and the try fails with what was missing, to be tried again.
- **The recipients** are a registration per category (「The notifier」): the owner's private room first,
  read from the receiver at the daemon's start where a category's list is empty; a room or an agent is
  added and removed through the routes and the tools. Every recipient of the category gets every notice of
  it; one that refuses is said once in the log.
- **A message goes through the receiver's instruction route** as a `notice` under the kind
  `research-report:<item>`, which the room's answerer says in its own voice; the link stands as given.

## Settings and store

- **Settings** (`docs/settings.md`): the listen address, the data folder, the cadence, the cap, the
  concurrency, the model road, the retry and restore waits, the engine watch, the runner's seat role and
  capabilities, the research settings' defaults, and the addresses and token files of agency, the tool
  host's control socket, the artifact service, the receiver and the arbiter's master, whose picture shows
  every node's card. Every address and token file is read from the files the other units already keep,
  named in the settings, never copied.
- **The store** (`docs/store.md`): SQLite under the data root's `research/` folder: `categories`,
  `sources`, `seen`, `queue`, `runs`, `reports`, `rooms`, `facts` (the economy category's figures),
  `stack` (the security category's inventory), `demands` (the arbiter demands held), and the runner's `research_runs`, `research_log`,
  `research_holds` and `research_settings`. Schema versioned and migrated at start.

## The routes and the tools

`docs/api.md` is the contract: `/health`, `/categories`, `/sources`, `/queue` and `/queue/{item}` (ask,
dismiss, taken, reorder), `/runs`, `/rooms`, `/facts`, `/stack`, `/state` (running or stopped, whole or by
category, with `PATCH` to stop and resume, and what holds the queue while a run's engine is not granted), and the runner's
`/research-runs`, `/research-settings` and `/branches`. The tools the tool host places under the namespace
`research` are the chat agent's doors to the same: `research/ask`, `research/queue`, `research/taken`,
`research/dismiss`, `research/categories`, `research/sources`, `research/rooms`, `research/stop`,
`research/resume` and `research/run`; the owner-only ones hold a capability the chat line alone is given. A
search run is shown `research/rulebook`, the reader of the rulebooks its record names.

## Phases

1. **The daemon with the loop:** sources, scan, queue, the deep dive on kakia's road, the notices to the
   owner's room, the tools, the deploy. The verification is the chat
   agent's to file on the owner's word; the bench lock is the worker skill's by hand, as today.
2. **The bench lock and the tiers:** the `bench` occupant in the arbiter's rules at tier 1, every service
   at 2, the skill's bench steps on tickets, the n3–n6 rule in the daemon, the restore wait.
3. **The runner is this unit's** (`docs/runner.md`): agency supplies seats and thrown-away sessions alone.
4. **The engine a run needs:** each run asks the arbiter's master for its engine and holds it for the run,
   the engine watch in place of the bench watch, concurrency as many as the arbiter grants, and a bench on
   as many cards as it is granted.
