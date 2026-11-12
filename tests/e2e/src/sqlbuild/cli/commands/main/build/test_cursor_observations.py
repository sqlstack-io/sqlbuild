"""A build records cursor MAX values and described source columns for plan previews."""

from __future__ import annotations

import subprocess
from pathlib import Path
from textwrap import dedent

import pytest

from tests.e2e.src.sqlbuild.cli.commands.main.build._test_types import (
    CursorObservationBuildE2ETestCase,
    DisplayOnlyObservationE2ETestCase,
    SourceColumnObservationE2ETestCase,
)
from tests.e2e.src.sqlbuild.cli.commands.shared.helpers import (
    execute_duckdb,
    normalize_cli_output,
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
    "sources/raw.yml": dedent(
        """
        sources:
          - name: raw_refunds
            description: Refunds described from an inline query.
            expression: |
              SELECT * FROM (VALUES (1, 100)) AS refunds(refund_id, amount_cents)
            type_enforcement: true
            columns:
              - name: refund_id
                type: INTEGER
              - name: amount_cents
                type: INTEGER
        """
    ).strip()
    + "\n",
    "models/refunds.sql": dedent(
        """
        MODEL (description "Refunds.", materialized table);

        SELECT refund_id, amount_cents FROM __source("raw_refunds")
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


@pytest.mark.parametrize(
    "test_case",
    [
        SourceColumnObservationE2ETestCase(
            description="builds record described source columns and plans record none",
            commands=(("build",), ("plan",), ("build",)),
            expected_observations=(("raw_refunds", '["refund_id","amount_cents"]', 2),),
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_expression_source_when_building_then_source_column_observations_are_recorded(
    tmp_path: Path, test_case: SourceColumnObservationE2ETestCase
) -> None:
    project_dir: Path = prepare_inline_project(
        tmp_path=tmp_path, project_name="source_observations", repo_files=_PROJECT_FILES
    )

    results: list[subprocess.CompletedProcess[str]] = [
        run_sqb(command=command, project_dir=project_dir) for command in test_case.commands
    ]
    observations: list[tuple[object, ...]] = query_duckdb(
        db_path=project_dir / "observations.duckdb",
        sql=(
            "SELECT node, value, COUNT(DISTINCT run_id) "
            "FROM main._sqlbuild_observations WHERE kind = 'source_columns' "
            "GROUP BY node, value ORDER BY node"
        ),
    )

    assert [result.returncode for result in results] == [0] * len(test_case.commands), results[
        -1
    ].stderr
    assert tuple(observations) == test_case.expected_observations


@pytest.mark.parametrize(
    "test_case",
    [
        DisplayOnlyObservationE2ETestCase(
            description="cursor_max past the live MAX still appends from the inclusive live bound",
            injected_observations=(
                ("order_ids", "integer", "999"),
                ("orders", "timestamp", "2099-01-01 00:00:00"),
            ),
            added_seed_rows="3,2026-01-01 02:00:00\n",
            expected_row_counts=(("order_ids", 4), ("orders", 4)),
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_wrong_cursor_observation_when_planning_and_building_then_live_max_is_used(
    tmp_path: Path, test_case: DisplayOnlyObservationE2ETestCase
) -> None:
    project_dir: Path = prepare_inline_project(
        tmp_path=tmp_path, project_name="display_only_observations", repo_files=_PROJECT_FILES
    )
    database: Path = project_dir / "observations.duckdb"
    first: subprocess.CompletedProcess[str] = run_sqb(command=("build",), project_dir=project_dir)
    assert first.returncode == 0, first.stderr
    seed: Path = project_dir / "seeds/raw_orders.csv"
    seed.write_text(seed.read_text() + test_case.added_seed_rows)

    honest: subprocess.CompletedProcess[str] = run_sqb(command=("plan",), project_dir=project_dir)
    node: str
    cursor_type: str
    value: str
    for node, cursor_type, value in test_case.injected_observations:
        execute_duckdb(
            db_path=database,
            sql=(
                "INSERT INTO main._sqlbuild_observations VALUES "
                f"('injected-{node}', 'cursor_max', '{node}', '{cursor_type}', '{value}', "
                "'injected-run', '2099-01-01 00:00:00')"
            ),
        )
    misled: subprocess.CompletedProcess[str] = run_sqb(command=("plan",), project_dir=project_dir)
    build: subprocess.CompletedProcess[str] = run_sqb(command=("build",), project_dir=project_dir)
    counts: tuple[tuple[str, int], ...] = tuple(
        (name, query_duckdb(db_path=database, sql=f"SELECT COUNT(*) FROM main.{name}")[0][0])
        for name, _ in test_case.expected_row_counts
    )

    assert honest.returncode == misled.returncode == 0, misled.stderr
    assert normalize_cli_output(misled.stdout) == normalize_cli_output(honest.stdout)
    assert build.returncode == 0, build.stderr
    assert counts == test_case.expected_row_counts
