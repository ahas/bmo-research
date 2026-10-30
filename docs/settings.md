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
| `retries` | 3 | How many times a failed try is tried again before the item is `failed` |
| `retry_minutes` | 30 | How long a failed try waits before it is tried again |
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

## Peers

Every peer's address and token are read from the files that peer already keeps under the data root's
`env/`, at each call, by the file names below; nothing of them is copied into this file.

| Key | Default | Meaning |
|---|---|---|
| `[agency] env` | `agency.env` | Holds `AGENCY_CONTROL_PORT`, agency's loopback port (3220 where it names none), and `AGENCY_CONTROL_TOKEN`, its token |
| `[receiver] env` | `receiver.env` | Holds `RECEIVER_ADDRESS`, the receiver's address, and `RECEIVER_TOKEN`, its token |
| `[artifact] address` | `http://127.0.0.1:3230` | The artifact service's address |
| `[artifact] token` | `artifact.token` | The file whose text is the artifact service's token |

An env file is read as `KEY=value` lines, as the unit that keeps it writes it.

## Example

```toml
listen = "127.0.0.1:3238"
concurrency = 1

[road]
provider = "registered"
model = "flash-next"
```
