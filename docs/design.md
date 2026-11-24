# The research daemon

bmo-research is the platform's research unit: a daemon that keeps a queue of things to research, finds new
ones on the sites the owner registers, deep-dives each one in the lab's own routine, files the report in the
archive and tells the agents and rooms that should hear of it. It researches and measures, and notifies;
it launches nothing, and the one job item it files is the repair of its own stuck research, for its
own worker: what a report calls for is the chat agent's to raise on the owner's word. It is the home of the
research feature; agency supplies it slots and sessions and nothing more.

## What it is for

- **Bongbos's routine, running on its own.** The owner's collaborator reads a maker's material, measures
  what it claims on the lab's own hardware and reports in a fixed shape (the rulebook
  `bmo-research-routine`, a skill of the skills repository: the investigation, the manual and the
  verification). The daemon does the
  first two shapes by itself, continuously, and the third on the owner's word.
- **Continuous, uncapped and cheap.** Everything its sources turn up waits in the queue, which has no cap;
  the lab's nodes pull from it, so as many deep dives run as the search line's role has places, and the
  daemon goes on until the owner stops it. The runs go out on kiki, the lab's own model, which costs
  nothing.
- **Semi-automatic.** A deep dive that finds a thing the maker's own documents show usable in the lab is
  reported to the owner first, as a notice the chat agent delivers. On his word the chat agent raises the
  verification — the maker's figures re-measured on our cards under a bench lock, by a job that follows the
  researching-and-benching skill — and what passes is registered with the arbiter as a service. The daemon
  is the researcher and the notifier in that exchange, never the one that files or launches.

## Principles

- **It calls other units through their documented APIs alone** and depends on no other unit's code:
  - agency, for a slot of the voyager or the seeker role and a session of that role on its search or
    browse line, and the turns into it (「Where the runner lives」);
  - the tool host, for the web namespace's listing a search's instructions carry and for the web tools'
    search engines' state;
  - explorer, for the web, through the tool host's `web` namespace in the runs it asks for; the daemon's
    own scan reads a feed or an API with its own HTTP client, since a feed is not a page an agent reads, and
    a `page` source's listing through the tool host's `web/web_fetch`, so explorer's pacing and Chrome
    stand round it;
  - artifact, to file a run's document, draw a deep dive's page again and keep a document's narration beside it
    (`POST /documents`, `PATCH /documents/{id}`, `POST /documents/{id}/attachments`, `POST /shares`), and to
    read a filed page as it keeps it, so a page drawn otherwise now is set again
    (`GET /documents/{id}/content`);
  - the voice unit, to speak a document's narration, a deep dive's page's and a room's research's alike
    (`POST /syntheses`);
  - the receiver, to tell a room (`POST /instructions`) and to learn the owner's private room (`GET /owner`);
  - the arbiter's master, to ask for the engine each run needs and to read why it waits (「The engine a run
    needs」).
  - the job service, for one item alone: the repair of an item stuck past its retries, filed for the repair worker
    (「Nothing is given up」).
  Any other job is the chat agent's to file.
- **Every per-deployment choice is a setting or a registration,** never code: the sources, the cadence,
  the model's road, the report rooms, the token files (`docs/settings.md`).
- **The daemon decides nothing a person should and files nothing an agent should.** It never approves a
  verification, never raises a queue item for what a report calls for, never registers a service with the arbiter, and never spends a
  paid road unless a setting names one. It notifies: every report, every `fit` item and every item stuck is
  an instruction the receiver hands to the room's agent, which says it in its own voice and acts on the
  owner's word.
- **A base never knows its consumers** and this unit is a consumer of every unit it calls; nothing here is
  branched on by agency, explorer or the arbiter.

## The loop

```
sources ──scan──▶ candidates ──new?──▶ queue ──a place let in──▶ deep dive ──▶ report ──▶ notice to the room's agent
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
   by name stands first. The queue has no cap, whole or per category: every new candidate the scan keeps is
   queued in the same pass, `waiting`, however many already wait.
4. **The deep dive.** A node pulls (「A node pulls」): the daemon asks agency for places of the search line's
   role, and only once a place is let in takes the next item from the queue and starts its run on that
   place, so as many deep dives run as the role's cap lets in and the arbiter grants cards for. The item
   becomes a research run on kiki's road at the exhaustive depth, asked as a manual where the thing is a model, an engine, a
   dataset or a tool, and as an investigation where it is a claim or an event. The run's question is the
   item's name, its maker's address and where it came from — the source that found it, the owner's ask, or
   the earlier research a follow-up continues — with the shape named, so the runner's research rulebook
   applies. An investigation's question also names the nearest earlier reports in its category, by subject
   proximity and title, so the run compares the new subject with them; a category holding none says so in
   one line. A try cut short by something it needs going away is asked again at once, uncounted; a try that
   fails on its own content is asked again after `retry_minutes`, and past its retries the item is stuck
   and the repair worker is asked to fix the cause (「Nothing is given up」).
5. **The report.** The document the run filed is the report: it opens by saying what the thing is and
   what it is for, and under its title says where the research came from. A run skipped on the age floor
   files no document and tells no one, so no report stands for it. The daemon tells every report
   room: the share link and one line saying what the thing is, naming the earlier document a follow-up
   continues, nothing else — never the grounds, never the sources. The first room is the owner's private room, read from the receiver at start; rooms are added
   and removed through the routes and the tools.
6. **The word.** A report whose document says the thing fits the lab (the manual's section on running it in
   the lab, under the heading `prompts/document.toml` names, names a card it fits) is marked `fit`, and its
   notice says so. The notice carries the data the chat agent acts on: the report's link, the thing's name
   and the reproduction plan's place in the document. It asks nothing of its reader: the chat agent's own
   rules have it put the question to the owner. The owner answers in the room; the chat agent files the
   verification item as it files any work, with the report's link in the brief, and tells the daemon the
   item is taken (`PATCH /queue/{item}` with `taken` and the item's number) or dismissed. An item nobody
   answers stays `fit` and is listed, never nagged about.
7. **The verification** is a job of the worker's, under the researching-and-benching skill and the bench
   lock (「The bench lock」), reported by the job itself as every job reports. Registering a service with
   the arbiter is the job's last step, on the owner's word in the item. The daemon takes no part in it
   beyond its own runs waiting while the bench holds the engine they need (「The engine a run needs」).

## A node pulls

- **The queue is a line the nodes take from.** An item stands `waiting` until a place of the search line's
  role is let in for it; only then is it taken and its try started on that place, and it stands `running`
  only while its run holds the place. While no place is let in, nothing is taken, however many items wait,
  so the daemon's tries running are never more than the role's cap.
- **A place is a slot of the search line's role asked before any item is taken.** It is asked under a
  request key of its own and the daemon's places' owner, on the road of the categories it may take from,
  and names no item.
- **The daemon asks one place for each try it may start.** On each road the running categories go out on,
  it asks as many places as that road has tries due — each waiting or stuck item whose retry is due, and a
  brief's whole gathering once it is due — less the places already waiting on that road, and never more,
  across every road, than the role's cap less the places it holds or waits for. The cap is agency's, read
  from the role as each pull is made; nothing of this unit sets it.
- **A place on a watched road asks its card first** (「The engine a run needs」), then its slot standing on
  that card, and waits in agency's line however long the line takes.
- **A place let in takes the next try on its road:** of the running categories on that road with a try
  due, the one whose turn it is (「Categories」), its next item or its gathering; the try's run starts on
  the place's slot and card and asks none of its own. A place let in when its road has nothing left to take
  gives its slot and its card back.
- **A place whose road has no try due any longer gives itself back** while it waits: every category on the
  road stopped, or the items it was asked for dismissed or taken by the places before it.
- **The pull wakes on an event, never on a clock:** a place let in, an item arriving in the queue (a scan
  that queued items, an ask, an item asked again or moved to the front, a category resumed) and a try's run
  ending. An item whose retry wait has ended is found by the scan's pass, which pulls as it ends; the scan
  keeps its cadence, which paces the reading of the sources and bounds nothing.
- **A room's research asks its own slot** when it is asked and waits in agency's line beside the daemon's
  places (`docs/runner.md`, 「One run, step by step」); the daemon counts no room's run among its places.
- **A stop and a restart give every place back:** the daemon releases everything its places' owner holds
  of agency as it stops and again as it starts, and every demand its store holds (「The engine a run needs」), so each item stands `waiting`
  or as its try left it (「Nothing is given up」).

## Categories

Research is done by category: each source belongs to one, and the category says why the daemon reads
it, what a deep dive of its items produces and where the report goes. A category is a registration
(`docs/settings.md`): its name, its purpose in a line, its report shape, its cadence, its road, its
report rooms and, for a topic, its own instruction and the calendar its findings are written to. The owner
adds, changes and takes away categories freely; three stand at the start:

| Category | Purpose | What a deep dive produces | What follows |
|---|---|---|---|
| `it` | new AI technology, as Bongbos's lab researches it | the manual or the investigation (the rulebook `bmo-research-routine`): the maker's own material, the lab's fit, the reproduction plan | a `fit` item is reported for the owner's word; approved, a verification job under the bench lock, and a service registered with the arbiter |
| `security` | vulnerability news that bears on the platform's own stack | an advisory investigation: what the flaw is, the affected versions, whether and where the platform runs them (the stack inventory below), the vendor's fix and the patch steps | each repository's worker is notified of its own part; the patch is the worker's work |
| `economy` | the economic news and indicators the owner's later portfolio and trading system will read | a brief, not a deep dive: the items of the day in one document with the figures they carry, the sources, and the facts kept for the economy store | the brief is notified to the owner's room now and to the trading agent later; the facts stand in the store for it to read |

- **The shape is the category's, not the source's.** `it` asks the research run for a manual or an
  investigation; `security` asks for the advisory shape; `economy` gathers a day's items into one run that
  writes the brief. The question templates under `prompts/` are one per shape.
- **A topic is a category whose question is the owner's own.** Its shape is `topic`: what to look for and how
  to report it is the category's `instruction`, written by the owner in his own words, and no template or
  rulebook is added for it. Like a brief it is gathered: once its `gatherMinutes` have
  passed — thirty minutes, a day, whatever the owner sets — one run is asked the instruction, with every
  item its sources turned up since under it where it has sources, and on the instruction alone where it has
  none, so a topic that is only a standing search needs no source at all. Its result is filed as a document
  and told to its recipients like any other.
- **A topic may write what it finds to the calendar.** A topic that names a calendar asks its run to close
  its result with a table of the dated things it found — an event's title, its start and end, its place and
  the page it was read on — and each row is written to that calendar as an event (the calendar unit's
  events route), the same row never twice: an event is keyed by its topic, its title and its start, so a
  later run that finds it again changes nothing. A row whose dates do not read is left out and said in the
  log.
- **The four shapes are one setting set.** A category is always the same registration — what it is for,
  where its items come from, how often, in which shape its run is asked, and what follows its result — and
  the three that stand at the start are three fillings of it, as a topic is a fourth.
- **The security category reads advisories and fixes, never exploits.** Its sources are vendor advisories,
  CVE feeds and distribution security lists; its deep dive judges exposure from the platform's stack
  inventory — the distributions, runtimes, libraries and programs the platform runs, kept as a registration
  the owner and the jobs maintain and joined by every unit's deploy record — and names the patch; writeups
  of how a flaw is exploited are not read past what the exposure judgement needs. A reported item says
  which hosts and repositories it touches.
- **The researcher owns the age floor.** The oldest recorded component, by the moment its build or version
  was recorded, is read afresh on every pass and handed every advisory in its question. An advisory whose
  recorded date predates it never joins the queue, and one already queued judges its own fixed builds against
  the floor before its research begins: where every fixed build appeared before it, the run is skipped whole
  — no document and no notice — and the queue keeps one line naming the floor. A version the inventory does
  not record is never read as old or new: one unknown row leaves the floor unknown, and an unknown floor
  skips nothing.
- **The economy category keeps facts.** Each brief's figures (an index, a rate, a price, a release) are
  written to the store with their source and date, so a later system reads a series, not documents.
- **Each category has its own cadence, road and rooms,** and a category can be stopped alone; a stopped
  category's items stay in the queue and no place takes them. The owner's private room is every category's
  first room.
- **The categories take turns at each place let in.** Of the running categories on the place's road with a
  try due, the place goes to the one whose last try was asked longest ago, one never tried first; an item's
  priority orders its own category's part of the queue and never another's. So one category's flood, or a
  string of asks in it, never starves another (`docs/api.md`, 「queue」).
- **A category the owner adds later** takes one of the four shapes. One of the first three reuses that
  shape's template and rulebook; a topic carries its own instruction and needs nothing added to this
  repository.

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

- **The research runner is this unit's** (`docs/runner.md`): the search, browse and reference instructions,
  composed with the rulebooks, which are skills of the skills repository the deployment places beside it, the
  request, the research settings, the rounds, the branch allowance, the filing of the
  document and the room's instruction, and the run's ledger and log. Agency supplies a slot of the role a
  run holds — `voyager` on its search line, `seeker` on its browse line — whose road is that of the agent of that role it is let in on,
  and sessions of that role, whose system prompt this unit hands in whole and whose record carries the run's
  settings under the labels this unit names; it runs nothing of research on its own.
- **Every research goes through the one runner:** the chat agent's `research/run`, any caller of
  `POST /research-runs`, and the daemon's own deep dive, which asks it on the road its settings name,
  kiki's: provider `registered`, model `flash-next`. The deep dive reads the run's end from the runner's
  ledger and the document from the run's row, and sends its own line to its rooms; a run it asks tells no
  room itself. The drawing service's `references` run is asked the same way and files a dossier for it, with
  no room told (`docs/runner.md`, 「The references dossier」).

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
- **Two registered models may be two engines sharing the same language-model cards,** as the settings'
  example has it: `flash-next` on the arbiter service `local-llm`, `flash-next-coder` on `local-llm-coder`. Every such card can hold either, and
  which one stands on it is the arbiter's grant. The daemon's runs go out
  on `flash-next`; their branches, and the verification job's co-workers, on `flash-next-coder`.
- **A place asks for its engine before its slot.** On a road that is one of the engine watch's
  registrations (`docs/settings.md`, `[engine]`), where the watch names a master, a place (「A node pulls」)
  asks the arbiter's master for one card of that registration's service, and asks its slot once a card is
  granted. The wait has no clock of the daemon's, and neither has the run (`docs/runner.md`, 「Nothing of a
  run is clocked」). A place whose road has no try due any longer gives the demand back, and no item was
  taken for it. The slot is let in standing on the demand's id, and so are the run's sessions, so agency
  asks no card of its own for them and places them on the granted card. The try the place takes holds the
  demand as its run's for the whole run, and gives it back when the run ends; the daemon gives back, at its
  start, every demand its store holds.
- **This unit uses at most one card of each engine,** the engine watch's `cards` (`docs/settings.md`), whoever
  in it asks: the daemon's places, a room's research, a review, a branch. Every demand it asks, directly or through
  agency's slots and sessions, stands in the watch's `pool`, and the arbiter's master holds that pool to its limit
  (the arbiter's `docs/placement.md`, 「A pool」). A demand beyond it waits in the master's line with the limit as
  its reason, holding nothing.
- **The search line's role's cap is how many places the daemon asks at once,** and on a watched road never more
  than the watch's `cards`; how many run is how many of them agency's line lets in and the arbiter's one rule
  gives cards.
- **A run's engine can be taken from it.** A bench or a claim stops the engine as the arbiter's lower tiers do; the
  run's turn in hand is cut, and its next turn waits for its card to be granted again. A demand of a better rank
  takes the card, stopping the engine only where it wants the other one: agency stops the turn in hand, its round is
  asked again as soon as its card is granted again, and the run goes on with nothing else of it lost
  (`docs/runner.md`, 「A turn's send」).
- **A run's branch is asked within the run's demand.** A branch goes to any other card the arbiter can
  open for the Coder engine; only where every other card is in use is the run's own card lent to it, the
  run's engine standing down until the branch is finished, however long it takes, and coming back after
  two engine loads. The search task prompt states this
  as a rule (`prompts/search/task.md`).
- **A try the arbiter cut is retried, not counted against its tries.** A try of a watched road that
  fails after its demand's card was taken, whether or not the demand has been granted again by the time the try
  ends, or after the master stopped holding it, is the eviction's: its item waits again at once with the try
  uncounted. The words of the refusal are not read; the evidence is the demand's answer and a turn that ended
  `card-taken`. A try's retries stand as they are.
- **The owner hears of an engine that stays away:** a place's demand still waiting after
  `restore_wait_minutes` is told to the owner's private room once; the demand goes on waiting.
- **Why nothing runs is shown:** the state route answers each hold — the place's demand and its reason
  (the engine starting, every card in use, a bench or a claim) — and the categories on its road, so the
  queue tool and the console can say it.
- **The verification is kiki's worker with its co-workers.** The notice for a `fit` item names that
  worker, and the chat agent files the item for it; its co-workers run on `flash-next-coder`, and its
  bench runs on the cards the arbiter grants it (「The bench lock」).
- **kiki's own turns (the chat companion and its worker) are agency's to hold** while their engine is
  away, as agency's rules for an engine the arbiter stands say; the daemon adds nothing to that.

## Nothing is given up

- **Nothing is parked on a clock while what it waits for is away.** A try that ends because something it
  needs could not be reached or was taken away — agency down or restarted under the run, the run's slot or
  session no longer held, the arbiter's card taken or the master restarted, this unit's own stop or restart — is
  interrupted: uncounted, its items back in the queue `waiting` as they stood, where the next place let in
  takes them up, however long that takes. The evidence is the source of the failure — a call that did not reach its unit, a status
  a unit answered, a demand's answer, the slot's finish — never the words of an error (`docs/api.md`, 「The
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
  retry wait, while the repair worker is handed the evidence — the item, its runs, their errors and the last lines
  of their log — through a repair item in the job service, to fix the cause in the prompts, the rulebooks (the
  skills repository's research skills), the runner or the engine's road and then ask the item again (`docs/api.md`, 「The repair」). The owner's
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
  notice to the room's answerer; an agent that answers no room — a repository's worker — is told by one run
  of agency's `worker` role at that repository's checkout, on a slot this unit asks and gives back, the
  notice as the turn's words (`docs/api.md`, 「The notices」). The turn carries the notice's data and a
  marker naming it alone; what the worker does with it is its own rules', which its session already
  carries.
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
  research report about the named thing, in the register of the owner's documents, its opening heading the
  report's title, kept a noun phrase within a title's bound — and with the terms it
  keeps exactly as written, its model and product names, figures with their units, code, links and fixed
  section names, handed to the translation model as its own reference translations rather than as an
  instruction. A switch at the page's top shows Korean first and English beside it, each named in its own
  words; the page opens on the reader's language side, Korean where that language is not held. A notice's
  own line is Korean from the start (`prompts/report.toml`).
- **The page is heard as well as read.** The run's narration script is spoken by the platform's voice unit:
  in Korean, translated through the same route with a note that it is a spoken narration, as the sound the
  archive plays at the document's head, and in English as it was written, kept beside it; the page's listen
  band plays the sound of the language shown, which is the language the page opened on until the reader
  switches it.
- **A research asked for a room is heard too.** It is written in its question's language, the owner's
  Korean when he asks in Korean, and its narration script is spoken as written in that language, as the
  sound the archive plays at the document's head; the room is told of the document only once that sound is
  attached (`docs/runner.md`, 「The narration's sound」).
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
- **A document's title names its subject and says nothing of the verdict:** a noun phrase, as a report's
  heading names its topic — a market survey of follow-me drones of the Mavic class and below, in the
  question's language — never a sentence, with no predicate, no finding and no clause after a comma or a
  dash; the verdict stands in the body and the narration. Every kind of run is held to it, a room's search
  and every deep dive's shape alike, and the Korean title a deep dive's page is given in translation as
  well.
- **A title holds at most 15 Korean syllables, its spaces not counted, or 30 characters in English,** so the
  archive shows it whole on a 360-pixel phone: the title row over a page is one line of 20-pixel type across
  328 pixels, cut where it runs over, which holds about 15 Hangul syllables with the spaces between them or
  30 Latin characters; the list row's two lines of 14-pixel type beside a cover picture hold more
  (bmo-artifact's `docs/artifact.md`, 「Where a person reads it」). The search task sets it on the `TITLE:`
  line (`prompts/search/task.md`).
- **A research asked for a room is told to it once its document carries its narration:** the answer in a
  line or two and the document's one share link, as ever, and nothing about the sound, which the archive
  plays at the document's head (`docs/runner.md`, 「The room」 and 「The narration's sound」).
- **A document that is not the research asked for is never reported:** a deep dive's result whose body holds
  none of its shape's sections is not filed, and the try fails with what was missing, to be tried again.
- **The recipients** are a registration per category (「The notifier」): the owner's private room first,
  read from the receiver at the daemon's start where a category's list is empty; a room or an agent is
  added and removed through the routes and the tools. Every recipient of the category gets every notice of
  it; one that refuses is said once in the log.
- **A message goes through the receiver's instruction route** as a `notice` under the kind
  `research-report:<item>`, which the room's answerer says in its own voice; the link stands as given.

## Settings and store

- **Settings** (`docs/settings.md`): the listen address, the data folder, the cadence, the model road, the retry and restore waits, the engine watch, the roles the runner's runs
  hold and their capabilities, the research settings' defaults, and the addresses and token files of agency,
  the tool host's control socket, the artifact service, the receiver and the arbiter's master, whose picture shows
  every node's card. Every address and token file is read from the files the other units already keep,
  named in the settings, never copied.
- **The store** (`docs/store.md`): SQLite under the data root's `research/` folder: `categories`,
  `sources`, `seen`, `queue`, `runs`, `reports`, `rooms`, `pages` (what each filed page and room's document
  holds and still lacks), `facts` (the economy category's figures),
  `stack` (the security category's inventory), `demands` (the arbiter demands held), and the runner's `research_runs`, `research_log`,
  `research_holds` and `research_settings`. The schema starts at the 1.0.0 baseline, laid at start where none is.

## The routes and the tools

`docs/api.md` is the contract: `/health`, `/categories`, `/sources`, `/queue` and `/queue/{item}` (ask,
dismiss, taken, reorder), `/runs`, `/rooms`, `/facts`, `/stack`, `/state` (running or stopped, whole or by
category, with `PATCH` to stop and resume, and what holds the queue while a run's engine is not granted), and the runner's
`/research-runs`, `/research-settings` and `/branches`. The tools the tool host places under the namespace
`research` are the chat agent's doors to the same: `research/ask`, `research/queue`, `research/taken`,
`research/dismiss`, `research/categories`, `research/sources`, `research/rooms`, `research/stop`,
`research/resume` and `research/run`; the owner-only ones hold a capability the chat line alone is given. A
search run is shown `research/rulebook`, the reader of the rulebooks its record names, and the chat line
`research/skill`, the reader of every research skill, each with the asker's own rule for its kind where
they keep one (`docs/api.md`, 「The tools」).

## Phases

1. **The daemon with the loop:** sources, scan, queue, the deep dive on kiki's road, the notices to the
   owner's room, the tools, the deploy. The verification is the chat
   agent's to file on the owner's word; the bench lock is the worker skill's by hand, as today.
2. **The bench lock and the tiers:** the `bench` occupant in the arbiter's rules at tier 1, every service
   at 2, the skill's bench steps on tickets, the n3–n6 rule in the daemon, the restore wait.
3. **The runner is this unit's** (`docs/runner.md`): agency supplies slots and role sessions alone.
4. **The engine a run needs:** each run asks the arbiter's master for its engine and holds it for the run,
   the engine watch in place of the bench watch, as many runs as the arbiter grants, and a bench on
   as many cards as it is granted.
