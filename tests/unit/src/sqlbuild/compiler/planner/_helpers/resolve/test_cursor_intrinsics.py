from __future__ import annotations

import pytest

from sqlbuild._native import (
    BackfillAction,
    BackfillResult,
    CursorOverridePair,
    ModelCursorSnapshot,
    PlannerInputError,
    WarehouseSnapshot,
    resolve_model_sql,
)
from sqlbuild.adapters.duckdb.classes.duckdb_adapter import DuckDbAdapter
from sqlbuild.compiler.compile.types import CompiledModelView
from tests.unit.src.sqlbuild.compiler.planner._helpers.resolve._test_types import (
    CursorIntrinsicResolutionTestCase,
)
from tests.unit.src.sqlbuild.compiler.planner._helpers.resolve.helpers import (
    build_cursor_intrinsic_model,
    build_empty_model_plan_context,
)


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
        adapter=DuckDbAdapter(),
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
        CursorIntrinsicResolutionTestCase(
            description="rejects non-microbatch full refresh",
            model_config={
                "materialized": "incremental",
                "cursor": "event_time",
                "cursor_type": "timestamp",
            },
            full_refresh=True,
            cursor_snapshots={},
            expected_error_fragment=(
                r"given a FULL backfill \(--full-refresh\).*full rebuild has no cursor interval"
            ),
        )
    ],
    ids=lambda case: case.description,
)
def test_given_missing_cursor_interval_when_resolving_then_rejects_intrinsics(
    test_case: CursorIntrinsicResolutionTestCase,
) -> None:
    model: CompiledModelView = build_cursor_intrinsic_model(config_values=test_case.model_config)

    with pytest.raises(PlannerInputError, match=test_case.expected_error_fragment) as raised:
        resolve_model_sql(
            adapter=DuckDbAdapter(),
            model=model,
            snapshot=WarehouseSnapshot(),
            context=build_empty_model_plan_context(),
            backfill=BackfillResult(action=BackfillAction.FORWARD_ONLY),
            full_refresh=test_case.full_refresh,
            cursor_overrides=CursorOverridePair(),
        )
    assert raised.value.code == "S204"
