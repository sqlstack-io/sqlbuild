"""Run one repository check over `--root` and report its errors."""

from __future__ import annotations

import argparse
import sys
from collections.abc import Callable
from pathlib import Path


def run_root_check(
    *,
    argv: list[str] | None,
    description: str | None,
    errors_under: Callable[[Path], tuple[str, ...]],
    messages: tuple[str, str],
) -> int:
    """Print the check's errors under the failure heading and fail, or print the success line."""
    failure_heading, success_line = messages
    parser: argparse.ArgumentParser = argparse.ArgumentParser(description=description)
    parser.add_argument("--root", type=Path, default=Path.cwd())
    arguments: argparse.Namespace = parser.parse_args(argv)
    errors: tuple[str, ...] = errors_under(arguments.root)
    if errors:
        print(failure_heading, file=sys.stderr)
        for error in errors:
            print(f"- {error}", file=sys.stderr)
        return 1
    print(success_line)
    return 0
