"""
Fixture generator for Rust parity tests.

Runs the Python integration test suite with instrumented helpers,
capturing (journal, cli_args, today_ts, expected_output) tuples for
each test, and writes them as JSON files to tests/fixtures/rust/.

Usage:
    uv run python generate_rust_fixtures.py

Each fixture file is named after the test function, e.g.:
    tests/fixtures/rust/test_inbox_basic.json

Schema of each fixture file:
{
  "test_name": "test_inbox_basic",
  "cli_args": "inbox",
  "today_ts": null,           # or int (UTC midnight) for time-dependent tests
  "journal": [                # list of WireItem dicts: [{uuid: {t, e, p}}, ...]
    {"AAA...": {"t": 0, "e": "Task6", "p": {...}}},
    ...
  ],
  "expected_output": "...\n"  # exact stdout after ANSI stripping
}
"""

from __future__ import annotations

import importlib
import inspect
import json
import os
import sys
import time
from contextlib import ExitStack
from datetime import datetime, timezone
from pathlib import Path
from typing import Any
from unittest.mock import patch

# Ensure repo root is on the path
ROOT = Path(__file__).resolve().parent
sys.path.insert(0, str(ROOT))

os.environ["TZ"] = "UTC"
time.tzset()

# ---------------------------------------------------------------------------
# Fixed "today" for reproducible fixtures: 2026-03-25 UTC midnight
# ---------------------------------------------------------------------------
FIXED_TODAY_TS = int(datetime(2026, 3, 25, tzinfo=timezone.utc).timestamp())

# ---------------------------------------------------------------------------
# Instrumentation state
# ---------------------------------------------------------------------------

_current_journal: list[dict] = []


# ---------------------------------------------------------------------------
# Import real helpers/store machinery BEFORE patching anything
# ---------------------------------------------------------------------------

from things_cloud.log_cache import _fold_item  # noqa: E402
from things_cloud.store import ThingsStore  # noqa: E402
import tests.helpers as _helpers_mod  # noqa: E402
import tests.mutating_fixtures as _mf_mod  # noqa: E402
import tests.conftest  # noqa: F401, E402  (sets TZ side-effect)
import things_cloud.store as _store_mod  # noqa: E402


# ---------------------------------------------------------------------------
# Instrumented versions
# ---------------------------------------------------------------------------


def _instrumented_build_store(journal: list[dict]) -> ThingsStore:
    global _current_journal
    _current_journal = list(journal)
    state: dict = {}
    for entry in journal:
        _fold_item(entry, state)
    return ThingsStore(state)


# ---------------------------------------------------------------------------
# Frozen datetime shim
# ---------------------------------------------------------------------------


class _FrozenDatetime:
    """Replaces a module's `datetime` binding so .now() returns a fixed date."""

    def __init__(self, fixed_ts: int, real_datetime):
        self._fixed_ts = fixed_ts
        self._real = real_datetime

    def now(self, tz=None):
        if tz is None:
            tz = timezone.utc
        return datetime.fromtimestamp(self._fixed_ts, tz=tz)

    def __call__(self, *args, **kwargs):
        return self._real(*args, **kwargs)

    def __getattr__(self, name):
        return getattr(self._real, name)


# ---------------------------------------------------------------------------
# Modules that call _today_ts() / _day_ts() — need frozen datetime
# ---------------------------------------------------------------------------

_TIME_DEPENDENT_MODULES = {
    "tests.test_integration_cmd_today",
    "tests.test_integration_cmd_anytime",
    "tests.test_integration_cmd_upcoming",
    "tests.test_integration_cmd_schedule",
    "tests.test_integration_cmd_reorder",
}

_TEST_MODULES = [
    "tests.test_integration_cmd_today",
    "tests.test_integration_cmd_inbox",
    "tests.test_integration_cmd_anytime",
    "tests.test_integration_cmd_someday",
    "tests.test_integration_cmd_upcoming",
    "tests.test_integration_cmd_logbook",
    "tests.test_integration_cmd_find",
    "tests.test_integration_cmd_projects",
    "tests.test_integration_cmd_project",
    "tests.test_integration_cmd_areas",
    "tests.test_integration_cmd_area",
    "tests.test_integration_cmd_tags",
]


def _run_test_fn(
    mod,
    module_name: str,
    fn_name: str,
    fn,
    is_time_dependent: bool,
) -> list[dict] | None:
    """
    Call a single test function with patched helpers, capture fixtures.
    Returns a list of fixture dicts (one per run_cli call), or None on failure.
    """
    global _current_journal
    _current_journal = []

    captured_outputs: list[tuple[str, str]] = []

    # Wrap run_cli to intercept every call
    def _capturing_run_cli(args: str, store: ThingsStore) -> str:
        out = _helpers_mod._orig_run_cli(args, store)
        captured_outputs.append((args, out))
        return out

    # Build patch list
    patch_targets: list = [
        # Patch run_cli in helpers module (affects any test that calls it via alias)
        patch.object(_helpers_mod, "run_cli", _capturing_run_cli),
        # Patch run_cli in the test module itself (from tests.helpers import run_cli)
        patch.object(mod, "run_cli", _capturing_run_cli),
        # Patch build_store_from_journal everywhere
        patch.object(
            _helpers_mod, "build_store_from_journal", _instrumented_build_store
        ),
        patch.object(_mf_mod, "build_store_from_journal", _instrumented_build_store),
    ]

    if hasattr(mod, "build_store_from_journal"):
        patch_targets.append(
            patch.object(mod, "build_store_from_journal", _instrumented_build_store)
        )
    if hasattr(mod, "store") and mod.store is _mf_mod.store:
        # test_find defines its own store() wrapper — patch build_store in that scope
        pass  # already covered by patching mod.build_store_from_journal above

    if is_time_dependent and hasattr(mod, "datetime"):
        frozen = _FrozenDatetime(FIXED_TODAY_TS, mod.datetime)
        patch_targets.append(patch.object(mod, "datetime", frozen))
        # Also freeze datetime in things_cloud.store so is_today/anytime use the same clock
        frozen_store = _FrozenDatetime(FIXED_TODAY_TS, _store_mod.datetime)
        patch_targets.append(patch.object(_store_mod, "datetime", frozen_store))

    sig = inspect.signature(fn)
    params = list(sig.parameters.keys())
    kwargs: dict[str, Any] = {}
    if "store_from_journal" in params:
        kwargs["store_from_journal"] = _instrumented_build_store
    if "store" in params:
        kwargs["store"] = _mf_mod.store

    with ExitStack() as stack:
        for p in patch_targets:
            stack.enter_context(p)
        try:
            fn(**kwargs)
        except SystemExit:
            pass
        except Exception as exc:
            print(f"    SKIP {fn_name}: {exc}", file=sys.stderr)
            return None

    if not captured_outputs:
        return None

    results = []
    for i, (args, out) in enumerate(captured_outputs):
        suffix = f"_{i}" if len(captured_outputs) > 1 else ""
        results.append(
            {
                "test_name": fn_name + suffix,
                "cli_args": args,
                "today_ts": FIXED_TODAY_TS if is_time_dependent else None,
                "journal": list(_current_journal),
                "expected_output": out,
            }
        )
    return results


# ---------------------------------------------------------------------------
# Parametrize support
# ---------------------------------------------------------------------------


def _extract_parametrize(fn) -> list[tuple[dict, str]] | None:
    """
    If fn has @pytest.mark.parametrize, return a list of (kwargs, suffix) pairs.
    Returns None if fn is not parametrized.
    """
    marks = getattr(fn, "pytestmark", None)
    if not marks:
        return None
    import pytest

    for mark in marks:
        if mark.name == "parametrize":
            argnames_raw, argvalues = mark.args[0], mark.args[1]
            if isinstance(argnames_raw, str):
                argnames = [a.strip() for a in argnames_raw.split(",")]
            else:
                argnames = list(argnames_raw)
            result = []
            for i, values in enumerate(argvalues):
                if not isinstance(values, (list, tuple)):
                    values = (values,)
                kwargs = dict(zip(argnames, values))
                # Build a safe suffix from the first value
                first_val = str(list(kwargs.values())[0])
                safe = (
                    first_val.replace(" ", "_").replace("/", "_").replace("-", "_")[:40]
                )
                result.append((kwargs, f"__{safe}"))
            return result
    return None


def _run_test_fn_with_extra_kwargs(
    mod, module_name, fn_name, fn, is_time_dependent, extra_kwargs
):
    """Run a parametrized test variant with extra_kwargs injected."""
    # Merge extra_kwargs into the standard fixture kwargs
    global _current_journal
    _current_journal = []

    captured_outputs: list[tuple[str, str]] = []

    def _capturing_run_cli(args: str, store: ThingsStore) -> str:
        out = _helpers_mod._orig_run_cli(args, store)
        captured_outputs.append((args, out))
        return out

    patch_targets: list = [
        patch.object(_helpers_mod, "run_cli", _capturing_run_cli),
        patch.object(mod, "run_cli", _capturing_run_cli),
        patch.object(
            _helpers_mod, "build_store_from_journal", _instrumented_build_store
        ),
        patch.object(_mf_mod, "build_store_from_journal", _instrumented_build_store),
    ]
    if hasattr(mod, "build_store_from_journal"):
        patch_targets.append(
            patch.object(mod, "build_store_from_journal", _instrumented_build_store)
        )
    if is_time_dependent and hasattr(mod, "datetime"):
        frozen = _FrozenDatetime(FIXED_TODAY_TS, mod.datetime)
        patch_targets.append(patch.object(mod, "datetime", frozen))
        frozen_store = _FrozenDatetime(FIXED_TODAY_TS, _store_mod.datetime)
        patch_targets.append(patch.object(_store_mod, "datetime", frozen_store))

    sig = inspect.signature(fn)
    params = list(sig.parameters.keys())
    kwargs: dict[str, Any] = {}
    if "store_from_journal" in params:
        kwargs["store_from_journal"] = _instrumented_build_store
    if "store" in params:
        kwargs["store"] = _mf_mod.store
    kwargs.update(extra_kwargs)

    with ExitStack() as stack:
        for p in patch_targets:
            stack.enter_context(p)
        try:
            fn(**kwargs)
        except SystemExit:
            pass
        except Exception as exc:
            print(f"    SKIP {fn_name}: {exc}", file=sys.stderr)
            return None

    if not captured_outputs:
        return None

    results = []
    for i, (args, out) in enumerate(captured_outputs):
        suffix = f"_{i}" if len(captured_outputs) > 1 else ""
        results.append(
            {
                "test_name": fn_name + suffix,
                "cli_args": args,
                "today_ts": FIXED_TODAY_TS if is_time_dependent else None,
                "journal": list(_current_journal),
                "expected_output": out,
            }
        )
    return results


# ---------------------------------------------------------------------------
# Main
# ---------------------------------------------------------------------------


def main() -> None:
    # Stash the real run_cli before any patching
    _helpers_mod._orig_run_cli = _helpers_mod.run_cli

    out_dir = ROOT / "tests" / "fixtures" / "rust"
    out_dir.mkdir(parents=True, exist_ok=True)

    total = 0
    skipped = 0

    for module_name in _TEST_MODULES:
        is_time_dependent = module_name in _TIME_DEPENDENT_MODULES
        print(f"Processing {module_name} ...")
        mod = importlib.import_module(module_name)

        for fn_name, fn in sorted(inspect.getmembers(mod, inspect.isfunction)):
            if not fn_name.startswith("test_"):
                continue

            # Handle @pytest.mark.parametrize by expanding parameter sets
            param_sets = _extract_parametrize(fn)
            if param_sets is not None:
                for param_kwargs, param_suffix in param_sets:
                    result = _run_test_fn_with_extra_kwargs(
                        mod,
                        module_name,
                        fn_name + param_suffix,
                        fn,
                        is_time_dependent,
                        param_kwargs,
                    )
                    if result is None:
                        skipped += 1
                        continue
                    for fixture in result:
                        name = fixture["test_name"]
                        path = out_dir / f"{name}.json"
                        path.write_text(
                            json.dumps(fixture, indent=2, ensure_ascii=False) + "\n"
                        )
                        print(f"  wrote {path.name}")
                        total += 1
                continue

            result = _run_test_fn(mod, module_name, fn_name, fn, is_time_dependent)
            if result is None:
                skipped += 1
                continue

            for fixture in result:
                name = fixture["test_name"]
                path = out_dir / f"{name}.json"
                path.write_text(
                    json.dumps(fixture, indent=2, ensure_ascii=False) + "\n"
                )
                print(f"  wrote {path.name}")
                total += 1

    print(f"\nDone: {total} fixtures written, {skipped} skipped.")


if __name__ == "__main__":
    main()
