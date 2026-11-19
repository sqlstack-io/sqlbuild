"""Release gate command entrypoint."""

from __future__ import annotations

from pathlib import Path

from scripts.release_gate._helpers.gate import get_release_gate_errors
from scripts.release_gate.exceptions import ReleaseGateError
from scripts.root_check.main.run_root_check import run_root_check


def run_release_gate_check(argv: list[str] | None = None) -> int:
    """Fail when anything but the trusted release checks can pass a release pull request."""
    return run_root_check(
        argv=argv,
        description=__doc__,
        errors_under=_gate_errors,
        messages=(
            "Release pull request Verify gate is invalid:",
            "Release pull request Verify gate is valid.",
        ),
    )


def _gate_errors(root: Path) -> tuple[str, ...]:
    try:
        return get_release_gate_errors(root)
    except ReleaseGateError as error:
        return (str(error),)
