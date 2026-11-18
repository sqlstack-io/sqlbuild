"""Parse guard command entrypoint."""

from __future__ import annotations

import argparse
import sys
from pathlib import Path

from scripts.parse_guard._helpers.scan import get_parse_guard_errors


def run_parse_guard_check(argv: list[str] | None = None) -> int:
    """Fail when a native SQL parse uses a library's default function-call depth."""
    parser: argparse.ArgumentParser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--root", type=Path, default=Path.cwd())
    arguments: argparse.Namespace = parser.parse_args(argv)
    errors: tuple[str, ...] = get_parse_guard_errors(arguments.root)
    if errors:
        print(
            "Parse with sqlbuild_analysis::parse_guard's options, not a library default:",
            file=sys.stderr,
        )
        for error in errors:
            print(f"- {error}", file=sys.stderr)
        return 1
    print("Every native SQL parse uses the shared parse guard.")
    return 0
