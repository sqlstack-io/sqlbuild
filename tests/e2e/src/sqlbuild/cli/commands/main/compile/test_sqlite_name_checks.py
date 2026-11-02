"""E2E tests for SQLite names checked by SQLQuack while polyglot keeps types and lineage."""

from __future__ import annotations

import json
import subprocess
from pathlib import Path

import pytest

from tests.e2e.src.sqlbuild.cli.commands.main.compile._test_types import (
    SqliteNameChecksTestCase,
)
from tests.e2e.src.sqlbuild.cli.commands.shared.helpers import prepare_inline_project, run_sqb

_PROJECT: str = 'name = "shop"\nadapter = "sqlite"\n\n[connection]\npath = ":memory:"\n'
_ORDERS: str = (
    "MODEL (description 'Orders.');\n\nSELECT 1 AS order_id, CAST(2.5 AS REAL) AS amount\n"
)


@pytest.mark.parametrize(
    "test_case",
    (
        SqliteNameChecksTestCase(
            description="a column the upstream model lacks is an error",
            model_sql='SELECT nope FROM __ref("orders")',
            expected_diagnostics=(("B002", "error", "Unknown column 'nope' in orders"),),
            expected_columns=(("nope", None),),
        ),
        SqliteNameChecksTestCase(
            description="ORDER BY a missing column is also reported, as a warning",
            model_sql='SELECT x FROM __ref("orders") WHERE x NOT NULL ORDER BY x',
            expected_diagnostics=(
                ("B002", "error", "Unknown column 'x' in orders"),
                ("B002", "error", "Unknown column 'x' in orders"),
                ("B002", "warning", "Unknown column 'x' in orders"),
            ),
            expected_columns=(("x", None),),
        ),
        SqliteNameChecksTestCase(
            description="names compare without case and polyglot's types stay",
            model_sql='SELECT Order_ID, amount FROM __ref("orders") AS o WHERE o.ORDER_ID > 0',
            expected_diagnostics=(),
            expected_columns=(("Order_ID", None), ("amount", "REAL")),
        ),
    ),
    ids=lambda case: case.description,
)
def test_given_sqlite_models_when_compiling_then_sqlquack_checks_names_only(
    test_case: SqliteNameChecksTestCase, tmp_path: Path
) -> None:
    project_dir: Path = prepare_inline_project(
        tmp_path=tmp_path,
        project_name="shop",
        repo_files={
            "sqlbuild_project.toml": _PROJECT,
            "models/orders.sql": _ORDERS,
            "models/report.sql": f"MODEL (description 'Report.');\n\n{test_case.model_sql}\n",
        },
    )

    result: subprocess.CompletedProcess[str] = run_sqb(
        project_dir=project_dir,
        command=("compile", "--json", "--no-cache", "--dag", "target/dag.json"),
    )

    payload: dict[str, object] = json.loads(result.stdout)
    diagnostics: list[dict[str, str]] = payload["diagnostics"]  # type: ignore[assignment]
    dag: dict[str, object] = json.loads((project_dir / "target" / "dag.json").read_text())
    nodes: list[dict[str, object]] = dag["nodes"]  # type: ignore[assignment]
    report: dict[str, object] = next(node for node in nodes if node["name"] == "report")
    columns: list[dict[str, str]] = report["columns"]  # type: ignore[assignment]
    assert (
        tuple((item["code"], item["severity"], item["message"]) for item in diagnostics)
        == test_case.expected_diagnostics
    ), result.stdout + result.stderr
    assert (
        tuple((column["name"], column.get("type")) for column in columns)
        == test_case.expected_columns
    )
