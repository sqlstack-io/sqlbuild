"""Command line for the SQL Server parse differential."""

from __future__ import annotations

import argparse
import json
from pathlib import Path
from typing import Any

from scripts.sqlserver_differential._helpers.captures import capture_paths, project_cases
from scripts.sqlserver_differential._helpers.harness import run_harness
from scripts.sqlserver_differential._helpers.listed import listed_cases
from scripts.sqlserver_differential._helpers.recorded import recorded_cases
from scripts.sqlserver_differential._helpers.report import summarize
from scripts.sqlserver_differential.constants import SOURCE_SEPARATOR
from scripts.sqlserver_differential.exceptions import SqlServerDifferentialError
from scripts.sqlserver_differential.models import RunOptions


def labelled_path(text: str) -> tuple[str, Path]:
    """A `LABEL=PATH` command line value."""
    label, separator, path = text.partition(SOURCE_SEPARATOR)
    if not separator or not label or not path:
        raise argparse.ArgumentTypeError(f"expected LABEL=PATH, got {text!r}")
    return label, Path(path)


def parse_options(argv: list[str] | None) -> RunOptions:
    """The run's options from the command line."""
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument(
        "--work-dir",
        type=labelled_path,
        action="append",
        default=[],
        help="LABEL=PATH of a `run_compiler_differential --stage-captures` work dir",
    )
    parser.add_argument(
        "--listed",
        type=labelled_path,
        action="append",
        default=[],
        help="LABEL=PATH of a JSON-lines file of SQL bodies (id, sql, kind, mode)",
    )
    parser.add_argument(
        "--recorded",
        type=labelled_path,
        action="append",
        default=[],
        help="LABEL=PATH of statements recorded by `scripts.record_sqlserver_statements`",
    )
    parser.add_argument("--sqlquack", type=Path, required=True)
    parser.add_argument("--out", type=Path, required=True)
    arguments = parser.parse_args(argv)
    return RunOptions(
        work_dirs=tuple(arguments.work_dir),
        listed=tuple(arguments.listed),
        recorded=tuple(arguments.recorded),
        sqlquack=arguments.sqlquack,
        out=arguments.out,
    )


def collect(options: RunOptions) -> list[dict[str, Any]]:
    """Every SQL body the options name."""
    cases: list[dict[str, Any]] = []
    for label, path in options.work_dirs:
        for capture in capture_paths(path):
            cases.extend(project_cases(path=capture, source=label))
    for label, path in options.listed:
        cases.extend(listed_cases(path=path, source=label))
    for label, path in options.recorded:
        cases.extend(recorded_cases(record_dir=path, source=label))
    if not cases:
        raise SqlServerDifferentialError("no SQL bodies to compare")
    return cases


def run_sqlserver_differential(argv: list[str] | None = None) -> int:
    """Compare both parsers on every body and write the results and their summary."""
    options: RunOptions = parse_options(argv)
    cases: list[dict[str, Any]] = collect(options)
    sql_by_id: dict[str, str] = {case["id"]: case["sql"] for case in cases}
    results: list[dict[str, Any]] = [
        {**result, "sql": sql_by_id[result["id"]]}
        for result in run_harness(sqlquack=options.sqlquack, cases=cases)
    ]
    options.out.mkdir(parents=True, exist_ok=True)
    (options.out / "results.jsonl").write_text(
        "".join(json.dumps(result) + "\n" for result in results), encoding="utf-8"
    )
    summary: dict[str, Any] = summarize(results)
    (options.out / "summary.json").write_text(
        json.dumps(summary, indent=2) + "\n", encoding="utf-8"
    )
    print(json.dumps({key: summary[key] for key in ("bodies", "unique_bodies", "categories")}))
    return 0
