"""Helpers for MotherDuck CLI e2e tests."""

from __future__ import annotations

from pathlib import Path
from typing import Any

from sqlbuild.adapters.motherduck.classes.motherduck_adapter import MotherDuckAdapter
from tests.integration.src.sqlbuild.adapters.motherduck.helpers import (
    build_motherduck_connection_config,
    build_unique_schema_name,
    fetch_rows,
    qualified_name,
)


def build_motherduck_project_toml(*, project_name: str, schema_name: str) -> str:
    """Build project TOML for an inline MotherDuck e2e project."""

    return (
        f'name = "{project_name}"\n'
        'adapter = "motherduck"\n'
        'default_target = "dev"\n\n'
        "[connection]\n"
        'token = "${ENV:SQB_TEST_MOTHERDUCK_TOKEN}"\n\n'
        "[targets.dev]\n"
        f'schema = "{schema_name}"\n\n'
        "[defaults]\n"
        'materialized = "table"\n'
    )


def prepare_motherduck_build_project(*, tmp_path: Path) -> tuple[Path, str]:
    """Prepare a small MotherDuck-backed SQLBuild project."""

    from tests.e2e.src.sqlbuild.cli.commands.shared.helpers import prepare_inline_project

    schema_name: str = build_unique_schema_name(prefix="sqlbuild_e2e_motherduck")
    project_dir: Path = prepare_inline_project(
        tmp_path=tmp_path,
        project_name="motherduck_build_project",
        repo_files={
            "sqlbuild_project.toml": build_motherduck_project_toml(
                project_name="motherduck_build_project",
                schema_name=schema_name,
            ),
            "sql/models/fact_orders.sql": (
                "MODEL (description 'Test model fact_orders.', materialized table);\n\n"
                "SELECT 1 AS order_id, 'classic' AS waffle_name UNION ALL "
                "SELECT 2 AS order_id, 'liege' AS waffle_name"
            ),
        },
    )
    return project_dir, schema_name


def cleanup_motherduck_schema(*, schema_name: str) -> None:
    """Drop the generated MotherDuck schema after a test completes."""

    adapter: MotherDuckAdapter = MotherDuckAdapter()
    config: dict[str, object] = build_motherduck_connection_config()
    connection: Any = adapter.connect(config)
    try:
        adapter.execute(connection=connection, sql=f"DROP SCHEMA IF EXISTS {schema_name} CASCADE")
    finally:
        adapter.close(connection)


def fetch_motherduck_rows(*, schema_name: str, sql: str) -> tuple[tuple[object, ...], ...]:
    """Fetch rows from MotherDuck using the configured test credentials."""

    adapter: MotherDuckAdapter = MotherDuckAdapter()
    config: dict[str, object] = build_motherduck_connection_config()
    connection: Any = adapter.connect(config)
    try:
        return fetch_rows(adapter=adapter, connection=connection, sql=sql)
    finally:
        adapter.close(connection)


def relation_name(*, schema_name: str, name: str) -> str:
    """Return a qualified MotherDuck relation name."""

    return qualified_name(schema=schema_name, name=name)
