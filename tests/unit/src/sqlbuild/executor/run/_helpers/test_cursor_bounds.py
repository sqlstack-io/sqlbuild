from __future__ import annotations

from datetime import UTC, date, datetime
from decimal import Decimal
from pathlib import Path
from typing import cast

import duckdb
import pytest

from sqlbuild._native import (
    CompiledObjectKey,
    CompiledRelationLocation,
    CompiledResourceType,
    CursorBounds,
    CursorGrain,
    CursorInputRelation,
    CursorType,
    CursorWatermarkMode,
    FutureCursorAction,
    FutureCursorSafetyError,
    FutureCursorsConfig,
    MaterializationType,
    ModelCursorSnapshot,
    ModelPlanEntry,
    PlanAction,
    PlanReason,
    StartCursorsConfig,
    normalize_cursor_snapshot_grain,
)
from sqlbuild._native import cursor_render as render
from sqlbuild._native import cursor_sentinel_to_token as sentinel_to_token
from sqlbuild.adapter.contract.classes.base_adapter import BaseAdapter
from sqlbuild.adapters.duckdb.classes.duckdb_adapter import DuckDbAdapter
from sqlbuild.errors.contracts.exceptions import ExecutorInputError
from sqlbuild.executor.run._helpers.validation.cursor_bounds import (
    build_runtime_cursor_spec,
    resolve_runtime_cursor_bounds,
    substitute_cursor_sentinels,
)
from sqlbuild.executor.run.exceptions import EmptyCursorInputsError
from sqlbuild.executor.run.models import RuntimeCursorInputRelation, RuntimeCursorSpec
from sqlbuild.executor.run.types import RuntimeCursorWatermarkMode
from tests.unit.src.sqlbuild.executor.run._helpers._test_types import (
    AuthoritativeRuntimeCursorOverrideTestCase,
    CursorSentinelSubstitutionErrorTestCase,
    CursorSentinelSubstitutionTestCase,
    MixedTemporalWatermarkTestCase,
    RuntimeCursorEndBoundTestCase,
    RuntimeCursorFailureTestCase,
    RuntimeCursorOverrideTestCase,
    RuntimeCursorPolicyTestCase,
    RuntimeCursorSpecBoundaryTestCase,
    RuntimeCursorStartTestCase,
    RuntimeExistingTargetOverrideTestCase,
    RuntimeFutureCursorTestCase,
    RuntimeIntegerWatermarkModeTestCase,
    RuntimeTargetMaxTestCase,
    RuntimeTargetProbeFailureTestCase,
    RuntimeWatermarkStatementTestCase,
)
from tests.unit.src.sqlbuild.executor.run._helpers.helpers import (
    FakeCursorAdapter,
    RecordingCursorAdapter,
)


@pytest.mark.parametrize(
    "test_case",
    (
        CursorSentinelSubstitutionTestCase(
            description="date and offset timestamp retain SQL token formats",
            start="2026-01-01",
            end="2026-02-01T00:00:00+00:00",
            sql="SELECT '__SQB_CURSOR_START__', '__SQB_CURSOR_END__'",
            expected_sql="SELECT '2026-01-01', '2026-02-01T00:00:00+00:00'",
        ),
    ),
    ids=lambda case: case.description,
)
def test_given_typed_runtime_bounds_when_substituting_then_preserves_sql_formats(
    test_case: CursorSentinelSubstitutionTestCase,
) -> None:
    result: str = substitute_cursor_sentinels(
        sql=test_case.sql,
        bounds=CursorBounds(start=test_case.start, end=test_case.end),
    )

    assert result == test_case.expected_sql


@pytest.mark.parametrize(
    "test_case",
    (
        RuntimeCursorSpecBoundaryTestCase(
            description="date timestamp and grain retain boundary formats",
            expected_grain="day",
            expected_start="2026-01-01",
            expected_end="2026-02-01T00:00:00+00:00",
        ),
    ),
    ids=lambda case: case.description,
)
def test_given_typed_planner_cursor_input_when_building_runtime_spec_then_preserves_typed_formats(
    test_case: RuntimeCursorSpecBoundaryTestCase,
) -> None:
    entry: ModelPlanEntry = ModelPlanEntry(
        key=CompiledObjectKey(resource_type=CompiledResourceType.MODEL, name="events"),
        name="events",
        relative_path=Path("sql/models/events.sql"),
        materialization_type=MaterializationType.INCREMENTAL,
        action=PlanAction.INCREMENTAL_DELETE_INSERT,
        reason=PlanReason.NORMAL_INCREMENTAL,
        destination=CompiledRelationLocation(
            database=None,
            schema="main",
            name="events",
            qualified_name="main.events",
        ),
        fingerprint_query_sql="SELECT 1",
        resolved_sql="SELECT 1",
        logical_ddl="",
        cursor_column="event_at",
        cursor_type=CursorType.TIMESTAMP,
        cursor_input_relations=(
            CursorInputRelation(
                relation="main.archive_events",
                cursor_column="event_at",
                cursor_grain=CursorGrain.DAY,
                terminal_cursor_start=test_case.expected_start,
                terminal_cursor_end=test_case.expected_end,
            ),
        ),
    )

    relation: RuntimeCursorInputRelation = build_runtime_cursor_spec(
        entry=entry
    ).cursor_input_relations[0]

    assert relation.cursor_grain == test_case.expected_grain
    assert relation.terminal_cursor_start is not None
    assert relation.terminal_cursor_end is not None
    assert render(value=relation.terminal_cursor_start) == test_case.expected_start
    assert render(value=relation.terminal_cursor_end) == test_case.expected_end


@pytest.mark.parametrize(
    "test_case",
    [RuntimeFutureCursorTestCase("runtime cap", "2026-09-01T00:00:00", "2026-09-03T12:00:01")],
    ids=lambda case: case.description,
)
def test_given_future_runtime_watermark_when_resolving_then_cap_uses_invocation_clock(
    test_case: RuntimeFutureCursorTestCase,
) -> None:
    connection: duckdb.DuckDBPyConnection = duckdb.connect(":memory:")
    connection.execute("CREATE TABLE upstream_data (cursor_value TIMESTAMP)")
    connection.execute(
        "INSERT INTO upstream_data VALUES (TIMESTAMP '2026-09-01'), (TIMESTAMP '2030-01-01')"
    )

    bounds: CursorBounds | None = resolve_runtime_cursor_bounds(
        adapter=cast(BaseAdapter, FakeCursorAdapter()),
        connection=connection,
        target_relation="target_data",
        target_database=None,
        target_schema=None,
        target_name="target_data",
        spec=RuntimeCursorSpec(
            cursor_column="cursor_value",
            cursor_type=CursorType.TIMESTAMP,
            cursor_grain=CursorGrain.SECOND,
            cursor_start=None,
            cursor_input_relations=(
                RuntimeCursorInputRelation(relation="upstream_data", cursor_column="cursor_value"),
            ),
            future_cursor_config=FutureCursorsConfig("2d", FutureCursorAction.CAP),
            invocation_time=datetime(2026, 9, 1, 12, 0, tzinfo=UTC),
        ),
    )

    assert bounds is not None
    assert sentinel_to_token(sentinel=bounds.start) == test_case.expected_start
    assert sentinel_to_token(sentinel=bounds.end) == test_case.expected_end
    assert bounds.future_safety is not None
    assert render(value=bounds.future_safety.discovered_end) == "2030-01-01T00:00:01"


@pytest.mark.parametrize(
    "test_case",
    [RuntimeFutureCursorTestCase("runtime error", "", "", "future cursor safety limit exceeded")],
    ids=lambda case: case.description,
)
def test_given_future_runtime_watermark_and_error_policy_when_resolving_then_fails_closed(
    test_case: RuntimeFutureCursorTestCase,
) -> None:
    connection: duckdb.DuckDBPyConnection = duckdb.connect(":memory:")
    connection.execute("CREATE TABLE upstream_data (cursor_value TIMESTAMP)")
    connection.execute("INSERT INTO upstream_data VALUES (TIMESTAMP '2026-09-01')")
    connection.execute("CREATE TABLE target_data (cursor_value TIMESTAMP)")
    connection.execute("INSERT INTO target_data VALUES (TIMESTAMP '2500-01-01')")

    with pytest.raises(FutureCursorSafetyError, match=test_case.expected_error_fragment):
        resolve_runtime_cursor_bounds(
            adapter=cast(BaseAdapter, FakeCursorAdapter(target_relation_exists=True)),
            connection=connection,
            target_relation="target_data",
            target_database=None,
            target_schema=None,
            target_name="target_data",
            spec=RuntimeCursorSpec(
                cursor_column="cursor_value",
                cursor_type=CursorType.TIMESTAMP,
                cursor_grain=CursorGrain.SECOND,
                cursor_start=None,
                cursor_input_relations=(
                    RuntimeCursorInputRelation(
                        relation="upstream_data", cursor_column="cursor_value"
                    ),
                ),
                future_cursor_config=FutureCursorsConfig("2d", FutureCursorAction.ERROR),
                invocation_time=datetime(2026, 9, 1, 12, 0, tzinfo=UTC),
            ),
        )


@pytest.mark.parametrize(
    "test_case",
    [
        RuntimeFutureCursorTestCase(
            "future target evidence",
            "2500-01-01T00:00:00",
            "2026-09-01T00:00:01",
            expected_determining_relation="input_a",
        )
    ],
    ids=lambda case: case.description,
)
def test_given_future_target_and_multiple_inputs_when_capping_then_start_and_input_evidence_are_preserved(
    test_case: RuntimeFutureCursorTestCase,
) -> None:
    connection: duckdb.DuckDBPyConnection = duckdb.connect(":memory:")
    connection.execute("CREATE TABLE target_data (cursor_value TIMESTAMP)")
    connection.execute("INSERT INTO target_data VALUES (TIMESTAMP '2500-01-01')")
    connection.execute("CREATE TABLE input_a (cursor_value TIMESTAMP)")
    connection.execute("INSERT INTO input_a VALUES (TIMESTAMP '2026-09-01')")
    connection.execute("CREATE TABLE input_b (cursor_value TIMESTAMP)")
    connection.execute("INSERT INTO input_b VALUES (TIMESTAMP '2026-09-02')")

    bounds: CursorBounds | None = resolve_runtime_cursor_bounds(
        adapter=cast(BaseAdapter, FakeCursorAdapter(target_relation_exists=True)),
        connection=connection,
        target_relation="target_data",
        target_database=None,
        target_schema=None,
        target_name="target_data",
        spec=RuntimeCursorSpec(
            cursor_column="cursor_value",
            cursor_type=CursorType.TIMESTAMP,
            cursor_grain=CursorGrain.SECOND,
            cursor_start=None,
            cursor_input_relations=(
                RuntimeCursorInputRelation(relation="input_b", cursor_column="cursor_value"),
                RuntimeCursorInputRelation(relation="input_a", cursor_column="cursor_value"),
            ),
            future_cursor_config=FutureCursorsConfig("2d", FutureCursorAction.CAP),
            invocation_time=datetime(2026, 9, 1, 12, 0, tzinfo=UTC),
        ),
    )

    assert bounds is not None
    assert sentinel_to_token(sentinel=bounds.start) == test_case.expected_start
    assert sentinel_to_token(sentinel=bounds.end) == test_case.expected_end
    assert bounds.future_safety is not None
    assert bounds.future_safety.determining_relation == test_case.expected_determining_relation
    assert tuple(item.relation for item in bounds.future_safety.inputs) == (
        "input_a",
        "input_b",
    )


@pytest.mark.parametrize(
    "test_case",
    [
        CursorSentinelSubstitutionErrorTestCase(
            description="unresolved cursor intrinsic",
            sql="SELECT __cursor_start()",
            expected_error_fragment="unresolved cursor markers",
        )
    ],
    ids=lambda case: case.description,
)
def test_given_runtime_bounds_when_substituting_then_rejects_unresolved_intrinsics(
    test_case: CursorSentinelSubstitutionErrorTestCase,
) -> None:
    with pytest.raises(ExecutorInputError, match=test_case.expected_error_fragment):
        substitute_cursor_sentinels(
            sql=test_case.sql,
            bounds=CursorBounds(start="1", end="2"),
        )


@pytest.mark.parametrize(
    "test_case",
    [
        RuntimeCursorStartTestCase(
            description="runtime timestamp bounds clamp to configured floor",
            target_max=None,
            upstream_min=datetime(2024, 1, 1, tzinfo=UTC),
            upstream_max=datetime(2024, 2, 1, tzinfo=UTC),
            cursor_type=CursorType.TIMESTAMP,
            warehouse_column_type="TIMESTAMPTZ",
            cursor_start="2024-01-15T00:00:00+00:00",
            expected_start="2024-01-15T00:00:00+00:00",
            expected_end="2024-02-01T00:00:01+00:00",
        ),
        RuntimeCursorStartTestCase(
            description="runtime integer bounds clamp to configured floor",
            target_max=None,
            upstream_min=50,
            upstream_max=200,
            cursor_type=CursorType.INTEGER,
            warehouse_column_type="INTEGER",
            cursor_start="100",
            expected_start="100",
            expected_end="201",
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_runtime_cursor_start_when_resolving_bounds_then_applies_lower_floor(
    test_case: RuntimeCursorStartTestCase,
) -> None:
    connection: duckdb.DuckDBPyConnection = duckdb.connect(":memory:")
    connection.execute(
        f"CREATE TABLE upstream_data (cursor_value {test_case.warehouse_column_type})"
    )
    connection.execute("INSERT INTO upstream_data VALUES (?)", [test_case.upstream_min])
    connection.execute("INSERT INTO upstream_data VALUES (?)", [test_case.upstream_max])

    cursor_bounds: CursorBounds | None = resolve_runtime_cursor_bounds(
        adapter=cast(BaseAdapter, FakeCursorAdapter()),
        connection=connection,
        target_relation="target_data",
        target_database=None,
        target_schema=None,
        target_name="target_data",
        spec=RuntimeCursorSpec(
            cursor_column="cursor_value",
            cursor_type=test_case.cursor_type,
            cursor_grain=None,
            cursor_start=test_case.cursor_start,
            cursor_input_relations=(
                RuntimeCursorInputRelation(
                    relation="upstream_data",
                    cursor_column="cursor_value",
                ),
            ),
        ),
    )

    assert cursor_bounds is not None
    assert sentinel_to_token(sentinel=cursor_bounds.start) == test_case.expected_start
    assert sentinel_to_token(sentinel=cursor_bounds.end) == test_case.expected_end


@pytest.mark.parametrize(
    "test_case",
    [
        RuntimeWatermarkStatementTestCase(
            description="first run reads standalone min and max in deterministic order",
            target_exists=False,
            expected_statements=(
                "SELECT MIN(cursor_value), MAX(cursor_value) FROM a_watermark",
                "SELECT MIN(cursor_value), MAX(cursor_value) FROM z_watermark",
            ),
            expected_bounds=CursorBounds(start="10", end="91"),
        ),
        RuntimeWatermarkStatementTestCase(
            description="existing target reads only standalone maxima and deduplicates inputs",
            target_exists=True,
            expected_statements=(
                "SELECT MAX(cursor_value) FROM target_data",
                "SELECT MAX(cursor_value) FROM a_watermark",
                "SELECT MAX(cursor_value) FROM z_watermark",
            ),
            expected_bounds=CursorBounds(start="50", end="91"),
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_multiple_physical_watermarks_when_resolving_then_reads_standalone_statements(
    test_case: RuntimeWatermarkStatementTestCase,
) -> None:
    connection: duckdb.DuckDBPyConnection = duckdb.connect(":memory:")
    connection.execute("CREATE TABLE a_watermark (cursor_value INTEGER)")
    connection.execute("INSERT INTO a_watermark VALUES (10), (90)")
    connection.execute("CREATE TABLE z_watermark (cursor_value INTEGER)")
    connection.execute("INSERT INTO z_watermark VALUES (20), (100)")
    connection.execute("CREATE TABLE target_data (cursor_value INTEGER)")
    connection.execute("INSERT INTO target_data VALUES (50)")
    adapter: RecordingCursorAdapter = RecordingCursorAdapter(
        target_relation_exists=test_case.target_exists
    )

    bounds: CursorBounds | None = resolve_runtime_cursor_bounds(
        adapter=cast(BaseAdapter, adapter),
        connection=connection,
        target_relation="target_data",
        target_database=None,
        target_schema=None,
        target_name="target_data",
        spec=RuntimeCursorSpec(
            cursor_column="cursor_value",
            cursor_type=CursorType.INTEGER,
            cursor_grain=None,
            cursor_start=None,
            cursor_input_relations=(
                RuntimeCursorInputRelation(relation="z_watermark", cursor_column="cursor_value"),
                RuntimeCursorInputRelation(relation="a_watermark", cursor_column="cursor_value"),
                RuntimeCursorInputRelation(relation="a_watermark", cursor_column="cursor_value"),
            ),
        ),
    )

    assert "UNION ALL" not in " ".join(adapter.statements)
    assert tuple(adapter.statements) == test_case.expected_statements
    assert bounds == test_case.expected_bounds


@pytest.mark.parametrize(
    "test_case",
    (
        RuntimeIntegerWatermarkModeTestCase(
            description="all_9_and_10",
            mode=CursorWatermarkMode.ALL,
            values=(9, 10),
            expected_bounds=CursorBounds(start="9", end="10"),
        ),
        RuntimeIntegerWatermarkModeTestCase(
            description="any_9_and_10",
            mode=CursorWatermarkMode.ANY,
            values=(9, 10),
            expected_bounds=CursorBounds(start="9", end="11"),
        ),
        RuntimeIntegerWatermarkModeTestCase(
            description="all_100_and_99",
            mode=CursorWatermarkMode.ALL,
            values=(100, 99),
            expected_bounds=CursorBounds(start="99", end="100"),
        ),
        RuntimeIntegerWatermarkModeTestCase(
            description="any_100_and_99",
            mode=CursorWatermarkMode.ANY,
            values=(100, 99),
            expected_bounds=CursorBounds(start="99", end="101"),
        ),
    ),
    ids=lambda case: case.description,
)
def test_given_multiple_integer_watermarks_when_resolving_then_uses_numeric_order(
    test_case: RuntimeIntegerWatermarkModeTestCase,
) -> None:
    connection: duckdb.DuckDBPyConnection = duckdb.connect(":memory:")
    connection.execute("CREATE TABLE input_a (cursor_value INTEGER)")
    connection.execute("CREATE TABLE input_b (cursor_value INTEGER)")
    connection.execute("INSERT INTO input_a VALUES (?)", [test_case.values[0]])
    connection.execute("INSERT INTO input_b VALUES (?)", [test_case.values[1]])

    bounds: CursorBounds | None = resolve_runtime_cursor_bounds(
        adapter=cast(BaseAdapter, FakeCursorAdapter()),
        connection=connection,
        target_relation="target_data",
        target_database=None,
        target_schema=None,
        target_name="target_data",
        spec=RuntimeCursorSpec(
            cursor_column="cursor_value",
            cursor_type=CursorType.INTEGER,
            cursor_grain=None,
            cursor_start=None,
            cursor_input_relations=(
                RuntimeCursorInputRelation(relation="input_a", cursor_column="cursor_value"),
                RuntimeCursorInputRelation(relation="input_b", cursor_column="cursor_value"),
            ),
            cursor_watermark_mode=test_case.mode,
        ),
    )

    assert bounds == test_case.expected_bounds


@pytest.mark.parametrize(
    "test_case",
    [
        MixedTemporalWatermarkTestCase(
            description="date and timestamp watermarks compare at common coarsest grain",
            expected_start="2024-01-01",
            expected_end="2024-02-02T00:00:00",
        )
    ],
    ids=lambda case: case.description,
)
def test_given_date_and_timestamp_watermarks_when_resolving_then_normalizes_comparison(
    test_case: MixedTemporalWatermarkTestCase,
) -> None:
    connection: duckdb.DuckDBPyConnection = duckdb.connect(":memory:")
    connection.execute("CREATE TABLE date_watermark (cursor_value DATE)")
    connection.execute("INSERT INTO date_watermark VALUES ('2024-01-01'), ('2024-02-02')")
    connection.execute("CREATE TABLE timestamp_watermark (cursor_value TIMESTAMP)")
    connection.execute(
        "INSERT INTO timestamp_watermark VALUES ('2024-01-01 12:00:00'), ('2024-02-01 23:00:00')"
    )

    bounds: CursorBounds | None = resolve_runtime_cursor_bounds(
        adapter=cast(BaseAdapter, FakeCursorAdapter()),
        connection=connection,
        target_relation="target_data",
        target_database=None,
        target_schema=None,
        target_name="target_data",
        spec=RuntimeCursorSpec(
            cursor_column="cursor_value",
            cursor_type=CursorType.TIMESTAMP,
            cursor_grain=CursorGrain.HOUR,
            cursor_start=None,
            cursor_input_relations=(
                RuntimeCursorInputRelation(
                    relation="date_watermark",
                    cursor_column="cursor_value",
                    cursor_grain=CursorGrain.DAY,
                ),
                RuntimeCursorInputRelation(
                    relation="timestamp_watermark",
                    cursor_column="cursor_value",
                    cursor_grain=CursorGrain.HOUR,
                ),
            ),
        ),
    )

    assert bounds is not None
    assert sentinel_to_token(sentinel=bounds.start) == test_case.expected_start
    assert sentinel_to_token(sentinel=bounds.end) == test_case.expected_end


@pytest.mark.parametrize(
    "test_case",
    [
        RuntimeCursorEndBoundTestCase(
            description="date cursor with day grain advances the end bound past the final date",
            upstream_min=date(2014, 1, 1),
            upstream_max=date(2014, 12, 31),
            cursor_type=CursorType.TIMESTAMP,
            cursor_grain=CursorGrain.DAY,
            warehouse_column_type="DATE",
            expected_start="2014-01-01",
            expected_end="2015-01-01",
        ),
        RuntimeCursorEndBoundTestCase(
            description="date cursor without a grain advances the end bound past the final date",
            upstream_min=date(2014, 1, 1),
            upstream_max=date(2014, 12, 31),
            cursor_type=CursorType.TIMESTAMP,
            cursor_grain=None,
            warehouse_column_type="DATE",
            expected_start="2014-01-01",
            expected_end="2015-01-01",
        ),
        RuntimeCursorEndBoundTestCase(
            description="decimal integer cursor advances the end bound past the final value",
            upstream_min=Decimal(50),
            upstream_max=Decimal(200),
            cursor_type=CursorType.INTEGER,
            cursor_grain=None,
            warehouse_column_type="DECIMAL(38,0)",
            expected_start="50",
            expected_end="201",
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_non_datetime_cursor_when_resolving_bounds_then_end_bound_includes_final_value(
    test_case: RuntimeCursorEndBoundTestCase,
) -> None:
    connection: duckdb.DuckDBPyConnection = duckdb.connect(":memory:")
    connection.execute(
        f"CREATE TABLE upstream_data (cursor_value {test_case.warehouse_column_type})"
    )
    connection.execute("INSERT INTO upstream_data VALUES (?)", [test_case.upstream_min])
    connection.execute("INSERT INTO upstream_data VALUES (?)", [test_case.upstream_max])

    cursor_bounds: CursorBounds | None = resolve_runtime_cursor_bounds(
        adapter=cast(BaseAdapter, FakeCursorAdapter()),
        connection=connection,
        target_relation="target_data",
        target_database=None,
        target_schema=None,
        target_name="target_data",
        spec=RuntimeCursorSpec(
            cursor_column="cursor_value",
            cursor_type=test_case.cursor_type,
            cursor_grain=test_case.cursor_grain,
            cursor_start=None,
            cursor_input_relations=(
                RuntimeCursorInputRelation(
                    relation="upstream_data",
                    cursor_column="cursor_value",
                ),
            ),
        ),
    )

    assert cursor_bounds is not None
    assert sentinel_to_token(sentinel=cursor_bounds.start) == test_case.expected_start
    assert sentinel_to_token(sentinel=cursor_bounds.end) == test_case.expected_end


@pytest.mark.parametrize(
    "test_case",
    [
        RuntimeCursorOverrideTestCase(
            description="end-cursor-ts override clamps a day-grain date range to the requested end",
            upstream_min=date(2014, 1, 1),
            upstream_max=date(2014, 12, 31),
            cursor_type=CursorType.TIMESTAMP,
            cursor_grain=CursorGrain.DAY,
            warehouse_column_type="DATE",
            start_cursor_override=None,
            end_cursor_override="2014-03-30",
            expected_start="2014-01-01",
            expected_end="2014-03-31",
        ),
        RuntimeCursorOverrideTestCase(
            description="start-cursor-ts override raises the start floor above the upstream min",
            upstream_min=date(2014, 1, 1),
            upstream_max=date(2014, 12, 31),
            cursor_type=CursorType.TIMESTAMP,
            cursor_grain=CursorGrain.DAY,
            warehouse_column_type="DATE",
            start_cursor_override="2014-02-01",
            end_cursor_override="2014-03-30",
            expected_start="2014-02-01",
            expected_end="2014-03-31",
        ),
        RuntimeCursorOverrideTestCase(
            description="end-cursor-ts override wider than the data never widens the window",
            upstream_min=date(2014, 1, 1),
            upstream_max=date(2014, 12, 31),
            cursor_type=CursorType.TIMESTAMP,
            cursor_grain=CursorGrain.DAY,
            warehouse_column_type="DATE",
            start_cursor_override=None,
            end_cursor_override="2015-06-01",
            expected_start="2014-01-01",
            expected_end="2015-01-01",
        ),
        RuntimeCursorOverrideTestCase(
            description="integer end-cursor override clamps the exclusive upper bound",
            upstream_min=50,
            upstream_max=200,
            cursor_type=CursorType.INTEGER,
            cursor_grain=None,
            warehouse_column_type="INTEGER",
            start_cursor_override="100",
            end_cursor_override="150",
            expected_start="100",
            expected_end="151",
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_cursor_overrides_when_resolving_runtime_bounds_then_clamps_to_requested_window(
    test_case: RuntimeCursorOverrideTestCase,
) -> None:
    connection: duckdb.DuckDBPyConnection = duckdb.connect(":memory:")
    connection.execute(
        f"CREATE TABLE upstream_data (cursor_value {test_case.warehouse_column_type})"
    )
    connection.execute("INSERT INTO upstream_data VALUES (?)", [test_case.upstream_min])
    connection.execute("INSERT INTO upstream_data VALUES (?)", [test_case.upstream_max])

    cursor_bounds: CursorBounds | None = resolve_runtime_cursor_bounds(
        adapter=cast(BaseAdapter, FakeCursorAdapter()),
        connection=connection,
        target_relation="target_data",
        target_database=None,
        target_schema=None,
        target_name="target_data",
        spec=RuntimeCursorSpec(
            cursor_column="cursor_value",
            cursor_type=test_case.cursor_type,
            cursor_grain=test_case.cursor_grain,
            cursor_start=None,
            cursor_input_relations=(
                RuntimeCursorInputRelation(
                    relation="upstream_data",
                    cursor_column="cursor_value",
                ),
            ),
            start_cursor_override=test_case.start_cursor_override,
            end_cursor_override=test_case.end_cursor_override,
        ),
    )

    assert cursor_bounds is not None
    assert sentinel_to_token(sentinel=cursor_bounds.start) == test_case.expected_start
    assert sentinel_to_token(sentinel=cursor_bounds.end) == test_case.expected_end


@pytest.mark.parametrize(
    "test_case",
    [
        RuntimeExistingTargetOverrideTestCase(
            description="explicit historical replay replaces a newer target high-water mark",
            upstream_min=date(2014, 1, 1),
            upstream_max=date(2014, 12, 31),
            target_max=date(2014, 12, 31),
            cursor_type=CursorType.TIMESTAMP,
            cursor_grain=CursorGrain.DAY,
            cursor_start="2014-01-01",
            start_cursor_override="2014-01-01",
            end_cursor_override="2014-03-30",
            warehouse_column_type="DATE",
            expected_bounds=CursorBounds(start="2014-01-01", end="2014-03-31"),
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_existing_newer_target_when_resolving_explicit_replay_then_uses_requested_start(
    test_case: RuntimeExistingTargetOverrideTestCase,
) -> None:
    connection: duckdb.DuckDBPyConnection = duckdb.connect(":memory:")
    connection.execute(
        f"CREATE TABLE upstream_data (cursor_value {test_case.warehouse_column_type})"
    )
    connection.execute("INSERT INTO upstream_data VALUES (?)", [test_case.upstream_min])
    connection.execute("INSERT INTO upstream_data VALUES (?)", [test_case.upstream_max])
    connection.execute(f"CREATE TABLE target_data (cursor_value {test_case.warehouse_column_type})")
    connection.execute("INSERT INTO target_data VALUES (?)", [test_case.target_max])

    cursor_bounds: CursorBounds | None = resolve_runtime_cursor_bounds(
        adapter=cast(BaseAdapter, FakeCursorAdapter(target_relation_exists=True)),
        connection=connection,
        target_relation="target_data",
        target_database=None,
        target_schema=None,
        target_name="target_data",
        spec=RuntimeCursorSpec(
            cursor_column="cursor_value",
            cursor_type=test_case.cursor_type,
            cursor_grain=test_case.cursor_grain,
            cursor_start=test_case.cursor_start,
            cursor_input_relations=(
                RuntimeCursorInputRelation(
                    relation="upstream_data",
                    cursor_column="cursor_value",
                    cursor_grain=test_case.cursor_grain,
                    is_model_backed=True,
                ),
            ),
            start_cursor_override=test_case.start_cursor_override,
            end_cursor_override=test_case.end_cursor_override,
        ),
    )

    assert cursor_bounds == test_case.expected_bounds


@pytest.mark.parametrize(
    "test_case",
    [
        RuntimeTargetMaxTestCase(
            description="existing target relation seeds start from target max",
            target_rows=(80, 100),
            upstream_min=50,
            upstream_max=200,
            cursor_type=CursorType.INTEGER,
            expected_start="100",
            expected_end="201",
        )
    ],
    ids=lambda case: case.description,
)
def test_given_existing_target_relation_when_resolving_bounds_then_starts_from_target_max(
    test_case: RuntimeTargetMaxTestCase,
) -> None:
    connection: duckdb.DuckDBPyConnection = duckdb.connect(":memory:")
    connection.execute("CREATE TABLE upstream_data (cursor_value INTEGER)")
    connection.execute("INSERT INTO upstream_data VALUES (?)", [test_case.upstream_min])
    connection.execute("INSERT INTO upstream_data VALUES (?)", [test_case.upstream_max])
    connection.execute("CREATE TABLE target_data (cursor_value INTEGER)")
    target_row: object
    for target_row in test_case.target_rows:
        connection.execute("INSERT INTO target_data VALUES (?)", [target_row])

    cursor_bounds: CursorBounds | None = resolve_runtime_cursor_bounds(
        adapter=cast(BaseAdapter, FakeCursorAdapter(target_relation_exists=True)),
        connection=connection,
        target_relation="target_data",
        target_database=None,
        target_schema=None,
        target_name="target_data",
        spec=RuntimeCursorSpec(
            cursor_column="cursor_value",
            cursor_type=test_case.cursor_type,
            cursor_grain=None,
            cursor_start=None,
            cursor_input_relations=(
                RuntimeCursorInputRelation(
                    relation="upstream_data",
                    cursor_column="cursor_value",
                ),
            ),
        ),
    )

    assert cursor_bounds is not None
    assert sentinel_to_token(sentinel=cursor_bounds.start) == test_case.expected_start
    assert sentinel_to_token(sentinel=cursor_bounds.end) == test_case.expected_end


@pytest.mark.parametrize(
    "test_case",
    [
        RuntimeTargetProbeFailureTestCase(
            description="target max query failure propagates instead of widening the window",
            expected_error_type=duckdb.CatalogException,
        )
    ],
    ids=lambda case: case.description,
)
def test_given_target_max_query_failure_when_resolving_bounds_then_propagates_error(
    test_case: RuntimeTargetProbeFailureTestCase,
) -> None:
    connection: duckdb.DuckDBPyConnection = duckdb.connect(":memory:")
    connection.execute("CREATE TABLE upstream_data (cursor_value INTEGER)")
    connection.execute("INSERT INTO upstream_data VALUES (50)")
    connection.execute("INSERT INTO upstream_data VALUES (200)")

    with pytest.raises(test_case.expected_error_type):
        resolve_runtime_cursor_bounds(
            adapter=cast(BaseAdapter, FakeCursorAdapter(target_relation_exists=True)),
            connection=connection,
            target_relation="target_data",
            target_database=None,
            target_schema=None,
            target_name="target_data",
            spec=RuntimeCursorSpec(
                cursor_column="cursor_value",
                cursor_type=CursorType.INTEGER,
                cursor_grain=None,
                cursor_start=None,
                cursor_input_relations=(
                    RuntimeCursorInputRelation(
                        relation="upstream_data",
                        cursor_column="cursor_value",
                    ),
                ),
            ),
        )


@pytest.mark.parametrize(
    "test_case",
    [
        RuntimeCursorPolicyTestCase(
            description="multiple inputs use the slowest common watermark",
            expected_bounds=CursorBounds(start="10", end="101"),
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_multiple_runtime_inputs_when_resolving_then_uses_common_watermark(
    test_case: RuntimeCursorPolicyTestCase,
) -> None:
    connection: duckdb.DuckDBPyConnection = duckdb.connect(":memory:")
    connection.execute("CREATE TABLE fast_input (cursor_value INTEGER)")
    connection.execute("INSERT INTO fast_input VALUES (10), (200)")
    connection.execute("CREATE TABLE slow_input (cursor_value INTEGER)")
    connection.execute(test_case.slow_input_setup_sql)

    bounds: CursorBounds | None = resolve_runtime_cursor_bounds(
        adapter=cast(BaseAdapter, FakeCursorAdapter()),
        connection=connection,
        target_relation="target_data",
        target_database=None,
        target_schema=None,
        target_name="target_data",
        spec=RuntimeCursorSpec(
            cursor_column="cursor_value",
            cursor_type=CursorType.INTEGER,
            cursor_grain=None,
            cursor_start=None,
            cursor_input_relations=(
                RuntimeCursorInputRelation(relation="fast_input", cursor_column="cursor_value"),
                RuntimeCursorInputRelation(relation="slow_input", cursor_column="cursor_value"),
            ),
        ),
    )

    assert bounds == test_case.expected_bounds


@pytest.mark.parametrize(
    "test_case",
    [
        RuntimeCursorFailureTestCase(
            description="all mode reports an empty required input",
            fast_input_setup_sql="INSERT INTO fast_input VALUES (10), (200)",
            expected_input_names=("stg_refunds.cursor_value",),
            expected_waiting_on_empty_inputs=True,
        ),
        RuntimeCursorFailureTestCase(
            description="legacy mode waits on an empty input like planned windows",
            fast_input_setup_sql="INSERT INTO fast_input VALUES (10), (200)",
            expected_input_names=("stg_refunds.cursor_value",),
            expected_waiting_on_empty_inputs=True,
            cursor_watermark_mode=RuntimeCursorWatermarkMode.LEGACY,
        ),
        RuntimeCursorFailureTestCase(
            description="legacy mode reports inputs that are all empty",
            fast_input_setup_sql="SELECT 1",
            expected_input_names=("stg_payments.cursor_value", "stg_refunds.cursor_value"),
            expected_waiting_on_empty_inputs=False,
            cursor_watermark_mode=RuntimeCursorWatermarkMode.LEGACY,
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_empty_runtime_inputs_when_resolving_then_reports_inputs_without_rows(
    test_case: RuntimeCursorFailureTestCase,
) -> None:
    connection: duckdb.DuckDBPyConnection = duckdb.connect(":memory:")
    connection.execute("CREATE TABLE fast_input (cursor_value INTEGER)")
    connection.execute(test_case.fast_input_setup_sql)
    connection.execute("CREATE TABLE slow_input (cursor_value INTEGER)")

    with pytest.raises(EmptyCursorInputsError) as exc_info:
        resolve_runtime_cursor_bounds(
            adapter=cast(BaseAdapter, FakeCursorAdapter()),
            connection=connection,
            target_relation="target_data",
            target_database=None,
            target_schema=None,
            target_name="target_data",
            spec=RuntimeCursorSpec(
                cursor_column="cursor_value",
                cursor_type=CursorType.INTEGER,
                cursor_grain=None,
                cursor_start=None,
                cursor_watermark_mode=test_case.cursor_watermark_mode,
                cursor_input_relations=(
                    RuntimeCursorInputRelation(
                        relation="fast_input",
                        cursor_column="cursor_value",
                        input_name="stg_payments",
                    ),
                    RuntimeCursorInputRelation(
                        relation="slow_input",
                        cursor_column="cursor_value",
                        input_name="stg_refunds",
                    ),
                ),
            ),
        )

    assert exc_info.value.input_names == test_case.expected_input_names
    assert exc_info.value.waiting_on_empty_inputs is test_case.expected_waiting_on_empty_inputs


@pytest.mark.parametrize(
    "test_case",
    [
        RuntimeCursorPolicyTestCase(
            description="applies lookback from target watermark",
            lookback="20s",
            expected_bounds=CursorBounds(start="80", end="201"),
        ),
        RuntimeCursorPolicyTestCase(
            description="applies bounded replay from exclusive end",
            backfill_duration="30s",
            expected_bounds=CursorBounds(start="171", end="201"),
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_runtime_replay_policy_when_resolving_then_matches_planner_policy(
    test_case: RuntimeCursorPolicyTestCase,
) -> None:
    connection: duckdb.DuckDBPyConnection = duckdb.connect(":memory:")
    connection.execute("CREATE TABLE upstream_data (cursor_value INTEGER)")
    connection.execute("INSERT INTO upstream_data VALUES (50), (200)")
    connection.execute("CREATE TABLE target_data (cursor_value INTEGER)")
    connection.execute("INSERT INTO target_data VALUES (100)")

    bounds: CursorBounds | None = resolve_runtime_cursor_bounds(
        adapter=cast(BaseAdapter, FakeCursorAdapter(target_relation_exists=True)),
        connection=connection,
        target_relation="target_data",
        target_database=None,
        target_schema=None,
        target_name="target_data",
        spec=RuntimeCursorSpec(
            cursor_column="cursor_value",
            cursor_type=CursorType.INTEGER,
            cursor_grain=None,
            cursor_start=None,
            cursor_input_relations=(
                RuntimeCursorInputRelation(relation="upstream_data", cursor_column="cursor_value"),
            ),
            lookback=test_case.lookback,
            backfill_duration=test_case.backfill_duration,
        ),
    )

    assert bounds == test_case.expected_bounds


@pytest.mark.parametrize(
    "test_case",
    [
        RuntimeCursorPolicyTestCase(
            description="eligible target max is floored to mixed common day grain",
            expected_bounds=CursorBounds(start="2026-08-31T00:00:00", end="2026-09-04T00:00:00"),
        )
    ],
    ids=lambda case: case.description,
)
def test_given_mixed_runtime_grains_when_capping_start_then_matches_planner_normalization(
    test_case: RuntimeCursorPolicyTestCase,
) -> None:
    connection: duckdb.DuckDBPyConnection = duckdb.connect(":memory:")
    connection.execute("CREATE TABLE upstream_data (cursor_value TIMESTAMP)")
    connection.execute(
        "INSERT INTO upstream_data VALUES ('2026-08-01 00:00:00'), ('2026-09-03 18:00:00')"
    )
    connection.execute("CREATE TABLE target_data (cursor_value TIMESTAMP)")
    connection.execute(
        "INSERT INTO target_data VALUES ('2026-08-31 18:00:00'), ('2026-09-03 15:00:00')"
    )

    bounds: CursorBounds | None = resolve_runtime_cursor_bounds(
        adapter=DuckDbAdapter(),
        connection=connection,
        target_relation="target_data",
        target_database=None,
        target_schema=None,
        target_name="target_data",
        spec=RuntimeCursorSpec(
            cursor_column="cursor_value",
            cursor_type=CursorType.TIMESTAMP,
            cursor_grain=CursorGrain.HOUR,
            cursor_start=None,
            cursor_input_relations=(
                RuntimeCursorInputRelation(
                    relation="upstream_data",
                    cursor_column="cursor_value",
                    cursor_grain=CursorGrain.DAY,
                ),
            ),
            incremental_strategy="delete_insert",
            start_cursor_config=StartCursorsConfig(max_ahead="0d", action=FutureCursorAction.CAP),
            invocation_time=datetime(2026, 9, 1, 12, tzinfo=UTC),
        ),
    )

    assert bounds is not None
    assert CursorBounds(start=bounds.start, end=bounds.end) == test_case.expected_bounds
    assert bounds.maximum_start_safety is not None
    planner_snapshot: ModelCursorSnapshot = normalize_cursor_snapshot_grain(
        cursor_snapshot=ModelCursorSnapshot(
            target_max="2026-09-03T15:00:00",
            upstream_mins=("2026-08-01T00:00:00",),
            upstream_maxes=("2026-09-03T18:00:00",),
            target_eligible_max="2026-08-31T18:00:00",
        ),
        cursor_type=CursorType.TIMESTAMP,
        effective_grain=CursorGrain.DAY,
    )
    assert (
        bounds.maximum_start_safety.highest_eligible_target_max
        == planner_snapshot.target_eligible_max
    )
    assert planner_snapshot.physical_target_max is not None
    assert render(value=planner_snapshot.physical_target_max) == "2026-09-03T15:00:00"


@pytest.mark.parametrize(
    "test_case",
    [
        RuntimeCursorPolicyTestCase(
            description="full refresh ignores old destination watermark",
            read_destination_cursor=False,
            expected_bounds=CursorBounds(start="10", end="201"),
        )
    ],
    ids=lambda case: case.description,
)
def test_given_full_refresh_runtime_discovery_when_resolving_then_ignores_old_target(
    test_case: RuntimeCursorPolicyTestCase,
) -> None:
    connection: duckdb.DuckDBPyConnection = duckdb.connect(":memory:")
    connection.execute("CREATE TABLE upstream_data (cursor_value INTEGER)")
    connection.execute("INSERT INTO upstream_data VALUES (10), (200)")
    connection.execute("CREATE TABLE target_data (cursor_value INTEGER)")
    connection.execute("INSERT INTO target_data VALUES (150)")

    bounds: CursorBounds | None = resolve_runtime_cursor_bounds(
        adapter=cast(BaseAdapter, FakeCursorAdapter(target_relation_exists=True)),
        connection=connection,
        target_relation="target_data",
        target_database=None,
        target_schema=None,
        target_name="target_data",
        spec=RuntimeCursorSpec(
            cursor_column="cursor_value",
            cursor_type=CursorType.INTEGER,
            cursor_grain=None,
            cursor_start=None,
            cursor_input_relations=(
                RuntimeCursorInputRelation(relation="upstream_data", cursor_column="cursor_value"),
            ),
            read_destination_cursor=test_case.read_destination_cursor,
        ),
    )

    assert bounds == test_case.expected_bounds


@pytest.mark.parametrize(
    "test_case",
    [
        AuthoritativeRuntimeCursorOverrideTestCase(
            description="timestamp override returns without runtime watermark SQL",
            cursor_type=CursorType.TIMESTAMP,
            cursor_grain=CursorGrain.HOUR,
            start_cursor_override="2026-01-02T10:00:00",
            end_cursor_override="2026-01-02T12:00:00",
            expected_bounds=CursorBounds(start="2026-01-02T10:00:00", end="2026-01-02T13:00:00"),
        ),
        AuthoritativeRuntimeCursorOverrideTestCase(
            description="date override returns without runtime watermark SQL",
            cursor_type=CursorType.TIMESTAMP,
            cursor_grain=CursorGrain.DAY,
            start_cursor_override="2026-01-02",
            end_cursor_override="2026-01-04",
            expected_bounds=CursorBounds(start="2026-01-02", end="2026-01-05"),
        ),
        AuthoritativeRuntimeCursorOverrideTestCase(
            description="integer override returns without runtime watermark SQL",
            cursor_type=CursorType.INTEGER,
            cursor_grain=None,
            start_cursor_override="10",
            end_cursor_override="20",
            expected_bounds=CursorBounds(start="10", end="21"),
        ),
        AuthoritativeRuntimeCursorOverrideTestCase(
            description="future timestamp override bypasses configured safety",
            cursor_type=CursorType.TIMESTAMP,
            cursor_grain=CursorGrain.DAY,
            start_cursor_override="2500-01-01",
            end_cursor_override="2500-01-04",
            expected_bounds=CursorBounds(start="2500-01-01", end="2500-01-05"),
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_complete_override_when_runtime_resolution_invoked_then_returns_without_sql(
    test_case: AuthoritativeRuntimeCursorOverrideTestCase,
) -> None:
    connection: duckdb.DuckDBPyConnection = duckdb.connect(":memory:")

    bounds: CursorBounds | None = resolve_runtime_cursor_bounds(
        adapter=cast(BaseAdapter, FakeCursorAdapter()),
        connection=connection,
        target_relation="missing_target",
        target_database=None,
        target_schema=None,
        target_name="missing_target",
        spec=RuntimeCursorSpec(
            cursor_column="cursor_value",
            cursor_type=test_case.cursor_type,
            cursor_grain=test_case.cursor_grain,
            cursor_start=None,
            cursor_input_relations=(
                RuntimeCursorInputRelation(
                    relation="missing_runtime_input",
                    cursor_column="cursor_value",
                    is_runtime_produced=True,
                ),
            ),
            start_cursor_override=test_case.start_cursor_override,
            end_cursor_override=test_case.end_cursor_override,
            future_cursor_config=FutureCursorsConfig("2d", FutureCursorAction.ERROR),
            invocation_time=datetime(2026, 9, 1, 12, 0, tzinfo=UTC),
        ),
    )

    assert bounds == test_case.expected_bounds
