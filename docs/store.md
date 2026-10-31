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
| `pattern` | A `page` source's link pattern, or null for another kind |
| `strategy` | How a `page` source's listing is read, `direct` or `browser`, or null for another kind |
| `require`, `exclude` | The filter's words, JSON lists |
| `every_minutes` | Its own cadence, or null for its category's |
| `item_limit` | How many items one reading asks for |
| `failure_count` | Readings failed since the last that did not |
| `last_error` | The last reading's failure sentence, or null when it succeeded |
| `read_at` | When it was last read, or null |
| `scan_count` | Readings made, failed or not |
| `last_yield` | Candidates the last reading that succeeded turned up past the filter, or null |
| `last_taken` | Of those, the ones it kept as new seen rows, `unqueued`, or null |
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
| `origin` | Where it came from: `source`, `asked` or `follow-up` |
| `because` | The words its ask was made with, or null |
| `parent_item_id`, `parent_document_id` | The item and the document a follow-up follows from, or null |
| `parent_title`, `parent_share_url` | That document's title and live link as they stood when it was asked, or null |
| `priority` | Its place in its category's queue, highest first |
| `state` | Its state |
| `tries`, `retry_at` | The tries made, and when the next may be made |
| `run_id` | The research run of the latest try |
| `document_id`, `title`, `description`, `share_url` | The document the run filed, and its live link |
| `languages`, `narrations` | The languages its document's page holds, and those whose narration it plays, JSON lists, Korean first |
| `translation_tries`, `narration_tries` | The Korean sides and the narrations asked for its page |
| `pending` | What its page still lacks before its report is told — `korean`, `narration-ko`, `narration-en` — a JSON list, empty where it lacks nothing |
| `pending_retry_at` | When what its page still lacks may be asked again, or null where nothing is pending |
| `fit`, `touches` | Whether the document asks the owner's word, and what an advisory touches |
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
| `run_id` | The research run, or null where the ask was refused |
| `state`, `outcome`, `error` | How it stands, the research run's outcome, and why it failed |
| `started_at`, `ended_at` | When it was asked and when it ended |

## `reports`

| Column | Holds |
|---|---|
| `id` | The report's key |
| `item_id` | The item told |
| `recipient` | Who was told: `room:<roomId>` or `agent:<agentName>` |
| `kind` | `report`, told once its page is whole; `failed`, a failed item's notice; `part`, a touched repository's part to its worker |
| `instruction_id` | The receiver's instruction, or null for a turn |
| `state`, `error` | `sent` or `refused`, and why |
| `at` | When it was sent |

## `renderings`

The Korean side of a deep dive's page, kept so the page can be drawn again (`docs/api.md`, 「The page」).

| Column | Holds |
|---|---|
| `document_id` | The page's document; the primary key |
| `title`, `description`, `body` | The Korean title, description (or null) and markdown body |
| `created_at` | When it was had |

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
| `run_id` | The research run whose brief held it |
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

## `research_runs`

The runner's ledger (`docs/runner.md`): one row per research run.

| Column | Holds |
|---|---|
| `id` | The run's key |
| `key`, `line` | The research key it was asked at, and the line it goes out on |
| `query`, `asker`, `options`, `room_id` | What was asked, by whom, with which options, and the room it is told to, or null |
| `provider`, `model` | The road it goes out on |
| `state`, `outcome`, `error` | `waiting`, `running` or `closed`; `answered`, `failed` or `cut` once closed; and why it failed |
| `started_at`, `closes_at`, `finished_at` | When it was asked, when its web tools close, and when it closed |
| `branches_allowed`, `branches` | How many branches it may open, and how many it opened |
| `record` | The run's research record, the JSON object its sessions carry under `labels.research` |
| `tells` | 1 where its end is told to its room, 0 for a run the deep dive asked |
| `turns` | The names of the agency turns it went out under, a JSON list |
| `written` | The answer it wrote last, kept for a cut run's room |
| `document_id`, `title`, `description`, `link` | The document it filed, its head, and its share link |

## `research_log`

| Column | Holds |
|---|---|
| `id` | The line's key |
| `run_id` | The research run it is about |
| `at`, `kind`, `text` | When, which step (`docs/api.md`, 「research runs」), and what it says |

## `research_holds`

What a research run holds of agency while it stands, so a restart gives it back.

| Column | Holds |
|---|---|
| `id` | The hold's key |
| `run_id` | The research run |
| `kind`, `value` | `seat` and the seat's token, or `session` and the session's id |

## `demands`

Every demand the daemon holds of the arbiter's master, kept until it is given back, so a start gives back
what a stopped daemon left (`docs/api.md`, 「The engine」).

| Column | Holds |
|---|---|
| `id` | The demand's id, as the master gave it |
| `try_id` | The try it was asked for, or null for a branch's |
| `run_id` | The research run it stands for: the try's once it is asked, or the run a branch was opened from |
| `within` | The demand it was asked within, a branch's run's, or null for a try's |
| `service` | The arbiter service it asks a card of |
| `asked_at` | When it was asked |

## `research_settings`

| Column | Holds |
|---|---|
| `id` | 1, the one row |
| `value` | The research settings as the owner last set them, a JSON object |
