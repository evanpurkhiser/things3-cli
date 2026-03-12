# things3

A small Things 3 CLI client backed by Things Cloud.

## Features

- Show `today`, `inbox`, `projects`, `areas`, `tags`, and `upcoming`
- Mark task status with `mark --done|--incomplete|--canceled`
- Uses cloud history replay (`t=0/1/2`) for current state
- Maintains a local append-only history cache in XDG state (`$XDG_STATE_HOME/things3/append-log`)

## Quick Start

Set auth interactively:

```bash
things3 set-auth
```

Run commands:

```bash
things3 today
things3 inbox
things3 mark <task-id> --done
```

Install as a `uv` tool:

```bash
uv tool install .
```

## Dev

```bash
uv run python -m py_compile cli.py things_cloud/client.py things_cloud/store.py
```
