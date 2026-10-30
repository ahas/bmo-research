# The store

One SQLite file, `research.db`, in the folder the settings' `store` names under the data root. Its schema
carries a version (SQLite's `user_version`); the daemon, and `bmo-research migrate`, bring it to the version
the binary holds as they start, one step at a time, and refuse a store newer than the binary. The step that
makes the store registers the three first categories (`docs/api.md`, 「categories」). Times are epoch
milliseconds; lists are JSON arrays.

## `categories`

| Column | Holds |
|---|---|
| `name` | The category's name; the primary key |
| `purpose` | Its purpose, a line |
| `shape` | `manual-or-investigation`, `advisory` or `brief` |
| `cap` | How many items the scan keeps waiting or running for it |
| `cadence_minutes` | How often its sources are read where a source names no cadence |
| `gather_minutes` | How often a brief category's items are gathered, or null |
| `provider`, `model` | Its road, or null for the settings' |
| `state`, `state_at` | `running` or `stopped`, and when it was last changed, or null |
| `created_at` | When it was registered |

## `sources`

| Column | Holds |
|---|---|
| `id` | The source's key |
| `category` | The category it belongs to |
| `kind` | Its kind (`docs/sources.md`) |
| `address` | Where it is read |
| `query` | The query its kind takes, or null |
| `require`, `exclude` | The filter's words, JSON lists |
| `every_minutes` | Its own cadence, or null for its category's |
| `item_limit` | How many items one reading asks for |
| `failure_count` | Readings failed since the last that did not |
| `last_error` | The last failure's sentence, or null |
| `read_at` | When it was last read, or null |
| `created_at`, `changed_at` | When it was registered and last changed |

## `seen`

Every candidate judged, once, by its key.

| Column | Holds |
|---|---|
| `key` | The canonical address, else the name lower-cased; the primary key |
| `category` | The category of the source it came from |
| `name`, `maker`, `address` | What the candidate is, who made it and the maker's page for it |
| `kind` | The item kind its source turns up |
| `published_at` | When it appeared, where the source says |
| `source_id` | The source it came from |
| `state` | `unqueued`, waiting for a slot; `queued`, an item stands for it; `archived`, the archive already holds it |
| `item_id` | The item made of it, or null |
| `found_at` | When it was read |

## `queue`

The items (`docs/api.md`, 「queue」).

| Column | Holds |
|---|---|
| `id` | The item's key |
| `category` | Its category |
| `key`, `name`, `maker`, `address`, `kind`, `shape` | The thing, as its candidate or the ask named it |
| `source_id` | The source it came from, or null for an ask |
| `asked` | 1 for an ask, 0 for an item the scan took in |
| `priority` | Its place in its category's queue, highest first |
| `state` | Its state |
| `tries`, `retry_at` | The tries made, and when the next may be made |
| `run_id` | Agency's ledger run of the latest try |
| `document_id`, `title`, `description`, `share_url` | The document the run filed, and its live link |
| `ko_document_id`, `ko_share_url` | The Korean rendering's document and its live link, or null |
| `translation_tries`, `translation_retry_at` | The renderings asked, and when a pending one may be asked again, or null where none is pending |
| `fit`, `fit_nodes`, `touches` | Whether the document asks the owner's word, the model nodes a manual's fit names (a JSON list) and what an advisory touches |
| `plan` | The heading of the document's plan section |
| `job_item_id` | The job item the chat agent filed for it |
| `error` | Why the latest try failed |
| `found_at`, `created_at`, `changed_at` | When its candidate was found, when it was queued and last changed |

## `runs`

| Column | Holds |
|---|---|
| `id` | The try's key |
| `category` | The category it ran for |
| `item_id`, `item_ids` | The item it was asked for, and every item it carries, a JSON list |
| `try` | The try's number |
| `run_id` | Agency's ledger run, or null where the ask was refused |
| `state`, `outcome`, `error` | How it stands, agency's outcome, and why it failed |
| `started_at`, `ended_at` | When it was asked and when it ended |

## `reports`

| Column | Holds |
|---|---|
| `id` | The report's key |
| `item_id` | The item told |
| `recipient` | Who was told: `room:<roomId>` or `agent:<agentName>` |
| `kind` | `report`; `translated`, a report's follow-up once its Korean rendering is filed; `failed`, a failed item's notice; `part`, a touched repository's part to its worker |
| `instruction_id` | The receiver's instruction, or null for a turn |
| `state`, `error` | `sent` or `refused`, and why |
| `at` | When it was sent |

## `recipients`

| Column | Holds |
|---|---|
| `id` | The registration's key |
| `category` | The category it hears |
| `kind` | `room` or `agent` |
| `room_id`, `agent_name` | The receiver's room, or the agent; one of the two |
| `origin` | `owner` or `added` |
| `added_at` | When it was added |

## `facts`

The figures the economy briefs carried, a series a later reader takes in.

| Column | Holds |
|---|---|
| `id` | The fact's key |
| `name`, `value`, `unit` | The figure, as the brief wrote it |
| `as_of` | The date it stands for, as the brief wrote it |
| `source_address` | Where it was read |
| `run_id` | Agency's ledger run whose brief held it |
| `document_id` | That brief's document |
| `created_at` | When it was kept |

## `stack`

The platform's stack inventory, the ground an advisory is judged on.

| Column | Holds |
|---|---|
| `id` | The row's key |
| `component`, `version` | A distribution, a runtime, a library or a program, and the version run |
| `place` | The host it runs on, or null |
| `repository` | The repository that runs it, or null |
| `worker` | The agent that keeps that repository, or null |
| `added_at` | When it was added |
