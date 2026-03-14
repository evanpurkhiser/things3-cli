"""ID utilities for Things-style task identifiers."""

from __future__ import annotations

import hashlib
import re
from uuid import UUID, uuid4

BASE58_ALPHABET = "123456789ABCDEFGHJKLMNPQRSTUVWXYZabcdefghijkmnopqrstuvwxyz"
UUID_RE = re.compile(
    r"^[0-9a-fA-F]{8}-"
    r"[0-9a-fA-F]{4}-"
    r"[0-9a-fA-F]{4}-"
    r"[0-9a-fA-F]{4}-"
    r"[0-9a-fA-F]{12}$"
)


def base58_encode(raw: bytes) -> str:
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
        encoded.append(BASE58_ALPHABET[rem])

    if not encoded:
        encoded.append(BASE58_ALPHABET[0])

    return (BASE58_ALPHABET[0] * zeros) + "".join(reversed(encoded))


def canonical_uuid_to_task_id(value: str) -> str:
    """Convert canonical UUID string to Things-style base58 id."""
    canonical = str(UUID(value)).upper()
    digest = hashlib.sha1(canonical.encode("utf-8")).digest()[:16]
    return base58_encode(digest)


def legacy_uuid_to_task_id(value: str) -> str | None:
    """Convert UUID-looking IDs to Things-style ids; return None for others."""
    if not isinstance(value, str) or not UUID_RE.match(value):
        return None
    return canonical_uuid_to_task_id(value)


def random_task_id() -> str:
    """Generate a new random Things-style task id."""
    return canonical_uuid_to_task_id(str(uuid4()))
