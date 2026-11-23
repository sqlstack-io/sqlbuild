"""Advanced integration tests for custom materializations."""

from __future__ import annotations

from collections.abc import Callable
from pathlib import Path
from typing import Any

import duckdb
import pytest

from sqlbuild._native import AuditPlanEntry, CompiledRelationLocation, ModelPlanEntry, PlanReason
from sqlbuild.adapter.contract.models import RelationInfo
from sqlbuild.adapter.contract.types import LifeCycleEventKind
from sqlbuild.adapters.duckdb.classes.duckdb_adapter import DuckDbAdapter
from sqlbuild.executor.build.models import BuildExecutionResult
from sqlbuild.executor.build.types import BuildStatus
from sqlbuild.executor.custom.models import MaterializationContext, MaterializationResult
from sqlbuild.executor.run.models import ModelExecutionResult
from sqlbuild.executor.run.types import ExecutionPhase
from sqlbuild.executor.scheduling.types import ExecutionStatus
from tests.integration.src.sqlbuild.executor.build.custom._test_types import (
    ExistingRelationTestCase,
    PartitionTrackingTestCase,
    PlaceholderExecutionTestCase,
    PrePromotionAuditTestCase,
    SchedulerRoutingTestCase,
)
from tests.integration.src.sqlbuild.executor.build.custom.helpers import (
    build_custom_plan_entry,
    build_existing_relation_capture_fn,
    build_failing_audit,
    build_placeholder_execution_fn,
    build_user_audit_fn,
    row_count,
    run_custom_entry,
    run_scheduler_build,
)

_PROJECT_YML: str = (
    'name = "demo"\n'
    'adapter = "duckdb"\n\n'
    "[connection]\n"
    'database = "test.duckdb"\n\n'
    "[settings]\n"
    'default_audit_severity = "error"\n'
)


# --- Partition tracking tests ---


@pytest.mark.parametrize(
    "test_case",
    [
        PartitionTrackingTestCase(
            description="first run builds all partitions and populates tracking table",
            setup_sql=(
                "CREATE TABLE main.raw_events (event_day VARCHAR, event_id INT)",
                "INSERT INTO main.raw_events VALUES "
                "('2024-01-01', 1), ('2024-01-02', 2), ('2024-01-03', 3)",
            ),
            expected_target_row_count=3,
            expected_tracking_row_count=3,
            expected_partition_values=("2024-01-01", "2024-01-02", "2024-01-03"),
            expected_statement_fragments=(
                "CREATE TABLE IF NOT EXISTS main.partition_state",
                "INSERT INTO main.partition_state VALUES",
            ),
            expected_log_fragments=(
                "checking partition state",
                "building initial partition range",
            ),
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_partition_tracked_materialization_when_first_run_then_builds_all_partitions(
    test_case: PartitionTrackingTestCase,
    tmp_path: Path,
) -> None:
    adapter: DuckDbAdapter = DuckDbAdapter()
    connection: duckdb.DuckDBPyConnection = duckdb.connect(":memory:")
    sql_stmt: str
    for sql_stmt in test_case.setup_sql:
        connection.execute(sql_stmt)

    entry: ModelPlanEntry = build_custom_plan_entry(
        sql=(
            "SELECT event_day, event_id FROM main.raw_events "
            "WHERE event_day >= @@@partition_start AND event_day < @@@partition_end"
        ),
        reason=PlanReason.FIRST_RUN,
        custom_config={
            "tracking_table": "main.partition_state",
            "partition_column": "event_day",
        },
        custom_placeholders={
            "partition_start": "'2024-01-01'",
            "partition_end": "'2024-01-04'",
        },
    )

    from tests.integration.src.sqlbuild.executor.build.custom.helpers import (
        build_partition_tracking_fn,
    )

    result: ModelExecutionResult = run_custom_entry(
        adapter=adapter,
        connection=connection,
        entry=entry,
        materialize_fn=build_partition_tracking_fn(),
    )

    assert result.status == ExecutionStatus.SUCCESS
    assert (
        row_count(connection, qualified_name="main.test_model")
        == test_case.expected_target_row_count
    )
    assert (
        row_count(connection, qualified_name="main.partition_state")
        == test_case.expected_tracking_row_count
    )
    cursor: Any = connection.execute(
        "SELECT partition_value FROM main.partition_state ORDER BY partition_value"
    )
    actual_partitions: tuple[str, ...] = tuple(row[0] for row in cursor.fetchall())
    assert actual_partitions == test_case.expected_partition_values

    fragment: str
    for fragment in test_case.expected_statement_fragments:
        assert any(
            event.kind == LifeCycleEventKind.SQL and fragment in event.content
            for event in result.lifecycle_events
        ), f"expected '{fragment}' in lifecycle_events"

    log_fragment: str
    for log_fragment in test_case.expected_log_fragments:
        assert any(
            event.kind == LifeCycleEventKind.LOG and log_fragment in event.content
            for event in result.lifecycle_events
        ), f"expected '{log_fragment}' in lifecycle_events"


# --- Existing relation detection tests ---


@pytest.mark.parametrize(
    "test_case",
    [
        ExistingRelationTestCase(
            description="first run with no existing target passes None",
            expected_row_count=1,
            expected_existing_was_none=True,
            existing_relation=None,
        ),
        ExistingRelationTestCase(
            description="subsequent run with existing target passes RelationInfo",
            setup_sql=(
                "CREATE TABLE main.test_model (old_col INT)",
                "INSERT INTO main.test_model VALUES (999)",
            ),
            existing_relation=RelationInfo(
                database=None,
                schema="main",
                name="test_model",
                relation_type="BASE TABLE",
            ),
            expected_row_count=1,
            expected_existing_was_none=False,
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_custom_materialization_when_target_state_varies_then_existing_relation_correct(
    test_case: ExistingRelationTestCase,
) -> None:
    adapter: DuckDbAdapter = DuckDbAdapter()
    connection: duckdb.DuckDBPyConnection = duckdb.connect(":memory:")
    sql_stmt: str
    for sql_stmt in test_case.setup_sql:
        connection.execute(sql_stmt)

    existing: RelationInfo | None = test_case.existing_relation

    entry: ModelPlanEntry = build_custom_plan_entry(sql="SELECT 1 AS id")
    captured: dict[str, Any] = {}

    result: ModelExecutionResult = run_custom_entry(
        adapter=adapter,
        connection=connection,
        entry=entry,
        materialize_fn=build_existing_relation_capture_fn(captured),
        existing_relation=existing,
    )

    assert result.status == ExecutionStatus.SUCCESS
    assert (captured["existing_relation"] is None) == test_case.expected_existing_was_none
    assert captured["is_first_run"] == test_case.expected_existing_was_none
    assert row_count(connection, qualified_name="main.test_model") == test_case.expected_row_count


# --- Placeholder execution tests ---


@pytest.mark.parametrize(
    "test_case",
    [
        PlaceholderExecutionTestCase(
            description="@@@placeholders substituted and SQL executes against database",
            model_sql=(
                "SELECT * FROM (VALUES (1, 'a'), (2, 'b'), (3, 'c')) AS t(id, name) "
                "WHERE id >= @@@start_id AND id < @@@end_id"
            ),
            placeholders={"start_id": "1", "end_id": "4"},
            substitutions={"start_id": "2", "end_id": "4"},
            expected_row_count=2,
        ),
        PlaceholderExecutionTestCase(
            description="@@@placeholder with string values and quoting",
            model_sql=(
                "SELECT * FROM (VALUES ('alice', 10), ('bob', 20), ('charlie', 30)) AS t(name, score) "
                "WHERE name = @@@target_name"
            ),
            placeholders={"target_name": "'alice'"},
            substitutions={"target_name": "'bob'"},
            expected_row_count=1,
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_custom_materialization_with_placeholders_when_substituted_then_executes(
    test_case: PlaceholderExecutionTestCase,
) -> None:
    adapter: DuckDbAdapter = DuckDbAdapter()
    connection: duckdb.DuckDBPyConnection = duckdb.connect(":memory:")
    entry: ModelPlanEntry = build_custom_plan_entry(
        sql=test_case.model_sql,
        custom_placeholders=test_case.placeholders,
    )

    result: ModelExecutionResult = run_custom_entry(
        adapter=adapter,
        connection=connection,
        entry=entry,
        materialize_fn=build_placeholder_execution_fn(test_case.substitutions),
    )

    assert result.status == ExecutionStatus.SUCCESS
    assert row_count(connection, qualified_name="main.test_model") == test_case.expected_row_count


# --- Pre-promotion audit with real data tests ---


@pytest.mark.parametrize(
    "test_case",
    [
        PrePromotionAuditTestCase(
            description="user audit against staging finds real rows and blocks promotion",
            expected_status=ExecutionStatus.FAILED,
            expected_failed_phase=ExecutionPhase.CUSTOM_MATERIALIZATION,
            expected_min_audit_row_count=1,
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_custom_materialization_when_audit_finds_rows_in_staging_then_blocks(
    test_case: PrePromotionAuditTestCase,
) -> None:
    adapter: DuckDbAdapter = DuckDbAdapter()
    connection: duckdb.DuckDBPyConnection = duckdb.connect(":memory:")
    entry: ModelPlanEntry = build_custom_plan_entry(sql="SELECT 1 AS id, 'bad_data' AS status")
    model_locations: dict[str, CompiledRelationLocation] = {"test_model": entry.destination}
    audit: AuditPlanEntry = build_failing_audit(name="check_rows", target_name="test_model")

    result: ModelExecutionResult = run_custom_entry(
        adapter=adapter,
        connection=connection,
        entry=entry,
        materialize_fn=build_user_audit_fn(expect_pass=False),
        model_audits=(audit,),
        model_locations=model_locations,
    )

    assert result.status == test_case.expected_status
    assert result.failed_phase == test_case.expected_failed_phase
    assert len(result.audit_results) == 1
    assert result.audit_results[0].row_count >= test_case.expected_min_audit_row_count


# --- Scheduler routing tests ---


@pytest.mark.parametrize(
    "test_case",
    [
        SchedulerRoutingTestCase(
            description="custom materialization dispatched through scheduler alongside regular table",
            project_files={
                "sqlbuild_project.toml": _PROJECT_YML,
                "sql/models/regular.sql": (
                    "MODEL (description 'Test model regular.', materialized table);\n\nSELECT 1 AS id, 'regular' AS name"
                ),
                "sql/models/custom_model.sql": (
                    "MODEL (description 'Test model custom_model.', materialized test_custom);\n\nSELECT 2 AS id, 'custom' AS name"
                ),
            },
            expected_status=BuildStatus.SUCCESS,
            expected_success_count=2,
            expected_query_results=(
                ("SELECT id, name FROM main.regular", ((1, "regular"),)),
                (
                    "SELECT id, name, custom_marker FROM main.custom_model",
                    ((2, "custom", "via_custom_fn"),),
                ),
            ),
        ),
        SchedulerRoutingTestCase(
            description="custom materialization downstream of regular table receives correct data",
            project_files={
                "sqlbuild_project.toml": _PROJECT_YML,
                "sql/models/upstream.sql": (
                    "MODEL (description 'Test model upstream.', materialized table);\n\nSELECT 10 AS id, 'upstream' AS origin"
                ),
                "sql/models/downstream_custom.sql": (
                    'MODEL (description "Test model downstream_custom.", materialized test_custom);\n\nSELECT id, origin FROM __ref("upstream")'
                ),
            },
            expected_status=BuildStatus.SUCCESS,
            expected_success_count=2,
            expected_query_results=(
                (
                    "SELECT id, origin, custom_marker FROM main.downstream_custom",
                    ((10, "upstream", "via_custom_fn"),),
                ),
            ),
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_build_with_custom_materialization_when_scheduled_then_routes_correctly(
    test_case: SchedulerRoutingTestCase,
    tmp_path: Path,
) -> None:
    adapter: DuckDbAdapter = DuckDbAdapter()
    project_dir: Path = tmp_path / "project"
    project_dir.mkdir(parents=True)
    db_path: Path = tmp_path / "test.duckdb"

    relative_path: str
    contents: str
    for relative_path, contents in test_case.project_files.items():
        file_path: Path = project_dir / relative_path
        file_path.parent.mkdir(parents=True, exist_ok=True)
        file_path.write_text(contents, encoding="utf-8")

    # Also create the python/materializations/ directory with test_custom.py
    mat_dir: Path = project_dir / "python" / "materializations"
    mat_dir.mkdir(parents=True)
    (mat_dir / "test_custom.py").write_text(
        "def materialize(ctx):\n"
        "    ctx.adapter.create_table_as(\n"
        "        ctx.connection, destination=ctx.destination, sql=ctx.sql,\n"
        "        statement_recorder=ctx.statement_recorder,\n"
        "    )\n"
        "    from sqlbuild.executor.custom.models import MaterializationResult\n"
        "    return MaterializationResult(relation=ctx.destination)\n",
        encoding="utf-8",
    )

    def custom_fn(ctx: MaterializationContext) -> MaterializationResult:
        augmented_sql: str = f"SELECT *, 'via_custom_fn' AS custom_marker FROM ({ctx.sql}) sub"
        ctx.adapter.create_table_as(
            connection=ctx.connection,
            destination=ctx.destination,
            sql=augmented_sql,
            statement_recorder=ctx.statement_recorder,
        )
        return MaterializationResult(relation=ctx.destination)

    custom_materializations: dict[
        str, Callable[[MaterializationContext], MaterializationResult]
    ] = {
        "test_custom": custom_fn,
    }

    result: BuildExecutionResult
    verify_connection: Any
    result, verify_connection = run_scheduler_build(
        project_files=test_case.project_files,
        project_dir=project_dir,
        db_path=db_path,
        adapter=adapter,
        custom_materializations=custom_materializations,
    )

    try:
        assert result.status == test_case.expected_status
        assert result.success_count == test_case.expected_success_count

        query: str
        expected_rows: tuple[tuple[object, ...], ...]
        for query, expected_rows in test_case.expected_query_results:
            cursor: Any = verify_connection.execute(query)
            actual_rows: tuple[tuple[object, ...], ...] = tuple(
                tuple(row) for row in cursor.fetchall()
            )
            assert actual_rows == expected_rows
    finally:
        adapter.close(verify_connection)
