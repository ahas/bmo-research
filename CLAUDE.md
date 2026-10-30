# Working in bmo-research

This repository holds the platform's research unit: the research runner, which composes a search's
instructions, runs it on a seat and thrown-away sessions agency supplies, asks its rounds, opens its branches
and files its document (`docs/runner.md`); and the daemon that keeps a queue of things to research, finds new
ones on the sources the owner registers, has each one researched by that runner in its category's shape, and
tells the category's recipients, rooms and agents; it files no job and launches nothing. It holds the
`bmo-research` command, the tools the tool host places under the namespace `research`, and the documents that
state how all of it works.

## Principles

- **The unit runs on its own,** with no context from another repository.
- **It talks to other units only through their documented APIs** (`docs/api.md`, 「Calls this unit makes」):
  agency, the artifact service, the receiver, the voice unit and the arbiter's master, each at the address
  and with the token its own files under the data root's `env/` hold, or the settings name, and never copied.
- **No other unit's code is depended on.** Among other repositories nothing is a dependency of any crate
  here; `tests/dependencies.rs` fails on one.
- **The daemon decides nothing a person should and files nothing an agent should.** It notifies; the work
  a report calls for is the chat agent's to file on the owner's word. It never spends a paid road unless a
  setting or a category names one.

## Rules

- **`docs/` is canonical.** `docs/design.md` is the design, `docs/api.md` the boundary, `docs/settings.md`
  every setting, `docs/store.md` the tables and `docs/sources.md` the source kinds. Read the document
  covering a change first; a change that would make it wrong repairs the document first, stating what is
  true now, and the code is then written to match. A change to the boundary changes `docs/api.md` and
  `docs/openapi.json` in the same commit; `bmo-research openapi` writes the latter, and a test fails where
  the committed document differs from what the code generates.
- **Documents state what is true now:** no change history, no quoted source, no file-and-line pins, and
  no caller's name in a document of the boundary.
- **English only** in code, comments, strings, tests and documents, and in commit messages. Korean stands
  under `prompts/` alone: the section names a Korean question's document is written under, the words of a report line
  and the words a filed document is read by. `tests/literals.rs` fails on Hangul anywhere else.
- **No personal data:** no name of the owner or of a host, no mail address, home path, token or private
  network address. A value that differs by installation is a setting; a path is written from `$HOME` or
  from a variable. `tests/personal_data.rs` and `tests/literals.rs` fail on one.
- **No hand-written parser for a general format.** TOML, JSON, feeds and env files are read by a maintained
  library.
- **Tests live in `tests/`.** The root package `research-checks` holds them; no test module sits in a
  source file. `tests/layout.rs` fails on a directory named after its parent and on another build tool's
  manifest at the root.
- **Versions** are the workspace's, `0.1.0-rev.<n>` before the first release, and move only at a release
  the owner declares (bmo-chat's `docs/conventions.md`, 「Versions」).

## Layout

- `crates/client`: the types of the boundary and the HTTP client, `bmo-research-client`.
- `crates/server`: the daemon and the `bmo-research` binary (`serve`, `health`, `openapi`, `migrate` and the
  `research` nouns).
- `tools/research`: the tool the tool host places, one executable answering every entry of the namespace.
- `prompts/`: the search and browse instructions, the rulebooks, the question templates and the words.
- `docs/`, `deploy/`, `tests/`, and `testbed.toml` at the root.

## Checks

- Static: `cargo check --workspace --all-targets`.
- Review: `cargo fmt --all --check` (108 columns, two-space indentation, as `rustfmt.toml` sets) and
  `cargo clippy --workspace --all-targets -- -D warnings`.
- Tests: `cargo test --workspace`.
