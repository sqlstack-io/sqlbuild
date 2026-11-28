"""A loaded source's declared ClickHouse types are literal: SQLLoad creates those very columns."""

from __future__ import annotations

import subprocess
from pathlib import Path

import pytest

from tests.e2e.src.sqlbuild.cli.commands.main.clickhouse._test_types import (
    ClickHouseNativeColumnTypesE2ETestCase,
)
from tests.e2e.src.sqlbuild.cli.commands.main.clickhouse.helpers import (
    build_clickhouse_project_toml,
    build_unique_schema_name,
    cleanup_clickhouse_schema,
    create_clickhouse_schema,
    fetch_clickhouse_rows,
)
from tests.e2e.src.sqlbuild.cli.commands.main.load.helpers import (
    build_native_column_project_files,
)
from tests.e2e.src.sqlbuild.cli.commands.shared.helpers import (
    prepare_inline_project,
    run_sqb,
    stringify_warehouse_rows,
)


@pytest.mark.parametrize(
    "test_case",
    [
        ClickHouseNativeColumnTypesE2ETestCase(
            description="types SQLLoad has no logical type for are created as declared",
            columns=(
                ("event_id", "BIGINT"),
                ("amount", "UInt32"),
                ("status", "LowCardinality(String)"),
            ),
            rows="[{'event_id': 1, 'amount': 7, 'status': 'loaded'}]",
            expected_types=(
                ("amount", "UInt32"),
                ("event_id", "Nullable(Int64)"),
                ("status", "LowCardinality(String)"),
            ),
            expected_rows=(("1", "7", "loaded"),),
        )
    ],
    ids=lambda case: case.description,
)
def test_given_clickhouse_native_column_types_when_loading_then_the_table_has_those_types(
    tmp_path: Path,
    test_case: ClickHouseNativeColumnTypesE2ETestCase,
    clickhouse_e2e_config: dict[str, object],
) -> None:
    schema_name: str = build_unique_schema_name(prefix="sqb_native_types")
    project_dir: Path = prepare_inline_project(
        tmp_path=tmp_path,
        project_name="native_column_types",
        repo_files=build_native_column_project_files(
            columns=test_case.columns, rows=test_case.rows
        ),
    )
    (project_dir / "sqlbuild_project.toml").write_text(
        build_clickhouse_project_toml(
            project_name="native_column_types",
            schema_name=schema_name,
            config=clickhouse_e2e_config,
        ),
        encoding="utf-8",
    )
    create_clickhouse_schema(config=clickhouse_e2e_config, schema_name=schema_name)

    try:
        result: subprocess.CompletedProcess[str] = run_sqb(
            command=("--no-color", "load", "--select", "raw_events"), project_dir=project_dir
        )
        types: tuple[tuple[object, ...], ...] = fetch_clickhouse_rows(
            config=clickhouse_e2e_config,
            sql=(
                "SELECT name, type FROM system.columns "
                f"WHERE database = '{schema_name}' AND table = 'raw_events' "
                "AND NOT startsWith(name, '_sqlload_') ORDER BY name"
            ),
        )
        rows: tuple[tuple[object, ...], ...] = fetch_clickhouse_rows(
            config=clickhouse_e2e_config,
            sql=f"SELECT event_id, amount, status FROM `{schema_name}`.`raw_events`",
        )

        assert result.returncode == 0, result.stdout + result.stderr
        assert stringify_warehouse_rows(types) == test_case.expected_types
        assert stringify_warehouse_rows(rows) == test_case.expected_rows
    finally:
        cleanup_clickhouse_schema(config=clickhouse_e2e_config, schema_name=schema_name)


if __name__ == "__main__":
    pytest.main([__file__, "-vv"])
