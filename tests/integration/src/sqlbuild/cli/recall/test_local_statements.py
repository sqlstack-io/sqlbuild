"""Statements of tests run locally are recorded by digest, without their SQL text."""

import subprocess
from pathlib import Path

import pytest

from tests.integration.src.sqlbuild.cli.recall._test_types import LocalStatementsTestCase
from tests.integration.src.sqlbuild.cli.recall.helpers import (
    INCOMPLETE_HISTORY,
    recall_rows,
    run,
    write_tested_project,
)

_STATEMENTS: str = (
    "SELECT COUNT(*) > 0, "
    "COALESCE(SUM(json_extract(payload, '$.run_on') = 'local'), 0) > 0, "
    "(SELECT COUNT(*) FROM recall_artifact_refs AS ref WHERE ref.digest IN ("
    "SELECT 'sha256:' || sql_digest FROM recall_events "
    "WHERE json_extract(payload, '$.run_on') = 'local')) "
    "FROM recall_events WHERE event_type IN ('statement_completed', 'statement_failed')"
)


@pytest.mark.parametrize(
    "test_case",
    [
        LocalStatementsTestCase(
            description="a local run records its statements with run_on local and no SQL text",
            args=("test", "--sql", "--run-on", "local"),
            sql=_STATEMENTS,
            expected_rows=[[1, 1, 0]],
        ),
        LocalStatementsTestCase(
            description="a warehouse run records no local statements",
            args=("test", "--sql", "--run-on", "warehouse"),
            sql=_STATEMENTS,
            expected_rows=[[1, 0, 0]],
        ),
        LocalStatementsTestCase(
            description="a warehouse run keeps the SQL text of its statements",
            args=("test", "--sql", "--run-on", "warehouse"),
            sql="SELECT COUNT(*) > 0 FROM recall_artifact_refs",
            expected_rows=[[1]],
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_tests_run_locally_when_recorded_then_their_statements_keep_no_sql_text(
    test_case: LocalStatementsTestCase, tmp_path: Path
) -> None:
    project_dir: Path = write_tested_project(tmp_path / "orders_shop")

    completed: subprocess.CompletedProcess[bytes] = run(project_dir, *test_case.args)
    rows: list[list[object]] = recall_rows(project_dir, test_case.sql)

    stderr: str = completed.stderr.decode(errors="replace")
    assert completed.returncode == 0, stderr
    assert INCOMPLETE_HISTORY.search(stderr) is None, stderr
    assert rows == test_case.expected_rows


if __name__ == "__main__":
    pytest.main([__file__, "-vv"])
