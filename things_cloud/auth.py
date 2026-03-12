"""Auth configuration storage for things-cli."""

from __future__ import annotations

import json
import os
from pathlib import Path

from things_cloud.dirs import auth_file_path


class AuthConfigError(RuntimeError):
    """Raised when auth configuration is missing or invalid."""


def _validate_auth(email: str, password: str) -> tuple[str, str]:
    email = (email or "").strip()
    password = password or ""
    if not email:
        raise AuthConfigError("Missing auth email.")
    if not password:
        raise AuthConfigError("Missing auth password.")
    return email, password


def load_auth() -> tuple[str, str]:
    path = auth_file_path()
    if not path.exists():
        raise AuthConfigError(
            f"Auth not configured. Run `things3 set-auth` to create {path}."
        )

    try:
        raw = json.loads(path.read_text(encoding="utf-8"))
    except Exception as exc:
        raise AuthConfigError(f"Failed reading auth config at {path}: {exc}") from exc

    if not isinstance(raw, dict):
        raise AuthConfigError(f"Auth config at {path} must be a JSON object.")

    return _validate_auth(str(raw.get("email", "")), str(raw.get("password", "")))


def write_auth(email: str, password: str) -> Path:
    email, password = _validate_auth(email, password)
    path = auth_file_path()
    path.parent.mkdir(parents=True, exist_ok=True)

    payload = {"email": email, "password": password}
    serialized = json.dumps(payload, separators=(",", ":"))

    tmp_path = path.with_suffix(".tmp")
    tmp_path.write_text(serialized, encoding="utf-8")
    with tmp_path.open("r", encoding="utf-8") as fp:
        fp.flush()
        os.fsync(fp.fileno())
    os.replace(tmp_path, path)
    os.chmod(path, 0o600)
    dir_fd = os.open(str(path.parent), os.O_DIRECTORY)
    try:
        os.fsync(dir_fd)
    finally:
        os.close(dir_fd)
    return path
