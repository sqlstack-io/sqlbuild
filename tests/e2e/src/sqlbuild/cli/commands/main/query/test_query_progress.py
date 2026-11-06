"""`sqb query` prints progress only for slow steps, on a terminal, and never on piped stdout."""

from __future__ import annotations

import subprocess
from collections.abc import Callable
from functools import partial
from pathlib import Path

import pytest

from tests.e2e.src.sqlbuild.cli.commands.main.query._test_types import QueryProgressTestCase
from tests.e2e.src.sqlbuild.cli.commands.shared.helpers import (
    prepare_inline_project,
    run_sqb,
    run_sqb_with_pty,
)

_PROJECT_TOML: str = (
    'name = "orders_project"\nadapter = "duckdb"\n\n[connection]\ndatabase = "orders.duckdb"\n'
)
_SLOW_SQL: str = "SELECT COUNT(*) AS orders FROM range(3000000000) AS t(i) WHERE i % 7 = 3"


_RUNNERS: dict[bool, Callable[..., subprocess.CompletedProcess[str]]] = {
    False: run_sqb,
    True: partial(run_sqb_with_pty, timeout_seconds=300.0),
}


@pytest.mark.parametrize(
    "test_case",
    [
        QueryProgressTestCase(
            description="piped output holds only the result",
            sql="SELECT 7 AS order_id",
            use_pty=False,
            expected_fragments=("order_id", "1 row"),
            unexpected_fragments=("Project discovery", "Connecting to", "Running query"),
        ),
        QueryProgressTestCase(
            description="a fast query on a terminal prints only its result",
            sql="SELECT 7 AS order_id",
            use_pty=True,
            expected_fragments=("order_id", "1 row"),
            unexpected_fragments=("Project discovery", "Connecting to", "Running query"),
        ),
        QueryProgressTestCase(
            description="a slow query on a terminal shows a transient line naming it",
            sql=_SLOW_SQL,
            use_pty=True,
            expected_fragments=("Running query...", "1 row"),
            unexpected_fragments=("Project discovery",),
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_query_when_running_then_progress_names_only_slow_steps(
    test_case: QueryProgressTestCase, tmp_path: Path
) -> None:
    project_dir: Path = prepare_inline_project(
        tmp_path=tmp_path,
        project_name="orders_project",
        repo_files={"sqlbuild_project.toml": _PROJECT_TOML},
    )

    completed: subprocess.CompletedProcess[str] = _RUNNERS[test_case.use_pty](
        command=("--no-color", "query", test_case.sql), project_dir=project_dir
    )

    assert completed.returncode == 0, completed.stdout + completed.stderr
    for fragment in test_case.expected_fragments:
        assert fragment in completed.stdout, completed.stdout
    for fragment in test_case.unexpected_fragments:
        assert fragment not in completed.stdout, completed.stdout


if __name__ == "__main__":
    pytest.main([__file__, "-vv"])
