"""E2E regression tests for append cursor rerun behavior."""

from __future__ import annotations

from pathlib import Path
from textwrap import dedent

import pytest

from tests.e2e.src.sqlbuild.cli.commands.main.build._test_types import (
    AppendCursorBuildE2ETestCase,
    TimestampCursorBuildOverrideE2ETestCase,
    UntypedCursorOverrideSelectionE2ETestCase,
)
from tests.e2e.src.sqlbuild.cli.commands.shared.helpers import (
    prepare_inline_project,
    query_duckdb,
    run_sqb,
)

_MIXED_CURSOR_PROJECT_FILES: dict[str, str] = {
    "sqlbuild_project.toml": dedent(
        """
        name = "mixed_cursor_project"
        adapter = "duckdb"

        [connection]
        database = "mixed_cursor.duckdb"
        """
    ).strip()
    + "\n",
    "sql/seeds/raw_orders.csv": "id,ordered_at\n1,2026-01-01 00:00:00\n2,2026-01-01 01:00:00\n",
    "sql/seeds/raw_orders.yml": dedent(
        """
        seeds:
          - name: raw_orders
            description: Raw orders.
            columns:
              - name: id
                type: INTEGER
              - name: ordered_at
                type: TIMESTAMP
        """
    ).strip()
    + "\n",
    "sql/models/orders.sql": dedent(
        """
        MODEL (description "Orders by timestamp cursor.",
          materialized incremental,
          incremental_strategy append,
          cursor ordered_at,
          cursor_type timestamp,
          cursor_grain second,
        );

        SELECT id, ordered_at
        FROM __seed("raw_orders")
        """
    ).strip()
    + "\n",
    "sql/models/order_ids.sql": dedent(
        """
        MODEL (description "Orders by integer cursor.",
          materialized incremental,
          incremental_strategy append,
          cursor id,
          cursor_type integer,
        );

        SELECT id FROM __seed("raw_orders")
        """
    ).strip()
    + "\n",
}


@pytest.mark.parametrize(
    "test_case",
    [
        AppendCursorBuildE2ETestCase(
            description=(
                "default inclusive append cursor rerun duplicates boundary row "
                "and captures late same-timestamp row"
            ),
            repo_files={
                "sqlbuild_project.toml": dedent(
                    """

                name = "append_cursor_project"

                adapter = "duckdb"



                [connection]

                database = "append_cursor.duckdb"



                [defaults]

                materialized = "table"

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
                      materialized incremental,
                      incremental_strategy append,
                      cursor ordered_at,
                      cursor_type timestamp,
                      cursor_grain second,
                    );

                    SELECT id, ordered_at
                    FROM __source("raw_orders")
                    """
                ).strip()
                + "\n",
            },
            initial_seed_sql=dedent(
                """
                CREATE TABLE main.raw_orders (id INTEGER, ordered_at TIMESTAMP);

                INSERT INTO main.raw_orders VALUES
                  (1, '2026-01-01 00:00:00'),
                  (2, '2026-01-01 01:00:00');
                """
            ).strip(),
            mutation_sql=(
                "INSERT INTO main.raw_orders VALUES (3, '2026-01-01 01:00:00')",
                "INSERT INTO main.raw_orders VALUES (4, '2026-01-01 02:00:00')",
            ),
            command=("--no-color", "build"),
            expected_exit_code=0,
            expected_runtime_sql_fragment=(
                "WHERE ordered_at >= TIMESTAMP '2026-01-01 01:00:00' "
                "AND ordered_at < TIMESTAMP '2026-01-01T02:00:01'"
            ),
            expected_query_results=(
                (
                    (
                        "SELECT id, CAST(ordered_at AS VARCHAR) "
                        "FROM main.orders ORDER BY id, ordered_at"
                    ),
                    (
                        (1, "2026-01-01 00:00:00"),
                        (2, "2026-01-01 01:00:00"),
                        (2, "2026-01-01 01:00:00"),
                        (3, "2026-01-01 01:00:00"),
                        (4, "2026-01-01 02:00:00"),
                    ),
                ),
            ),
        )
    ],
    ids=lambda case: case.description,
)
def test_given_append_cursor_project_when_rerunning_build_then_boundary_behavior_matches_expected(
    test_case: AppendCursorBuildE2ETestCase,
    tmp_path: Path,
) -> None:
    project_dir: Path = prepare_inline_project(
        tmp_path=tmp_path,
        project_name="append_cursor_project",
        repo_files=test_case.repo_files,
    )
    db_path: Path = project_dir / "append_cursor.duckdb"

    import duckdb

    connection: duckdb.DuckDBPyConnection = duckdb.connect(str(db_path))
    connection.execute(test_case.initial_seed_sql)
    connection.close()

    first_result: object = run_sqb(command=test_case.command, project_dir=project_dir)
    assert first_result.returncode == test_case.expected_exit_code, (
        first_result.stdout + first_result.stderr
    )

    connection = duckdb.connect(str(db_path))
    statement: str
    for statement in test_case.mutation_sql:
        connection.execute(statement)
    connection.close()

    second_result: object = run_sqb(command=test_case.command, project_dir=project_dir)
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


@pytest.mark.parametrize(
    "test_case",
    [
        TimestampCursorBuildOverrideE2ETestCase(
            description="build honors timestamp cursor start override",
            repo_files={
                "sqlbuild_project.toml": dedent(
                    """
                    name = "timestamp_cursor_override_project"
                    adapter = "duckdb"

                    [connection]
                    database = "timestamp_cursor_override.duckdb"

                    [defaults]
                    materialized = "table"
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
                      materialized incremental,
                      incremental_strategy append,
                      cursor ordered_at,
                      cursor_type timestamp,
                      cursor_grain second,
                    );

                    SELECT id, ordered_at
                    FROM __source("raw_orders")
                    """
                ).strip()
                + "\n",
            },
            initial_seed_sql=dedent(
                """
                CREATE TABLE main.raw_orders (id INTEGER, ordered_at TIMESTAMP);

                INSERT INTO main.raw_orders VALUES
                  (1, '2026-01-01 00:00:00'),
                  (2, '2026-01-01 01:00:00'),
                  (3, '2026-01-01 02:00:00');
                """
            ).strip(),
            command=(
                "--no-color",
                "build",
                "--start-cursor-ts",
                "2026-01-01T01:00:00",
                "--end-cursor-ts",
                "2026-01-01T03:00:00",
            ),
            expected_exit_code=0,
            expected_runtime_sql_fragment=(
                "WHERE ordered_at >= TIMESTAMP '2026-01-01T01:00:00' "
                "AND ordered_at < TIMESTAMP '2026-01-01T03:00:01'"
            ),
            expected_query_results=(
                (
                    ("SELECT id, CAST(ordered_at AS VARCHAR) FROM main.orders ORDER BY id"),
                    (
                        (2, "2026-01-01 01:00:00"),
                        (3, "2026-01-01 02:00:00"),
                    ),
                ),
            ),
        ),
        TimestampCursorBuildOverrideE2ETestCase(
            description="build types untyped cursor flags from the selected timestamp model",
            repo_files={
                "sqlbuild_project.toml": dedent(
                    """
                    name = "timestamp_cursor_override_project"
                    adapter = "duckdb"

                    [connection]
                    database = "timestamp_cursor_override.duckdb"

                    [defaults]
                    materialized = "table"
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
                      materialized incremental,
                      incremental_strategy append,
                      cursor ordered_at,
                      cursor_type timestamp,
                      cursor_grain second,
                    );

                    SELECT id, ordered_at
                    FROM __source("raw_orders")
                    """
                ).strip()
                + "\n",
            },
            initial_seed_sql=dedent(
                """
                CREATE TABLE main.raw_orders (id INTEGER, ordered_at TIMESTAMP);

                INSERT INTO main.raw_orders VALUES
                  (1, '2026-01-01 00:00:00'),
                  (2, '2026-01-01 01:00:00'),
                  (3, '2026-01-01 02:00:00');
                """
            ).strip(),
            command=(
                "--no-color",
                "build",
                "--start-cursor",
                "2026-01-01T01:00:00",
                "--end-cursor",
                "2026-01-01T03:00:00",
            ),
            expected_exit_code=0,
            expected_runtime_sql_fragment=(
                "WHERE ordered_at >= TIMESTAMP '2026-01-01T01:00:00' "
                "AND ordered_at < TIMESTAMP '2026-01-01T03:00:01'"
            ),
            expected_query_results=(
                (
                    ("SELECT id, CAST(ordered_at AS VARCHAR) FROM main.orders ORDER BY id"),
                    (
                        (2, "2026-01-01 01:00:00"),
                        (3, "2026-01-01 02:00:00"),
                    ),
                ),
            ),
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_timestamp_cursor_start_override_when_building_then_lower_bound_is_applied(
    test_case: TimestampCursorBuildOverrideE2ETestCase,
    tmp_path: Path,
) -> None:
    project_dir: Path = prepare_inline_project(
        tmp_path=tmp_path,
        project_name="timestamp_cursor_override_project",
        repo_files=test_case.repo_files,
    )
    db_path: Path = project_dir / "timestamp_cursor_override.duckdb"

    import duckdb

    connection: duckdb.DuckDBPyConnection = duckdb.connect(str(db_path))
    connection.execute(test_case.initial_seed_sql)
    connection.close()

    result: object = run_sqb(command=test_case.command, project_dir=project_dir)
    assert result.returncode == test_case.expected_exit_code, result.stdout + result.stderr

    runtime_sql: str = (project_dir / "target" / "run" / "sql" / "models" / "orders.sql").read_text(
        encoding="utf-8"
    )
    assert test_case.expected_runtime_sql_fragment in runtime_sql

    query: str
    expected_rows: tuple[tuple[object, ...], ...]
    for query, expected_rows in test_case.expected_query_results:
        actual_rows: list[tuple[object, ...]] = query_duckdb(db_path=db_path, sql=query)
        assert tuple(tuple(row) for row in actual_rows) == expected_rows


@pytest.mark.parametrize(
    "test_case",
    [
        UntypedCursorOverrideSelectionE2ETestCase(
            description="selection mixing cursor types is rejected",
            command=("--no-color", "plan", "--start-cursor", "1"),
            expected_exit_code=1,
            expected_output_fragments=(
                "S303",
                "--start-cursor and --end-cursor need one cursor type, but the selection mixes "
                "integer cursor models (order_ids) and timestamp cursor models (orders)",
            ),
        ),
        UntypedCursorOverrideSelectionE2ETestCase(
            description="positional selection of one cursor type types the flags",
            command=("--no-color", "plan", "+order_ids", "--start-cursor", "1"),
            expected_exit_code=0,
            expected_output_fragments=("order_ids",),
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_untyped_cursor_flags_when_planning_selection_then_types_must_agree(
    test_case: UntypedCursorOverrideSelectionE2ETestCase,
    tmp_path: Path,
) -> None:
    project_dir: Path = prepare_inline_project(
        tmp_path=tmp_path,
        project_name="mixed_cursor_project",
        repo_files=_MIXED_CURSOR_PROJECT_FILES,
    )

    result: object = run_sqb(command=test_case.command, project_dir=project_dir)

    assert result.returncode == test_case.expected_exit_code, result.stdout + result.stderr
    fragment: str
    for fragment in test_case.expected_output_fragments:
        assert fragment in result.stdout + result.stderr
