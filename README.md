**English** | [한국어](README.ko.md) | [日本語](README.ja.md) | [简体中文](README.zh-CN.md)

# bmo-research

The platform's research unit: a daemon that keeps a queue of things to research by category, finds new
ones on the sources the owner registers (the Hugging Face Hub, arXiv, GitHub releases, feeds), has each one
researched by its own research runner in its category's shape, and tells the category's recipients the
share link of the document the run filed. The runner (`docs/runner.md`) also answers every research the chat
agent asks with `research/run`, and the drawing service's `references` runs, on slots and role sessions agency
supplies. `it` asks for a manual or an investigation, and a manual saying the
thing fits one of the lab's cards asks the owner's word; `security` asks for an advisory judged against
the platform's stack, and one that touches it asks whether to patch; `economy` gathers a day's items into
one brief and keeps its figures. The daemon researches and notifies: the work a report calls for is the
chat agent's to file on the owner's word.

`docs/design.md` is the design; `docs/api.md` the routes, the tools and the calls the unit makes;
`docs/settings.md` the settings; `docs/store.md` the tables; `docs/sources.md` the source kinds.

## Layout

| Folder | Holds |
|---|---|
| `crates/` | The Rust workspace: `client` (the boundary's types and the HTTP client) and `server` (the daemon, its routes, its store, the scan and the deep dive, and the `bmo-research` binary) |
| `tools/` | `research`, the tool the tool host places under the namespace `research`: its manifest, its schemas and its executable's crate |
| `prompts/` | The Korean question templates, one per shape, the notices' words and the words a filed document is read by |
| `docs/` | The design, the API and its OpenAPI document, the settings, the store and the sources |
| `src/` | The repository's own package, which holds no code |
| `tests/` | Every test: the store and queue rules, the source readers, the scan and the deep dive against stand-in servers, the routes, and the repository's checks |
| `deploy/` | The user unit the deployment restarts and watches |

At the root stand the Cargo workspace, `rustfmt.toml`, the language ledger `korean-ledger.json`, the
testbed's `testbed.toml`, `install.toml`, `CLAUDE.md` and the MIT `LICENSE`.
