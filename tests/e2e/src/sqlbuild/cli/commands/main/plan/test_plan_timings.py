"""E2E coverage for `sqb plan --timings`."""

from __future__ import annotations

from pathlib import Path

import pytest

from tests.e2e.src.sqlbuild.cli.commands.main.plan._test_types import (
    PlanTimingsE2ETestCase,
    PlanWithoutTimingsE2ETestCase,
)
from tests.e2e.src.sqlbuild.cli.commands.main.plan.helpers import (
    PLAN_TIMINGS_HEADER,
    PlanTimingTable,
    migrated_plan_output,
    plan_timing_table,
)


@pytest.mark.parametrize(
    "test_case",
    [
        PlanTimingsE2ETestCase(
            description="--timings prints each step with Other last, then the total",
            plan_args=("--timings",),
            expected_first_step="Discovering the project",
            expected_last_step="Other",
            expected_steps=(
                "Discovering the project",
                "Compiling the project",
                "Evaluating rules",
                "Connecting to the warehouse",
                "Reading warehouse state",
            ),
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_timings_when_planning_then_the_steps_add_up_to_the_total(
    test_case: PlanTimingsE2ETestCase,
    tmp_path: Path,
) -> None:
    output: str = migrated_plan_output(tmp_path, test_case.plan_args)

    table: PlanTimingTable = plan_timing_table(output)
    assert table.header == ["Plan", "timings", "seconds", "share"], output
    assert len(table.steps) == table.row_count, output
    assert table.steps[0][0] == test_case.expected_first_step, output
    assert table.steps[-1][0] == test_case.expected_last_step, output
    step_names: list[str] = [name for name, _ in table.steps]
    assert list(filter(set(test_case.expected_steps).__contains__, step_names)) == list(
        test_case.expected_steps
    ), output
    assert "Reading build history" not in output, output
    assert sum(seconds for _, seconds in table.steps) == pytest.approx(
        table.total_seconds, abs=0.05 * len(table.steps)
    ), output


@pytest.mark.parametrize(
    "test_case",
    [
        PlanWithoutTimingsE2ETestCase(
            description="a plan without --timings prints no table",
            plan_args=(),
            expected_table_printed=False,
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_no_timings_flag_when_planning_then_no_table_is_printed(
    test_case: PlanWithoutTimingsE2ETestCase,
    tmp_path: Path,
) -> None:
    output: str = migrated_plan_output(tmp_path, test_case.plan_args)

    assert (PLAN_TIMINGS_HEADER in output) == test_case.expected_table_printed, output
