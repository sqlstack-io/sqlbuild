"""Tests for snapshot execution adapter SQL rendering boundaries."""

from __future__ import annotations

from collections import defaultdict
from pathlib import Path
from typing import Any

import pytest

from sqlbuild._native import (
    CompiledObjectKey,
    CompiledRelationLocation,
    CompiledResourceType,
    HistoricalInput,
    MaterializationType,
    ModelPlanEntry,
    PlanAction,
    PlanReason,
)
from sqlbuild.adapters.duckdb.classes.duckdb_adapter import DuckDbAdapter
from sqlbuild.executor.run._helpers.materializations.snapshot import execute_snapshot_entry
from sqlbuild.executor.run.models import ModelExecutionResult, ModelMaterializationContext
from sqlbuild.executor.scheduling.types import ExecutionStatus
from tests.unit.src.sqlbuild.executor.run._helpers._test_types import (
    SnapshotAdapterRenderingTestCase,
)


class _SnapshotRenderingAdapter(DuckDbAdapter):
    def __init__(self, *, marker: str) -> None:
        super().__init__()
        self.marker: str = marker
        self.rendered_timestamp_changes: bool = False
        self.rendered_historical_timestamp_changes: bool = False
        self.rendered_initial_historical_timestamp_change_records: bool = False
        self.rendered_historical_timestamp_change_records: bool = False
        self.rendered_historical_timestamp_invalidate_hard_deletes: bool | None = None

    def render_apply_timestamp_snapshot_changes(
        self,
        *,
        destination: str,
        origin: str,
        unique_key: tuple[str, ...],
        updated_at_column: str,
        observed_at_column: str | None,
        valid_from_column: str,
        valid_to_column: str,
        initial_valid_from: str | None,
        output_columns: tuple[str, ...],
        invalidate_hard_deletes: bool,
    ) -> tuple[str, ...]:
        del (
            destination,
            origin,
            unique_key,
            updated_at_column,
            observed_at_column,
            valid_from_column,
            valid_to_column,
            initial_valid_from,
            output_columns,
            invalidate_hard_deletes,
        )
        self.rendered_timestamp_changes = True
        return (f"INSERT INTO main.rendered_snapshot_sql VALUES ('{self.marker}')",)

    def render_apply_historical_timestamp_snapshot_changes(
        self,
        *,
        destination: str,
        origin: str,
        unique_key: tuple[str, ...],
        updated_at_column: str,
        observed_at_column: str,
        valid_from_column: str,
        valid_to_column: str,
        output_columns: tuple[str, ...],
        invalidate_hard_deletes: bool,
    ) -> tuple[str, ...]:
        del (
            destination,
            origin,
            unique_key,
            updated_at_column,
            observed_at_column,
            valid_from_column,
            valid_to_column,
            output_columns,
        )
        self.rendered_historical_timestamp_changes = True
        self.rendered_historical_timestamp_invalidate_hard_deletes = invalidate_hard_deletes
        return (f"INSERT INTO main.rendered_snapshot_sql VALUES ('{self.marker}')",)

    def render_apply_historical_timestamp_changes(
        self,
        *,
        destination: str,
        origin: str,
        unique_key: tuple[str, ...],
        updated_at_column: str,
        valid_from_column: str,
        valid_to_column: str,
        output_columns: tuple[str, ...],
    ) -> tuple[str, ...]:
        del (
            destination,
            origin,
            unique_key,
            updated_at_column,
            valid_from_column,
            valid_to_column,
            output_columns,
        )
        self.rendered_historical_timestamp_change_records = True
        return (f"INSERT INTO main.rendered_snapshot_sql VALUES ('{self.marker}')",)

    def render_create_initial_historical_timestamp_changes_destination(
        self,
        *,
        table_type: str,
        destination: str,
        origin: str,
        unique_key: tuple[str, ...],
        updated_at_column: str,
        valid_from_column: str,
        valid_to_column: str,
        output_columns: tuple[str, ...],
    ) -> tuple[str, ...]:
        del (
            table_type,
            origin,
            unique_key,
            updated_at_column,
            valid_from_column,
            valid_to_column,
            output_columns,
        )
        self.rendered_initial_historical_timestamp_change_records = True
        return (f"CREATE TABLE {destination} AS SELECT '{self.marker}' AS marker",)


@pytest.mark.parametrize(
    "test_case",
    [
        SnapshotAdapterRenderingTestCase(
            description="timestamp snapshot DML is rendered by adapter",
            expected_rendered_marker="adapter-rendered-timestamp",
        )
    ],
    ids=lambda case: case.description,
)
def test_given_existing_snapshot_target_when_executing_then_uses_adapter_rendered_dml(
    test_case: SnapshotAdapterRenderingTestCase,
) -> None:
    adapter: _SnapshotRenderingAdapter = _SnapshotRenderingAdapter(
        marker=test_case.expected_rendered_marker
    )
    connection: Any = adapter.connect({"database": ":memory:"})
    connection.execute("CREATE TABLE main.rendered_snapshot_sql (marker VARCHAR)")
    connection.execute(
        "CREATE TABLE main.customer_snapshot AS "
        "SELECT 1 AS customer_id, 'basic' AS plan, "
        "TIMESTAMP '2024-01-01 00:00:00' AS updated_at, "
        "TIMESTAMP '2024-01-01 00:00:00' AS valid_from, "
        "CAST(NULL AS TIMESTAMP) AS valid_to"
    )
    entry: ModelPlanEntry = ModelPlanEntry(
        key=CompiledObjectKey(
            resource_type=CompiledResourceType.MODEL,
            name="customer_snapshot",
        ),
        name="customer_snapshot",
        relative_path=Path("sql/models/customer_snapshot.sql"),
        materialization_type=MaterializationType.SNAPSHOT,
        action=PlanAction.SNAPSHOT,
        reason=PlanReason.NORMAL_INCREMENTAL,
        destination=CompiledRelationLocation(
            database=None,
            schema="main",
            name="customer_snapshot",
            qualified_name="main.customer_snapshot",
        ),
        fingerprint_query_sql="SELECT 1 AS customer_id",
        resolved_sql=(
            "SELECT 1 AS customer_id, 'pro' AS plan, TIMESTAMP '2024-01-02 00:00:00' AS updated_at"
        ),
        logical_ddl="",
        unique_key=("customer_id",),
        snapshot_strategy="timestamp",
        updated_at_column="updated_at",
    )

    result: ModelExecutionResult = execute_snapshot_entry(
        context=ModelMaterializationContext(
            entry=entry,
            adapter=adapter,
            connection=connection,
            model_locations={},
            seed_locations={},
            source_map={},
            python_source_read_map={},
            model_audits=(),
            run_id="test_run",
            query_change_tracking=False,
        ),
    )
    rendered_rows: tuple[tuple[object, ...], ...] = tuple(
        tuple(row)
        for row in connection.execute("SELECT marker FROM main.rendered_snapshot_sql").fetchall()
    )
    lifecycle_sql_by_insert_status: defaultdict[bool, list[str]] = defaultdict(list)
    for event in result.lifecycle_events:
        lifecycle_sql_by_insert_status[event.content.startswith("INSERT INTO")].append(
            event.content
        )
    lifecycle_sql: tuple[str, ...] = tuple(lifecycle_sql_by_insert_status[True])

    assert result.status == ExecutionStatus.SUCCESS
    assert adapter.rendered_timestamp_changes is True
    assert rendered_rows == ((test_case.expected_rendered_marker,),)
    assert lifecycle_sql == (
        f"INSERT INTO main.rendered_snapshot_sql VALUES ('{test_case.expected_rendered_marker}')",
    )

    adapter.close(connection)


@pytest.mark.parametrize(
    "test_case",
    [
        SnapshotAdapterRenderingTestCase(
            description="historical timestamp snapshot DML is rendered by adapter",
            expected_rendered_marker="adapter-rendered-historical-timestamp",
        )
    ],
    ids=lambda case: case.description,
)
def test_given_existing_historical_timestamp_snapshot_when_executing_then_uses_adapter_dml(
    test_case: SnapshotAdapterRenderingTestCase,
) -> None:
    adapter: _SnapshotRenderingAdapter = _SnapshotRenderingAdapter(
        marker=test_case.expected_rendered_marker
    )
    connection: Any = adapter.connect({"database": ":memory:"})
    connection.execute("CREATE TABLE main.rendered_snapshot_sql (marker VARCHAR)")
    connection.execute(
        "CREATE TABLE main.customer_snapshot AS "
        "SELECT 1 AS customer_id, 'basic' AS plan, "
        "TIMESTAMP '2024-01-01 00:00:00' AS updated_at, "
        "TIMESTAMP '2024-01-02 00:00:00' AS observed_at, "
        "TIMESTAMP '2024-01-01 00:00:00' AS valid_from, "
        "CAST(NULL AS TIMESTAMP) AS valid_to"
    )
    entry: ModelPlanEntry = ModelPlanEntry(
        key=CompiledObjectKey(
            resource_type=CompiledResourceType.MODEL,
            name="customer_snapshot",
        ),
        name="customer_snapshot",
        relative_path=Path("sql/models/customer_snapshot.sql"),
        materialization_type=MaterializationType.SNAPSHOT,
        action=PlanAction.SNAPSHOT,
        reason=PlanReason.NORMAL_INCREMENTAL,
        destination=CompiledRelationLocation(
            database=None,
            schema="main",
            name="customer_snapshot",
            qualified_name="main.customer_snapshot",
        ),
        fingerprint_query_sql="SELECT 1 AS customer_id",
        resolved_sql=(
            "SELECT 1 AS customer_id, 'pro' AS plan, "
            "TIMESTAMP '2024-01-03 00:00:00' AS updated_at, "
            "TIMESTAMP '2024-01-04 00:00:00' AS observed_at"
        ),
        logical_ddl="",
        unique_key=("customer_id",),
        snapshot_strategy="timestamp",
        updated_at_column="updated_at",
        observed_at_column="observed_at",
        historical_input=HistoricalInput.SNAPSHOT,
        invalidate_hard_deletes=True,
    )

    result: ModelExecutionResult = execute_snapshot_entry(
        context=ModelMaterializationContext(
            entry=entry,
            adapter=adapter,
            connection=connection,
            model_locations={},
            seed_locations={},
            source_map={},
            python_source_read_map={},
            model_audits=(),
            run_id="test_run",
            query_change_tracking=False,
        ),
    )
    rendered_rows: tuple[tuple[object, ...], ...] = tuple(
        tuple(row)
        for row in connection.execute("SELECT marker FROM main.rendered_snapshot_sql").fetchall()
    )

    assert result.status == ExecutionStatus.SUCCESS
    assert adapter.rendered_historical_timestamp_changes is True
    assert adapter.rendered_historical_timestamp_invalidate_hard_deletes is True
    assert rendered_rows == ((test_case.expected_rendered_marker,),)

    adapter.close(connection)


@pytest.mark.parametrize(
    "test_case",
    [
        SnapshotAdapterRenderingTestCase(
            description="historical timestamp changes DML is rendered by adapter",
            expected_rendered_marker="adapter-rendered-historical-timestamp-changes",
        )
    ],
    ids=lambda case: case.description,
)
def test_given_existing_historical_timestamp_changes_snapshot_when_executing_then_uses_adapter_dml(
    test_case: SnapshotAdapterRenderingTestCase,
) -> None:
    adapter: _SnapshotRenderingAdapter = _SnapshotRenderingAdapter(
        marker=test_case.expected_rendered_marker
    )
    connection: Any = adapter.connect({"database": ":memory:"})
    connection.execute("CREATE TABLE main.rendered_snapshot_sql (marker VARCHAR)")
    connection.execute(
        "CREATE TABLE main.customer_snapshot AS "
        "SELECT 1 AS customer_id, 'basic' AS plan, "
        "TIMESTAMP '2024-01-01 00:00:00' AS updated_at, "
        "TIMESTAMP '2024-01-02 00:00:00' AS observed_at, "
        "TIMESTAMP '2024-01-01 00:00:00' AS valid_from, "
        "CAST(NULL AS TIMESTAMP) AS valid_to"
    )
    entry: ModelPlanEntry = ModelPlanEntry(
        key=CompiledObjectKey(
            resource_type=CompiledResourceType.MODEL,
            name="customer_snapshot",
        ),
        name="customer_snapshot",
        relative_path=Path("sql/models/customer_snapshot.sql"),
        materialization_type=MaterializationType.SNAPSHOT,
        action=PlanAction.SNAPSHOT,
        reason=PlanReason.NORMAL_INCREMENTAL,
        destination=CompiledRelationLocation(
            database=None,
            schema="main",
            name="customer_snapshot",
            qualified_name="main.customer_snapshot",
        ),
        fingerprint_query_sql="SELECT 1 AS customer_id",
        resolved_sql=(
            "SELECT 1 AS customer_id, 'pro' AS plan, "
            "TIMESTAMP '2024-01-03 00:00:00' AS updated_at, "
            "TIMESTAMP '2024-01-04 00:00:00' AS observed_at"
        ),
        logical_ddl="",
        unique_key=("customer_id",),
        snapshot_strategy="timestamp",
        updated_at_column="updated_at",
        observed_at_column="observed_at",
        historical_input=HistoricalInput.CHANGES,
    )

    result: ModelExecutionResult = execute_snapshot_entry(
        context=ModelMaterializationContext(
            entry=entry,
            adapter=adapter,
            connection=connection,
            model_locations={},
            seed_locations={},
            source_map={},
            python_source_read_map={},
            model_audits=(),
            run_id="test_run",
            query_change_tracking=False,
        ),
    )
    rendered_rows: tuple[tuple[object, ...], ...] = tuple(
        tuple(row)
        for row in connection.execute("SELECT marker FROM main.rendered_snapshot_sql").fetchall()
    )

    assert result.status == ExecutionStatus.SUCCESS
    assert adapter.rendered_historical_timestamp_change_records is True
    assert rendered_rows == ((test_case.expected_rendered_marker,),)

    adapter.close(connection)


@pytest.mark.parametrize(
    "test_case",
    [
        SnapshotAdapterRenderingTestCase(
            description="initial historical timestamp changes target is rendered by adapter",
            expected_rendered_marker="adapter-rendered-initial-historical-timestamp-changes",
        )
    ],
    ids=lambda case: case.description,
)
def test_given_new_historical_timestamp_changes_when_executing_then_uses_adapter_target_sql(
    test_case: SnapshotAdapterRenderingTestCase,
) -> None:
    adapter: _SnapshotRenderingAdapter = _SnapshotRenderingAdapter(
        marker=test_case.expected_rendered_marker
    )
    connection: Any = adapter.connect({"database": ":memory:"})
    entry: ModelPlanEntry = ModelPlanEntry(
        key=CompiledObjectKey(
            resource_type=CompiledResourceType.MODEL,
            name="customer_snapshot",
        ),
        name="customer_snapshot",
        relative_path=Path("sql/models/customer_snapshot.sql"),
        materialization_type=MaterializationType.SNAPSHOT,
        action=PlanAction.SNAPSHOT,
        reason=PlanReason.NORMAL_INCREMENTAL,
        destination=CompiledRelationLocation(
            database=None,
            schema="main",
            name="customer_snapshot",
            qualified_name="main.customer_snapshot",
        ),
        fingerprint_query_sql="SELECT 1 AS customer_id",
        resolved_sql=(
            "SELECT 1 AS customer_id, 'basic' AS plan, "
            "TIMESTAMP '2024-01-01 00:00:00' AS updated_at, "
            "TIMESTAMP '2024-01-02 00:00:00' AS observed_at"
        ),
        logical_ddl="",
        unique_key=("customer_id",),
        snapshot_strategy="timestamp",
        updated_at_column="updated_at",
        observed_at_column="observed_at",
        historical_input=HistoricalInput.CHANGES,
    )

    result: ModelExecutionResult = execute_snapshot_entry(
        context=ModelMaterializationContext(
            entry=entry,
            adapter=adapter,
            connection=connection,
            model_locations={},
            seed_locations={},
            source_map={},
            python_source_read_map={},
            model_audits=(),
            run_id="test_run",
            query_change_tracking=False,
        ),
    )
    rendered_rows: tuple[tuple[object, ...], ...] = tuple(
        tuple(row)
        for row in connection.execute("SELECT marker FROM main.customer_snapshot").fetchall()
    )

    assert result.status == ExecutionStatus.SUCCESS
    assert adapter.rendered_initial_historical_timestamp_change_records is True
    assert rendered_rows == ((test_case.expected_rendered_marker,),)

    adapter.close(connection)
