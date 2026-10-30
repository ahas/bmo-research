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
| `concurrency` | 1 | How many runs stand under way at once, across every category |
| `retries` | 3 | How many times a failed try is tried again before the item is `failed`, and a page's pending Korean side before it is no longer asked |
| `retry_minutes` | 30 | How long a failed try, or what a page still lacks, waits before it is tried again |
| `restore_wait_minutes` | 20 | How long the queue waits on a watched registration's health once a bench is gone before the owner's private room is told, once, that the model has not come back |
| `poll_seconds` | 30 | How long the daemon waits between two readings of a run under way |
| `turn_wait_seconds` | 20 | How long a notice to an agent waits on its turn's answer before it is counted as handed |
| `model_nodes` | `["n3", "n4", "n5", "n6"]` | The nodes the runs' model stands on; a manual's fit naming one tells the chat agent to have the bench carried by a worker standing elsewhere |
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

## The bench watch: `[bench_watch]`

Which registered models the runs stand on, on which nodes, and where the daemon reads whether a bench holds
one of them (`docs/design.md`, 「The n3–n6 restore path」). A category whose road is the watch's `provider`
and one of its `models` starts no run while a bench's ticket stands on one of that model's nodes, nor after,
until the registration answers its health.

| Key | Default | Meaning |
|---|---|---|
| `address` | `http://127.0.0.1:7440` | The arbiter's master, whose cluster picture shows every node's tickets |
| `token` | empty | The file under the data root's `env/` whose text is the master's bearer token; empty where the master takes none |
| `occupant` | `bench` | The occupant a bench's ticket stands under in the arbiter's rules |
| `provider` | `registered` | The provider whose models the watch names |
| `models` | `flash-next` on `n3` | Each watched model, a table of `registration`, its name in agency's registry, and `nodes`, the nodes it stands on; an empty list watches nothing |

## The runner: `[runner]`

How a research run goes out (`docs/runner.md`).

| Key | Default | Meaning |
|---|---|---|
| `role` | `scout` | The co-worker role a run's own seat is asked under |
| `writing_margin_minutes` | 10 | How long past its clock a run's sessions are bounded, for writing the result |
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
| `wait_seconds` | 900 | How long one narration's synthesis is waited on before it is counted as not made |

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

An env file is read as `KEY=value` lines, as the unit that keeps it writes it.

## Example

```toml
listen = "127.0.0.1:3238"
concurrency = 1

[road]
provider = "registered"
model = "flash-next"

[bench_watch]
address = "http://127.0.0.1:7440"

[[bench_watch.models]]
registration = "flash-next"
nodes = ["n3"]
```
