"""Regression coverage for stateless sequential microbatch builds."""

from pathlib import Path
from textwrap import dedent
from typing import Any

import pytest

from sqlbuild.executor.build.models import BuildExecutionResult
from sqlbuild.executor.build.types import BuildStatus
from tests.integration.src.sqlbuild.executor.build.concurrent._test_types import (
    ConcurrentBuildTestCase,
    StatelessSequentialMicrobatchTestCase,
)
from tests.integration.src.sqlbuild.executor.build.concurrent.helpers import (
    RecordingDuckDbAdapter,
    read_order_batch_rows,
    run_concurrent_build,
)


@pytest.mark.parametrize(
    "test_case",
    (
        StatelessSequentialMicrobatchTestCase(
            description="plain watermark",
            model_limit_sql="",
            initial_source_sql=(
                "INSERT INTO raw_events VALUES (1, 0, 'a'), (2, 1, 'b'), (3, 2, 'c')"
            ),
            incremental_source_sql="INSERT INTO raw_events VALUES (4, 3, 'd')",
            expected_initial_batch_count=3,
            expected_repeated_batch_count=1,
            expected_incremental_batch_count=2,
            expected_initial_rows=((1, 0), (2, 1), (3, 2)),
            expected_repeated_rows=((1, 0), (2, 1), (3, 2)),
            expected_incremental_rows=((1, 0), (2, 1), (3, 2), (4, 3)),
        ),
        StatelessSequentialMicrobatchTestCase(
            description="cap from end",
            model_limit_sql="microbatch_limit (max_batches 2, action cap_from_end),",
            initial_source_sql=(
                "INSERT INTO raw_events VALUES "
                "(1, 0, 'a'), (2, 1, 'b'), (3, 2, 'c'), "
                "(4, 3, 'd'), (5, 4, 'e')"
            ),
            incremental_source_sql="INSERT INTO raw_events VALUES (6, 5, 'f')",
            expected_initial_batch_count=2,
            expected_repeated_batch_count=1,
            expected_incremental_batch_count=2,
            expected_initial_rows=((4, 3), (5, 4)),
            expected_repeated_rows=((4, 3), (5, 4)),
            expected_incremental_rows=((4, 3), (5, 4), (6, 5)),
        ),
    ),
    ids=lambda case: case.description,
)
def test_given_sequential_microbatch_when_repeated_then_uses_only_physical_watermarks(
    test_case: StatelessSequentialMicrobatchTestCase,
    tmp_path: Path,
) -> None:
    project_files: dict[str, str] = {
        "sqlbuild_project.toml": dedent(
            """
            name = "stateless_sequential_microbatch"
            adapter = "duckdb"

            [connection]
            database = "test.duckdb"

            [settings]
            concurrency = 2
            microbatch_unaccounted_partition_policy = "synthesize"
            """
        ).strip()
        + "\n",
        "sql/sources/raw.yml": dedent(
            """
            sources:
              - name: raw_events
                description: Test source raw_events.
                schema: main
                table: raw_events
            """
        ).strip()
        + "\n",
        "sql/models/orders.sql": dedent(
            f"""
            MODEL (description "Test model orders.",
              materialized incremental,
              incremental_strategy delete_insert,
              incremental_mode microbatch,
              microbatch_strategy watermark,
              cursor_watermark_mode all,
              cursor batch_id,
              cursor_type integer,
              cursor_start 0,
              cursor_inputs (
                raw_events (column batch_id, roles [filter, watermark]),
              ),
              batch_size "1",
              batch_concurrency 1,
              {test_case.model_limit_sql}
            );

            SELECT id, batch_id, payload
            FROM __source("raw_events")
            WHERE batch_id >= __cursor_start()
              AND batch_id < __cursor_end()
            """
        ).strip()
        + "\n",
    }
    for relative_path, contents in project_files.items():
        destination: Path = tmp_path / relative_path
        destination.parent.mkdir(parents=True, exist_ok=True)
        destination.write_text(contents, encoding="utf-8")

    adapter: RecordingDuckDbAdapter = RecordingDuckDbAdapter()
    db_path: Path = tmp_path / "test.duckdb"
    initial_result: BuildExecutionResult = run_concurrent_build(
        test_case=ConcurrentBuildTestCase(
            description="initial sequential build",
            project_files=project_files,
            max_concurrency=2,
            expected_status=BuildStatus.SUCCESS,
            setup_sql=(
                "CREATE TABLE raw_events (id INTEGER, batch_id INTEGER, payload VARCHAR)",
                test_case.initial_source_sql,
            ),
        ),
        project_dir=tmp_path,
        db_path=db_path,
        adapter=adapter,
    )
    initial_state_statements: tuple[str, ...] = adapter.microbatch_state_statements()
    initial_rows: tuple[tuple[object, ...], ...] = read_order_batch_rows(
        adapter=adapter, db_path=db_path
    )
    adapter.clear_executed_sql()

    repeated_result: BuildExecutionResult = run_concurrent_build(
        test_case=ConcurrentBuildTestCase(
            description="repeated sequential build",
            project_files=project_files,
            max_concurrency=2,
            expected_status=BuildStatus.SUCCESS,
        ),
        project_dir=tmp_path,
        db_path=db_path,
        adapter=adapter,
    )
    repeated_state_statements: tuple[str, ...] = adapter.microbatch_state_statements()
    repeated_rows: tuple[tuple[object, ...], ...] = read_order_batch_rows(
        adapter=adapter, db_path=db_path
    )
    adapter.clear_executed_sql()

    incremental_result: BuildExecutionResult = run_concurrent_build(
        test_case=ConcurrentBuildTestCase(
            description="incremental sequential build",
            project_files=project_files,
            max_concurrency=2,
            expected_status=BuildStatus.SUCCESS,
            setup_sql=(test_case.incremental_source_sql,),
        ),
        project_dir=tmp_path,
        db_path=db_path,
        adapter=adapter,
    )
    incremental_state_statements: tuple[str, ...] = adapter.microbatch_state_statements()

    connection: Any = adapter.connect({"database": str(db_path)})
    try:
        state_table_count: int = connection.execute(
            "SELECT COUNT(*) FROM information_schema.tables "
            "WHERE table_schema = 'main' AND table_name = '_sqlbuild_microbatches'"
        ).fetchone()[0]
    finally:
        adapter.close(connection)

    assert initial_result.status == BuildStatus.SUCCESS
    assert repeated_result.status == BuildStatus.SUCCESS
    assert incremental_result.status == BuildStatus.SUCCESS
    assert initial_result.model_results[0].batch_count == test_case.expected_initial_batch_count
    assert repeated_result.model_results[0].batch_count == test_case.expected_repeated_batch_count
    assert (
        incremental_result.model_results[0].batch_count
        == test_case.expected_incremental_batch_count
    )
    assert len(initial_state_statements) == test_case.expected_state_statement_count
    assert len(repeated_state_statements) == test_case.expected_state_statement_count
    assert len(incremental_state_statements) == test_case.expected_state_statement_count
    assert state_table_count == test_case.expected_state_statement_count
    assert initial_rows == test_case.expected_initial_rows
    assert repeated_rows == test_case.expected_repeated_rows
    assert (
        read_order_batch_rows(adapter=adapter, db_path=db_path)
        == test_case.expected_incremental_rows
    )


if __name__ == "__main__":
    pytest.main([__file__, "-vv"])
