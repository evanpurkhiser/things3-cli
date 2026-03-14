"""Append-only local log cache for Things Cloud history items."""

from __future__ import annotations

import json
import os
import time
from contextlib import contextmanager
from dataclasses import dataclass
import fcntl
from pathlib import Path
from urllib.error import HTTPError

from things_cloud.client import ThingsCloudClient
from things_cloud.dirs import append_log_dir
from things_cloud.ids import legacy_uuid_to_task_id


def _normalize_ids(value):
    if isinstance(value, str):
        return legacy_uuid_to_task_id(value) or value
    if isinstance(value, list):
        return [_normalize_ids(v) for v in value]
    if isinstance(value, dict):
        out = {}
        for k, v in value.items():
            new_k = legacy_uuid_to_task_id(k) if isinstance(k, str) else None
            out[new_k or k] = _normalize_ids(v)
        return out
    return value


def _normalize_item_ids(item: dict) -> dict:
    normalized: dict = {}
    for uuid, obj in item.items():
        new_uuid = legacy_uuid_to_task_id(uuid) or uuid
        normalized[new_uuid] = _normalize_ids(obj)
    return normalized


def _fold_item(item: dict, state: dict[str, dict]) -> None:
    item = _normalize_item_ids(item)
    for uuid, obj in item.items():
        t = obj.get("t", 0)
        entity = obj.get("e")
        props = obj.get("p", {})

        if t == 0:
            state[uuid] = {"e": entity, "p": dict(props)}
        elif t == 1:
            if uuid in state:
                state[uuid]["p"].update(props)
                if entity:
                    state[uuid]["e"] = entity
            else:
                state[uuid] = {"e": entity, "p": dict(props)}
        elif t == 2:
            state.pop(uuid, None)


@dataclass
class _CursorData:
    next_start_index: int = 0
    history_key: str = ""


def _read_cursor(path: Path) -> _CursorData:
    if not path.exists():
        return _CursorData()
    try:
        data = json.loads(path.read_text(encoding="utf-8"))
        return _CursorData(
            next_start_index=int(data.get("next_start_index", 0)),
            history_key=str(data.get("history_key", "")),
        )
    except Exception:
        return _CursorData()


def _write_cursor(path: Path, next_start_index: int, history_key: str = "") -> None:
    payload = json.dumps(
        {
            "next_start_index": next_start_index,
            "history_key": history_key,
            "updated_at": time.time(),
        },
        separators=(",", ":"),
    )
    tmp = path.with_suffix(".tmp")
    tmp.write_text(payload, encoding="utf-8")
    with tmp.open("r", encoding="utf-8") as fp:
        fp.flush()
        os.fsync(fp.fileno())
    os.replace(tmp, path)
    dir_fd = os.open(str(path.parent), os.O_DIRECTORY)
    try:
        os.fsync(dir_fd)
    finally:
        os.close(dir_fd)


@contextmanager
def _sync_lock(lock_path: Path):
    lock_path.parent.mkdir(parents=True, exist_ok=True)
    with lock_path.open("w", encoding="utf-8") as lock_fp:
        fcntl.flock(lock_fp.fileno(), fcntl.LOCK_EX)
        try:
            yield
        finally:
            fcntl.flock(lock_fp.fileno(), fcntl.LOCK_UN)


def sync_append_log(client: ThingsCloudClient, cache_dir: Path) -> None:
    cache_dir.mkdir(parents=True, exist_ok=True)
    log_path = cache_dir / "things.log"
    cursor_path = cache_dir / "cursor.json"
    lock_path = cache_dir / "sync.lock"

    with _sync_lock(lock_path):
        cursor = _read_cursor(cursor_path)
        start_index = cursor.next_start_index

        if not client.history_key:
            if cursor.history_key:
                client.history_key = cursor.history_key
            else:
                client.authenticate()

        def _fetch_page(idx: int) -> dict:
            """Fetch a page, re-authenticating once if the history key is stale."""
            try:
                return client.get_items_page(idx)
            except HTTPError as e:
                if e.code in (401, 403, 404):
                    client.authenticate()
                    return client.get_items_page(idx)
                raise

        with log_path.open("a", encoding="utf-8") as fp:
            while True:
                page = _fetch_page(start_index)
                items = page.get("items", [])
                end = page.get("end-total-content-size", 0)
                latest = page.get("latest-total-content-size", 0)
                client.head_index = page.get("current-item-index", client.head_index)

                for item in items:
                    fp.write(json.dumps(item, separators=(",", ":")) + "\n")

                if items:
                    fp.flush()
                    os.fsync(fp.fileno())
                    start_index += len(items)
                    _write_cursor(cursor_path, start_index, client.history_key or "")

                if not items:
                    break
                if end >= latest:
                    break

        # Persist history_key even when no new items were fetched
        if client.history_key and client.history_key != cursor.history_key:
            _write_cursor(cursor_path, start_index, client.history_key)


def _read_state_cache(cache_dir: Path) -> tuple[dict[str, dict], int]:
    """Load cached folded state. Returns (state, byte_offset) or ({}, 0)."""
    path = cache_dir / "state_cache.json"
    if not path.exists():
        return {}, 0
    try:
        data = json.loads(path.read_text(encoding="utf-8"))
        return data["state"], data["log_offset"]
    except Exception:
        return {}, 0


def _write_state_cache(
    cache_dir: Path, state: dict[str, dict], log_offset: int
) -> None:
    """Atomically write the folded state cache."""
    path = cache_dir / "state_cache.json"
    payload = json.dumps(
        {"log_offset": log_offset, "state": state},
        separators=(",", ":"),
    )
    tmp = path.with_suffix(".tmp")
    tmp.write_text(payload, encoding="utf-8")
    os.replace(tmp, path)


def fold_state_from_append_log(cache_dir: Path) -> dict[str, dict]:
    log_path = cache_dir / "things.log"

    if not log_path.exists():
        return {}

    # Try to resume from cached state
    state, byte_offset = _read_state_cache(cache_dir)

    new_lines = 0
    with log_path.open("r", encoding="utf-8") as fp:
        fp.seek(byte_offset)
        line_no = 0
        while True:
            line = fp.readline()
            if not line:
                break
            line_no += 1
            stripped = line.strip()
            if not stripped:
                continue
            try:
                item = json.loads(stripped)
            except json.JSONDecodeError as exc:
                probe = fp.read(1)
                if probe == "":
                    break
                fp.seek(fp.tell() - 1)
                raise RuntimeError(
                    f"Corrupt log entry at {log_path} (offset {byte_offset}, line {line_no})"
                ) from exc
            _fold_item(item, state)
            new_lines += 1

        end_offset = fp.tell()

    # Persist cache if we folded new entries
    if new_lines > 0:
        _write_state_cache(cache_dir, state, end_offset)

    return state


def get_state_with_append_log(client: ThingsCloudClient) -> dict[str, dict]:
    cache_path = append_log_dir()
    sync_append_log(client, cache_path)
    return fold_state_from_append_log(cache_path)
