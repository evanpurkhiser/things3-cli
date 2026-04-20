# things3-cloud

[![Main](https://github.com/evanpurkhiser/things3-cloud/actions/workflows/main.yml/badge.svg)](https://github.com/evanpurkhiser/things3-cloud/actions/workflows/main.yml)

A Rust command-line client for [Things 3](https://culturedcode.com/things/) that talks
directly to the Things Cloud API.

```bash
$ things3 today
⭑ Today  (2 tasks)

  A ▢ Plan day

☽ This Evening

  K ▢ Review finances
```

```bash
things3 find --query "rent" --deadline "<=2026-03-31"
things3 new "Follow up with team" --when today
things3 schedule <task-id> --deadline 2026-04-10
things3 mark <task-id> --done
```

## Install

Via Homebrew:

```bash
brew install evanpurkhiser/personal/things3-cloud
```

Via Arch Linux AUR:

```bash
yay -S things3-cloud
```

From crates.io:

```bash
cargo install things3-cloud
```

From source:

```bash
cargo install --path .
```

## Configure auth

```bash
things3 set-auth
```

You can also set credentials via environment variables:

```bash
export THINGS3_EMAIL="you@example.com"
export THINGS3_PASSWORD="your-password"
```

Environment variables override values in the auth file.

## Supported features

Roadmap items are tracked in [`ROADMAP.md`](ROADMAP.md).

- [x] Configure auth with `set-auth` (stored in XDG state)
- [x] Incremental sync with local cache for fast startup
- [x] `--no-sync` flag to skip cloud sync and use local cache only

**Views**
- [x] `inbox`, `today`, `upcoming`, `anytime`, `someday`, `logbook`
- [x] `projects` / `project <id>`, `areas` / `area <id>`, `tags`
- [x] Show notes via `--detailed`
- [x] Show checklist items via `--detailed`
- [x] `find` with title/notes/checklists and tag/area/project/status/date filters

**Tasks**
- [x] `new` — create tasks with title, notes, tags, when, deadline, and position
- [x] `edit` — rename, set/remove notes, move, add/remove tags (multi-ID supported)
- [x] `edit --add-checklist/--remove-checklist/--rename-checklist`
- [x] `mark` — done/incomplete/canceled (multi-ID supported)
- [x] `mark --check/--uncheck/--check-cancel` for checklist toggles
- [x] `schedule` — when/start date, deadline, today/evening/someday
- [x] `reorder` — reorder tasks within lists
- [x] `delete` — trash tasks

**Projects / Areas / Tags**
- [x] `projects new` / `projects edit`
- [x] `areas new` / `areas edit`
- [x] `tags new` / `tags edit` / `tags delete`

## Testing

Run all Rust tests:

```bash
cargo test --all-targets
```

The CLI snapshot suite uses `trycmd` test cases in `trycmd/`.

## Related projects

**Things Cloud API clients** (same approach as this project)

- [wbopan/things-cloud-mcp](https://github.com/wbopan/things-cloud-mcp) (Go MCP server via Cloud API)
- [arthursoares/things-cloud-sdk](https://github.com/arthursoares/things-cloud-sdk) (Go SDK for Things Cloud)

**Local database / URL scheme clients** (read from local SQLite, write via URL scheme or AppleScript)

- [ossianhempel/things3-cli](https://github.com/ossianhempel/things3-cli) (Go CLI, local database)
- [thingsapi/things.py](https://github.com/thingsapi/things.py) (Python library, reads database)
- [thingsapi/things-cli](https://github.com/thingsapi/things-cli) (Python CLI built on things.py)
- [GarthDB/rust-things3](https://github.com/GarthDB/rust-things3) (Rust library and CLI with MCP)

**MCP servers** (local, via AppleScript or URL scheme)

- [drjforrest/mcp-things3](https://github.com/drjforrest/mcp-things3) (Python MCP, AppleScript + x-call)
- [rossshannon/Things3-MCP](https://github.com/rossshannon/Things3-MCP) (Python MCP with read/write)
