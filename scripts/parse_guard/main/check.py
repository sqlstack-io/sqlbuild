"""Parse guard command entrypoint."""

from __future__ import annotations

from scripts.parse_guard._helpers.scan import get_parse_guard_errors
from scripts.root_check.main.run_root_check import run_root_check


def run_parse_guard_check(argv: list[str] | None = None) -> int:
    """Fail when a native SQL parse uses a library's default function-call depth."""
    return run_root_check(
        argv=argv,
        description=__doc__,
        errors_under=get_parse_guard_errors,
        messages=(
            "Parse with sqlbuild_analysis::parse_guard's options, not a library default:",
            "Every native SQL parse uses the shared parse guard.",
        ),
    )
