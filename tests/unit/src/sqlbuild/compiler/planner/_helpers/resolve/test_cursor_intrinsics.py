from __future__ import annotations

import pytest

from sqlbuild._native import (
    BackfillAction,
    BackfillResult,
    CursorOverridePair,
    Dialect,
    ModelCursorSnapshot,
    WarehouseSnapshot,
    resolve_model_sql,
)
from sqlbuild.adapters.clickhouse.classes.clickhouse_adapter import ClickHouseAdapter
from sqlbuild.adapters.duckdb.classes.duckdb_adapter import DuckDbAdapter
from sqlbuild.adapters.snowflake.classes.snowflake_adapter import SnowflakeAdapter
from sqlbuild.adapters.sqlite.classes.sqlite_adapter import SqliteAdapter
from sqlbuild.compiler.compile.types import CompiledModelView
from tests.unit.src.sqlbuild.compiler.planner._helpers.resolve._test_types import (
    CursorIntrinsicResolutionTestCase,
    FullBuildIntrinsicTestCase,
)
from tests.unit.src.sqlbuild.compiler.planner._helpers.resolve.helpers import (
    build_cursor_intrinsic_model,
    build_empty_model_plan_context,
)

_TIMESTAMP: dict[str, object] = {
    "materialized": "incremental",
    "cursor": "event_time",
    "cursor_type": "timestamp",
}
_INTEGER: dict[str, object] = {
    "materialized": "incremental",
    "cursor": "event_id",
    "cursor_type": "integer",
}


@pytest.mark.parametrize(
    "test_case",
    [
        CursorIntrinsicResolutionTestCase(
            description="renders planner-owned adapter literals",
            model_config={
                "materialized": "incremental",
                "cursor": "event_time",
                "cursor_type": "timestamp",
            },
            full_refresh=False,
            cursor_snapshots={
                "events": ModelCursorSnapshot(
                    target_max="2026-01-01T00:00:00",
                    upstream_mins=("2025-01-01T00:00:00",),
                    upstream_maxes=("2026-01-02T00:00:00",),
                )
            },
            expected_sql=(
                "SELECT TIMESTAMP '2026-01-01T00:00:00' AS batch_start, "
                "TIMESTAMP '2026-01-02T00:00:01' AS batch_end"
            ),
        ),
        CursorIntrinsicResolutionTestCase(
            description="renders typed full-refresh microbatch sentinels",
            model_config={
                "materialized": "incremental",
                "cursor": "event_time",
                "cursor_type": "timestamp",
                "incremental_mode": "microbatch",
            },
            full_refresh=True,
            cursor_snapshots={},
            expected_sql=(
                "SELECT TIMESTAMP '__SQB_CURSOR_START__' AS batch_start, "
                "TIMESTAMP '__SQB_CURSOR_END__' AS batch_end"
            ),
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_cursor_interval_when_resolving_then_renders_expected_sql(
    test_case: CursorIntrinsicResolutionTestCase,
) -> None:
    model: CompiledModelView = build_cursor_intrinsic_model(config_values=test_case.model_config)

    resolved_sql: str = resolve_model_sql(
        dialect=Dialect(DuckDbAdapter()),
        model=model,
        snapshot=WarehouseSnapshot(cursor_snapshots=test_case.cursor_snapshots),
        context=build_empty_model_plan_context(),
        backfill=BackfillResult(action=BackfillAction.FORWARD_ONLY),
        full_refresh=test_case.full_refresh,
        cursor_overrides=CursorOverridePair(),
    )

    assert resolved_sql == test_case.expected_sql
    assert "__cursor_" not in resolved_sql


@pytest.mark.parametrize(
    "test_case",
    [
        FullBuildIntrinsicTestCase(
            description="DuckDB --full-refresh spans every timestamp",
            adapter=DuckDbAdapter,
            model_config=_TIMESTAMP,
            full_refresh=True,
            full_rebuild_cause=None,
            expected_sql=(
                "SELECT TIMESTAMP '0001-01-01 00:00:00' /* (unbounded) */ AS batch_start, TIMESTAMP '9999-12-31 23:59:59.999999' /* (unbounded) */ AS batch_end"
            ),
        ),
        FullBuildIntrinsicTestCase(
            description="DuckDB first run spans every timestamp",
            adapter=DuckDbAdapter,
            model_config=_TIMESTAMP,
            full_refresh=False,
            full_rebuild_cause="the destination doesn't exist",
            expected_sql=(
                "SELECT TIMESTAMP '0001-01-01 00:00:00' /* (unbounded) */ AS batch_start, TIMESTAMP '9999-12-31 23:59:59.999999' /* (unbounded) */ AS batch_end"
            ),
        ),
        FullBuildIntrinsicTestCase(
            description="DuckDB cursor_start floor starts the span",
            adapter=DuckDbAdapter,
            model_config={**_TIMESTAMP, "cursor_start": "2024-01-01"},
            full_refresh=True,
            full_rebuild_cause=None,
            expected_sql=(
                "SELECT TIMESTAMP '2024-01-01' AS batch_start, TIMESTAMP '9999-12-31 23:59:59.999999' /* (unbounded) */ AS batch_end"
            ),
        ),
        FullBuildIntrinsicTestCase(
            description="DuckDB integer cursor spans the 64-bit range",
            adapter=DuckDbAdapter,
            model_config=_INTEGER,
            full_refresh=True,
            full_rebuild_cause=None,
            expected_sql=(
                "SELECT -9223372036854775808 /* (unbounded) */ AS batch_start, 9223372036854775807 /* (unbounded) */ AS batch_end"
            ),
        ),
        FullBuildIntrinsicTestCase(
            description="DuckDB integer floor starts the span",
            adapter=DuckDbAdapter,
            model_config={**_INTEGER, "cursor_start": 100},
            full_refresh=True,
            full_rebuild_cause=None,
            expected_sql=(
                "SELECT 100 AS batch_start, 9223372036854775807 /* (unbounded) */ AS batch_end"
            ),
        ),
        FullBuildIntrinsicTestCase(
            description="DuckDB date-grain cursor spans every timestamp",
            adapter=DuckDbAdapter,
            model_config={**_TIMESTAMP, "cursor_grain": "day"},
            full_refresh=True,
            full_rebuild_cause=None,
            expected_sql=(
                "SELECT TIMESTAMP '0001-01-01 00:00:00' /* (unbounded) */ AS batch_start, TIMESTAMP '9999-12-31 23:59:59.999999' /* (unbounded) */ AS batch_end"
            ),
        ),
        FullBuildIntrinsicTestCase(
            description="Snowflake --full-refresh spans every timestamp",
            adapter=SnowflakeAdapter,
            model_config=_TIMESTAMP,
            full_refresh=True,
            full_rebuild_cause=None,
            expected_sql=(
                "SELECT '0001-01-01 00:00:00' /* (unbounded) */ AS batch_start, '9999-12-31 23:59:59.999999' /* (unbounded) */ AS batch_end"
            ),
        ),
        FullBuildIntrinsicTestCase(
            description="Snowflake integer cursor spans the 64-bit range",
            adapter=SnowflakeAdapter,
            model_config=_INTEGER,
            full_refresh=True,
            full_rebuild_cause=None,
            expected_sql=(
                "SELECT -9223372036854775808 /* (unbounded) */ AS batch_start, 9223372036854775807 /* (unbounded) */ AS batch_end"
            ),
        ),
        FullBuildIntrinsicTestCase(
            description="ClickHouse spans DateTime64's range",
            adapter=ClickHouseAdapter,
            model_config=_TIMESTAMP,
            full_refresh=True,
            full_rebuild_cause=None,
            expected_sql=(
                "SELECT parseDateTime64BestEffort('1900-01-01 00:00:00', 6, 'UTC') /* (unbounded) */ AS batch_start, parseDateTime64BestEffort('2299-12-31 23:59:59.999999', 6, 'UTC') /* (unbounded) */ AS batch_end"
            ),
        ),
        FullBuildIntrinsicTestCase(
            description="SQLite spans every timestamp as sortable text",
            adapter=SqliteAdapter,
            model_config=_TIMESTAMP,
            full_refresh=True,
            full_rebuild_cause=None,
            expected_sql=(
                "SELECT '0001-01-01 00:00:00' /* (unbounded) */ AS batch_start, '9999-12-31 23:59:59.999999' /* (unbounded) */ AS batch_end"
            ),
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_a_full_build_when_resolving_then_intrinsics_span_the_cursor_range(
    test_case: FullBuildIntrinsicTestCase,
) -> None:
    model: CompiledModelView = build_cursor_intrinsic_model(config_values=test_case.model_config)

    resolved_sql: str = resolve_model_sql(
        dialect=Dialect(test_case.adapter()),
        model=model,
        snapshot=WarehouseSnapshot(),
        context=build_empty_model_plan_context(),
        backfill=BackfillResult(action=BackfillAction.FORWARD_ONLY),
        full_refresh=test_case.full_refresh,
        cursor_overrides=CursorOverridePair(),
        replaces_relation=True,
        full_rebuild_cause=test_case.full_rebuild_cause,
    )

    assert resolved_sql == test_case.expected_sql
