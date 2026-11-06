"""Postgres differential command entrypoint."""

from __future__ import annotations

import json
from pathlib import Path
from typing import Any

from scripts.pg_differential._helpers.captures import capture_paths, mutated_cases, project_cases
from scripts.pg_differential._helpers.harness import run_harness
from scripts.pg_differential._helpers.options import parse_options
from scripts.pg_differential._helpers.report import summarise
from scripts.pg_differential.exceptions import PgDifferentialError
from scripts.pg_differential.models import RunOptions


def run_pg_differential(argv: list[str] | None = None) -> int:
    """Write per-case comparisons and a summary for every captured Postgres model."""
    options: RunOptions = parse_options(argv)
    cases: list[dict[str, Any]] = []
    try:
        for source, work_dir in options.work_dirs:
            for path in capture_paths(work_dir):
                cases.extend(project_cases(path=path, source=source))
        variants: list[dict[str, Any]] = []
        if options.mutations:
            for case in cases:
                variants.extend(mutated_cases(case))
        cases.extend(variants)
        results: list[dict[str, Any]] = run_harness(sqlquack=options.sqlquack, cases=cases)
    except PgDifferentialError as error:
        print(error)
        return 1
    out: Path = options.out
    out.mkdir(parents=True, exist_ok=True)
    case_lines: str = "".join(json.dumps(case, ensure_ascii=False) + "\n" for case in cases)
    result_lines: str = "".join(json.dumps(item, ensure_ascii=False) + "\n" for item in results)
    (out / "cases.jsonl").write_text(case_lines, encoding="utf-8")
    (out / "results.jsonl").write_text(result_lines, encoding="utf-8")
    summary: dict[str, Any] = summarise(cases=cases, results=results)
    summary_text: str = json.dumps(summary, indent=2, ensure_ascii=False) + "\n"
    (out / "summary.json").write_text(summary_text, encoding="utf-8")
    overview: dict[str, Any] = {}
    for source, part in summary["sources"].items():
        overview[source] = {"cases": part["cases"], "parse": part["parse"]}
        overview[source]["verdicts"] = part["verdicts"]
    print(json.dumps(overview, indent=2))
    return 0
