"""Convert one Snowflake statement with the SQLQuack executable."""

from __future__ import annotations

import json
import subprocess
import tempfile
from typing import Any

from sqlbuild.errors.contracts.exceptions import ExecutorInputError
from sqlbuild.executor.testing._helpers.local_duckdb import run_converted_statement
from sqlbuild.executor.testing.constants import LOCAL_TEST_ENGINE_CODE
from sqlbuild.executor.testing.models import (
    LocalStatementRun,
    LocalTestEngine,
    SqlQuackConversion,
    SqlQuackRefusal,
)


def convert_statement(
    *, engine: LocalTestEngine, sql: str, timeout_seconds: float | None
) -> SqlQuackConversion:
    """DuckDB SQL from a clean report, or SQLQuack's refusals; raises on any other report."""

    with tempfile.NamedTemporaryFile("w", suffix=".sql", encoding="utf-8") as statement:
        statement.write(sql)
        statement.flush()
        try:
            completed: subprocess.CompletedProcess[str] = subprocess.run(
                [str(engine.executable), "convert", "--json", *_options(engine), statement.name],
                capture_output=True,
                text=True,
                timeout=timeout_seconds,
                check=False,
            )
        except OSError as error:
            raise ExecutorInputError(
                f"SQLQuack at {engine.executable} could not be run: {error}",
                code=LOCAL_TEST_ENGINE_CODE,
            ) from error
    return _checked_report(completed)


def _checked_report(completed: subprocess.CompletedProcess[str]) -> SqlQuackConversion:
    try:
        report: Any = json.loads(completed.stdout)
    except json.JSONDecodeError:
        report = None
    unusable: str = (
        f"SQLQuack exited {completed.returncode} without a usable conversion report: "
        f"{completed.stderr.strip()[-2000:]}"
    )
    if not isinstance(report, dict):
        raise ExecutorInputError(unusable, code=LOCAL_TEST_ENGINE_CODE)
    refusals: object = report.get("refusals", [])
    assumptions: object = report.get("assumptions", [])
    notes: object = report.get("notes", [])
    if (
        not isinstance(refusals, list)
        or not all(isinstance(item, dict) for item in refusals)
        or not _strings(assumptions)
        or not _strings(notes)
    ):
        raise ExecutorInputError(unusable, code=LOCAL_TEST_ENGINE_CODE)
    sql: object = report.get("sql")
    conversion: SqlQuackConversion = SqlQuackConversion(
        converted=False,
        sql="",
        refusals=tuple(
            SqlQuackRefusal(
                construct=str(item.get("construct", "")),
                category=str(item.get("category", "")),
                reason=str(item.get("reason", "")),
            )
            for item in refusals
        ),
        assumptions=tuple(assumptions),
        notes=tuple(notes),
    )
    if conversion.refusals:
        return conversion
    if (
        completed.returncode != 0
        or report.get("converted") is not True
        or not isinstance(sql, str)
        or not sql.strip()
    ):
        raise ExecutorInputError(
            f"{unusable} (converted={report.get('converted')!r}, no refusals)",
            code=LOCAL_TEST_ENGINE_CODE,
        )
    return SqlQuackConversion(
        converted=True,
        sql=sql,
        assumptions=conversion.assumptions,
        notes=conversion.notes,
    )


def _strings(value: object) -> bool:
    return isinstance(value, list) and all(isinstance(item, str) for item in value)


def _options(engine: LocalTestEngine) -> list[str]:
    options: list[str] = (
        ["--policy", str(engine.policy)] if engine.policy is not None else ["--builtin-policy"]
    )
    if engine.strict:
        options.append("--strict")
    for name, value in engine.session_parameters:
        options.extend(("--session", f"{name}={value}"))
    return options


def run_converted_query(
    *, engine: LocalTestEngine, sql: str, timeout_seconds: float
) -> LocalStatementRun:
    """Convert and run one diagnostic query, each step within the time limit; raises otherwise."""

    conversion: SqlQuackConversion = convert_statement(
        engine=engine, sql=sql, timeout_seconds=timeout_seconds
    )
    if not conversion.converted:
        raise ExecutorInputError(
            "SQLQuack refused the query: "
            + "; ".join(refusal.describe() for refusal in conversion.refusals),
            code=LOCAL_TEST_ENGINE_CODE,
        )
    run: LocalStatementRun = run_converted_statement(
        sql=conversion.sql, timeout_seconds=timeout_seconds
    )
    if run.error is not None:
        raise ExecutorInputError(run.error, code=LOCAL_TEST_ENGINE_CODE)
    return run
