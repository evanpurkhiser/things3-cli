"""XDG directory helpers for things-cli stateful storage."""

from __future__ import annotations

import os
from pathlib import Path

APP_NAME = "things3"
LEGACY_APP_NAME = "things-cli"


def _xdg_state_home() -> Path:
    state_home = os.environ.get("XDG_STATE_HOME")
    if state_home:
        return Path(state_home).expanduser()
    return Path.home() / ".local" / "state"


def app_state_dir() -> Path:
    state_home = _xdg_state_home()
    target = state_home / APP_NAME
    legacy = state_home / LEGACY_APP_NAME

    if target.exists() or not legacy.exists():
        return target

    try:
        os.replace(legacy, target)
        return target
    except OSError:
        return target


def append_log_dir() -> Path:
    return app_state_dir() / "append-log"


def auth_file_path() -> Path:
    return app_state_dir() / "auth.json"
