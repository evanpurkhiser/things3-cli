# things-cli

A small Things CLI client backed by Things Cloud.

## Features

- Show `today`, `inbox`, `projects`, `areas`, `tags`, and `upcoming`
- Mark task status with `mark --done|--incomplete|--canceled`
- Uses cloud history replay (`t=0/1/2`) for current state

## Quick Start

Create a `config.py` file:

```python
EMAIL = "you@example.com"
PASSWORD = "app-password-or-things-password"
```

Run commands with `uv`:

```bash
uv run cli.py today
uv run cli.py inbox
uv run cli.py mark <task-id> --done
```

## Dev

```bash
uv run python -m py_compile cli.py things_cloud/client.py things_cloud/store.py
```
