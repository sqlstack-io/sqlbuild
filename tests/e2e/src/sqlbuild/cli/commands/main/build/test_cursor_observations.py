"""A build records the cursor MAX of each successful incremental model for plan previews."""

from __future__ import annotations

import subprocess
from pathlib import Path
from textwrap import dedent

import pytest

from tests.e2e.src.sqlbuild.cli.commands.main.build._test_types import (
    CursorObservationBuildE2ETestCase,
)
from tests.e2e.src.sqlbuild.cli.commands.shared.helpers import (
    prepare_inline_project,
    query_duckdb,
    run_sqb,
)

_PROJECT_FILES: dict[str, str] = {
    "sqlbuild_project.toml": dedent(
        """
        name = "cursor_observation_project"
        adapter = "duckdb"

        [connection]
        database = "observations.duckdb"
        """
    ).strip()
    + "\n",
    "seeds/raw_orders.csv": "id,ordered_at\n1,2026-01-01 00:00:00\n2,2026-01-01 01:00:00\n",
    "seeds/raw_orders.yml": dedent(
        """
        seeds:
          - name: raw_orders
            description: Raw orders.
            columns:
              - name: id
                type: INTEGER
              - name: ordered_at
                type: TIMESTAMP
        """
    ).strip()
    + "\n",
    "models/orders.sql": dedent(
        """
        MODEL (description "Orders by timestamp cursor.",
          materialized incremental,
          incremental_strategy append,
          cursor ordered_at,
          cursor_type timestamp,
          cursor_grain second,
        );

        SELECT id, ordered_at
        FROM __seed("raw_orders")
        """
    ).strip()
    + "\n",
    "models/order_ids.sql": dedent(
        """
        MODEL (description "Orders by integer cursor.",
          materialized incremental,
          incremental_strategy append,
          cursor id,
          cursor_type integer,
        );

        SELECT id FROM __seed("raw_orders")
        """
    ).strip()
    + "\n",
    "models/order_count.sql": dedent(
        """
        MODEL (description "Order count without a cursor.", materialized table);

        SELECT COUNT(*) AS order_count FROM __seed("raw_orders")
        """
    ).strip()
    + "\n",
}


@pytest.mark.parametrize(
    "test_case",
    [
        CursorObservationBuildE2ETestCase(
            description="each build appends one cursor_max per incremental model",
            builds=2,
            expected_observations=(
                ("order_ids", "integer", "2", 2),
                ("orders", "timestamp", "2026-01-01 01:00:00", 2),
            ),
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_incremental_models_when_building_then_cursor_max_observations_are_recorded(
    tmp_path: Path, test_case: CursorObservationBuildE2ETestCase
) -> None:
    project_dir: Path = prepare_inline_project(
        tmp_path=tmp_path, project_name="cursor_observations", repo_files=_PROJECT_FILES
    )

    results: list[subprocess.CompletedProcess[str]] = [
        run_sqb(command=("build",), project_dir=project_dir) for _ in range(test_case.builds)
    ]
    observations: list[tuple[object, ...]] = query_duckdb(
        db_path=project_dir / "observations.duckdb",
        sql=(
            "SELECT node, cursor_type, value, COUNT(DISTINCT run_id) "
            "FROM main._sqlbuild_observations WHERE kind = 'cursor_max' "
            "GROUP BY node, cursor_type, value ORDER BY node"
        ),
    )

    assert [result.returncode for result in results] == [0] * test_case.builds, results[-1].stderr
    assert "cursor observations" not in "".join(result.stderr for result in results)
    assert tuple(observations) == test_case.expected_observations
