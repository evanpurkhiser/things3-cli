# things3

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
- [x] Show lists: `today`, `anytime`, `inbox`, `upcoming`
- [x] List projects with `projects`, view project detail with `project <id>`
- [x] List areas with `areas`, view area detail with `area <id>`
- [x] List tags with `tags`
- [x] Update task status with `mark --done|--incomplete|--canceled`
- [x] Replay cloud history (`t=0/1/2`) with append-only local cache in XDG state
- [x] Cache folded state and history key for fast startup (~450ms vs ~1500ms)
- [ ] Mark multiple tasks at once (`mark --done <id1> <id2> ...`)
- [ ] Show task details (notes, checklist items)
- [ ] `add` — create tasks/projects with title, notes, tags, checklist items, project, area, dates
- [ ] `edit` — modify existing tasks/projects
  - [ ] Rename (title)
  - [ ] Set/remove tags
  - [ ] Set/remove notes
  - [ ] Add/remove/toggle checklist items
  - [ ] Delete (trash)
  - [ ] Set/remove recurrence
- [ ] `schedule` — set when/start date, deadline, today/evening/someday
- [ ] `move` — move tasks/projects between projects, areas, headings, inbox
- [ ] `reorder` — reorder tasks, projects, and headings within their lists
- [ ] Show `someday` list and `logbook` (completed tasks with date filtering)
- [ ] Heading management — create/rename/delete/reorder headings within projects
- [ ] `find` / filters (title, tag, area, project, status, date range)
- [ ] Machine-readable output (`--json`, `--toon`) for scripting and LLM/tool use
- [ ] Testing
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
