"""`sqb compile` refuses a graph every plan refuses, whether or not anything selects its nodes."""

from __future__ import annotations

import subprocess
from pathlib import Path

import pytest

from tests.e2e.src.sqlbuild.cli.commands.main.build.helpers import (
    ALLOW_ROUND_TRIPS,
    SCORING_TASK,
    round_trip_project_files,
    round_trip_refusal,
)
from tests.e2e.src.sqlbuild.cli.commands.main.compile._test_types import (
    GraphBoundaryCompileTestCase,
)
from tests.e2e.src.sqlbuild.cli.commands.shared.helpers import (
    prepare_inline_project,
    run_sqb,
)


@pytest.mark.parametrize(
    "test_case",
    [
        GraphBoundaryCompileTestCase(
            description="an allowed round trip compiles",
            project_settings=ALLOW_ROUND_TRIPS,
            edited_files={},
            expected_exit_code=0,
            expected_output_fragments=(),
        ),
        GraphBoundaryCompileTestCase(
            description="a cycle through a task, models, a source and its loader fails compile",
            project_settings=ALLOW_ROUND_TRIPS,
            edited_files={"python/nodes/tasks/scoring.py": round_trip_refusal("cycle_task.py")},
            expected_exit_code=1,
            expected_output_fragments=(
                "S000",
                "loader 'partner_scores' is in a dependency cycle: loader 'partner_scores' -> "
                "asset 'score_orders' -> model 'scored_orders' -> source 'partner_scores' -> "
                "loader 'partner_scores'",
            ),
        ),
        GraphBoundaryCompileTestCase(
            description="a cycle of model refs alone fails compile with the plan's message",
            project_settings=ALLOW_ROUND_TRIPS,
            edited_files={
                "sql/models/order_totals.sql": round_trip_refusal("order_totals.sql"),
                "sql/models/order_ranks.sql": round_trip_refusal("order_ranks.sql"),
            },
            expected_exit_code=1,
            expected_output_fragments=(
                "error[S000]: Dependency cycle detected involving: "
                "model:order_ranks, model:order_totals",
            ),
        ),
        GraphBoundaryCompileTestCase(
            description="a round trip without the setting fails compile",
            project_settings="",
            edited_files={},
            expected_exit_code=1,
            expected_output_fragments=(
                "loader 'partner_scores' depends on asset 'score_orders', which reads SQL",
                "set `allow_warehouse_round_trips = true` under [settings]",
            ),
        ),
        GraphBoundaryCompileTestCase(
            description="a task depending on a terminal loader instead of its source fails compile",
            project_settings="",
            edited_files={
                "python/nodes/tasks/scoring.py": round_trip_refusal("unread_task.py"),
                "python/nodes/tasks/reporting.py": round_trip_refusal("terminal_loader_reader.py"),
            },
            expected_exit_code=1,
            expected_output_fragments=(
                "Python node 'report_scores' depends on terminal loader 'partner_scores'; "
                "depend on source 'partner_scores' instead",
            ),
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_a_graph_every_plan_refuses_when_compiling_then_compile_fails_with_the_same_error(
    tmp_path: Path,
    test_case: GraphBoundaryCompileTestCase,
) -> None:
    project_dir: Path = prepare_inline_project(
        tmp_path=tmp_path,
        project_name="partner_scoring",
        repo_files=round_trip_project_files(
            project_settings=test_case.project_settings, task_body=SCORING_TASK
        )
        | test_case.edited_files,
    )

    result: subprocess.CompletedProcess[str] = run_sqb(
        command=("--no-color", "compile"), project_dir=project_dir
    )

    output: str = result.stdout + result.stderr
    assert result.returncode == test_case.expected_exit_code, output
    assert all(fragment in output for fragment in test_case.expected_output_fragments), output
