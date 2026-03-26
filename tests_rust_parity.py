from __future__ import annotations

import subprocess
from pathlib import Path


def test_python_integration_fixtures_still_pass() -> None:
    root = Path(__file__).resolve().parent
    cmd = [
        "uv",
        "run",
        "pytest",
        "tests/test_integration_cmd_today.py",
        "tests/test_integration_cmd_inbox.py",
        "tests/test_integration_cmd_anytime.py",
        "tests/test_integration_cmd_someday.py",
        "tests/test_integration_cmd_upcoming.py",
        "tests/test_integration_cmd_logbook.py",
    ]
    proc = subprocess.run(cmd, cwd=root, capture_output=True, text=True)
    assert proc.returncode == 0, proc.stdout + "\n" + proc.stderr
