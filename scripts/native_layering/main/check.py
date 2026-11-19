"""Native crate layering command entrypoint."""

from __future__ import annotations

from pathlib import Path

from scripts.native_layering._helpers.layering import get_native_layering_errors
from scripts.native_layering._helpers.versions import get_native_version_errors
from scripts.root_check.main.run_root_check import run_root_check


def run_native_layering_check(argv: list[str] | None = None) -> int:
    """Fail when a native crate breaks the declared layering or the release versions differ."""
    return run_root_check(
        argv=argv,
        description=__doc__,
        errors_under=_layering_and_version_errors,
        messages=("Native crate layering is invalid:", "Native crate layering is valid."),
    )


def _layering_and_version_errors(root: Path) -> tuple[str, ...]:
    return get_native_layering_errors(root) + get_native_version_errors(root)
