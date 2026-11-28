# The research tools

The `research` namespace is the researcher's doors. Call each entry as your session lists it, or through
`bmo use research/<entry> --json '{...}'`. An answer marked `isError` is a refusal: read its words for what to do
next. Entries that change what the researcher reads or whom it tells act only on the owner's word.

## Asking the researcher

- `research/ask` puts a thing into the researcher's own queue under a category (`it`, `security`, `economy`):
  `{"name": "<the thing>", "address": "<its maker's page>", "category": "it", "because": "<one line: why it is asked>"}`.
  Its report comes back to the room as an instruction.
- An ask that continues a report the person is discussing (the one they name, else the last one you reported in
  this room) names the report it follows from: `follows`, the item that report was told for, and `document`, that
  report's document, either alone where only one is known. Where the room's lines hold neither, find the item
  with `research/queue`. Say in `because` one line of what in that report the question takes up.
- `research/queue` reads where the queue stands: what runs, what waits and what was reported, each with its
  report's link.

## A web research of your own

- `research/run` sends a search out at a depth and answers at once with the run's id; its result comes back to the
  room as an instruction when the run ends. `key` is the depth: `search:basic` for one value or one fact,
  `search:thorough` for a question with a few angles, `search:exhaustive` for a deep dive, `browse` for a task that
  must open and click through pages. `query` is the question written whole; `roomId` the room this turn answers;
  `asker` who asked (default: the owner). Name `model` and `provider` only where the owner named them.
  Example: `{"key": "search:thorough", "query": "...", "roomId": 123, "asker": "<their name>"}`.

## A request a research skill binds

- A request you answer yourself that a research skill binds (places to eat or food to order, a thing to buy, where
  to put money, and the other kinds `research/skill`'s description names) follows that skill: read it first with
  `research/skill`, naming the asker in `asker`, so their own rule for that kind comes with it.

## What the researcher tells you

- The researcher never files work. Its notices reach you as instructions, each with a report's link and one line.
- A notice asking for the owner's word on a verification is yours to put to him. On his word, file it as an item
  with the report's link in the brief, and record that with `research/taken` (`item`, the research item, and
  `job`, the item you filed). A worker's own part of a security notice reaches that worker by itself.
- `research/dismiss` drops a research item on the owner's word; nothing turns it up again.

## Changing what it reads and whom it tells

- `research/categories`, `research/sources`, `research/recipients` and `research/rooms` read with
  `{"action": "list"}` and, on the owner's word, change with `add`, `remove` or `change`.
- `research/stop` and `research/resume` hold and release the researcher's loop, whole or one `category`. Tell the
  owner a stop before you make it, and confirm it by reading the category's state back with `research/queue`.
