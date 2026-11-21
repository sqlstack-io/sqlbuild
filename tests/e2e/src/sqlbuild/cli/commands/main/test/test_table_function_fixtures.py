"""A `__table_fn__` fixture stands for a partitioned call's whole output, emitted once."""

from __future__ import annotations

import subprocess
from pathlib import Path

import pytest

from tests.e2e.src.sqlbuild.cli.commands.main.test._test_types import (
    PartitionedCallFixtureE2ETestCase,
)
from tests.e2e.src.sqlbuild.cli.commands.main.test.helpers import partitioned_call_fixture_files
from tests.e2e.src.sqlbuild.cli.commands.shared.helpers import prepare_inline_project, run_sqb


@pytest.mark.parametrize(
    "test_case",
    [
        PartitionedCallFixtureE2ETestCase(
            description="three payments of one customer read the one fixture row once",
            call=(
                '__table_fn("smooth_payments")(p.customer_id, p.amount) '
                "OVER (PARTITION BY p.customer_id)"
            ),
            expected_rows="SELECT 1 AS customer_id, 20.0::DOUBLE AS amount_smoothed",
            expected_exit_code=0,
            expected_stdout_fragment="PASS=1",
        ),
        PartitionedCallFixtureE2ETestCase(
            description="a fixture row repeated per input row fails the partitioned call's test",
            call=(
                '__table_fn("smooth_payments")(p.customer_id, p.amount) '
                "OVER (PARTITION BY p.customer_id)"
            ),
            expected_rows=(
                "SELECT 1 AS customer_id, 20.0::DOUBLE AS amount_smoothed "
                "FROM (VALUES (1), (2), (3)) AS v (n)"
            ),
            expected_exit_code=1,
            expected_stdout_fragment="FAIL=1",
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_partitioned_call_fixture_when_testing_then_model_reads_each_fixture_row_once(
    test_case: PartitionedCallFixtureE2ETestCase,
    tmp_path: Path,
) -> None:
    project_dir: Path = prepare_inline_project(
        tmp_path=tmp_path,
        project_name="payments_fixtures",
        repo_files=partitioned_call_fixture_files(
            call=test_case.call, expected_rows=test_case.expected_rows
        ),
    )

    tested: subprocess.CompletedProcess[str] = run_sqb(command=("test",), project_dir=project_dir)

    output: str = tested.stdout + tested.stderr
    assert tested.returncode == test_case.expected_exit_code, (test_case.description, output)
    assert test_case.expected_stdout_fragment in tested.stdout, (test_case.description, output)


if __name__ == "__main__":
    pytest.main([__file__, "-vv"])
