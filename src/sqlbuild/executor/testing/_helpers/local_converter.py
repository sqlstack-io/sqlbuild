"""Convert Snowflake statements and SQL project functions with the SQLQuack executable."""

from __future__ import annotations

import json
import subprocess
import tempfile
from pathlib import Path
from typing import Any, cast

from sqlbuild.errors.contracts.exceptions import ExecutorInputError
from sqlbuild.executor.testing._helpers.local_duckdb import run_converted_statement
from sqlbuild.executor.testing.constants import (
    LOCAL_TEST_ENGINE_CODE,
    SQLQUACK_FUNCTION_FAILED_PREFIX,
    SQLQUACK_FUNCTION_REFUSED_PREFIX,
)
from sqlbuild.executor.testing.models import (
    LocalFunctionUse,
    LocalStatementRun,
    LocalTestEngine,
    SqlQuackConversion,
    SqlQuackFunction,
    SqlQuackRefusal,
)


def convert_statement(
    *, engine: LocalTestEngine, sql: str, timeout_seconds: float | None, schema: str | None = None
) -> SqlQuackConversion:
    """DuckDB SQL from a clean report, or SQLQuack's refusals; raises on any other report."""

    with tempfile.TemporaryDirectory(prefix="sqb-sqlquack-") as directory:
        statement: Path = Path(directory) / "statement.sql"
        statement.write_text(sql, encoding="utf-8")
        declared: list[str] = []
        if schema is not None:
            schema_file: Path = Path(directory) / "schema.json"
            schema_file.write_text(schema, encoding="utf-8")
            declared = ["--schema", str(schema_file)]
        completed: subprocess.CompletedProcess[str] = _run(
            engine=engine,
            arguments=["convert", "--json", *_options(engine), *declared, str(statement)],
            timeout_seconds=timeout_seconds,
        )
    return _checked_report(completed)


def convert_function(
    *, engine: LocalTestEngine, name: str, schema: str, body: str, timeout_seconds: float
) -> SqlQuackFunction:
    """A SQL project function's body as a DuckDB macro body, or why SQLQuack refused it."""

    with tempfile.TemporaryDirectory(prefix="sqb-sqlquack-") as directory:
        body_file: Path = Path(directory) / "body.sql"
        body_file.write_text(body, encoding="utf-8")
        schema_file: Path = Path(directory) / "schema.json"
        schema_file.write_text(schema, encoding="utf-8")
        try:
            completed: subprocess.CompletedProcess[str] = _run(
                engine=engine,
                arguments=[
                    "function",
                    *_options(engine),
                    "--name",
                    name,
                    "--schema",
                    str(schema_file),
                    str(body_file),
                ],
                timeout_seconds=timeout_seconds,
            )
        except subprocess.TimeoutExpired:
            return SqlQuackFunction(
                refusal=f"SQLQuack did not convert its body within {timeout_seconds:g}s"
            )
    return _checked_function(completed)


def _run(
    *, engine: LocalTestEngine, arguments: list[str], timeout_seconds: float | None
) -> subprocess.CompletedProcess[str]:
    try:
        return subprocess.run(
            [str(engine.executable), *arguments],
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


def _checked_function(completed: subprocess.CompletedProcess[str]) -> SqlQuackFunction:
    reason: str = completed.stderr.strip().splitlines()[-1] if completed.stderr.strip() else ""
    if completed.returncode != 0:
        return SqlQuackFunction(
            refusal=reason.removeprefix(SQLQUACK_FUNCTION_REFUSED_PREFIX).removeprefix(
                SQLQUACK_FUNCTION_FAILED_PREFIX
            )
            or f"SQLQuack exited {completed.returncode} without a reason"
        )
    try:
        report: Any = json.loads(completed.stdout)
    except json.JSONDecodeError:
        report = None
    parameters: object = report.get("parameters") if isinstance(report, dict) else None
    body: object = report.get("body") if isinstance(report, dict) else None
    provenance: object = report.get("provenance", {}) if isinstance(report, dict) else None
    divergences: object = (
        provenance.get("divergences", {}) if isinstance(provenance, dict) else None
    )
    if (
        not _strings(parameters)
        or not isinstance(body, str)
        or not body.strip()
        or not isinstance(divergences, dict)
    ):
        raise ExecutorInputError(
            "SQLQuack exited 0 without a usable function report: "
            f"{completed.stderr.strip()[-2000:]}",
            code=LOCAL_TEST_ENGINE_CODE,
        )
    return SqlQuackFunction(
        parameters=tuple(cast(list[str], parameters)),
        body=body,
        divergences=tuple(sorted(str(divergence) for divergence in divergences)),
    )


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
    *,
    engine: LocalTestEngine,
    sql: str,
    timeout_seconds: float,
    functions: LocalFunctionUse | None = None,
) -> LocalStatementRun:
    """Convert and run one diagnostic query, each step within the time limit; raises otherwise."""

    use: LocalFunctionUse = functions or LocalFunctionUse()
    conversion: SqlQuackConversion = convert_statement(
        engine=engine, sql=sql, timeout_seconds=timeout_seconds, schema=use.schema
    )
    if not conversion.converted:
        raise ExecutorInputError(
            "SQLQuack refused the query: "
            + "; ".join(refusal.describe() for refusal in conversion.refusals),
            code=LOCAL_TEST_ENGINE_CODE,
        )
    run: LocalStatementRun = run_converted_statement(
        sql=conversion.sql, timeout_seconds=timeout_seconds, setup=use.setup
    )
    if run.error is not None:
        raise ExecutorInputError(run.error, code=LOCAL_TEST_ENGINE_CODE)
    return run
