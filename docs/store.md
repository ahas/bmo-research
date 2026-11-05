# The store

One SQLite file, `research.db`, in the folder the settings' `store` names under the data root. Its schema
carries a version (SQLite's `user_version`); the daemon, and `bmo-research migrate`, bring it to the version
the binary holds as they start, one step at a time, and refuse a store newer than the binary. The step that
makes the store registers the three first categories (`docs/api.md`, 「categories」). A write that finds
the file locked by another process waits for the lock to be let go, however long that takes. Times are epoch
milliseconds; lists are JSON arrays.

## `categories`

| Column | Holds |
|---|---|
| `name` | The category's name; the primary key |
| `purpose` | Its purpose, a line |
| `shape` | `manual-or-investigation`, `advisory` or `brief` |
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
| `last_taken` | Of those, the ones it kept as new seen rows, or null |
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
| `state` | `unqueued`, kept and not yet queued, which the scan's pass queues before it ends; `queued`, an item stands for it; `archived`, the archive already holds it; `stale`, an advisory whose fixed builds predate the platform's age floor, never queued |
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
| `tries`, `retry_at` | The tries counted — each failed one and the one under way, an interrupted one taken off — and when the next may be made |
| `run_id` | The research run of the latest try |
| `document_id`, `title`, `description`, `share_url` | The document the run filed, and its live link; what its page holds and lacks is the document's `pages` row |
| `fit`, `touches` | Whether the document asks the owner's word, and what an advisory touches |
| `plan` | The heading of the document's plan section |
| `job_item_id` | The job item the chat agent filed for it |
| `repair_item_id` | The job item filed for the repair worker while it is stuck, or null; cleared as it is asked again |
| `error`, `error_at` | Why the latest try failed or was interrupted, and when that try ended; null while a try is under way and after one that filed its document, until the archive answers 404 for that document, which it then names with when that was read |
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
| `kind` | `report`, told once its page is whole; `stuck`, the notice of an item that became stuck; `part`, a touched repository's part to its worker |
| `instruction_id` | The receiver's instruction, or null for a turn |
| `state`, `error` | `sent` or `refused`, and why |
| `at` | When it was sent |

## `pages`

What each filed document's page holds and still lacks, one row per document: a deep dive's page, finished
for the item its run was asked for, and the document of a run told to its room, finished for that run
(`docs/api.md`, 「The page」; `docs/runner.md`, 「The narration's sound」).

| Column | Holds |
|---|---|
| `document_id` | The document; the primary key |
| `item_id` | The item the page is finished for, or null for the document of a run told to its room, finished for the run that filed it |
| `languages`, `narrations` | The languages the page holds, and those whose narration it plays, JSON lists, Korean first |
| `pending` | What it still lacks before its report or its room is told — `korean`, `narration-ko`, `narration-en`, `review`, `review-fixes` — a JSON list, empty where it lacks nothing |
| `pending_retry_at` | When what it still lacks may be asked again, or null where nothing is pending |
| `translation_tries`, `narration_tries` | The Korean sides and the narrations asked for it |
| `gone_at` | When the archive answered 404 for the document, closing the page as failed, or null while the document stands; a closed page is never drawn or asked again |

## `renderings`

The Korean side of a deep dive's page, kept so the page can be drawn again (`docs/api.md`, 「The page」).

| Column | Holds |
|---|---|
| `document_id` | The page's document; the primary key |
| `title`, `description`, `body` | The Korean title, description (or null) and markdown body |
| `created_at` | When it was had |

## `syntheses`

The synthesis asked of the voice unit for each narration a page still lacks, kept until its sound is
attached, so a narration has one synthesis living at a time (`docs/api.md`, 「The page」).

| Column | Holds |
|---|---|
| `document_id`, `language` | The page's document and the narration's language, `ko` or `en`; the primary key |
| `synthesis_id`, `characters` | The voice unit's synthesis, and how many characters it was asked to speak |
| `asked_at` | When it was asked |

## `translated_parts`

Each part of a deep dive's Korean side as it is had, or as a Korean side set aside held it in the part's
place, so a Korean side asked again translates only the parts it lacks; a part kept is taken only while it
agrees with its English part (`docs/api.md`, 「The page」).

| Column | Holds |
|---|---|
| `document_id`, `english` | The page's document and the part's English text; the primary key together |
| `korean` | The part's translation |

## `page_texts`

What a deep dive's page holds beside the run's result: the review's rewrites and the Korean narration's
words.

| Column | Holds |
|---|---|
| `document_id`, `name` | The page's document and the text's name; the primary key together: `body-en` the English body as the review rewrote it, `narration-en` the English script as the review rewrote it, `narration-ko` the Korean narration's words as they were last spoken or rewritten |
| `text` | The text |

## `narration_asks`

How often each narration of a page was asked anew, which its synthesis's key carries
(`docs/api.md`, 「The page」).

| Column | Holds |
|---|---|
| `document_id`, `language` | The page's document and the narration's language; the primary key together |
| `renewed` | How many times it was asked anew: after a synthesis that ended without its sound, or a review that rewrote it |

## `page_reviews`

A deep dive's page's one review (`docs/runner.md`, 「The review」).

| Column | Holds |
|---|---|
| `document_id` | The page's document; the primary key |
| `answer` | What the review wrote |
| `applied` | The indices of its fixes applied or left out, a JSON list |
| `remade` | The languages whose narration the review had made again, a JSON list |
| `reviewed_at` | When it answered |

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
| `started_at`, `finished_at` | When it was asked, and when it closed |
| `branches_allowed`, `branches` | How many branches it may open, and how many it opened |
| `record` | The run's research record, the JSON object its sessions carry under `labels.research` |
| `tells` | 1 where its end is told to its room, 0 for a run the deep dive asked |
| `turns` | The names of the agency turns it went out under, a JSON list |
| `written` | The answer it wrote last, kept for a cut run's room and for a run that continues from it |
| `continues` | The run whose result it continues from (`docs/runner.md`, 「A run that continues」), or null |
| `answered` | The rounds that answered, in the order they were asked, a JSON list |
| `review_asks` | How many times its page's review asked agency for a slot, which the review's request key carries (`docs/runner.md`, 「The review」) |
| `document_id`, `title`, `description`, `link` | The document it filed, its head, and its share link |

## `research_log`

| Column | Holds |
|---|---|
| `id` | The line's key |
| `run_id` | The research run it is about |
| `at`, `kind`, `text` | When, which step (`docs/api.md`, 「research runs」), and what it says |

## `research_holds`

What a research run, its page's review, or a notice to an agent holds of agency while it stands, so a
restart knows whose owner to give back and which sessions to end (`docs/runner.md`, 「A restart」): a slot's
row names the run or the item whose owner asked it, and a session's row the session agency may not have
ended with the owner's release, one bound to no slot or still in a turn.

| Column | Holds |
|---|---|
| `id` | The hold's key |
| `run_id` | The research run; for a notice's hold, the item the notice is about |
| `kind`, `value` | `slot` and the slot request's token, or `session` and the session's id, for a run or its review; `notice-slot` or `notice-session` and the same for a notice |

## `demands`

Every demand the daemon holds of the arbiter's master, kept until it is given back, so a start gives back
what a stopped daemon left (`docs/api.md`, 「The engine」).

| Column | Holds |
|---|---|
| `id` | The demand's id, as the master gave it |
| `try_id` | The try that holds it, once the place it was asked for has taken one; null for a place that has taken none yet and for a branch's |
| `run_id` | The research run it stands for: the try's once it is asked, or the run a branch was opened from; null for a place's |
| `within` | The demand it was asked within, a branch's run's, or null for a place's and a try's |
| `service` | The arbiter service it asks a card of |
| `asked_at` | When it was asked |

## `research_settings`

| Column | Holds |
|---|---|
| `id` | 1, the one row |
| `value` | The research settings as the owner last set them, a JSON object |
