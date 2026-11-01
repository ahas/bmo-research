# Settings

The daemon reads one TOML file: the one `BMO_RESEARCH_SETTINGS` names, else `research/settings.toml`
under the data root. The data root is the folder `BMO_DATA_DIR` names, else `$HOME/.local/share/bmo/data`.
A file that does not stand leaves every default; a key the file does not name keeps its default; a key
the daemon does not know is refused, naming it. A path may open with `~/`, the home folder.

## The daemon

| Key | Default | Meaning |
|---|---|---|
| `listen` | `127.0.0.1:3238` | The loopback address the routes are answered on |
| `token_file` | `research.token` | The file under the data root's `env/` holding the bearer token every route takes; written at the first start where it does not stand, mode 0600 |
| `store` | `research` | The folder under the data root the store `research.db` stands in |
| `tick_seconds` | 30 | How often the loop looks for a source due, a slot free and an item to run |
| `concurrency` | 4 | How many runs the daemon asks for at once, across every category; how many run is how many cards the arbiter grants them (`docs/design.md`, 「The engine a run needs」) |
| `retries` | 3 | How many times a failed try is tried again before the item is `stuck` and the helper is asked to fix the cause; a stuck item is still tried on every retry wait, and an interrupted try is not counted (`docs/api.md`, 「The deep dive」) |
| `retry_minutes` | 5 | How long a failed try, or what a page still lacks, waits before it is tried again; what a page lacks is asked on every such wait until it is had, and an interrupted try waits for none |
| `restore_wait_minutes` | 20 | How long a run's demand for its engine waits before the owner's private room is told, once, that the engine has not come back; the demand goes on waiting |
| `poll_seconds` | 30 | How long the daemon waits between two readings of a run under way |
| `turn_wait_seconds` | 20 | How long a notice to an agent waits on its turn's answer before it is counted as handed |
| `user_agent` | `bmo-research` | The `User-Agent` the daemon reads the sources with |

What differs by category — its shape, its cap, its cadence, its road and its recipients — is no setting but
the category's registration (`docs/api.md`, 「categories」), kept in the store.

## The road: `[road]`

| Key | Default | Meaning |
|---|---|---|
| `key` | `search:exhaustive` | The research key a run is asked at |
| `provider` | `registered` | The provider a run goes out on, where its category names none |
| `model` | `flash-next` | The model a run goes out on, where its category names none |

## The Korean side: `[translation]`

How a page's body is cut for agency's translator (`docs/api.md`, 「The page」).

| Key | Default | Meaning |
|---|---|---|
| `part_chars` | 2500 | The most characters one part sent to the translator holds |
| `parts_out` | 2 | How many parts of one document are out with the translator at once |
| `parts_out_total` | 4 | How many translations this unit has out with the translator at once across every page, its titles, sections and narrations included: the translator's slots. The rest wait their turn here, in the order they were asked |

## The engine watch: `[engine]`

Which registered models the runs stand on, the arbiter service each one's engine is, and the arbiter's
master that grants them (`docs/design.md`, 「The engine a run needs」). Where the watch names a master, a
category whose road is the watch's `provider` and one of its `models` starts no run until the master grants
the run a card of that model's `service`, and a branch of such a run is asked within the run's card. Which
card answers is the master's; no setting names a node.

| Key | Default | Meaning |
|---|---|---|
| `address` | empty | The arbiter's master, which grants the demands; empty asks no demand, and every run goes out as soon as a slot frees |
| `token` | empty | The file under the data root's `env/` whose text is the master's bearer token; empty where the master takes none |
| `provider` | `registered` | The provider whose models the watch names |
| `models` | `flash-next` served by `lab-arena` | Each watched model, a table of `registration`, its name in agency's registry, and `service`, the arbiter service its engine is; an empty list watches nothing |

## The runner: `[runner]`

How a research run goes out (`docs/runner.md`).

| Key | Default | Meaning |
|---|---|---|
| `role` | `scout` | The co-worker role a run's own seat is asked under |
| `writing_margin_minutes` | 10 | How long past its clock a run's sessions are bounded, for writing the result |
| `review_minutes` | 20 | How long a deep dive's review session is bounded (`docs/runner.md`, 「The review」) |
| `capabilities` | `room.chat`, `room.tools`, `web.read`, `web.sites`, `web.branches`, `web.rulebooks` | The capabilities a run's session record keeps, handed as `toolsAllowed`: `web.branches` is left out where the run may open no branch, `web.rulebooks` where it is handed no rulebook |

## The research settings' defaults: `[research_settings]`

What every research run is held to where the owner changed nothing through `PUT /research-settings`
(`docs/api.md`, 「research settings」); a change is kept in the store and stands over these.

| Key | Default | Meaning |
|---|---|---|
| `branch_role` | `scout` | The co-worker role a branch is seated under |
| `strategy` | `direct-then-browser` | How the web tools read a page: `direct-then-browser`, `browser-first` or `direct-only` |
| `host_interval_ms` | none | The gap between two direct requests to one host; the web tools' own where none is named |
| `engine_interval_ms` | none | The gap between two requests to one search engine; the web tools' own where none is named |
| `keys` | below | Each research key's table: `branches`, `minutes`, and `pages` and `branch_pages` where a ceiling holds |

| Key | `branches` | `minutes` | `pages` | `branch_pages` |
|---|---|---|---|---|
| `search:basic` | 0 | 5 | 3 | none |
| `search:thorough` | 2 | 15 | 8 | 4 |
| `search:exhaustive` | 5 | 30 | 20 | 6 |
| `browse` | 0 | 15 | none | none |

A file naming `keys` names every key it keeps; the four above are the research keys.

## The tool host: `[tool_host]`

Where the tool host's control socket answers (`docs/api.md`, 「Calls this unit makes」).

| Key | Default | Meaning |
|---|---|---|
| `socket` | empty | The control socket's path; empty for `bmo/mcp-host.sock` under the runtime folder `XDG_RUNTIME_DIR` names, else `/run/user/<uid>` |
| `issuer_token` | `tool-host/issuer-token` | The file under the data root whose text is the host's issuer token |

## The narration: `[voice]`

How a deep dive's narration is spoken (`docs/api.md`, 「The page」).

| Key | Default | Meaning |
|---|---|---|
| `name` | empty | The voice the narration is spoken in; empty for the first voice the voice unit lists |

## Peers

Every peer's address and token are read from the files that peer already keeps under the data root's
`env/`, at each call, by the file names below; nothing of them is copied into this file.

| Key | Default | Meaning |
|---|---|---|
| `[agency] env` | `agency.env` | Holds `AGENCY_CONTROL_PORT`, agency's loopback port (3220 where it names none), and `AGENCY_CONTROL_TOKEN`, its token |
| `[receiver] env` | `receiver.env` | Holds `RECEIVER_ADDRESS`, the receiver's address, and `RECEIVER_TOKEN`, its token |
| `[artifact] address` | `http://127.0.0.1:3230` | The artifact service's address |
| `[artifact] token` | `artifact.token` | The file whose text is the artifact service's token |
| `[voice] settings` | `voice.toml` | The voice unit's own settings file, whose `address` is where it answers and whose `token_file` names the file under `env/` holding its token |
| `[job] settings` | `bmo-job.toml` | The job service's own settings file, whose `[server] listen` is where it answers (on the loopback where it listens on every address) and whose `[server] token_file` names the file holding its token, under `env/` where the name is not absolute |
| `[job] queue` | `todo` | The job queue a stuck item's repair is filed in (`docs/api.md`, 「The repair」) |
| `[job] assignee` | `helper` | The worker a repair is filed for |
| `[job] labels` | `["lane=repair"]` | The labels a repair carries, each `key=value`; the deployment's job service names the lane unattended, so the repair opens without an approval |

An env file is read as `KEY=value` lines, as the unit that keeps it writes it.

## Example

```toml
listen = "127.0.0.1:3238"
concurrency = 4

[road]
provider = "registered"
model = "flash-next"

[engine]
address = "http://127.0.0.1:7440"

[[engine.models]]
registration = "flash-next"
service = "lab-arena"

[[engine.models]]
registration = "flash-next-coder"
service = "lab-arena-coder"
```
