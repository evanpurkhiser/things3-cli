# things3

> [!NOTE]
> This project is written completely using Claude. No review of the code has been done.

A command-line interface for [Things 3](https://culturedcode.com/things/) that communicates
directly with the Things Cloud API — keeping all your tasks in sync across your Apple devices.

## Quick Start

Authenticate with your Things Cloud credentials:

```
$ things3 set-auth
Things Cloud email: you@example.com
Things Cloud password: ••••••••••••
```

Then run commands:

```
$ things3 today
⭑ Today  (5 tasks)

  ○ Morning routine
  ○ Complete taxes  ⚑ due by 2026-04-10
  ○ Update software packages

  ☽ This Evening
  ○ Write down favorite thing about today
```

## Supported features

- [x] Configure auth with `set-auth` (stored in XDG state)
- [x] Incremental sync with local cache for fast startup

**Views**
- [x] `inbox`, `today`, `upcoming`, `anytime`, `someday`, `logbook`
- [x] `projects` / `project <id>`, `areas` / `area <id>`, `tags`
- [x] Show notes via `--detailed` flag
- [x] Show checklist items via `--detailed`
- [ ] `find` / filters (title, tag, area, project, status, date range)
- [ ] Machine-readable output (`--json`, `--toon`) for scripting and LLM/tool use

**Tasks**
- [x] `new` — create tasks with title, notes, tags, when, position
- [x] `edit` — rename, set/remove notes, move between containers
- [x] `mark` — set status to done/incomplete/canceled (supports multiple IDs)
- [x] `schedule` — set when/start date, deadline, today/evening/someday
- [x] `reorder` — reorder within lists
- [x] `delete` — trash tasks
- [ ] Set/remove tags via `edit`
- [ ] Add/remove/toggle checklist items via `edit`
- [ ] Set/remove recurrence via `edit`

**Projects**
- [x] `projects new` — create projects with title, tags, when, area
- [ ] `edit` projects (title, notes, move to area)
- [ ] Heading management — create/rename/delete/reorder headings within projects

**Areas**
- [x] `areas new` — create areas with title
- [ ] `edit` areas (title, tags)

**Testing**
- [ ] Sync engine tests (append log replay, incremental fold, state caching)
- [ ] Things store tests (task queries, project progress, prefix resolution)
- [ ] Command output tests (formatting, grouping, filtering)
- [ ] E2E snapshot tests (full command output against fixture data)

## Install

Install as a `uv` tool:

```bash
uv tool install .
```

## Dev

```bash
uv run python -m py_compile cli.py things_cloud/client.py things_cloud/store.py
```
