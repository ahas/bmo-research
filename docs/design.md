# The research daemon

bmo-research is the platform's research unit: a daemon that keeps a queue of things to research, finds new
ones on the sites the owner registers, deep-dives each one in the lab's own routine, files the report in the
archive and tells the agents and rooms that should hear of it. It researches and measures, and notifies;
it files no job and launches nothing: what a report calls for is the chat agent's to raise on the owner's
word. It is the home of the research feature; agency supplies it sessions and seats and nothing more.

## What it is for

- **Bongbos's routine, running on its own.** The owner's collaborator reads a maker's material, measures
  what it claims on the lab's own hardware and reports in a fixed shape (agency's rulebook
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
  - agency, for a session on the search line and a seat of the co-worker pool (`POST /research-runs`
    while the runner still lives there, with the run asked on kakia's road; later the generic seat and
    session routes only, 「Where the runner lives」);
  - explorer, for the web, through the tool host's `web` namespace in the runs it asks for; the daemon's
    own scan reads sources with its own HTTP client, since a feed is not a page an agent reads;
  - artifact, to file a report (`POST /documents`, `POST /shares`) where the run does not file it itself;
  - the receiver, to tell a room (`POST /instructions`) and to learn the owner's private room (`GET /owner`);
  - the arbiter, to read whether a bench holds a node its model stands on (「The n3–n6 restore path」).
  The job service it never calls: a job is the chat agent's to file.
- **Every per-deployment choice is a setting or a registration,** never code: the sources, the cap, the
  cadence, the model's road, the report rooms, the token files (`docs/settings.md`).
- **The daemon decides nothing a person should and files nothing an agent should.** It never approves a
  verification, never raises a queue item, never registers a service with the arbiter, and never spends a
  paid road unless a setting names one. It notifies: every report, every `fit` item and every failure is
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
4. **The deep dive.** One run at a time (`concurrency`, 1 to start with): the item becomes a research run
   on kakia's road at the exhaustive depth, asked as a manual where the thing is a model, an engine, a
   dataset or a tool, and as an investigation where it is a claim or an event. The run's question is the
   item's name and its maker's address, with the shape named, so the search runner's research rulebook
   applies. A run that is refused (no road carries it) or cut is retried after `retry_minutes`, three
   times, then marked failed and reported once.
5. **The report.** The document the run filed is the report. The daemon tells every report room: the
   share link and one line saying what the thing is, nothing else — never the grounds, never the
   sources. The first room is the owner's private room, read from the receiver at start; rooms are added
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
   beyond stepping its own runs off the node the bench takes (「The n3–n6 restore path」).

## Categories

Research is done by category: each source belongs to one, and the category says why the daemon reads
it, what a deep dive of its items produces, where the report goes and how much of the queue it may
hold. A category is a registration (`docs/settings.md`): its name, its purpose in a line, its report
shape, its cap, its cadence, its road, and its report rooms. Three stand at the start:

| Category | Purpose | What a deep dive produces | What follows |
|---|---|---|---|
| `it` | new AI technology, as Bongbos's lab researches it | the manual or the investigation (agency's `research.md`): the maker's own material, the lab's fit, the reproduction plan | a `fit` item is reported for the owner's word; approved, a verification job under the bench lock, and a service registered with the arbiter |
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
- **Each category has its own cap, cadence, road and rooms,** so one category's flood never starves
  another, and a category can be stopped alone. The owner's private room is every category's first room.
- **A category the owner adds later** needs a shape: one of the three above, or a new template under
  `prompts/` with its rulebook in agency's `prompts/rules/` until the runner moves here.

## Sources

A source is a registration: a category, a kind, an address and, where the kind takes them, a query and a filter.

| Kind | Reads | Items |
|---|---|---|
| `huggingface-trending` | the Hub's models API ordered by trending, a page | model repositories: id, author, last modified, tags |
| `huggingface-search` | the Hub's models API with a search and a sort | the same |
| `arxiv` | the arXiv API with a query, newest first | papers: id, title, authors, submitted, abstract |
| `github-releases` | a repository's releases API | releases: tag, name, published |
| `rss` | a feed | entries: title, link, published |
| `page` | one page read through explorer's `web_fetch` where a maker's blog has no feed | links the page lists, by a selector |

- A source carries `every_minutes` where it is read more or less often than the daemon's cadence, and a
  `filter`: words an item must carry and words that rule it out, so a trending page's chat fine-tunes do
  not fill the queue.
- A source that fails is said once in the log and read again next time; its failures are counted on the row
  and shown.

## Where the runner lives

- **To start with, the research run is agency's** (`POST /research-runs` with `key`, `query`, `provider`,
  `model`, and no room), and agency files the document and queues the room's instruction; the daemon reads
  the run's end from the ledger (`GET /runs/{runId}`) and the document it filed from the run's record, and
  sends its own line to its rooms. The daemon asks the run on the road its settings name, kakia's:
  provider `registered`, model `flash-next`.
- **Then the runner moves here:** the search line's instructions, the rulebooks, the rounds, the branch
  allowance and the filing become this unit's, and agency is asked only for a seat and a thrown-away
  session whose system prompt this unit hands in whole. The chat agent's `run_research` becomes this
  unit's tool. That move is a plan of its own (`docs/plan.md`) and changes nothing of the loop above.

## The bench lock

- **A bench owns the lab while it measures.** The verification job takes the card it measures on through
  the arbiter, as the occupant `bench`, at tier 1: trainer stands at 0 and takes the card outright, every
  service stands at 2 and gives way to the bench, and the bench gives way to trainer. The tiers are the
  owner's rules on each node (`occupants`), which stand over a service's own declaration.
- **The lock is a ticket under `bench`,** asked with `POST /tickets` naming the node's cards and the memory
  the bench needs, and given back with `DELETE` when the measurement is done. The job runs with
  `BMO_CARD_SERVICE=bench` so every process it starts on the node is the bench's. A ticket told to give
  way — trainer claimed the card — ends the measurement at its next boundary and the job waits for its turn
  again; a measurement cut in the middle is thrown away and run again whole.
- **What the bench stops comes back by the arbiter's own road:** an occupant that yielded is restored when
  the ticket is given back, its units loaded back or its process started again, as the arbiter's placement
  rules say. The job never stops a service by hand and never starts one by hand.
- **The release is proved before the report,** as the researching-and-benching skill says: the ticket
  given back, each yielded service answering its own health, and the arbiter's card showing no bench.

## The n3–n6 restore path

kakia and its workers run on lab-arena, the engine on n3 to n6; a bench on one of those nodes evicts lab-arena
there, and with it the model the daemon's own research runs go out on. So:

- **The researcher is kakia and the bench is kakia's worker with its three coders.** The daemon's runs go
  out on `flash-next`, the supervisor's model on n3; the verification job goes to kakia's worker, which
  writes the bench code through its three co-workers, one on each of the coder instances `flash-next-coder`
  serves on n4, n5 and n6 behind the gateway. The notice for a `fit` item names that worker, and the chat
  agent files the item for it. A bench that takes one of the three coder nodes evicts one instance and
  leaves two serving; a bench on n3 evicts `flash-next` itself, which the arbiter restores when the ticket
  is given back, and the daemon's own runs wait meanwhile.
- **The daemon pauses its own runs while a bench holds a node its model stands on.** It reads the
  arbiter's card state before each run and while one runs; a bench ticket standing on kakia's node holds
  the queue, and a run cut by the eviction is retried after the bench is gone, not counted against its
  three tries.
- **lab-arena comes back as the arbiter restores it,** by its own adapter (`yield_by = "process"`: the process
  is ended and started again), and the gateway's registration answers health again. The daemon waits on
  that health before the queue goes on, and reports once if it stays down past `restore_wait_minutes`.
- **kakia's own turns (the chat companion and its worker) are agency's to hold** during the eviction: a
  registered model's engine serving one slot makes a conversation's turn wait, as agency's companion
  rules say; the bench adds nothing to that beyond its length, which the report line says.

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
  the search runner writes its document in English, as its rules say a result follows the question's
  language; the rulebooks, the brain rows a run writes and every record of the unit are English as the
  platform's are.
- **What reaches the owner is Korean, rendered by the platform's translator.** Once a run's document is
  filed, the daemon has its body translated through agency's translation route on the `translate` road —
  the registry's translation model first — part by part, each part with a note that gives the translator
  its context: that the text is a research report about the named thing, that model and product names,
  figures, units, code, links and the fixed section names stay as written, and that the Korean is the
  register of the owner's documents. The Korean rendering is filed as a document of its own beside the
  English one, labelled as its translation, and the notice carries the Korean document's link, the English
  one named after it. A notice's own line is Korean from the start (`prompts/report.toml`).
- **A translation that cannot be had** (no road carries the translate line, a part refused as too long after
  cutting) leaves the notice with the English link and a line saying the Korean rendering is pending; the
  daemon tries again on its retry wait and sends the Korean link when it has it.

## Reports and rooms

- **A report line** is the Korean document's share link and one line in Korean saying what the thing is
  and the verdict in a few words, and for a `fit` item the question; the English document's link stands
  after it. Nothing else goes into the room: the
  document holds the grounds.
- **The recipients** are a registration per category (「The notifier」): the owner's private room first,
  read from the receiver at the daemon's start where a category's list is empty; a room or an agent is
  added and removed through the routes and the tools. Every recipient of the category gets every notice of
  it; one that refuses is said once in the log.
- **A message goes through the receiver's instruction route** as a `notice` under the kind
  `research-report:<item>`, which the room's answerer says in its own voice; the link stands as given.

## Settings and store

- **Settings** (`docs/settings.md`): the listen address, the data folder, the cadence, the cap, the
  concurrency, the model road, the retry and restore waits, the assignees for benches by node, and the
  addresses and token files of agency, explorer's tool host, the artifact service, the receiver, the job
  service and the arbiter of each node. Every address and token file is read from the data root's `env/`
  files the other units already keep, named in the settings, never copied.
- **The store** (`docs/store.md`): SQLite under the data root's `research/` folder: `categories`,
  `sources`, `seen`, `queue`, `runs`, `reports`, `rooms`, `facts` (the economy category's figures),
  `stack` (the security category's inventory). Schema versioned and migrated at start.

## The routes and the tools

`docs/api.md` is the contract: `/health`, `/categories`, `/sources`, `/queue` and `/queue/{item}` (ask,
dismiss, taken, reorder), `/runs`, `/rooms`, `/facts`, `/stack`, `/state` (running or stopped, whole or by
category, with `PATCH` to stop and resume). The tools the tool host places under the namespace `research`
are the chat agent's doors to the same: `research/ask`, `research/queue`, `research/taken`,
`research/dismiss`, `research/categories`, `research/sources`, `research/rooms`, `research/stop`,
`research/resume`; the owner-only ones hold a capability the chat line alone is given.

## Phases

1. **The daemon with the loop on agency's runner:** sources, scan, queue, the deep dive asked of agency on
   kakia's road, the notices to the owner's room, the tools, the deploy. The verification is the chat
   agent's to file on the owner's word; the bench lock is the worker skill's by hand, as today.
2. **The bench lock and the tiers:** the `bench` occupant in the arbiter's rules at tier 1, every service
   at 2, the skill's bench steps on tickets, the n3–n6 rule in the daemon, the restore wait.
3. **The runner moves here** from agency (`docs/plan.md`).
