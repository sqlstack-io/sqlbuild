"""A loaded source's declared Postgres types are literal: SQLLoad creates those very columns."""

from __future__ import annotations

import subprocess
from pathlib import Path

import pytest

from tests.e2e.src.sqlbuild.cli.commands.main.load.helpers import (
    build_native_column_project_files,
)
from tests.e2e.src.sqlbuild.cli.commands.main.postgres._test_types import (
    PostgresNativeColumnTypesE2ETestCase,
)
from tests.e2e.src.sqlbuild.cli.commands.main.postgres.helpers import (
    build_postgres_project_toml,
    build_unique_schema_name,
    cleanup_postgres_schema,
    ensure_postgres_schema_ready,
    fetch_postgres_rows,
    relation_name,
)
from tests.e2e.src.sqlbuild.cli.commands.shared.helpers import (
    prepare_inline_project,
    run_sqb,
    stringify_warehouse_rows,
)


@pytest.mark.parametrize(
    "test_case",
    [
        PostgresNativeColumnTypesE2ETestCase(
            description="types SQLLoad has no logical type for are created as declared",
            columns=(("event_id", "BIGINT"), ("amount", "float4"), ("ratio", "numeric")),
            rows="[{'event_id': 1, 'amount': 1.5, 'ratio': 0.25}]",
            expected_types=(("amount", "real"), ("event_id", "bigint"), ("ratio", "numeric")),
            expected_rows=(("1", "1.5", "0.25"),),
        )
    ],
    ids=lambda case: case.description,
)
def test_given_postgres_native_column_types_when_loading_then_the_table_has_those_types(
    tmp_path: Path,
    test_case: PostgresNativeColumnTypesE2ETestCase,
    postgres_e2e_config: dict[str, object],
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
        build_postgres_project_toml(
            project_name="native_column_types",
            schema_name=schema_name,
            config=postgres_e2e_config,
        ),
        encoding="utf-8",
    )
    ensure_postgres_schema_ready(schema_name=schema_name, config=postgres_e2e_config)

    try:
        result: subprocess.CompletedProcess[str] = run_sqb(
            command=("--no-color", "load", "--select", "raw_events"), project_dir=project_dir
        )
        types: tuple[tuple[object, ...], ...] = fetch_postgres_rows(
            config=postgres_e2e_config,
            sql=(
                "SELECT column_name, data_type FROM information_schema.columns "
                f"WHERE table_schema = '{schema_name}' AND table_name = 'raw_events' "
                "AND column_name NOT LIKE '\\_sqlload\\_%' ORDER BY column_name"
            ),
        )
        rows: tuple[tuple[object, ...], ...] = fetch_postgres_rows(
            config=postgres_e2e_config,
            sql=(
                "SELECT event_id, amount, ratio FROM "
                f"{relation_name(schema_name=schema_name, name='raw_events')}"
            ),
        )

        assert result.returncode == 0, result.stdout + result.stderr
        assert stringify_warehouse_rows(types) == test_case.expected_types
        assert stringify_warehouse_rows(rows) == test_case.expected_rows
    finally:
        cleanup_postgres_schema(schema_name=schema_name, config=postgres_e2e_config)


if __name__ == "__main__":
    pytest.main([__file__, "-vv"])
