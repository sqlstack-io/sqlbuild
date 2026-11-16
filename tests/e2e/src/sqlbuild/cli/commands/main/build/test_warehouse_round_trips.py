"""E2E coverage for a warehouse round trip: SQL -> task -> loader -> source -> model in one build."""

from __future__ import annotations

import subprocess
from pathlib import Path

import pytest

from tests.e2e.src.sqlbuild.cli.commands.main.build._test_types import (
    ChangedRoundTripE2ETestCase,
    WarehouseRoundTripE2ETestCase,
)
from tests.e2e.src.sqlbuild.cli.commands.main.build.helpers import (
    ALLOW_ROUND_TRIPS,
    FAILING_SCORING_TASK,
    SCORING_TASK,
    build_round_trip_project,
)
from tests.e2e.src.sqlbuild.cli.commands.shared.helpers import (
    query_duckdb,
    run_sqb,
    table_exists,
)


@pytest.mark.parametrize(
    "test_case",
    [
        WarehouseRoundTripE2ETestCase(
            description="one build runs the SQL, the task, the loader and the model in order",
            project_settings=ALLOW_ROUND_TRIPS,
            task_body=SCORING_TASK,
            expected_exit_code=0,
            expected_output_order=(
                "table     orders               OK",
                "task      score_orders OK",
                "source    partner_scores       OK",
                "table     scored_orders        OK",
            ),
            expected_output_fragments=("rows=2",),
            expected_scored_rows=((1, 10), (2, 20)),
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_a_warehouse_round_trip_when_building_then_the_loader_runs_between_its_task_and_readers(
    tmp_path: Path,
    test_case: WarehouseRoundTripE2ETestCase,
) -> None:
    database, output = build_round_trip_project(tmp_path=tmp_path, test_case=test_case)

    assert (
        tuple(
            query_duckdb(
                db_path=database,
                sql="SELECT order_id, score FROM main.scored_orders ORDER BY order_id",
            )
        )
        == test_case.expected_scored_rows
    ), output


@pytest.mark.parametrize(
    "test_case",
    [
        WarehouseRoundTripE2ETestCase(
            description="a failing task skips the loader and the models reading its source",
            project_settings=ALLOW_ROUND_TRIPS,
            task_body=FAILING_SCORING_TASK,
            expected_exit_code=1,
            expected_output_order=(
                "table     orders               OK",
                "task      score_orders FAIL",
                "source    partner_scores       SKIP",
                "table     scored_orders        SKIP",
            ),
            expected_output_fragments=("hard skip: Upstream task 'score_orders' failed",),
            expected_scored_rows=None,
            expected_scored_table_exists=False,
        ),
        WarehouseRoundTripE2ETestCase(
            description="without the setting the build refuses the round trip",
            project_settings="",
            task_body=SCORING_TASK,
            expected_exit_code=1,
            expected_output_order=(),
            expected_output_fragments=(
                "loader 'partner_scores' depends on task 'score_orders', which reads SQL",
                "set `allow_warehouse_round_trips = true` under [settings]",
            ),
            expected_scored_rows=None,
            expected_scored_table_exists=False,
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_a_round_trip_that_cannot_complete_when_building_then_its_readers_are_not_built(
    tmp_path: Path,
    test_case: WarehouseRoundTripE2ETestCase,
) -> None:
    database, output = build_round_trip_project(tmp_path=tmp_path, test_case=test_case)

    assert (
        table_exists(db_path=database, table_name="scored_orders")
        == test_case.expected_scored_table_exists
    ), output


@pytest.mark.parametrize(
    "test_case",
    [
        ChangedRoundTripE2ETestCase(
            description="a changed upstream model carries through the task, loader and source",
            orders_sql=(
                'MODEL (description "Orders to score", materialized table);\n\n'
                "SELECT 1 AS order_id UNION ALL SELECT 2 UNION ALL SELECT 3\n"
            ),
            expected_output_fragments=(
                "task      score_orders OK",
                "source    partner_scores       OK",
                "table     scored_orders        OK",
            ),
            expected_scored_rows=((1, 10), (2, 20), (3, 30)),
        ),
        ChangedRoundTripE2ETestCase(
            description="unchanged SQL leaves the task unrun and the loader softly skipped",
            orders_sql=(
                'MODEL (description "Orders to score", materialized table);\n\n'
                "SELECT 1 AS order_id UNION ALL SELECT 2 AS order_id\n"
            ),
            expected_output_fragments=(
                "soft skip: Upstream task 'score_orders' did not run; the SQL it reads was not "
                "rebuilt",
                "Completed successfully",
            ),
            expected_scored_rows=((1, 10), (2, 20)),
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_a_built_round_trip_when_building_changed_then_the_decision_follows_the_chain(
    tmp_path: Path,
    test_case: ChangedRoundTripE2ETestCase,
) -> None:
    database, _ = build_round_trip_project(
        tmp_path=tmp_path,
        test_case=WarehouseRoundTripE2ETestCase(
            description="first build",
            project_settings=ALLOW_ROUND_TRIPS,
            task_body=SCORING_TASK,
            expected_exit_code=0,
            expected_output_order=(),
            expected_output_fragments=(),
            expected_scored_rows=None,
        ),
    )
    project_dir: Path = database.parent
    (project_dir / "models" / "orders.sql").write_text(test_case.orders_sql, encoding="utf-8")

    result: subprocess.CompletedProcess[str] = run_sqb(
        command=("--no-color", "build", "--changed"), project_dir=project_dir
    )

    output: str = result.stdout + result.stderr
    assert result.returncode == 0, output
    assert all(fragment in output for fragment in test_case.expected_output_fragments), output
    assert (
        tuple(
            query_duckdb(
                db_path=database,
                sql="SELECT order_id, score FROM main.scored_orders ORDER BY order_id",
            )
        )
        == test_case.expected_scored_rows
    ), output
