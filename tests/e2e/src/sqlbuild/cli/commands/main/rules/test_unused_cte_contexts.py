"""End-to-end coverage for framework-owned CTE reachability contexts."""

from __future__ import annotations

import subprocess
from pathlib import Path
from textwrap import dedent

import pytest

from tests.e2e.src.sqlbuild.cli.commands.main.rules._test_types import (
    UnusedCteContextTestCase,
)
from tests.e2e.src.sqlbuild.cli.commands.shared.helpers import run_sqb


@pytest.mark.parametrize(
    "test_case",
    [
        UnusedCteContextTestCase(
            description="table function argument keeps its input CTE reachable",
            files={
                "functions/sql/customer_orders.sql": dedent(
                    """
                    FUNCTION (description "Test function customer_orders.",
                      arguments (customer_id INTEGER),
                      returns table (customer_id INTEGER, order_id INTEGER)
                    );

                    SELECT customer_id AS customer_id, customer_id * 10 AS order_id
                    """
                ).strip()
                + "\n",
                "models/order_summary.sql": dedent(
                    """
                    MODEL (description "Test model order_summary.", materialized table);

                    WITH customer_ids AS (
                      SELECT 7 AS customer_id
                    ), fallback_ids AS (
                      SELECT 11 AS customer_id
                    ), fetched_orders AS (
                      SELECT *
                      FROM __table_fn("customer_orders")(
                        (SELECT MAX(customer_ids.customer_id)
                         FROM customer_ids, fallback_ids)
                      )
                    )
                    SELECT * FROM fetched_orders
                    """
                ).strip()
                + "\n",
            },
            expected_returncode=0,
            expected_finding_count=0,
        ),
        UnusedCteContextTestCase(
            description="generic audit argument may reference its local base CTE",
            files={
                "models/orders.sql": (
                    "MODEL (description 'Test model orders.',\n  materialized table,\n  audits [\n    order_group_rate (\n"
                    '      evaluation_sql "SELECT 1 AS is_ok FROM base",\n'
                    "      thresholds (error (below 0.5)),\n"
                    "    ),\n  ],\n);\n\nSELECT 1 AS order_id\n"
                ),
                "audits/generic/order_group_rate.sql": dedent(
                    """
                    AUDIT (
                      evaluation measurement,
                      value measured_value,
                      sample_count sample_count,
                      sample_unit groups
                    );

                    MEASURE (
                      WITH base AS (
                        SELECT * FROM @relation
                      ), evaluated AS (
                        -- The supplied SQL may read any CTE declared above it.
                        @evaluation_sql
                      )
                      SELECT COUNT(*) AS sample_count, AVG(is_ok) AS measured_value
                      FROM evaluated
                    );
                    """
                ).strip()
                + "\n",
            },
            expected_returncode=0,
            expected_finding_count=0,
        ),
        UnusedCteContextTestCase(
            description="opaque audit SQL does not hide a later dead CTE",
            files={
                "models/orders.sql": (
                    "MODEL (description 'Test model orders.',\n  materialized table,\n  audits [\n    order_group_rate (\n"
                    '      evaluation_sql "SELECT 1 AS is_ok FROM base",\n'
                    "      thresholds (error (below 0.5)),\n"
                    "    ),\n  ],\n);\n\nSELECT 1 AS order_id\n"
                ),
                "audits/generic/order_group_rate.sql": dedent(
                    """
                    AUDIT (
                      evaluation measurement,
                      value measured_value,
                      sample_count sample_count,
                      sample_unit groups
                    );

                    MEASURE (
                      WITH base AS (
                        SELECT * FROM @relation
                      ), evaluated AS (
                        @evaluation_sql
                      ), dead_orders AS (
                        SELECT 1 AS order_id
                      )
                      SELECT COUNT(*) AS sample_count, AVG(is_ok) AS measured_value
                      FROM evaluated
                    );
                    """
                ).strip()
                + "\n",
            },
            expected_returncode=1,
            expected_finding_count=1,
        ),
        UnusedCteContextTestCase(
            description="test harness output keeps its helper CTE reachable",
            files={
                "models/raw_orders.sql": (
                    "MODEL (description 'Test model raw_orders.', materialized table);\n\nSELECT 7 AS order_id, 25 AS amount\n"
                ),
                "models/orders.sql": (
                    'MODEL (description "Test model orders.", materialized table);\n\nSELECT * FROM __ref("raw_orders")\n'
                ),
                "tests/unit/orders_test.sql": dedent(
                    """
                    TEST ();

                    WITH __ref__raw_orders AS (
                      SELECT 7 AS order_id, 25 AS amount
                    ), expected_rows AS (
                      SELECT 7 AS order_id, 25 AS amount
                    ), __expected__orders AS (
                      SELECT order_id, amount FROM expected_rows
                    )
                    SELECT 1
                    """
                ).strip()
                + "\n",
            },
            expected_returncode=0,
            expected_finding_count=0,
        ),
        UnusedCteContextTestCase(
            description="ordinary audit parameters do not hide a dead CTE",
            files={
                "models/orders.sql": (
                    "MODEL (description 'Test model orders.', materialized table, audits [order_rows]);\n\nSELECT 1 AS order_id\n"
                ),
                "audits/generic/order_rows.sql": dedent(
                    """
                    AUDIT ();

                    WITH dead_orders AS (
                      SELECT 1 AS order_id
                    ), live_orders AS (
                      SELECT * FROM @relation
                    )
                    SELECT * FROM live_orders
                    """
                ).strip()
                + "\n",
            },
            expected_returncode=1,
            expected_finding_count=1,
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_framework_cte_context_when_running_rule_then_live_cte_is_not_reported(
    tmp_path: Path,
    test_case: UnusedCteContextTestCase,
) -> None:
    project_dir: Path = tmp_path / "project"
    project_dir.mkdir()
    (project_dir / "sqlbuild_project.toml").write_text(
        'name = "unused_cte_contexts"\nadapter = "duckdb"\n',
        encoding="utf-8",
    )
    for relative_path, contents in test_case.files.items():
        target: Path = project_dir / relative_path
        target.parent.mkdir(parents=True, exist_ok=True)
        target.write_text(contents, encoding="utf-8")

    result: subprocess.CompletedProcess[str] = run_sqb(
        command=("--no-color", "rules", "run", "SQLCLEAN005"),
        project_dir=project_dir,
    )

    assert result.returncode == test_case.expected_returncode, result.stdout + result.stderr
    assert result.stdout.count("[SQLCLEAN005]") == test_case.expected_finding_count
