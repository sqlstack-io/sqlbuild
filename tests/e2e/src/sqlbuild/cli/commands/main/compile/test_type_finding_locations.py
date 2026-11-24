"""E2E coverage of authored locations for cast and set-operation type findings."""

from __future__ import annotations

import json
import subprocess
from pathlib import Path
from typing import Any

import pytest

from tests.e2e.src.sqlbuild.cli.commands.main.compile._test_types import TypeFindingLocationCase
from tests.e2e.src.sqlbuild.cli.commands.shared.helpers import prepare_inline_project, run_sqb


@pytest.mark.parametrize(
    "test_case",
    (
        TypeFindingLocationCase(
            description="two failing literal casts report two findings at their casts",
            adapter="duckdb",
            query_sql=(
                "WITH final AS (\n"
                "  SELECT\n"
                "    CAST(TIMESTAMP '2026-04-01' AS INTEGER) AS ordered_epoch,\n"
                "    CAST(TIMESTAMP '2026-04-02' AS INTEGER) AS shipped_epoch\n"
                ")\n"
                "SELECT f.ordered_epoch, f.shipped_epoch FROM final AS f\n"
            ),
            expected_diagnostics=(
                ("B218", "Cannot cast TIMESTAMP to INTEGER", 5, 5),
                ("B218", "Cannot cast TIMESTAMP to INTEGER", 6, 5),
            ),
        ),
        TypeFindingLocationCase(
            description="set-operation coercion points at the branch projection",
            adapter="snowflake",
            query_sql=(
                "WITH final AS (\n"
                "  SELECT 1 AS order_id\n"
                "  UNION ALL\n"
                "  SELECT 'pending'\n"
                ")\n"
                "SELECT f.order_id FROM final AS f\n"
            ),
            expected_diagnostics=(
                (
                    "B215",
                    "Set-operation column 1 may fail during runtime conversion: "
                    "accumulated type NUMBER, next type VARCHAR",
                    6,
                    10,
                ),
            ),
        ),
    ),
    ids=lambda case: case.description,
)
def test_given_type_findings_without_native_spans_when_compiling_then_each_is_located(
    test_case: TypeFindingLocationCase, tmp_path: Path
) -> None:
    project_dir: Path = prepare_inline_project(
        tmp_path=tmp_path,
        project_name="orders",
        repo_files={
            "sqlbuild_project.toml": f'name = "orders"\nadapter = "{test_case.adapter}"\n',
            "sql/models/orders.sql": (
                'MODEL (description "Order epochs", database warehouse, schema analytics);\n\n'
                + test_case.query_sql
            ),
        },
    )

    result: subprocess.CompletedProcess[str] = run_sqb(
        project_dir=project_dir, command=("--no-color", "compile", "--json", "--no-cache")
    )

    payload: dict[str, Any] = json.loads(result.stdout)
    assert result.returncode == 1, result.stdout + result.stderr
    assert (
        tuple(
            (item["code"], item["message"], item["line"], item["column"])
            for item in payload["diagnostics"]
        )
        == test_case.expected_diagnostics
    )


if __name__ == "__main__":
    pytest.main([__file__, "-vv"])
