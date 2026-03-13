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
- [x] List projects with `projects`
- [x] List areas with `areas`
- [x] List tags with `tags`
- [x] Update task status with `mark --done|--incomplete|--canceled`
- [x] Replay cloud history (`t=0/1/2`) with append-only local cache in XDG state
- [ ] Show `someday` list and special lists (`tomorrow`, `deadlines`, `repeating`)
- [ ] Create tasks from CLI (`add`) with list/date/project/tag targeting
- [ ] Edit/move tasks (`schedule`, `tag`, `move`) for inbox processing workflows
- [ ] Add `find` / filters (tag, area, project, status) for fast retrieval
- [ ] Add machine-readable output (`--json`) optimized for LLM/tool use

## Install

Install as a `uv` tool:

```bash
uv tool install .
```

## Dev

```bash
uv run python -m py_compile cli.py things_cloud/client.py things_cloud/store.py
```
