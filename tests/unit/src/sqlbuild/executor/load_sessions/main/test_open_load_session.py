from __future__ import annotations

from pathlib import Path
from typing import Any

import pytest

from sqlbuild._native import resolve_adapter, run_sqlload_project
from sqlbuild.adapter.contract.classes.base_adapter import BaseAdapter
from sqlbuild.adapter.contract.models import QueryResult
from tests.unit.src.sqlbuild.executor.load_sessions.main._test_types import (
    SessionLoadTestCase,
    UnsupportedAdapterTestCase,
)
from tests.unit.src.sqlbuild.executor.load_sessions.main.helpers import load_errors

_PROJECT: str = """
default_target = "dev"

[connections.landing]
kind = "filesystem"
root = "{root}"

[targets.dev]
connection = "warehouse"
loader_schema = "raw"

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
        SessionLoadTestCase(
            description="a merge loads each file through SQLBuild's DuckDB connection",
            files=(
                ("1.csv", "id,name\n1,first\n2,second\n"),
                ("2.csv", "id,name\n2,second again\n3,third\n"),
            ),
            expected_rows=((1, "first"), (2, "second again"), (3, "third")),
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_csv_files_when_loading_through_a_load_session_then_the_table_holds_the_merged_rows(
    test_case: SessionLoadTestCase, tmp_path: Path
) -> None:
    landing: Path = tmp_path / "landing" / "orders"
    landing.mkdir(parents=True)
    adapter: BaseAdapter = resolve_adapter(adapter_name="duckdb")
    connection: Any = adapter.connect({"database": str(tmp_path / "warehouse.duckdb")})
    project: str = _PROJECT.format(root=(tmp_path / "landing").as_posix())
    errors: list[str | None] = []
    for file_name, text in test_case.files:
        (landing / file_name).write_text(text)
        errors.extend(load_errors(project=project, adapter=adapter, base_connection=connection))

    result: QueryResult = adapter.query(
        connection=connection, sql="SELECT id, name FROM raw.orders ORDER BY id", limit=None
    )

    assert errors == [None] * len(test_case.files), test_case.description
    assert result.rows == test_case.expected_rows, test_case.description


@pytest.mark.parametrize(
    "test_case",
    [
        UnsupportedAdapterTestCase(
            description="SQLite can't hold loaded tables yet",
            adapter_name="sqlite",
            expected_error_fragment="loads and loaders can't write to sqlite yet",
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_an_adapter_sqlload_cant_load_into_when_running_then_the_error_names_it(
    test_case: UnsupportedAdapterTestCase, tmp_path: Path
) -> None:
    project: str = _PROJECT.format(root=tmp_path.as_posix())

    with pytest.raises(RuntimeError, match=test_case.expected_error_fragment):
        run_sqlload_project(
            project=project,
            sessions={
                "warehouse": {
                    "adapter": test_case.adapter_name,
                    "database": None,
                    "opener": None,
                    "description": "",
                }
            },
            tables=([], []),
        )
