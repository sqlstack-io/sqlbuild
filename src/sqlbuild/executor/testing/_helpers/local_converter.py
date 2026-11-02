"""Convert one Snowflake statement with the SQLQuack executable."""

from __future__ import annotations

import json
import subprocess
import tempfile
from collections.abc import Mapping
from typing import Any

from sqlbuild.errors.contracts.exceptions import ExecutorInputError
from sqlbuild.executor.testing._helpers.local_duckdb import run_converted_statement
from sqlbuild.executor.testing.constants import (
    LOCAL_TEST_ENGINE_CODE,
    SQLQUACK_CONVERT_TIMEOUT_SECONDS,
)
from sqlbuild.executor.testing.models import (
    LocalStatementRun,
    LocalTestEngine,
    SqlQuackConversion,
    SqlQuackRefusal,
)


def convert_statement(*, engine: LocalTestEngine, sql: str) -> SqlQuackConversion:
    """Convert one Snowflake statement to DuckDB SQL, or return SQLQuack's refusals."""

    with tempfile.NamedTemporaryFile("w", suffix=".sql", encoding="utf-8") as statement:
        statement.write(sql)
        statement.flush()
        completed: subprocess.CompletedProcess[str] = subprocess.run(
            [str(engine.executable), "convert", "--json", *_options(engine), statement.name],
            capture_output=True,
            text=True,
            timeout=SQLQUACK_CONVERT_TIMEOUT_SECONDS,
            check=False,
        )
    try:
        report: Any = json.loads(completed.stdout)
    except json.JSONDecodeError:
        report = None
    if not isinstance(report, dict):
        raise ExecutorInputError(
            f"SQLQuack exited {completed.returncode} without a conversion report: "
            f"{completed.stderr.strip()[-2000:]}",
            code=LOCAL_TEST_ENGINE_CODE,
        )
    return SqlQuackConversion(
        converted=bool(report.get("converted")),
        sql=str(report.get("sql") or ""),
        refusals=tuple(_refusal(item) for item in report.get("refusals") or ()),
        assumptions=tuple(str(item) for item in report.get("assumptions") or ()),
        notes=tuple(str(item) for item in report.get("notes") or ()),
    )


def _options(engine: LocalTestEngine) -> list[str]:
    options: list[str] = (
        ["--policy", str(engine.policy)] if engine.policy is not None else ["--bundled-policy"]
    )
    if engine.strict:
        options.append("--strict")
    for name, value in engine.session_parameters:
        options.extend(("--session", f"{name}={value}"))
    return options


def _refusal(item: Mapping[str, Any]) -> SqlQuackRefusal:
    return SqlQuackRefusal(
        construct=str(item.get("construct", "")),
        category=str(item.get("category", "")),
        reason=str(item.get("reason", "")),
    )


def run_converted_query(*, engine: LocalTestEngine, sql: str) -> LocalStatementRun:
    """Convert and run one diagnostic query; raises when it is refused or fails."""

    conversion: SqlQuackConversion = convert_statement(engine=engine, sql=sql)
    if not conversion.converted:
        raise ExecutorInputError(
            "SQLQuack refused the query: "
            + "; ".join(refusal.describe() for refusal in conversion.refusals),
            code=LOCAL_TEST_ENGINE_CODE,
        )
    run: LocalStatementRun = run_converted_statement(sql=conversion.sql, timeout_seconds=None)
    if run.error is not None:
        raise ExecutorInputError(run.error, code=LOCAL_TEST_ENGINE_CODE)
    return run
