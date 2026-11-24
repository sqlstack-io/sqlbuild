"""E2E regression test for template expression helpers in config surfaces."""

from __future__ import annotations

from pathlib import Path
from textwrap import dedent

import pytest

from tests.e2e.src.sqlbuild.cli.commands.main.build._test_types import (
    TemplateExpressionsBuildE2ETestCase,
)
from tests.e2e.src.sqlbuild.cli.commands.shared.helpers import (
    prepare_inline_project,
    query_duckdb,
    run_sqb,
    table_exists,
)

_REPO_FILES: dict[str, str] = {
    "sqlbuild_project.toml": dedent(
        """

    name = "template_expressions_project"

    adapter = "duckdb"

    default_target = "dev"



    [connection]

    database = "template_expressions.duckdb"



    [defaults]

    materialized = "incremental"

    incremental_strategy = "append"

    append_cursor_inclusive = "${if(eq(ENV:APPEND_INCLUSIVE, '0'), false, true)}"



    [targets]



    [targets.dev]

    schema = "${coalesce(ENV:TEMPLATE_SCHEMA, 'tpl_${ENV:TEMPLATE_USER}')}"

        """
    ).strip()
    + "\n",
    "sql/sources/raw.yml": dedent(
        """
        sources:
          - name: raw_orders
            description: Test source raw_orders.
            schema: main
            table: raw_orders
        """
    ).strip()
    + "\n",
    "sql/models/orders.sql": dedent(
        """
        MODEL (description "Test model orders.",
          cursor ordered_at,
          cursor_type timestamp,
          cursor_grain second,
          alias '${if(eq(ENV:ALIAS_MODE, "zero"), "orders_zero", if(
            ne(ENV:ALIAS_MODE, "zero"),
            "orders_nonzero",
            "orders_nonzero"
          ))}',
        );

        SELECT id, ordered_at
        FROM __source("raw_orders")
        """
    ).strip()
    + "\n",
}
_INITIAL_SEED_SQL: str = dedent(
    """
    CREATE TABLE main.raw_orders (id INTEGER, ordered_at TIMESTAMP);

    INSERT INTO main.raw_orders VALUES
      (1, '2026-01-01 00:00:00'),
      (2, '2026-01-01 01:00:00');
    """
).strip()
_MUTATION_SQL: tuple[str, ...] = (
    "INSERT INTO main.raw_orders VALUES (3, '2026-01-01 01:00:00')",
    "INSERT INTO main.raw_orders VALUES (4, '2026-01-01 02:00:00')",
)
_EXPECTED_RUNTIME_SQL_FRAGMENT: str = (
    "WHERE ordered_at > TIMESTAMP '2026-01-01 00:00:00' "
    "AND ordered_at < TIMESTAMP '2026-01-01T02:00:01'"
)
_EXPECTED_QUERY_RESULTS: tuple[tuple[str, tuple[tuple[object, ...], ...]], ...] = (
    (
        (
            "SELECT id, CAST(ordered_at AS VARCHAR) "
            "FROM tpl_demo.orders_nonzero ORDER BY id, ordered_at"
        ),
        (
            (2, "2026-01-01 01:00:00"),
            (2, "2026-01-01 01:00:00"),
            (3, "2026-01-01 01:00:00"),
            (4, "2026-01-01 02:00:00"),
        ),
    ),
)


@pytest.mark.parametrize(
    "test_case",
    [
        TemplateExpressionsBuildE2ETestCase(
            description="build uses if eq ne, coalesce and quoted-argument templates in config",
            repo_files=_REPO_FILES,
            initial_seed_sql=_INITIAL_SEED_SQL,
            mutation_sql=_MUTATION_SQL,
            env={"APPEND_INCLUSIVE": "0", "TEMPLATE_USER": "demo", "ALIAS_MODE": "one"},
            command=("--no-color", "build"),
            expected_exit_code=0,
            expected_runtime_sql_fragment=_EXPECTED_RUNTIME_SQL_FRAGMENT,
            expected_query_results=_EXPECTED_QUERY_RESULTS,
        ),
        TemplateExpressionsBuildE2ETestCase(
            description="a set schema variable wins without reading the quoted fallback's variable",
            repo_files=_REPO_FILES,
            initial_seed_sql=_INITIAL_SEED_SQL,
            mutation_sql=_MUTATION_SQL,
            env={"APPEND_INCLUSIVE": "0", "TEMPLATE_SCHEMA": "tpl_demo", "ALIAS_MODE": "one"},
            command=("--no-color", "build"),
            expected_exit_code=0,
            expected_runtime_sql_fragment=_EXPECTED_RUNTIME_SQL_FRAGMENT,
            expected_query_results=_EXPECTED_QUERY_RESULTS,
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_template_expression_project_when_building_then_config_expressions_resolve(
    test_case: TemplateExpressionsBuildE2ETestCase,
    tmp_path: Path,
) -> None:
    project_dir: Path = prepare_inline_project(
        tmp_path=tmp_path,
        project_name="template_expressions_project",
        repo_files=test_case.repo_files,
    )
    db_path: Path = project_dir / "template_expressions.duckdb"

    import duckdb

    connection: duckdb.DuckDBPyConnection = duckdb.connect(str(db_path))
    connection.execute(test_case.initial_seed_sql)
    connection.close()

    first_result: object = run_sqb(
        command=test_case.command,
        project_dir=project_dir,
        env=test_case.env,
    )
    assert first_result.returncode == test_case.expected_exit_code, (
        first_result.stdout + first_result.stderr
    )

    assert table_exists(db_path=db_path, schema="tpl_demo", table_name="orders_nonzero")

    connection = duckdb.connect(str(db_path))
    statement: str
    for statement in test_case.mutation_sql:
        connection.execute(statement)
    connection.close()

    second_result: object = run_sqb(
        command=test_case.command,
        project_dir=project_dir,
        env=test_case.env,
    )
    assert second_result.returncode == test_case.expected_exit_code, (
        second_result.stdout + second_result.stderr
    )

    runtime_sql: str = (project_dir / "target" / "run" / "sql" / "models" / "orders.sql").read_text(
        encoding="utf-8"
    )
    assert test_case.expected_runtime_sql_fragment in runtime_sql

    query: str
    expected_rows: tuple[tuple[object, ...], ...]
    for query, expected_rows in test_case.expected_query_results:
        actual_rows: list[tuple[object, ...]] = query_duckdb(db_path=db_path, sql=query)
        assert tuple(tuple(row) for row in actual_rows) == expected_rows
