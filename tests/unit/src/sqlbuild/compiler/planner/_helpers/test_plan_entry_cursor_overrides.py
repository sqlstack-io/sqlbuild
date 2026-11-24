"""Tests for plan-entry cursor override handling."""

from __future__ import annotations

from dataclasses import replace
from pathlib import Path

import pytest

from sqlbuild._native import (
    MICROBATCH_END_SENTINEL,
    MICROBATCH_START_SENTINEL,
    BackfillAction,
    BackfillResult,
    CompiledModel,
    CompiledObjectKey,
    CompiledRelationLocation,
    CompiledResourceType,
    CompileModelConfig,
    CursorBounds,
    CursorInputRelation,
    ModelCursorSnapshot,
    WarehouseSnapshot,
    compute_microbatch_range,
    compute_plan_cursor_bounds,
)
from sqlbuild._native import cursor_sentinel_to_token as sentinel_to_token
from sqlbuild.compiler.compile.types import CompiledModelView
from tests.unit.src.sqlbuild.compiler.planner._helpers._test_types import (
    AuthoritativeCursorOverrideTestCase,
    MicrobatchCursorEndPlanTestCase,
    PlanEntryCursorGrainTestCase,
    PlanEntryCursorOverrideTestCase,
    PlannerOwnedMixedGrainRangeTestCase,
)
from tests.unit.src.sqlbuild.compiler.planner._helpers.helpers import (
    microbatch_model_with_cursor_end,
)


@pytest.mark.parametrize(
    "test_case",
    (
        MicrobatchCursorEndPlanTestCase(
            description="timestamp microbatch keeps runtime sentinels and clamps concrete range",
            cursor_type="timestamp",
            cursor_grain="month",
            authored_cursor_grain={"cursor_grain": "month"},
            batch_size="1mo",
            cursor_end="2025-12-01",
            target_max="2025-10-01",
            upstream_min="2025-01-01",
            upstream_max="2026-02-01",
            expected_microbatch_range=CursorBounds(start="2025-09-01T00:00:00", end="2025-12-01"),
        ),
        MicrobatchCursorEndPlanTestCase(
            description="integer microbatch keeps runtime sentinels and clamps concrete range",
            cursor_type="integer",
            cursor_grain=None,
            authored_cursor_grain={},
            batch_size="5",
            cursor_end="20",
            target_max="10",
            upstream_min="0",
            upstream_max="30",
            expected_microbatch_range=CursorBounds(start="10", end="20"),
        ),
    ),
    ids=lambda case: case.description,
)
def test_given_planner_owned_microbatch_with_cursor_end_when_planning_then_sentinels_are_deferred(
    test_case: MicrobatchCursorEndPlanTestCase,
) -> None:
    model: CompiledModelView = microbatch_model_with_cursor_end(test_case)
    snapshot: WarehouseSnapshot = WarehouseSnapshot(
        cursor_snapshots={
            model.name: ModelCursorSnapshot(
                target_max=test_case.target_max,
                upstream_mins=(test_case.upstream_min,),
                upstream_maxes=(test_case.upstream_max,),
            )
        }
    )
    backfill: BackfillResult = BackfillResult(action=BackfillAction.FORWARD_ONLY)

    plan_bounds: CursorBounds | None = compute_plan_cursor_bounds(
        model=model,
        snapshot=snapshot,
        backfill=backfill,
        full_refresh=False,
        start_cursor_override=None,
        end_cursor_override=None,
        runtime_owned_cursor_bounds=False,
    )
    microbatch_range: CursorBounds | None = compute_microbatch_range(
        model=model,
        snapshot=snapshot,
        backfill=backfill,
        start_cursor_override=None,
        end_cursor_override=None,
        runtime_owned_cursor_bounds=False,
        cursor_input_relations=(
            CursorInputRelation(relation="main.raw_orders", cursor_column="cursor_value"),
        ),
        future_cursor_config=None,
        start_cursor_config=None,
        invocation_time=None,
        full_refresh=False,
    )

    assert plan_bounds == CursorBounds(
        start=MICROBATCH_START_SENTINEL,
        end=MICROBATCH_END_SENTINEL,
    )
    assert microbatch_range == test_case.expected_microbatch_range


@pytest.mark.parametrize(
    "test_case",
    [
        PlanEntryCursorOverrideTestCase(
            description="explicit overrides advance the inclusive end to an exclusive bound",
            start_cursor_override="2026-01-02T00:00:00",
            end_cursor_override="2026-01-04T00:00:00",
            expected_bounds=CursorBounds(
                start="2026-01-02T00:00:00",
                end="2026-01-04T00:00:01",
            ),
        )
    ],
    ids=lambda case: case.description,
)
def test_given_explicit_cursor_overrides_without_snapshot_when_planning_then_uses_overrides(
    test_case: PlanEntryCursorOverrideTestCase,
) -> None:
    model: CompiledModelView = CompiledModel(
        key=CompiledObjectKey(resource_type=CompiledResourceType.MODEL, name="orders"),
        deps=(),
        name="orders",
        relative_path=Path("sql/models/orders.sql"),
        query_sql="SELECT 1",
        references=(),
        config=CompileModelConfig(
            values={
                "materialized": "incremental",
                "incremental_strategy": "delete_insert",
                "cursor": "ordered_at",
                "cursor_type": "timestamp",
            }
        ),
        destination=CompiledRelationLocation(
            database=None,
            schema="dev__physical",
            name="orders__v_123",
            qualified_name="dev__physical.orders__v_123",
        ),
    )

    bounds: CursorBounds | None = compute_plan_cursor_bounds(
        model=model,
        snapshot=WarehouseSnapshot(),
        backfill=BackfillResult(action=BackfillAction.BOUNDED, duration="7d"),
        full_refresh=False,
        start_cursor_override=test_case.start_cursor_override,
        end_cursor_override=test_case.end_cursor_override,
        runtime_owned_cursor_bounds=True,
    )

    assert bounds == test_case.expected_bounds


@pytest.mark.parametrize(
    "test_case",
    [
        AuthoritativeCursorOverrideTestCase(
            description="timestamp hour override controls normal and microbatch ranges",
            cursor_type="timestamp",
            cursor_grain="hour",
            authored_cursor_grain={"cursor_grain": "hour"},
            batch_size="1h",
            start_override="2026-01-02T10:00:00",
            end_override="2026-01-02T12:00:00",
            expected_bounds=CursorBounds(start="2026-01-02T10:00:00", end="2026-01-02T13:00:00"),
        ),
        AuthoritativeCursorOverrideTestCase(
            description="plain date day override controls normal and microbatch ranges",
            cursor_type="timestamp",
            cursor_grain="day",
            authored_cursor_grain={"cursor_grain": "day"},
            batch_size="1d",
            start_override="2026-01-02",
            end_override="2026-01-04",
            expected_bounds=CursorBounds(start="2026-01-02", end="2026-01-05"),
        ),
        AuthoritativeCursorOverrideTestCase(
            description="integer override controls normal and microbatch ranges",
            cursor_type="integer",
            cursor_grain=None,
            authored_cursor_grain={},
            batch_size="5",
            start_override="10",
            end_override="20",
            expected_bounds=CursorBounds(start="10", end="21"),
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_runtime_owned_missing_snapshot_when_overrides_complete_then_all_ranges_match(
    test_case: AuthoritativeCursorOverrideTestCase,
) -> None:
    model: CompiledModelView = CompiledModel(
        key=CompiledObjectKey(resource_type=CompiledResourceType.MODEL, name="orders"),
        deps=(),
        name="orders",
        relative_path=Path("sql/models/orders.sql"),
        query_sql="SELECT 1",
        references=(),
        config=CompileModelConfig(
            values={
                "materialized": "incremental",
                "incremental_strategy": "delete_insert",
                "cursor": "cursor_value",
                "cursor_type": test_case.cursor_type,
                **test_case.authored_cursor_grain,
            }
        ),
        destination=CompiledRelationLocation(
            database=None,
            schema="analytics",
            name="orders",
            qualified_name="analytics.orders",
        ),
    )
    snapshot: WarehouseSnapshot = WarehouseSnapshot(
        cursor_snapshots={
            "orders": ModelCursorSnapshot(
                target_max=None,
                upstream_mins=(),
                upstream_maxes=(),
                unavailable_watermark_tags=("orders__producer__max",),
            )
        }
    )

    microbatch_model: CompiledModelView = replace(
        model,
        config=CompileModelConfig(
            values={
                **model.config.values,
                "incremental_mode": "microbatch",
                "batch_size": test_case.batch_size,
            }
        ),
    )
    plan_bounds: CursorBounds | None = compute_plan_cursor_bounds(
        model=model,
        snapshot=snapshot,
        backfill=BackfillResult(action=BackfillAction.FORWARD_ONLY),
        full_refresh=False,
        start_cursor_override=test_case.start_override,
        end_cursor_override=test_case.end_override,
        runtime_owned_cursor_bounds=True,
    )
    microbatch_range: CursorBounds | None = compute_microbatch_range(
        model=microbatch_model,
        snapshot=snapshot,
        backfill=BackfillResult(action=BackfillAction.FORWARD_ONLY),
        start_cursor_override=test_case.start_override,
        end_cursor_override=test_case.end_override,
        runtime_owned_cursor_bounds=True,
        cursor_input_relations=(),
        future_cursor_config=None,
        start_cursor_config=None,
        invocation_time=None,
        full_refresh=False,
    )

    assert plan_bounds == test_case.expected_bounds
    assert microbatch_range == test_case.expected_bounds


@pytest.mark.parametrize(
    "test_case",
    [
        PlannerOwnedMixedGrainRangeTestCase(
            description="planner-owned day input replays the coarsest timestamp bucket",
            target_max="2026-04-04 14:00:00",
            upstream_max="2026-04-04 00:00:00",
            downstream_grain="hour",
            upstream_grain="day",
            batch_size="6h",
            expected_bounds=CursorBounds(
                start="2026-04-03T18:00:00",
                end="2026-04-05T00:00:00",
            ),
        ),
        PlannerOwnedMixedGrainRangeTestCase(
            description="planner-owned same-grain input aligns target and watermark bounds",
            target_max="2026-04-04 14:37:00",
            upstream_max="2026-04-04 16:37:00",
            downstream_grain="hour",
            upstream_grain="hour",
            batch_size="1h",
            expected_bounds=CursorBounds(
                start="2026-04-04T13:00:00",
                end="2026-04-04T17:00:00",
            ),
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_planner_owned_model_backed_input_when_grain_is_coarser_then_range_is_concrete(
    test_case: PlannerOwnedMixedGrainRangeTestCase,
) -> None:
    model: CompiledModelView = CompiledModel(
        key=CompiledObjectKey(resource_type=CompiledResourceType.MODEL, name="hourly_context"),
        deps=(),
        name="hourly_context",
        relative_path=Path("sql/models/hourly_context.sql"),
        query_sql="SELECT 1",
        references=(),
        config=CompileModelConfig(
            values={
                "materialized": "incremental",
                "incremental_strategy": "delete_insert",
                "incremental_mode": "microbatch",
                "batch_size": test_case.batch_size,
                "cursor": "activity_hour",
                "cursor_type": "timestamp",
                "cursor_grain": test_case.downstream_grain,
            }
        ),
        destination=CompiledRelationLocation(
            database=None,
            schema="main",
            name="hourly_context",
            qualified_name="main.hourly_context",
        ),
    )
    snapshot: WarehouseSnapshot = WarehouseSnapshot(
        cursor_snapshots={
            "hourly_context": ModelCursorSnapshot(
                target_max=test_case.target_max,
                upstream_mins=(),
                upstream_maxes=(test_case.upstream_max,),
                expected_watermark_count=1,
            )
        }
    )

    microbatch_range: CursorBounds | None = compute_microbatch_range(
        model=model,
        snapshot=snapshot,
        backfill=BackfillResult(action=BackfillAction.FORWARD_ONLY),
        start_cursor_override=None,
        end_cursor_override=None,
        runtime_owned_cursor_bounds=False,
        cursor_input_relations=(
            CursorInputRelation(
                relation="main.daily_rollup",
                cursor_column="activity_day",
                cursor_grain=test_case.upstream_grain,
                is_model_backed=True,
                is_runtime_produced=False,
            ),
        ),
        future_cursor_config=None,
        start_cursor_config=None,
        invocation_time=None,
        full_refresh=False,
    )

    assert microbatch_range == test_case.expected_bounds


@pytest.mark.parametrize(
    "test_case",
    [
        PlanEntryCursorGrainTestCase(
            description="hour grain aligns plan DML bound to the next hour boundary",
            cursor_grain="hour",
            upstream_max="2026-01-04T12:37:00",
            expected_end="2026-01-04T13:00:00",
        )
    ],
    ids=lambda case: case.description,
)
def test_given_snapshot_cursor_grain_when_planning_then_dml_bound_matches_query_interval(
    test_case: PlanEntryCursorGrainTestCase,
) -> None:
    model: CompiledModelView = CompiledModel(
        key=CompiledObjectKey(resource_type=CompiledResourceType.MODEL, name="orders"),
        deps=(),
        name="orders",
        relative_path=Path("sql/models/orders.sql"),
        query_sql="SELECT 1",
        references=(),
        config=CompileModelConfig(
            values={
                "materialized": "incremental",
                "incremental_strategy": "delete_insert",
                "cursor": "ordered_at",
                "cursor_type": "timestamp",
                "cursor_grain": test_case.cursor_grain,
            }
        ),
        destination=CompiledRelationLocation(
            database=None,
            schema="dev__physical",
            name="orders__v_123",
            qualified_name="dev__physical.orders__v_123",
        ),
    )

    bounds: CursorBounds | None = compute_plan_cursor_bounds(
        model=model,
        snapshot=WarehouseSnapshot(
            cursor_snapshots={
                "orders": ModelCursorSnapshot(
                    target_max="2026-01-04T11:00:00",
                    upstream_mins=("2026-01-01T00:00:00",),
                    upstream_maxes=(test_case.upstream_max,),
                )
            }
        ),
        backfill=BackfillResult(action=BackfillAction.FORWARD_ONLY),
        full_refresh=False,
        start_cursor_override=None,
        end_cursor_override=None,
        runtime_owned_cursor_bounds=False,
    )

    assert bounds is not None
    assert sentinel_to_token(sentinel=bounds.end) == test_case.expected_end
