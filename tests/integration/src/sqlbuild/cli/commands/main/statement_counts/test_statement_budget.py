"""Housekeeping statements real CLI commands issue on DuckDB, counted by kind.

Model work is not budgeted; the kinds below are the per-model and per-batch housekeeping that a
remote warehouse pays a round trip for: drops of build intermediates, fingerprint writes, delta
row counts, and column reads.
"""

from __future__ import annotations

from itertools import chain
from pathlib import Path

import pytest

from tests.integration.src.sqlbuild.cli.commands.main.statement_counts._test_types import (
    StatementBudgetTestCase,
    StatementStep,
)
from tests.integration.src.sqlbuild.cli.commands.main.statement_counts.helpers import (
    budgeted,
    recreated_after_drop,
    run_step,
    write_orders_project,
)

_BUDGETED_PREFIXES: tuple[str, ...] = (
    "DROP ",
    "SELECT COUNT ",
    "INFORMATION_SCHEMA ",
    "CREATE TABLE IF NOT EXISTS _sqlbuild_fingerprints",
    "INSERT _sqlbuild_fingerprints",
)
_FIRST_BUILD: dict[str, int] = {
    "CREATE TABLE IF NOT EXISTS _sqlbuild_fingerprints": 1,
    "DROP TABLE delta": 1,
    "DROP TABLE relation": 2,
    "INFORMATION_SCHEMA COLUMNS": 4,
    "INFORMATION_SCHEMA TABLES": 7,
    "INSERT _sqlbuild_fingerprints": 4,
    "SELECT COUNT delta": 1,
}
_UNCHANGED_BUILD: dict[str, int] = {
    "DROP TABLE delta": 2,
    "DROP TABLE relation": 2,
    "DROP TABLE staging": 1,
    "INFORMATION_SCHEMA COLUMNS": 6,
    "INFORMATION_SCHEMA TABLES": 3,
}


@pytest.mark.parametrize(
    "test_case",
    [
        StatementBudgetTestCase(
            description="table, view, merge and microbatch models, then an audit, tests and a scenario",
            steps=(
                StatementStep("first build", ("build",), 0, _FIRST_BUILD),
                StatementStep(
                    "a new order",
                    ("build",),
                    1,
                    _UNCHANGED_BUILD,
                ),
                StatementStep(
                    "nothing changed",
                    ("build",),
                    0,
                    _UNCHANGED_BUILD,
                ),
                StatementStep("audit", ("audit",), 0, {}),
                StatementStep(
                    "unit tests", ("test", "--run-on", "warehouse"), 0, {"DROP TABLE relation": 2}
                ),
                StatementStep(
                    "scenario",
                    ("scenario", "test"),
                    0,
                    {
                        "DROP TABLE relation": 2,
                        "DROP VIEW relation": 1,
                        "INFORMATION_SCHEMA COLUMNS": 1,
                        "INFORMATION_SCHEMA TABLES": 2,
                    },
                ),
            ),
        )
    ],
    ids=lambda case: case.description,
)
def test_given_orders_project_when_running_commands_then_housekeeping_stays_within_budget(
    test_case: StatementBudgetTestCase,
    tmp_path: Path,
    capsys: pytest.CaptureFixture[str],
) -> None:
    project_dir: Path = write_orders_project(project_dir=tmp_path)

    runs: tuple[tuple[int, str, dict[str, int], tuple[str, ...]], ...] = tuple(
        run_step(project_dir=project_dir, args=step.args, new_orders=step.new_orders, capsys=capsys)
        for step in test_case.steps
    )

    assert [
        (step.description, run[0]) for step, run in zip(test_case.steps, runs, strict=True)
    ] == [(step.description, 0) for step in test_case.steps], "".join(run[1] for run in runs)
    assert [budgeted(run[2], _BUDGETED_PREFIXES) for run in runs] == [
        step.expected_counts for step in test_case.steps
    ]
    assert (
        tuple(chain.from_iterable(recreated_after_drop(run[3]) for run in runs))
        == test_case.expected_recreated_after_drop
    )


if __name__ == "__main__":
    pytest.main([__file__, "-vv"])
