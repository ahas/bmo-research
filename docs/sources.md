# Sources

A source is a registration (`docs/api.md`, 「sources」): the category it belongs to, a kind, the address it
is read at, a query where the kind takes one, a link pattern and a strategy where the kind is `page`, a
filter, its own cadence where it has one, and how many items one reading asks for. The daemon reads it at
most once every `every_minutes` (its category's `cadenceMinutes` where it names none), and only while its
category runs: a `page` source through explorer (「A page」), every other kind with its own HTTP client,
under the `User-Agent` the settings name. Any kind serves any category: a security list's feed is an `rss`
source of `security`, an economic release's feed one of `economy`.

## The kinds

| Kind | Default address | Query | Reads | Item kind |
|---|---|---|---|---|
| `huggingface-trending` | `https://huggingface.co/api/models` | none | the address with `sort=trendingScore` and `limit` | `model` |
| `huggingface-search` | `https://huggingface.co/api/models` | the search words | the address with `search`, `sort=lastModified`, `direction=-1` and `limit` | `model` |
| `arxiv` | `https://export.arxiv.org/api/query` | an arXiv `search_query` | the address with `search_query`, `sortBy=submittedDate`, `sortOrder=descending` and `max_results` | `paper` |
| `github-releases` | `https://api.github.com` | the repository, `owner/name` | `<address>/repos/<query>/releases` with `per_page` | `release` |
| `rss` | none, required | none | the address: an RSS or an Atom feed | `entry` |
| `page` | none, required | none | the address through explorer: a listing page with no feed | `entry` |

Each item becomes a candidate: a name, the maker, the address of the maker's own page, the date it
appeared, and the words the filter reads.

- **`huggingface-*`:** a model repository of the Hub's JSON list. The name is its `id`; the maker its
  `author`, else the part of the id before the slash; the address `https://huggingface.co/<id>`; the date
  its `createdAt`, else its `lastModified`; the words the id, the tags and the pipeline tag.
- **`arxiv`:** an entry of the Atom feed. The name is its title, its whitespace folded; the maker its
  first author; the address its `abs` page with the version suffix dropped; the date its `published`; the
  words the title, the abstract and the categories.
- **`github-releases`:** a release of the JSON list, drafts passed over. The name is the repository's name
  and the release's tag; the maker the repository's owner; the address its `html_url`; the date its
  `published_at`; the words the name, the tag and the release notes.
- **`rss`:** an entry of the feed, read by `feed-rs`. The name is its title; the maker the feed's title;
  the address its first link; the date its published time, else its updated time; the words the title and
  the summary.
- **`page`:** a link of the listing page that matches the source's pattern (「A page」). The name is the
  link's text, its whitespace folded and cut at 200 characters; the maker the page's title, else its
  host; the address the link, absolute; no date; the words the text of every link to that address and
  the address.

## A page

A `page` source reads a maker's listing — a news or blog index with no feed — and takes the posts it
links to.

- **Its fields:** `address`, the listing page; `pattern`, which links are posts; and `strategy`, how
  explorer reads the page, `direct` where it names none.
- **The pattern** is a path prefix where it begins with `/`: a link matches where it stands on the
  listing's own host and its path begins with the prefix and goes on past it, so `/news/` takes
  `/news/a-post` and not `/news/` itself. Any other pattern is a regular expression, searched in the
  link's whole absolute address. A link to the listing itself never matches.
- **The page is read through explorer,** never by the daemon's own client, so explorer's pacing, address
  checks and Chrome stand round every reading: the tool host's `web/web_fetch` asked for the page's markup
  as served, read on slice by slice to its end, at most 16 slices of 100,000 characters (`docs/api.md`,
  「Calls this unit makes」).
- **The two strategies** are explorer's: `direct`, its direct client, and its real Chrome only after a
  block or an empty shell; `browser`, its real Chrome from the start, for a listing whose links a script
  draws.
- **The links** are the page's anchors with an `href`, each resolved against the address the reading
  ended at, its fragment dropped; a link that is no `http` or `https` address is passed over. A post
  linked more than once is one candidate, its name the first non-empty text among its links, a link's
  `aria-label` or `title` standing for a text it lacks, and the address where no link has one. The first
  `limit` posts, in the page's order, are the reading's candidates, and the filter is applied to them.

## The filter

- **`require`:** every word must stand in the candidate's words; **`exclude`:** none may. Both are matched
  without regard to case, as parts of the text, so `gguf` holds in `Model-GGUF`. An empty list asks
  nothing.
- A candidate the filter rules out is not kept as seen, so a filter loosened later takes it in.

## From a candidate to an item

1. **Its key** is its address, its scheme and a trailing slash dropped and lower-cased, else its name
   lower-cased.
2. **A candidate whose key a seen row holds** is passed over: it was judged once.
3. **The archive is asked** with the artifact service's `GET /documents?q=<name>&limit=1`. A document there
   keeps the candidate as seen in the state `archived`. An archive that cannot be asked leaves the candidate
   unseen, so the next reading judges it again.
4. **Otherwise the candidate is kept as seen,** `unqueued`, the moment it is judged.
5. **Every `unqueued` seen row is queued** in the same pass, its category's oldest found first, each becoming
   a `waiting` item of the shape its category takes, and in a `manual-or-investigation` category the shape
   its item kind takes (`docs/api.md`, 「The questions」). The queue has no cap, so no seen row waits outside
   it; a pass that queued an item pulls (`docs/design.md`, 「A node pulls」).

## What a reading leaves on its source

Every reading, whether it succeeds or fails, sets the source's `readAt` to its time and adds one to its
`scanCount`; before the first reading `readAt` is null and the count 0.

- **A reading that succeeds** sets `lastYield`, how many candidates it turned up that the filter let
  through, and `lastTaken`, how many of those it kept as seen: new to the seen rows and not held by the
  archive, so each is queued. Both are null before a reading first succeeds.
- **A reading that fails** leaves `lastYield` and `lastTaken` as the last reading that succeeded set them.

## Failures

- **A reading that fails** — the address does not answer, answers a status that is no success, or answers
  a body its kind does not read; for a `page`, explorer fails the page, or the page holds no link the
  pattern matches, each said apart — is said once in the log, counted on the source's `failureCount`,
  kept as its `lastError`, and read again at its next turn. A reading that succeeds sets the count back
  to 0 and `lastError` to null.
