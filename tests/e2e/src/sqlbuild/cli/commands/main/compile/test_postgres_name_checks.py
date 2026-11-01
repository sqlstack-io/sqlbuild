"""E2E tests for Postgres names checked by SQLQuack while polyglot keeps types and lineage."""

from __future__ import annotations

import json
import subprocess
from pathlib import Path

import pytest

from tests.e2e.src.sqlbuild.cli.commands.main.compile._test_types import (
    PostgresNameChecksTestCase,
)
from tests.e2e.src.sqlbuild.cli.commands.shared.helpers import prepare_inline_project, run_sqb

_PROJECT: str = 'name = "shop"\nadapter = "postgres"\n\n[defaults]\ndatabase = "example"\nschema = "analytics"\n'
_ORDERS: str = (
    "MODEL (description 'Orders.');\n\n"
    "SELECT 1 AS order_id, CAST('2.5' AS NUMERIC(10, 2)) AS amount\n"
)


@pytest.mark.parametrize(
    "test_case",
    (
        PostgresNameChecksTestCase(
            description="a CTE reference's column aliases name its columns",
            model_sql=(
                'WITH o AS (SELECT order_id, amount FROM __ref("orders"))\n'
                "SELECT a.id1, b.id2, a.amount1 > 0 AS positive FROM o AS a(id1, amount1), o AS b(id2)"
            ),
            expected_diagnostics=(),
            expected_columns=(("id1", None), ("id2", None), ("positive", "BOOLEAN")),
        ),
        PostgresNameChecksTestCase(
            description="a relation's name reads its whole row",
            model_sql="SELECT r, r IS NULL AS missing FROM (VALUES (1, 2)) AS r(a, b)",
            expected_diagnostics=(),
            expected_columns=(("r", None), ("missing", "BOOLEAN")),
        ),
        PostgresNameChecksTestCase(
            description="a column the aliased CTE lacks is still an error",
            model_sql=(
                'WITH o AS (SELECT order_id FROM __ref("orders"))\nSELECT a.nope FROM o AS a(id1)'
            ),
            expected_diagnostics=(("B002", "error", "Unknown column 'nope' in o (as a)"),),
            expected_columns=(("nope", None),),
        ),
        PostgresNameChecksTestCase(
            description="polyglot's types and lineage stay for the model's columns",
            model_sql='SELECT order_id, amount FROM __ref("orders")',
            expected_diagnostics=(),
            expected_columns=(("order_id", None), ("amount", "DECIMAL(10, 2)")),
        ),
    ),
    ids=lambda case: case.description,
)
def test_given_postgres_models_when_compiling_then_sqlquack_checks_names_only(
    test_case: PostgresNameChecksTestCase, tmp_path: Path
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
