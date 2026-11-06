"""Command-line options of the Postgres differential."""

from __future__ import annotations

import argparse
from pathlib import Path

from scripts.pg_differential.constants import SOURCE_SEPARATOR
from scripts.pg_differential.models import RunOptions


def labelled(text: str) -> tuple[str, Path]:
    """`LABEL=PATH` as `(LABEL, PATH)`; a bare path is labelled by its directory name."""
    label, separator, path = text.partition(SOURCE_SEPARATOR)
    if not separator:
        return Path(text).name, Path(text)
    return label, Path(path)


def parse_options(argv: list[str] | None) -> RunOptions:
    """Read the command line."""
    parser: argparse.ArgumentParser = argparse.ArgumentParser(
        description="Compare SQLBuild's Polyglot analysis of Postgres models with SQLQuack's."
    )
    parser.add_argument(
        "--work-dir",
        action="append",
        required=True,
        help=(
            "A compiler differential --work-dir run with --stage-captures, as LABEL=PATH to "
            "report it as its own source; repeatable."
        ),
    )
    parser.add_argument("--sqlquack", type=Path, required=True, help="A SQLQuack checkout.")
    parser.add_argument("--out", type=Path, required=True, help="Directory for the results.")
    parser.add_argument(
        "--no-mutations", action="store_true", help="Skip the name-breaking query variants."
    )
    arguments: argparse.Namespace = parser.parse_args(argv)
    return RunOptions(
        work_dirs=tuple(labelled(text) for text in arguments.work_dir),
        sqlquack=arguments.sqlquack,
        out=arguments.out,
        mutations=not arguments.no_mutations,
    )
