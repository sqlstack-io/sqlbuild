"""Native stub check command entrypoint."""

from __future__ import annotations

import importlib
from pathlib import Path

from scripts.native_stub._helpers.declarations import stub_drift
from scripts.native_stub.constants import NATIVE_MODULE, STUB_PATH
from scripts.root_check.main.run_root_check import run_root_check


def run_native_stub_check(argv: list[str] | None = None) -> int:
    """Fail when the stub declares a runtime name the built native extension does not register."""
    return run_root_check(
        argv=argv,
        description=__doc__,
        errors_under=_unregistered_names,
        messages=(
            f"{STUB_PATH} declares names {NATIVE_MODULE} does not register:",
            f"{STUB_PATH} declares only registered names.",
        ),
    )


def _unregistered_names(root: Path) -> tuple[str, ...]:
    registered: frozenset[str] = frozenset(dir(importlib.import_module(NATIVE_MODULE)))
    stub_text: str = (root / STUB_PATH).read_text(encoding="utf-8")
    return stub_drift(stub_text=stub_text, registered=registered)
