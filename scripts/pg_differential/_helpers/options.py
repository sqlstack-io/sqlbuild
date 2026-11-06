"""Command-line options of the Postgres differential."""

from __future__ import annotations

import argparse
from pathlib import Path

from scripts.pg_differential.models import RunOptions


def parse_options(argv: list[str] | None) -> RunOptions:
    """Read the command line."""
    parser: argparse.ArgumentParser = argparse.ArgumentParser(
        description="Compare SQLBuild's Polyglot analysis of Postgres models with SQLQuack's."
    )
    parser.add_argument(
        "--work-dir",
        type=Path,
        action="append",
        required=True,
        help="A compiler differential --work-dir run with --stage-captures; repeatable.",
    )
    parser.add_argument("--sqlquack", type=Path, required=True, help="A SQLQuack checkout.")
    parser.add_argument("--out", type=Path, required=True, help="Directory for the results.")
    parser.add_argument(
        "--no-mutations", action="store_true", help="Skip the name-breaking query variants."
    )
    arguments: argparse.Namespace = parser.parse_args(argv)
    return RunOptions(
        work_dirs=tuple(arguments.work_dir),
        sqlquack=arguments.sqlquack,
        out=arguments.out,
        mutations=not arguments.no_mutations,
    )
