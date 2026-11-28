"""A loaded source's declared DuckDB types are literal: SQLLoad creates those very columns."""

from __future__ import annotations

import subprocess
from pathlib import Path

import pytest

from tests.e2e.src.sqlbuild.cli.commands.main.load._test_types import (
    NativeColumnTypesE2ETestCase,
)
from tests.e2e.src.sqlbuild.cli.commands.main.load.helpers import (
    build_native_column_project_files,
)
from tests.e2e.src.sqlbuild.cli.commands.shared.helpers import (
    prepare_inline_project,
    query_duckdb,
    run_sqb,
)


@pytest.mark.parametrize(
    "test_case",
    [
        NativeColumnTypesE2ETestCase(
            description="types SQLLoad has no logical type for are created as declared",
            columns=(("event_id", "BIGINT"), ("amount", "UBIGINT"), ("tiny", "USMALLINT")),
            rows="[{'event_id': 1, 'amount': 7, 'tiny': 3}]",
            expected_types=(("amount", "UBIGINT"), ("event_id", "BIGINT"), ("tiny", "USMALLINT")),
            expected_rows=((1, 7, 3),),
        )
    ],
    ids=lambda case: case.description,
)
def test_given_duckdb_native_column_types_when_loading_then_the_table_has_those_types(
    tmp_path: Path, test_case: NativeColumnTypesE2ETestCase
) -> None:
    project_dir: Path = prepare_inline_project(
        tmp_path=tmp_path,
        project_name="source_loader_schema_behavior",
        repo_files=build_native_column_project_files(
            columns=test_case.columns, rows=test_case.rows
        ),
    )

    result: subprocess.CompletedProcess[str] = run_sqb(
        command=("--no-color", "load", "--select", "raw_events"), project_dir=project_dir
    )

    assert result.returncode == 0, result.stdout + result.stderr
    database: Path = project_dir / "source_loader_schema_behavior.duckdb"
    types: list[tuple[object, ...]] = query_duckdb(
        db_path=database,
        sql=(
            "SELECT column_name, data_type FROM information_schema.columns "
            "WHERE table_name = 'raw_events' AND NOT starts_with(column_name, '_sqlload_') "
            "ORDER BY column_name"
        ),
    )
    rows: list[tuple[object, ...]] = query_duckdb(
        db_path=database, sql="SELECT event_id, amount, tiny FROM raw_events"
    )
    assert tuple(types) == test_case.expected_types
    assert tuple(rows) == test_case.expected_rows


if __name__ == "__main__":
    pytest.main([__file__, "-vv"])
