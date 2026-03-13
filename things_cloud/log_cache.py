"""Append-only local log cache for Things Cloud history items."""

from __future__ import annotations

import json
import os
import re
import hashlib
import time
from contextlib import contextmanager
import fcntl
from pathlib import Path
from uuid import UUID

from things_cloud.client import ThingsCloudClient
from things_cloud.dirs import append_log_dir


_BASE58_ALPHABET = "123456789ABCDEFGHJKLMNPQRSTUVWXYZabcdefghijkmnopqrstuvwxyz"
_UUID_RE = re.compile(
    r"^[0-9a-fA-F]{8}-"
    r"[0-9a-fA-F]{4}-"
    r"[0-9a-fA-F]{4}-"
    r"[0-9a-fA-F]{4}-"
    r"[0-9a-fA-F]{12}$"
)


def _base58_encode(raw: bytes) -> str:
    if not raw:
        return ""

    zeros = 0
    for b in raw:
        if b == 0:
            zeros += 1
        else:
            break

    num = int.from_bytes(raw, "big")
    encoded: list[str] = []
    while num > 0:
        num, rem = divmod(num, 58)
        encoded.append(_BASE58_ALPHABET[rem])

    if not encoded:
        encoded.append(_BASE58_ALPHABET[0])

    return (_BASE58_ALPHABET[0] * zeros) + "".join(reversed(encoded))


def _legacy_uuid_to_task_id(value: str) -> str | None:
    if not isinstance(value, str) or not _UUID_RE.match(value):
        return None

    canonical = str(UUID(value)).upper()
    digest = hashlib.sha1(canonical.encode("utf-8")).digest()[:16]
    return _base58_encode(digest)


def _normalize_ids(value):
    if isinstance(value, str):
        return _legacy_uuid_to_task_id(value) or value
    if isinstance(value, list):
        return [_normalize_ids(v) for v in value]
    if isinstance(value, dict):
        out = {}
        for k, v in value.items():
            new_k = _legacy_uuid_to_task_id(k) if isinstance(k, str) else None
            out[new_k or k] = _normalize_ids(v)
        return out
    return value


def _normalize_item_ids(item: dict) -> dict:
    normalized: dict = {}
    for uuid, obj in item.items():
        new_uuid = _legacy_uuid_to_task_id(uuid) or uuid
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


def _read_cursor(path: Path) -> int:
    if not path.exists():
        return 0
    try:
        data = json.loads(path.read_text(encoding="utf-8"))
        return int(data.get("next_start_index", 0))
    except Exception:
        return 0


def _write_cursor(path: Path, next_start_index: int) -> None:
    payload = json.dumps(
        {
            "next_start_index": next_start_index,
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
        start_index = _read_cursor(cursor_path)

        if not client.history_key:
            client.authenticate()

        with log_path.open("a", encoding="utf-8") as fp:
            while True:
                page = client.get_items_page(start_index)
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
                    _write_cursor(cursor_path, start_index)

                if not items:
                    break
                if end >= latest:
                    break


def fold_state_from_append_log(cache_dir: Path) -> dict[str, dict]:
    state: dict[str, dict] = {}
    log_path = cache_dir / "things.log"

    if not log_path.exists():
        return state

    with log_path.open("r", encoding="utf-8") as fp:
        line_no = 0
        while True:
            line = fp.readline()
            if not line:
                break
            line_no += 1
            line = line.strip()
            if not line:
                continue
            try:
                item = json.loads(line)
            except json.JSONDecodeError as exc:
                probe = fp.read(1)
                if probe == "":
                    break
                fp.seek(fp.tell() - 1)
                raise RuntimeError(
                    f"Corrupt log entry at {log_path}:{line_no}"
                ) from exc
            _fold_item(item, state)

    return state


def get_state_with_append_log(client: ThingsCloudClient) -> dict[str, dict]:
    cache_path = append_log_dir()
    sync_append_log(client, cache_path)
    return fold_state_from_append_log(cache_path)
