# things3

[![Tests](https://github.com/evanpurkhiser/things3-cloud/actions/workflows/tests.yml/badge.svg)](https://github.com/evanpurkhiser/things3-cloud/actions/workflows/tests.yml)

A Rust command-line client for [Things 3](https://culturedcode.com/things/) that talks
directly to the Things Cloud API.

## Install

From source:

```bash
cargo install --path .
```

From crates.io:

```bash
cargo install things3-cloud
```

## Build

```bash
cargo build
```

## Configure auth

```bash
things3 set-auth
```

## Usage

```bash
$ things3 today
⭑ Today  (9 tasks)

  LZ4 ▢ Follow up with team
      │ Shared notes for context
      │
    D ├╴○ draft update
    J └╴○ review checklist
  4HY ▢ Task with notes
      │ Multi-line note example
      └ with a second line
  Uuq ▢ Review inbox and prioritize
  699 ▢ Prepare weekly summary  [Planning]
  H47 ▢ Submit reimbursement  ⚑ due by 2026-04-10

  ☽ This Evening
  8KU ▢ Reflect on highlights
  63E ▢ Reset workspace for tomorrow
```

```bash
things3 find --query "rent" --deadline "<=2026-03-31"
things3 new "Follow up with team" --when today
things3 schedule <task-id> --deadline 2026-04-10
things3 mark <task-id> --done
```

## Supported features

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
