# The research daemon

bmo-research is the platform's research unit: a daemon that keeps a queue of things to research, finds new
ones on the sites the owner registers, deep-dives each one in the lab's own routine, files the report in the
archive, tells the owner, and carries what he approves on to a verification on the lab's machines. It is
the home of the research feature; agency supplies it sessions and seats and nothing more.

## What it is for

- **Bongbos's routine, running on its own.** The owner's collaborator reads a maker's material, measures
  what it claims on the lab's own hardware and reports in a fixed shape (agency's rulebook
  `prompts/rules/research.md`: the investigation, the manual and the verification). The daemon does the
  first two shapes by itself, continuously, and the third on the owner's word.
- **Continuous, capped and cheap.** The queue has a cap; whenever a slot frees, the daemon fills it from
  what its sources turned up, unconditionally, and goes on until the owner stops it. The runs go out on
  kakia, the lab's own model, which costs nothing.
- **Semi-automatic.** A deep dive that finds a thing the maker's own documents show usable in the lab is
  reported to the owner first. After his approval the verification runs — the maker's figures re-measured
  on our cards under a bench lock — and what passes is registered with the arbiter as a service.

## Principles

- **It calls other units through their documented APIs alone** and depends on no other unit's code:
  - agency, for a session on the search line and a seat of the co-worker pool (`POST /research-runs`
    while the runner still lives there, with the run asked on kakia's road; later the generic seat and
    session routes only, 「Where the runner lives」);
  - explorer, for the web, through the tool host's `web` namespace in the runs it asks for; the daemon's
    own scan reads sources with its own HTTP client, since a feed is not a page an agent reads;
  - artifact, to file a report (`POST /documents`, `POST /shares`) where the run does not file it itself;
  - the receiver, to tell a room (`POST /instructions`) and to learn the owner's private room (`GET /owner`);
  - the job service, to raise a verification item (`POST /items`);
  - the arbiter, for the bench lock (a ticket under the occupant `bench`).
- **Every per-deployment choice is a setting or a registration,** never code: the sources, the cap, the
  cadence, the model's road, the report rooms, the token files (`docs/settings.md`).
- **The daemon decides nothing a person should.** It never approves a verification, never registers a
  service with the arbiter by itself, and never spends a paid road unless a setting names one.
- **A base never knows its consumers** and this unit is a consumer of every unit it calls; nothing here is
  branched on by agency, explorer or the arbiter.

## The loop

```
sources ──scan──▶ candidates ──new?──▶ queue (cap) ──slot free──▶ deep dive ──▶ report ──▶ owner's room
                                                                                │
                                                     approval ◀─── owner ◀──────┘
                                                        │
                                                        ▼
                                       verification item (job) ── bench lock ── registration
```

1. **Scan.** On the cadence, each source is read and its items are turned into candidates: a name, the
   maker, the address of the maker's own page, the date it appeared, and the source it came from.
2. **New?** A candidate is new where no seen row holds its key (the canonical address, else the name
   lower-cased), no archive document carries its name in its title or labels (`GET /documents?q=`), and
   the owner has not dismissed it. A new candidate is kept as seen the moment it is read, so it is judged
   once.
3. **The queue.** New candidates wait in the queue by the order they were found; one the owner asked for
   by name stands first. The queue holds at most `cap` items waiting or running; a scan that finds more
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
6. **Approval.** A report whose document says the thing fits the lab (the manual's 「우리 랩에서 구동하려면」
   names a card it fits) is marked `fit`, and the room line says so and asks for the word. The owner
   approves or dismisses through the chat agent's tool or the route; an item nobody answers stays `fit`
   and is listed, never nagged about.
7. **The verification.** An approved item raises one queue item in the job service — the title naming the
   thing, the brief carrying the report's link and its reproduction plan, the label `research` and the
   assignee the settings name for benches — and the worker's researching-and-benching skill carries it
   under the bench lock (「The bench lock」). The daemon reads the item's state and, once it closes, tells
   the rooms the verification's link. Registering a service with the arbiter is the job's last step, on
   the owner's word in the item, never the daemon's.

## Sources

A source is a registration: a kind, an address and, where the kind takes them, a query and a filter.

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

- **The daemon never benches what it stands on without stepping off it first.** A verification whose bench
  names n3 to n6 is raised with the assignee the settings name for those nodes, a worker whose own road is
  not lab-arena's (a Claude Code worker, or one on a member road), so the bench job itself survives the
  eviction. The daemon refuses to raise a bench item on a node whose worker would be the thing evicted,
  and says so in the item's note instead.
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

## Reports and rooms

- **A report line** is the share link and one line in the owner's language saying what the thing is and
  the verdict in a few words, and for a `fit` item the question. Nothing else goes into the room: the
  document holds the grounds.
- **The rooms** are a registration: the owner's private room first, read from the receiver at the daemon's
  start where the list is empty; a room is added and removed through `/rooms` and the tools. Every room
  gets every report line; a room that refuses is said once in the log.
- **A message goes through the receiver's instruction route** as a `notice` under the kind
  `research-report:<item>`, which the room's answerer says in its own voice; the link stands as given.

## Settings and store

- **Settings** (`docs/settings.md`): the listen address, the data folder, the cadence, the cap, the
  concurrency, the model road, the retry and restore waits, the assignees for benches by node, and the
  addresses and token files of agency, explorer's tool host, the artifact service, the receiver, the job
  service and the arbiter of each node. Every address and token file is read from the data root's `env/`
  files the other units already keep, named in the settings, never copied.
- **The store** (`docs/store.md`): SQLite under the data root's `research/` folder: `sources`, `seen`,
  `queue`, `runs`, `reports`, `rooms`, `settings_changes`. Schema versioned and migrated at start.

## The routes and the tools

`docs/api.md` is the contract: `/health`, `/sources`, `/queue` and `/queue/{item}` (ask, dismiss, approve,
reorder), `/runs`, `/rooms`, `/state` (running or stopped, with `PATCH` to stop and resume). The tools the
tool host places under the namespace `research` are the chat agent's doors to the same: `research/ask`,
`research/queue`, `research/approve`, `research/dismiss`, `research/sources`, `research/rooms`,
`research/stop`, `research/resume`; the owner-only ones hold a capability the chat line alone is given.

## Phases

1. **The daemon with the loop on agency's runner:** sources, scan, queue, the deep dive asked of agency on
   kakia's road, the report line to the owner's room, the tools, the deploy. The verification is raised as
   a job item; the bench lock is the worker skill's by hand, as today.
2. **The bench lock and the tiers:** the `bench` occupant in the arbiter's rules at tier 1, every service
   at 2, the skill's bench steps on tickets, the n3–n6 rule in the daemon, the restore wait.
3. **The runner moves here** from agency (`docs/plan.md`).
