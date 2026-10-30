# Deep dives

- This rulebook sets how a deep dive works: how it frames its question, reads the maker of a thing
  before anyone writing about it, sets the maker's figures as the baseline, what its document holds
  and the narration it carries. Its deciding section stands in the search runner's prompt and the
  run reads the rest once that section says it applies.

## Whether this rulebook applies at all

- **It applies** to every run at the thorough or exhaustive depth: such a run is a deep dive,
  whatever its question. A run at the basic depth is none.
- **How much binds depends on the result.** Every rule here binds a deep dive whose result has a
  body; a brief result and study material keep only 「The maker's word first」 and the done
  condition in 「The question」.

## The question

- **The question is framed toward a judgement, not a collection.** Before the first query, settle
  what the reader will decide from the document. For a thing somebody made (a model, an engine, a
  library, a tool, a product, a service) that is whether we can use it, what of ours it touches
  and what to try next, and the done condition is drawn from that judgement.
- **A deep dive about a thing somebody made is not done while none of its maker's own material
  was read.** Where every attempt failed, the first line is `ANSWERED: no`, and UNCONFIRMED names
  each maker's page tried and what it answered. No count of queries or pages stands in for this.

## The maker's word first

- **The maker is read before anyone writing about it.** Its primary material is found and read
  first: the official documentation, the repository's README and docs, the release notes and
  changelog, the model card, the configuration files, the licence, the reproduction scripts, the
  paper in full, the official blog, the registry page and its discussions. Articles, blogs,
  forums and videos come after, read as what others say about it.
- **Read to the end of what bears on the question:** a model card's every table and its
  footnotes, a paper's method, its evaluation and the conditions under each table, never the
  abstract standing for the paper.
- **A repository is opened, not judged by its front page.** The file tree, the README, the docs
  folder, the examples or cookbook, the wiki, the releases, recent pull requests and issues, and
  the code itself where a claim turns on how something is done. A page a listed site tool answers
  is read with that tool; any other file is read with web_fetch at its raw address.
- **The maker's claims, others' findings and our judgement are told apart sentence by sentence.**
  Each sentence of the maker's carries its source's mark and, where the source is long, the part it
  stands in: a section, a table, a file. A sentence that is our judgement reads as ours. A maker's
  claim no independent source checks says so beside it.
- **Disagreement is set side by side.** Where another source differs from the maker, both stand
  with their sources and the difference is said. A fault raised in public (an issue, a forum
  thread, a registry discussion) is followed to whether it has since been fixed, and the document
  says which.

## The maker's figures as the baseline

- **Every figure of performance carries the maker's own beside it.** For a model or an engine,
  the maker's published figure for the same measure stands as the baseline, with its conditions
  (the benchmark and its split, the settings, the hardware, the version) and who measured it: the
  maker, a third party, or us.
- **A figure stands beside its pair:** one setting against another, the maker's claim against an
  independent measurement, the thing against what we already run where it would take that place.
- **A judgement resting on figures nobody reproduced is marked `no baseline`**: a tuning, a
  comparison, a better or worse that no one here measured under the maker's conditions. It is
  never written as measured.
- **A value nobody measured is a calculation shown whole**, what goes in and the arithmetic, so the
  reader can redo it: the weight bytes ÷ 2^30 and a cache estimate set against a card's memory, an
  index as vectors × dimensions × bytes.
- **Whether we could run it is judged against our own equipment** as the request names it, each
  machine or card coming out as fitting, not fitting, or what blocks it. A request naming no
  equipment leaves the fit in UNCONFIRMED rather than guessed.
- **A value not checked is marked `estimated` or `needs checking`**, its ground in brackets after it.

## What the document holds

- **A deep dive about a thing somebody made takes one of the three shapes `research.md` sets** — the
  investigation, the manual or the verification — and that rulebook names its sections; the rules
  below hold inside whichever shape it takes.
- **What the maker states comes first in the middle,** a section of its own: what it says the thing
  supports, its limits and its figures with their conditions, before any section of what others
  found or what we judge.
- **The verdict is a judgement:** whether to use it, for what, and what blocks it.
- **The next step is a reproduction plan** where the answer leans on the maker's figures: which
  figure, on which set, with which settings, about how long it takes, and whether the maker
  publishes the script and the data.
- **What was read and what was not** is the middle's last section: every primary material read,
  and every one that could not be read with what it answered, or none.
- **Tables and lists stand on lines of their own,** never run into a paragraph.

## The narration

- **Every deep dive with a body carries one:** a script for the ear, written apart from the
  document, which the chat agent's voice speaks and the document keeps at its top, so its reader can listen
  while looking at the page.
- **It is not the document read out.** It tells a listener what the thing is, the verdict and the
  two or three things deciding it, what is still uncertain and what to try next, in sentences of
  its own.
- **It is written to be heard,** in the question's language: plain sentences, no persona, no list,
  table, footnote mark, address, code or structure word; only the few figures that matter, rounded
  where the exact value adds nothing; a long identifier said by its short name.
- **About three to five minutes aloud,** some 1,500 to 2,500 characters of Korean or 400 to 700
  words of English, and never more.
