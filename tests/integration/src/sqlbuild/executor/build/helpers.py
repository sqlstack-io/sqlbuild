"""Test helpers for build executor integration tests."""

from __future__ import annotations

import json
from collections.abc import Callable
from dataclasses import replace
from pathlib import Path
from textwrap import dedent
from typing import Any

import duckdb
import pytest

from sqlbuild._native import (
    CompilePipelineOptions,
    CompilePipelineResult,
    DiscoveredProjectInputs,
    MicrobatchLimitAction,
    PlanOutput,
    TableTypePlanEntry,
    discover_project_inputs,
    run_compile_pipeline,
)
from sqlbuild._native import run_cli as main
from sqlbuild.adapter.contract.types import TablePromotionMode
from sqlbuild.adapters.duckdb.classes.duckdb_adapter import DuckDbAdapter
from sqlbuild.executor.build.classes.build_scheduler import BuildScheduler
from sqlbuild.executor.build.main._execute import execute_build_plan
from sqlbuild.executor.build.models import (
    BuildExecutionResult,
    BuildRuntimeParams,
)
from sqlbuild.executor.scheduling.types import ExecutionStatus
from tests.integration.src.sqlbuild.executor.build._test_types import (
    BuildExecutionTestCase,
)


def capped_dependency_producer_sql(*, action: MicrobatchLimitAction | None) -> str:
    """Return the timestamp capped-producer model used by graph tests."""

    limit_blocks: dict[MicrobatchLimitAction | None, str] = {
        None: "",
        **{
            member: dedent(
                f"""
                microbatch_limit (
                  max_batches 3,
                  action {member.value},
                ),
                """
            )
            for member in MicrobatchLimitAction
        },
    }
    limit_sql: str = limit_blocks[action]
    return dedent(
        f"""
        MODEL (description "Test model.",
          materialized incremental,
          incremental_strategy delete_insert,
          incremental_mode microbatch,
          microbatch_strategy watermark,
          cursor event_time,
          cursor_type timestamp,
          cursor_grain day,
          cursor_start '2026-01-01',
          cursor_end '2026-01-06',
          cursor_watermark_mode all,
          cursor_inputs (
            raw_events (column event_time, roles [filter, watermark]),
          ),
          batch_size 1d,
          lookback 1d,
          {limit_sql}
        );
        SELECT id, event_time
        FROM __source("raw_events")
        """
    )


def capped_dependency_consumer_sql(
    *, watermark_mode: str, input_name: str = "capped_events"
) -> str:
    """Return the timestamp consumer model used by capped graph tests."""

    return dedent(
        f"""
        MODEL (description "Test model.",
          materialized incremental,
          incremental_strategy delete_insert,
          incremental_mode microbatch,
          microbatch_strategy watermark,
          cursor event_time,
          cursor_type timestamp,
          cursor_grain day,
          cursor_start '2026-01-01',
          cursor_watermark_mode {watermark_mode},
          cursor_inputs (
            {input_name} (column event_time, roles [filter, watermark]),
          ),
          batch_size 1d,
          lookback 1d,
        );
        SELECT id, event_time
        FROM __ref("{input_name}")
        """
    )


def capped_filter_consumer_sql(*, input_name: str = "capped_events") -> str:
    """Return a microbatch consumer that uses the capped model only for filtering."""

    return dedent(
        f"""
        MODEL (description "Test model.",
          materialized incremental,
          incremental_strategy delete_insert,
          incremental_mode microbatch,
          microbatch_strategy watermark,
          cursor event_time,
          cursor_type timestamp,
          cursor_grain day,
          cursor_start '2026-01-01',
          cursor_watermark_mode all,
          cursor_inputs (
            {input_name} (column event_time, roles [filter]),
            raw_events (column event_time, roles [watermark]),
          ),
          batch_size 1d,
        );
        SELECT capped.id, capped.event_time
        FROM __ref("{input_name}") AS capped
        JOIN __source("raw_events") AS raw_events USING (id)
        """
    )


def plain_capped_consumer_sql(*, materialized: str) -> str:
    """Return a non-microbatch consumer of a capped model."""

    return dedent(
        f"""
        MODEL (description "Test model.", materialized {materialized});
        SELECT id, event_time FROM __ref("capped_events")
        """
    )


def dependency_view_sql(*, upstream_name: str) -> str:
    """Return a view forwarding one producer relation."""

    return dedent(
        f"""
        MODEL (description "Test model.", materialized view);
        SELECT id, event_time FROM __ref("{upstream_name}")
        """
    )


def capped_microbatch_intermediary_sql(*, input_role: str) -> str:
    """Return a microbatch intermediary with a capped filter or watermark input."""

    cursor_inputs: dict[str, str] = {
        "filter": dedent(
            """
            capped_events (column event_time, roles [filter]),
            raw_events (column event_time, roles [watermark]),
            """
        ),
        "watermark": "capped_events (column event_time, roles [filter, watermark]),",
    }
    queries: dict[str, str] = {
        "filter": dedent(
            """
            SELECT capped.id, capped.event_time
            FROM __ref("capped_events") AS capped
            JOIN __source("raw_events") AS raw_events USING (id)
            """
        ),
        "watermark": 'SELECT id, event_time FROM __ref("capped_events")',
    }
    return dedent(
        f"""
        MODEL (description "Test model.",
          materialized incremental,
          incremental_strategy delete_insert,
          incremental_mode microbatch,
          microbatch_strategy watermark,
          cursor event_time,
          cursor_type timestamp,
          cursor_grain day,
          cursor_start '2026-01-01',
          cursor_watermark_mode all,
          cursor_inputs (
            {cursor_inputs[input_role]}
          ),
          batch_size 1d,
        );
        {queries[input_role]}
        """
    )


def capped_dependency_warehouse_files(*, project_dir: Path) -> dict[str, str]:
    """Create the capped graph's raw source table and return project files that read it."""

    database_path: Path = project_dir / "warehouse.duckdb"
    connection: duckdb.DuckDBPyConnection = duckdb.connect(str(database_path))
    try:
        connection.execute(
            "CREATE TABLE main.raw_events AS "
            "SELECT 1 AS id, TIMESTAMP '2026-01-01 00:00:00' AS event_time"
        )
    finally:
        connection.close()
    return {
        "sqlbuild_project.toml": (
            'name = "capped_dependency"\nadapter = "duckdb"\n\n'
            f"[connection]\ndatabase = {json.dumps(str(database_path))}\n"
        ),
        "sql/sources/raw.yml": (
            "sources:\n  - name: raw_events\n    description: Test source raw_events.\n"
            "    schema: main\n    table: raw_events\n"
        ),
    }


def compile_capped_microbatch_intermediary_project(
    *, project_dir: Path, adapter: DuckDbAdapter, input_role: str
) -> CompilePipelineResult:
    """Compile a capped producer through a microbatch intermediary."""

    write_build_project_files(
        project_dir=project_dir,
        project_files={
            **capped_dependency_warehouse_files(project_dir=project_dir),
            "sql/models/capped_events.sql": capped_dependency_producer_sql(
                action=MicrobatchLimitAction.CAP_FROM_END
            ),
            "sql/models/intermediate_events.sql": capped_microbatch_intermediary_sql(
                input_role=input_role
            ),
            "sql/models/downstream_events.sql": capped_dependency_consumer_sql(
                watermark_mode="all", input_name="intermediate_events"
            ),
        },
    )
    return run_compile_pipeline(
        discovered_inputs=discover_project_inputs(project_dir=project_dir),
        adapter=adapter,
        options=CompilePipelineOptions(no_sql_validation=True),
    )


def compile_capped_dependency_project(
    *,
    project_dir: Path,
    adapter: DuckDbAdapter,
    action: MicrobatchLimitAction | None,
    consumer_sql: str,
    intermediary_names: tuple[str, ...] = (),
) -> CompilePipelineResult:
    """Compile a capped-producer project with optional intermediary views."""

    project_files: dict[str, str] = {
        **capped_dependency_warehouse_files(project_dir=project_dir),
        "sql/models/capped_events.sql": capped_dependency_producer_sql(action=action),
        "sql/models/downstream_events.sql": consumer_sql,
    }
    upstream_name: str = "capped_events"
    for intermediary_name in intermediary_names:
        project_files[f"sql/models/{intermediary_name}.sql"] = dependency_view_sql(
            upstream_name=upstream_name
        )
        upstream_name = intermediary_name
    write_build_project_files(
        project_dir=project_dir,
        project_files=project_files,
    )
    return run_compile_pipeline(
        discovered_inputs=discover_project_inputs(project_dir=project_dir),
        adapter=adapter,
        options=CompilePipelineOptions(no_sql_validation=True),
    )


class SharedConnectionDuckDbAdapter(DuckDbAdapter):
    """Plan against the same in-memory warehouse that the build executes against."""

    def __init__(self, *, connection: Any) -> None:
        super().__init__()
        self._shared_connection: Any = connection

    def connect(self, config: dict[str, Any]) -> Any:
        del config
        return self._shared_connection

    def close(self, connection: Any) -> None:
        del connection


def run_build_for_project(
    *,
    test_case: BuildExecutionTestCase,
    project_dir: Path,
    adapter: DuckDbAdapter,
    connection: Any,
) -> BuildExecutionResult:
    """Discover, compile, plan, then execute a build for a fake project."""

    sql: str
    for sql in test_case.setup_sql:
        connection.execute(sql)

    discovered: DiscoveredProjectInputs = discover_project_inputs(project_dir=project_dir)
    pipeline_result: CompilePipelineResult = run_compile_pipeline(
        discovered_inputs=discovered,
        adapter=SharedConnectionDuckDbAdapter(connection=connection),
        options=CompilePipelineOptions(no_sql_validation=True),
    )

    plan: PlanOutput = pipeline_result.plan_output

    settings_mode: str | None = discovered.project_config.settings.table_promotion_mode
    promotion_mode: TablePromotionMode = TablePromotionMode(
        settings_mode or adapter.default_table_promotion_mode()
    )
    return execute_build_plan(
        plan=plan,
        adapter=adapter,
        connection_config={"database": str(project_dir / "test.duckdb")},
        connections=(connection,),
        scheduler_connection=connection,
        runtime=BuildRuntimeParams(
            promotion_mode=promotion_mode,
            run_id="test_run",
            query_change_tracking=test_case.query_change_tracking,
            snapshots=discovered.project_config.snapshots,
            allow_snapshot_schema_change=test_case.allow_snapshot_schema_change,
            run_audits=test_case.run_audits,
            run_tests=test_case.run_tests,
            fail_fast=test_case.fail_fast,
        ),
    )


def write_build_project_files(*, project_dir: Path, project_files: dict[str, str]) -> None:
    """Write one build integration project's declared files."""

    for relative_path, contents in project_files.items():
        destination: Path = project_dir / relative_path
        destination.parent.mkdir(parents=True, exist_ok=True)
        destination.write_text(contents, encoding="utf-8")


def verify_model_statuses(
    *,
    result: BuildExecutionResult,
    test_case: BuildExecutionTestCase,
) -> None:
    """Assert per-model execution statuses match expected."""

    actual_statuses: dict[str, ExecutionStatus] = {
        r.model_name: r.status for r in result.model_results
    }
    expected_name: str
    expected_status: ExecutionStatus
    for expected_name, expected_status in test_case.expected_model_statuses:
        assert actual_statuses.get(expected_name) == expected_status

    actual_errors: dict[str, str] = {
        r.model_name: r.error_message or "" for r in result.model_results
    }
    expected_fragment: str
    for expected_name, expected_fragment in test_case.expected_model_error_fragments:
        assert expected_fragment in actual_errors.get(expected_name, "")

    actual_error_codes: dict[str, str | None] = {
        r.model_name: r.error_code for r in result.model_results
    }
    expected_code: str
    for expected_name, expected_code in test_case.expected_model_error_codes:
        assert actual_error_codes.get(expected_name) == expected_code


def verify_function_statuses(
    *,
    result: BuildExecutionResult,
    test_case: BuildExecutionTestCase,
) -> None:
    """Assert per-function execution statuses and error fragments match expected."""

    actual_statuses: dict[str, ExecutionStatus] = {
        r.function_name: r.status for r in result.function_results
    }
    expected_name: str
    expected_status: ExecutionStatus
    for expected_name, expected_status in test_case.expected_function_statuses:
        assert actual_statuses.get(expected_name) == expected_status

    actual_errors: dict[str, str] = {
        r.function_name: r.error_message or "" for r in result.function_results
    }
    expected_fragment: str
    for expected_name, expected_fragment in test_case.expected_function_error_fragments:
        assert expected_fragment in actual_errors.get(expected_name, "")

    actual_error_codes: dict[str, str | None] = {
        r.function_name: r.error_code for r in result.function_results
    }
    expected_code: str
    for expected_name, expected_code in test_case.expected_function_error_codes:
        assert actual_error_codes.get(expected_name) == expected_code


def verify_test_counts(
    *,
    result: BuildExecutionResult,
    test_case: BuildExecutionTestCase,
) -> None:
    """Assert test result counts match expected."""

    assert len(result.test_results) == test_case.expected_test_count

    actual_error_codes: dict[str, str | None] = {
        r.test_name: r.error_code for r in result.test_results
    }
    expected_code: str
    for expected_name, expected_code in test_case.expected_test_error_codes:
        assert actual_error_codes.get(expected_name) == expected_code


def verify_audit_counts(
    *,
    result: BuildExecutionResult,
    test_case: BuildExecutionTestCase,
) -> None:
    """Assert audit result counts match expected."""

    total_model_audits: int = sum(len(r.audit_results) for r in result.model_results)
    assert total_model_audits == test_case.expected_model_audit_count
    assert len(result.source_audit_results) == test_case.expected_source_audit_count
    assert len(result.end_audit_results) == test_case.expected_end_audit_count
    assert result.warning_count == test_case.expected_warning_count


def verify_warehouse_state(
    *,
    connection: Any,
    test_case: BuildExecutionTestCase,
) -> None:
    """Assert actual warehouse state matches expected queries and missing relations."""

    query: str
    expected_rows: tuple[tuple[object, ...], ...]
    for query, expected_rows in test_case.expected_query_results:
        cursor: Any = connection.execute(query)
        actual_rows: tuple[tuple[object, ...], ...] = tuple(tuple(row) for row in cursor.fetchall())
        assert actual_rows == expected_rows, (
            f"Query: {query}\nExpected: {expected_rows}\nActual: {actual_rows}"
        )

    relation: str
    for relation in test_case.expected_missing_relations:
        schema: str
        name: str
        schema, _, name = relation.rpartition(".")
        name = name or schema
        schema = schema.removesuffix(name)
        cursor = connection.execute(
            "SELECT 1 FROM information_schema.tables "
            f"WHERE table_name = '{name}'" + f" AND table_schema = '{schema}'" * bool(schema)
        )
        assert cursor.fetchone() is None, f"Relation {relation} should not exist but was found"


def drift_for_models(*, model_names: tuple[str, ...], monkeypatch: pytest.MonkeyPatch) -> None:
    """Hand the build scheduler a plan reporting permanent-desired drift for the named models."""

    original: Callable[..., None] = BuildScheduler.__init__

    def drifted_init(self: BuildScheduler, *, plan: PlanOutput, **kwargs: Any) -> None:
        destinations: dict[str, Any] = {
            entry.name: entry.destination for entry in plan.model_entries
        }
        entries: tuple[TableTypePlanEntry, ...] = tuple(
            TableTypePlanEntry(
                model_name=name,
                destination=destinations[name],
                copy_name=f"__sqb_type_swap__{name}",
                desired_type="permanent",
                actual_type="transient",
                source="target",
                downgrade=False,
                downgrade_policy="require_confirmation",
            )
            for name in model_names
        )
        original(self, plan=replace(plan, table_type_entries=entries), **kwargs)

    monkeypatch.setattr(BuildScheduler, "__init__", drifted_init)


def recording_table_type_conversion(*, conversions: list[tuple[str, int]]) -> Callable[..., None]:
    """Record each converted model with its row count at conversion time."""

    def record(*, entry: TableTypePlanEntry, adapter: object, connection: Any) -> None:
        del adapter
        rows: list[tuple[int]] = connection.execute(
            f"SELECT COUNT(*) FROM {entry.model_name}"
        ).fetchall()
        conversions.append((entry.model_name, rows[0][0]))

    return record


def failing_table_type_conversion(
    *, conversions: list[tuple[str, int]], error: str
) -> Callable[..., None]:
    """Record each attempted conversion, then fail it with the given message."""

    record: Callable[..., None] = recording_table_type_conversion(conversions=conversions)

    def fail(*, entry: TableTypePlanEntry, adapter: object, connection: Any) -> None:
        record(entry=entry, adapter=adapter, connection=connection)
        raise RuntimeError(error)

    return fail


def build_project_json(*, project_dir: Path, capsys: Any) -> tuple[int, dict[str, Any]]:
    """Run one real CLI build with JSON output."""

    exit_code: int = main(["--project-dir", str(project_dir), "build", "--json"])
    return exit_code, json.loads(capsys.readouterr().out)


def duckdb_row_count(*, database: Path, table: str) -> int:
    """Count rows in one table of a DuckDB project database."""

    with duckdb.connect(str(database)) as connection:
        rows: list[tuple[int]] = connection.execute(f"SELECT COUNT(*) FROM {table}").fetchall()
    return rows[0][0]
