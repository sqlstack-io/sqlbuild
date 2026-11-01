"""A star projected beside a cast constant keeps every starred column through the real CLI."""

from __future__ import annotations

import json
import subprocess
from pathlib import Path
from typing import Any

import pytest

from tests.e2e.src.sqlbuild.cli.commands.main.compile._test_types import (
    StarBesideCastConstantTestCase,
)
from tests.e2e.src.sqlbuild.cli.commands.shared.helpers import (
    prepare_inline_project,
    query_duckdb,
    run_sqb,
)

_PROJECT_TOML: str = (
    'name = "orders"\nadapter = "duckdb"\n[connection]\ndatabase = "orders.duckdb"\n\n'
    '[[rules.rule_ignores]]\nrules = ["SQLCLEAN039"]\npaths = ["models/flagged_orders.sql"]\n'
    'reason = "One case stars over a derived table on purpose."\n'
)
_BASE_ORDERS_SQL: str = (
    "MODEL (description 'Orders.', materialized table);\n\n"
    "SELECT 1 AS order_id, 10 AS amount_cents\n"
)
_FLAGGED_ORDERS_HEADER: str = (
    "MODEL (description 'Flagged orders.', materialized table, unique_key order_id);\n\n"
)


@pytest.mark.parametrize(
    "test_case",
    [
        StarBesideCastConstantTestCase(
            description="star over a derived table beside a cast constant",
            query_sql=(
                "SELECT *, CAST(1 AS INTEGER) AS is_flagged\n"
                'FROM (SELECT order_id, amount_cents FROM __ref("base_orders")) AS edited'
            ),
            expected_columns=("order_id", "amount_cents", "is_flagged"),
        ),
        StarBesideCastConstantTestCase(
            description="star over a reference beside a cast constant",
            query_sql='SELECT *, CAST(1 AS INTEGER) AS is_flagged FROM __ref("base_orders")',
            expected_columns=("order_id", "amount_cents", "is_flagged"),
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_star_beside_cast_constant_when_compiling_and_building_then_starred_columns_are_kept(
    test_case: StarBesideCastConstantTestCase,
    tmp_path: Path,
) -> None:
    project_dir: Path = prepare_inline_project(
        tmp_path=tmp_path,
        project_name="orders_project",
        repo_files={
            "sqlbuild_project.toml": _PROJECT_TOML,
            "models/base_orders.sql": _BASE_ORDERS_SQL,
            "models/flagged_orders.sql": f"{_FLAGGED_ORDERS_HEADER}{test_case.query_sql}\n",
        },
    )

    compiled: subprocess.CompletedProcess[str] = run_sqb(
        project_dir=project_dir, command=("compile", "--no-cache", "--json")
    )
    payload: dict[str, Any] = json.loads(compiled.stdout)
    column_counts: dict[str, int] = {
        model["name"]: model["column_count"] for model in payload["resources"]["models"]
    }
    built: subprocess.CompletedProcess[str] = run_sqb(project_dir=project_dir, command=("build",))
    built_columns: list[tuple[Any, ...]] = query_duckdb(
        db_path=project_dir / "orders.duckdb",
        sql=(
            "SELECT column_name FROM information_schema.columns "
            "WHERE table_name = 'flagged_orders' ORDER BY ordinal_position"
        ),
    )

    assert compiled.returncode == 0, f"{test_case.description}: {compiled.stdout}{compiled.stderr}"
    assert payload["diagnostics"] == [], test_case.description
    assert column_counts["flagged_orders"] == len(test_case.expected_columns), test_case.description
    assert built.returncode == 0, f"{test_case.description}: {built.stdout}{built.stderr}"
    assert tuple(row[0] for row in built_columns) == test_case.expected_columns, (
        test_case.description
    )


if __name__ == "__main__":
    pytest.main([__file__, "-vv"])
