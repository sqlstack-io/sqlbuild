"""Tests for planner-owned microbatch limit decisions."""

from pathlib import Path

import pytest

from sqlbuild._native import (
    CompiledObjectKey,
    CompiledRelationLocation,
    CompiledResourceType,
    CompileInputError,
    CursorBounds,
    CursorInputRelation,
    IncrementalMode,
    MaterializationType,
    MicrobatchLimitAction,
    ModelPlanEntry,
    PlanAction,
    PlanEntryBuildInputs,
    PlanReason,
    PlanWarning,
    apply_configured_microbatch_limits,
    apply_microbatch_limit,
)
from sqlbuild._native import cursor_sentinel_to_token as sentinel_to_token
from tests.unit.src.sqlbuild.compiler.planner._helpers._test_types import (
    ConfiguredMicrobatchPolicyTestCase,
    EffectiveMicrobatchLimitPlanningTestCase,
    MicrobatchLimitPlanningErrorTestCase,
    MicrobatchLimitPlanningTestCase,
    RollingWindowProjectLimitTestCase,
)


@pytest.mark.parametrize(
    "test_case",
    [
        ConfiguredMicrobatchPolicyTestCase(
            description="cli authorization replaces model cap with hard ceiling",
            configured_limit=35,
            configured_action="warn",
            is_cli_override=True,
            expected_limit=35,
            expected_action="error",
            expected_range_start="2026-01-01T00:00:00",
            expected_warning_count=0,
        ),
        ConfiguredMicrobatchPolicyTestCase(
            description="project ceiling remains outside model cap",
            configured_limit=40,
            configured_action="error",
            is_cli_override=False,
            expected_limit=7,
            expected_action="cap_from_end",
            expected_range_start="2026-01-29T00:00:00",
            expected_warning_count=1,
            expected_safety_limit=40,
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_nested_model_policy_when_applying_outer_limit_then_precedence_is_explicit(
    test_case: ConfiguredMicrobatchPolicyTestCase,
) -> None:
    entry: ModelPlanEntry = ModelPlanEntry(
        key=CompiledObjectKey(resource_type=CompiledResourceType.MODEL, name="events"),
        name="events",
        relative_path=Path("sql/models/events.sql"),
        materialization_type=MaterializationType.INCREMENTAL,
        action=PlanAction.INCREMENTAL_DELETE_INSERT,
        reason=PlanReason.NORMAL_INCREMENTAL,
        destination=CompiledRelationLocation(None, "main", "events", "main.events"),
        fingerprint_query_sql="SELECT 1",
        resolved_sql="SELECT 1",
        logical_ddl="",
        incremental_mode=IncrementalMode.MICROBATCH,
        microbatch_strategy="watermark",
        cursor_type="timestamp",
        batch_size="1d",
        microbatch_range=CursorBounds(start="2026-01-01T00:00:00", end="2026-02-05T00:00:00"),
        microbatch_limit=7,
        declared_microbatch_limit_action=MicrobatchLimitAction.CAP_FROM_END,
    )

    limited_entry, warnings = apply_configured_microbatch_limits(
        entry=entry,
        inputs=PlanEntryBuildInputs(
            max_microbatches=test_case.configured_limit,
            max_microbatches_is_override=test_case.is_cli_override,
            microbatch_limit_action=MicrobatchLimitAction(test_case.configured_action),
        ),
    )

    assert limited_entry.microbatch_limit == test_case.expected_limit
    assert limited_entry.microbatch_limit_action == MicrobatchLimitAction(test_case.expected_action)
    assert limited_entry.microbatch_range is not None
    assert (
        sentinel_to_token(sentinel=limited_entry.microbatch_range.start)
        == test_case.expected_range_start
    )
    assert limited_entry.microbatch_safety_limit == test_case.expected_safety_limit
    assert len(warnings) == test_case.expected_warning_count


@pytest.mark.parametrize(
    "test_case",
    [
        EffectiveMicrobatchLimitPlanningTestCase(
            description="effective batch uses coarse replay grain for limit",
            batch_size="effective",
            expected_count=3,
            expected_warning=False,
        ),
        EffectiveMicrobatchLimitPlanningTestCase(
            description="fixed daily batch stays fixed for coarse replay grain limit",
            batch_size="1d",
            expected_count=90,
            expected_warning=True,
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_coarse_effective_grain_when_applying_limit_then_authored_batch_semantics_are_used(
    test_case: EffectiveMicrobatchLimitPlanningTestCase,
) -> None:
    entry: ModelPlanEntry = ModelPlanEntry(
        key=CompiledObjectKey(resource_type=CompiledResourceType.MODEL, name="events"),
        name="events",
        relative_path=Path("sql/models/events.sql"),
        materialization_type=MaterializationType.INCREMENTAL,
        action=PlanAction.INCREMENTAL_DELETE_INSERT,
        reason=PlanReason.NORMAL_INCREMENTAL,
        destination=CompiledRelationLocation(None, "main", "events", "main.events"),
        fingerprint_query_sql="SELECT 1",
        resolved_sql="SELECT 1",
        logical_ddl="",
        incremental_mode=IncrementalMode.MICROBATCH,
        cursor_type="timestamp",
        cursor_grain="day",
        cursor_input_relations=(
            CursorInputRelation(
                relation="monthly_events",
                cursor_column="event_time",
                cursor_grain="month",
            ),
        ),
        batch_size=test_case.batch_size,
        microbatch_range=CursorBounds(start="2026-01-01", end="2026-04-01"),
    )

    limited_entry, warning = apply_microbatch_limit(
        entry=entry,
        max_batches=3,
        action=MicrobatchLimitAction.WARN,
    )

    assert limited_entry.microbatch_limit_count == test_case.expected_count
    assert (warning is not None) is test_case.expected_warning


@pytest.mark.parametrize(
    "test_case",
    [
        MicrobatchLimitPlanningTestCase(
            description="warn preserves complete planner-owned range",
            max_batches=2,
            action="warn",
            expected_count=3,
            expected_warning=True,
        ),
        MicrobatchLimitPlanningTestCase(
            description="numeric override permits intentional backfill",
            max_batches=3,
            action="error",
            expected_count=3,
            expected_warning=False,
        ),
        MicrobatchLimitPlanningTestCase(
            description="cap from start selects earliest integer batches",
            max_batches=2,
            action="cap_from_start",
            expected_count=3,
            expected_warning=True,
            expected_range_start="0",
            expected_range_end="20",
        ),
        MicrobatchLimitPlanningTestCase(
            description="large integer cap remains arithmetic",
            max_batches=10,
            action="cap_from_start",
            expected_count=20,
            expected_warning=True,
            range_start="0",
            range_end="200000000",
            cursor_type="integer",
            batch_size="10000000",
            expected_range_start="0",
            expected_range_end="100000000",
        ),
        MicrobatchLimitPlanningTestCase(
            description="cap from end selects latest seven daily batches",
            max_batches=7,
            action="cap_from_end",
            expected_count=35,
            expected_warning=True,
            range_start="2026-01-01T00:00:00",
            range_end="2026-02-05T00:00:00",
            cursor_type="timestamp",
            batch_size="1d",
            expected_range_start="2026-01-29T00:00:00",
            expected_range_end="2026-02-05T00:00:00",
        ),
        MicrobatchLimitPlanningTestCase(
            description="cap from end preserves forward alignment with partial final batch",
            max_batches=2,
            action="cap_from_end",
            expected_count=4,
            expected_warning=True,
            range_start="2026-01-01T00:00:00",
            range_end="2026-01-04T12:00:00",
            cursor_type="timestamp",
            batch_size="1d",
            expected_range_start="2026-01-03T00:00:00",
            expected_range_end="2026-01-04T12:00:00",
        ),
        MicrobatchLimitPlanningTestCase(
            description="skipped model does not apply error limit",
            max_batches=1,
            action="error",
            expected_count=None,
            expected_warning=False,
            plan_action="skip",
        ),
        MicrobatchLimitPlanningTestCase(
            description="equal-bound full refresh is one batch",
            max_batches=1,
            action="error",
            expected_count=1,
            expected_warning=False,
            plan_action="create_table",
            range_start="10",
            range_end="10",
            expected_range_start="10",
            expected_range_end="10",
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_planner_owned_range_when_applying_limit_then_expected_decision_is_preserved(
    test_case: MicrobatchLimitPlanningTestCase,
) -> None:
    entry: ModelPlanEntry = ModelPlanEntry(
        key=CompiledObjectKey(resource_type=CompiledResourceType.MODEL, name="events"),
        name="events",
        relative_path=Path("sql/models/events.sql"),
        materialization_type=MaterializationType.INCREMENTAL,
        action=PlanAction(test_case.plan_action),
        reason=PlanReason.NORMAL_INCREMENTAL,
        destination=CompiledRelationLocation(None, "main", "events", "main.events"),
        fingerprint_query_sql="SELECT 1",
        resolved_sql="SELECT 1",
        logical_ddl="",
        incremental_mode=IncrementalMode.MICROBATCH,
        cursor_type=test_case.cursor_type,
        batch_size=test_case.batch_size,
        microbatch_range=CursorBounds(start=test_case.range_start, end=test_case.range_end),
    )
    action: MicrobatchLimitAction = MicrobatchLimitAction(test_case.action)

    limited_entry, warning = apply_microbatch_limit(
        entry=entry, max_batches=test_case.max_batches, action=action
    )

    assert limited_entry.microbatch_limit_count == test_case.expected_count
    assert (warning is not None) is test_case.expected_warning
    assert isinstance(warning, PlanWarning) is test_case.expected_warning
    assert limited_entry.microbatch_range is not None
    assert (
        sentinel_to_token(sentinel=limited_entry.microbatch_range.start)
        == test_case.expected_range_start
    )
    assert (
        sentinel_to_token(sentinel=limited_entry.microbatch_range.end)
        == test_case.expected_range_end
    )


@pytest.mark.parametrize(
    "test_case",
    [
        MicrobatchLimitPlanningErrorTestCase(
            description="error rejects planner-owned range",
            max_batches=2,
            action="error",
            expected_error_fragment="MICROBATCH LIMIT EXCEEDED",
        )
    ],
    ids=lambda case: case.description,
)
def test_given_excess_planner_owned_range_when_action_is_error_then_planning_fails(
    test_case: MicrobatchLimitPlanningErrorTestCase,
) -> None:
    entry: ModelPlanEntry = ModelPlanEntry(
        key=CompiledObjectKey(resource_type=CompiledResourceType.MODEL, name="events"),
        name="events",
        relative_path=Path("sql/models/events.sql"),
        materialization_type=MaterializationType.INCREMENTAL,
        action=PlanAction.INCREMENTAL_DELETE_INSERT,
        reason=PlanReason.NORMAL_INCREMENTAL,
        destination=CompiledRelationLocation(None, "main", "events", "main.events"),
        fingerprint_query_sql="SELECT 1",
        resolved_sql="SELECT 1",
        logical_ddl="",
        incremental_mode=IncrementalMode.MICROBATCH,
        cursor_type="integer",
        batch_size="10",
        microbatch_range=CursorBounds(start="0", end="30"),
    )

    with pytest.raises(CompileInputError, match=test_case.expected_error_fragment):
        apply_microbatch_limit(
            entry=entry,
            max_batches=test_case.max_batches,
            action=MicrobatchLimitAction(test_case.action),
        )


@pytest.mark.parametrize(
    "test_case",
    [
        RollingWindowProjectLimitTestCase(
            description="a first build from cursor_start is held to the project limit",
            action="create_table",
            reason="first_run",
            expected_error_fragment="MICROBATCH LIMIT EXCEEDED",
        ),
        RollingWindowProjectLimitTestCase(
            description="a full refresh is held to the project limit",
            action="create_table",
            reason="full_refresh",
            expected_error_fragment="MICROBATCH LIMIT EXCEEDED",
        ),
        RollingWindowProjectLimitTestCase(
            description="an incremental run over the lookback is not limited",
            action="incremental_delete_insert",
            reason="normal_incremental",
            expected_error_fragment=None,
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_a_rolling_window_entry_when_applying_project_limit_then_only_replacing_builds_are_held(
    test_case: RollingWindowProjectLimitTestCase,
) -> None:
    entry: ModelPlanEntry = ModelPlanEntry(
        key=CompiledObjectKey(resource_type=CompiledResourceType.MODEL, name="events"),
        name="events",
        relative_path=Path("sql/models/events.sql"),
        materialization_type=MaterializationType.INCREMENTAL,
        action=PlanAction(test_case.action),
        reason=PlanReason(test_case.reason),
        destination=CompiledRelationLocation(None, "main", "events", "main.events"),
        fingerprint_query_sql="SELECT 1",
        resolved_sql="SELECT 1",
        logical_ddl="",
        incremental_mode=IncrementalMode.MICROBATCH,
        microbatch_strategy="rolling_window",
        cursor_type="timestamp",
        batch_size="1d",
        microbatch_range=CursorBounds(start="2026-01-01T00:00:00", end="2026-02-05T00:00:00"),
    )
    inputs: PlanEntryBuildInputs = PlanEntryBuildInputs(
        max_microbatches=2,
        max_microbatches_is_override=False,
        microbatch_limit_action=MicrobatchLimitAction.ERROR,
    )

    if test_case.expected_error_fragment is not None:
        with pytest.raises(CompileInputError, match=test_case.expected_error_fragment):
            apply_configured_microbatch_limits(entry=entry, inputs=inputs)
        return
    limited_entry, _ = apply_configured_microbatch_limits(entry=entry, inputs=inputs)
    assert limited_entry.microbatch_range == entry.microbatch_range
