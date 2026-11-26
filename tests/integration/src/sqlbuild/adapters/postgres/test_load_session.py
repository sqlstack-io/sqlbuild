from __future__ import annotations

import json
from pathlib import Path
from typing import Any

import pytest

from sqlbuild._native import run_sqlload_project
from sqlbuild.adapter.contract.models import QueryResult
from sqlbuild.adapters.postgres.classes.postgres_adapter import PostgresAdapter
from sqlbuild.executor.load_sessions.classes.load_session import LoadSession
from sqlbuild.executor.load_sessions.main.open_load_session import open_load_session
from tests.integration.src.sqlbuild.adapters.postgres._test_types import (
    PostgresLoadSessionTestCase,
)

_PROJECT: str = """
default_target = "dev"

[connections.landing]
kind = "filesystem"
root = "{root}"

[targets.dev]
connection = "warehouse"
loader_schema = "{schema}"

[sources.orders]
kind = "files"
connection = "landing"
path = "orders/"
format = "csv"
write_strategy = "merge"
unique_key = ["id"]
order = "path"
"""


@pytest.mark.parametrize(
    "test_case",
    [
        PostgresLoadSessionTestCase(
            description="a merge copies each file in through SQLBuild's Postgres connection",
            files=(
                ("1.csv", "id,name\n1,first\n2,second\n"),
                ("2.csv", "id,name\n2,second again\n3,third\n"),
            ),
            expected_rows=((1, "first"), (2, "second again"), (3, "third")),
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_csv_files_when_loading_through_a_postgres_load_session_then_rows_merge(
    test_case: PostgresLoadSessionTestCase,
    tmp_path: Path,
    adapter: PostgresAdapter,
    connection: Any,
    postgres_schema: str,
    postgres_container_config: dict[str, object],
) -> None:
    landing: Path = tmp_path / "orders"
    landing.mkdir()
    project: str = _PROJECT.format(root=tmp_path.as_posix(), schema=postgres_schema)

    def opener() -> LoadSession:
        return open_load_session(
            adapter=adapter, connection_config=postgres_container_config, base_connection=None
        )

    session: dict[str, object] = {
        "adapter": "postgres",
        "database": None,
        "opener": opener,
        "description": "the test database",
    }
    errors: list[str | None] = []
    for file_name, text in test_case.files:
        (landing / file_name).write_text(text)
        tables: list[dict[str, Any]] = json.loads(
            run_sqlload_project(project=project, sessions={"warehouse": session}, select=[])
        )
        errors.extend(table["error"] for table in tables)
    result: QueryResult = adapter.query(
        connection=connection,
        sql=f"SELECT id, name FROM {postgres_schema}.orders ORDER BY id",
        limit=None,
    )

    assert errors == [None] * len(test_case.files), test_case.description
    assert result.rows == test_case.expected_rows, test_case.description
