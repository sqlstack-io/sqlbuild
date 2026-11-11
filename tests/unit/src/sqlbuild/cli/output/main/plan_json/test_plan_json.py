"""Tests for JSON output serialization."""

from __future__ import annotations

import json
from typing import Any

import pytest

from sqlbuild._native import (
    BackfillAction,
    CursorBounds,
    CursorInputEvidence,
    CursorInputRelation,
    FutureCursorAction,
    FutureCursorSafetyEvidence,
    MaterializationType,
    MaximumStartSafetyEvidence,
    MicrobatchLimitAction,
    PlanAction,
    PlanOutput,
    PlanReason,
    PythonIdentityStatus,
    PythonPlanEntry,
    PythonRunPhase,
    WarningSeverity,
    format_plan_json,
)
from sqlbuild.compiler.python_nodes.types import PythonNodeKind
from tests.unit.src.sqlbuild.cli.output.main.plan.helpers import (
    build_discovered_provider_usage,
    build_model_entry,
    build_plan_output,
    build_plan_provider_usage,
    build_seed_entry,
    build_source_load_entry,
    build_warning,
)
from tests.unit.src.sqlbuild.cli.output.main.plan_json._test_types import (
    FutureCursorPlanJsonTestCase,
    JsonOutputTestCase,
    MicrobatchLimitPlanJsonTestCase,
    SelectionDiagnosticsPlanJsonTestCase,
)


@pytest.mark.parametrize(
    "test_case",
    [
        MicrobatchLimitPlanJsonTestCase(
            description="microbatch limit decision is preserved",
            expected_limit=2,
            expected_count=3,
            expected_action="warn",
            expected_warning="MICROBATCH LIMIT EXCEEDED",
        )
    ],
    ids=lambda case: case.description,
)
def test_given_microbatch_limit_decision_when_formatting_plan_json_then_metadata_is_preserved(
    test_case: MicrobatchLimitPlanJsonTestCase,
) -> None:
    warning: str = test_case.expected_warning
    plan: PlanOutput = build_plan_output(
        model_entries=(
            build_model_entry(
                name="events",
                incremental_mode="microbatch",
                microbatch_limit=test_case.expected_limit,
                microbatch_limit_count=test_case.expected_count,
                microbatch_limit_action=MicrobatchLimitAction.WARN,
                microbatch_limit_warning=warning,
            ),
        )
    )

    payload: dict[str, object] = json.loads(format_plan_json(plan=plan))
    microbatch: Any = payload["models"][0]["microbatch_state"]  # type: ignore[index]

    assert microbatch == {
        "completion_tracking": "universal",
        "batch_concurrency": 1,
        "unaccounted_partition_policy": None,
        "reconciliation": "runtime",
        "limit": test_case.expected_limit,
        "count": test_case.expected_count,
        "action": test_case.expected_action,
        "warning": test_case.expected_warning,
    }


@pytest.mark.parametrize(
    "test_case",
    [FutureCursorPlanJsonTestCase("future cursor plan structure", "cap")],
    ids=lambda case: case.description,
)
def test_given_future_cursor_cap_when_formatting_plan_json_then_structured_bounds_are_exposed(
    test_case: FutureCursorPlanJsonTestCase,
) -> None:
    evidence: FutureCursorSafetyEvidence = FutureCursorSafetyEvidence(
        action=FutureCursorAction.CAP,
        max_distance="1d",
        invocation_time="2026-09-01T12:00:00+00:00",
        discovered_start="2500-01-01",
        discovered_end="2500-01-02",
        applied_start="2500-01-01",
        applied_end="2026-09-03",
        maximum_allowed_start="2026-09-02",
        maximum_allowed_end="2026-09-03",
        future_start_detected=True,
        future_end_detected=True,
        determining_relation="raw.events",
        determining_cursor_column="occurred_at",
        inputs=(
            CursorInputEvidence(
                relation="raw.events",
                cursor_column="occurred_at",
                minimum="2026-01-01",
                maximum="2500-01-01",
            ),
        ),
    )
    plan: PlanOutput = build_plan_output(
        model_entries=(
            build_model_entry(
                name="events",
                cursor_bounds=CursorBounds(
                    start="2500-01-01",
                    end="2026-09-03",
                    future_safety=evidence,
                ),
            ),
        )
    )

    payload: dict[str, object] = json.loads(format_plan_json(plan=plan))
    model: Any = payload["models"][0]

    assert model["future_cursor_safety"]["action"] == test_case.expected_action
    assert model["future_cursor_safety"]["discovered_bounds"] == {
        "start": "2500-01-01",
        "end": "2500-01-02",
    }
    assert model["future_cursor_safety"]["applied_bounds"] == {
        "start": "2500-01-01",
        "end": "2026-09-03",
    }
    assert model["future_cursor_safety"]["determining_input"] == {
        "relation": "raw.events",
        "cursor_column": "occurred_at",
    }


@pytest.mark.parametrize(
    "test_case",
    [FutureCursorPlanJsonTestCase("maximum start plan structure", "cap")],
    ids=lambda case: case.description,
)
def test_given_maximum_start_cap_when_formatting_plan_json_then_physical_evidence_is_exposed(
    test_case: FutureCursorPlanJsonTestCase,
) -> None:
    evidence: MaximumStartSafetyEvidence = MaximumStartSafetyEvidence(
        action=FutureCursorAction.CAP,
        max_ahead="0d",
        invocation_time="2026-09-01T12:00:00+00:00",
        physical_target_max="2026-09-03",
        highest_eligible_target_max="2026-09-01",
        effective_start="2026-08-30T00:00:00",
        maximum_allowed_start="2026-09-01",
        target_relation="analytics.events",
        cursor_column="event_at",
    )
    plan: PlanOutput = build_plan_output(
        model_entries=(
            build_model_entry(
                name="events",
                cursor_bounds=CursorBounds(
                    start=evidence.effective_start,
                    end="2026-09-04",
                    maximum_start_safety=evidence,
                ),
            ),
        )
    )

    payload: dict[str, object] = json.loads(format_plan_json(plan=plan))
    model: Any = payload["models"][0]

    assert model["maximum_start_safety"] == {
        "action": test_case.expected_action,
        "max_ahead": "0d",
        "invocation_time": "2026-09-01T12:00:00+00:00",
        "physical_target_max": "2026-09-03",
        "highest_eligible_target_max": "2026-09-01",
        "effective_start": "2026-08-30T00:00:00",
        "maximum_allowed_start": "2026-09-01",
        "input": {"relation": "analytics.events", "cursor_column": "event_at"},
    }


@pytest.mark.parametrize(
    "test_case",
    [SelectionDiagnosticsPlanJsonTestCase("direct diagnostics disabled evidence", False, "direct")],
    ids=lambda case: case.description,
)
def test_given_direct_diagnostics_policy_when_formatting_plan_json_then_evidence_is_exposed(
    test_case: SelectionDiagnosticsPlanJsonTestCase,
) -> None:
    plan: PlanOutput = build_plan_output(
        metadata={
            "selection_diagnostics": {
                "mode": "direct",
                "enabled": test_case.enabled,
            }
        }
    )

    payload: dict[str, object] = json.loads(format_plan_json(plan=plan))
    metadata: Any = payload["metadata"]

    assert metadata["selection_diagnostics"] == {
        "mode": test_case.expected_mode,
        "enabled": test_case.enabled,
    }


@pytest.mark.parametrize(
    "test_case",
    [
        JsonOutputTestCase(
            description="mixed plan json preserves full refresh and incremental reasons",
            plan_output=build_plan_output(
                model_entries=(
                    build_model_entry(
                        name="rebuilt_orders",
                        action=PlanAction.CREATE_TABLE,
                        reason=PlanReason.FULL_REFRESH,
                        materialization_type=MaterializationType.INCREMENTAL,
                        incremental_strategy="delete_insert",
                    ),
                    build_model_entry(
                        name="appended_orders",
                        action=PlanAction.INCREMENTAL_APPEND,
                        reason=PlanReason.NORMAL_INCREMENTAL,
                        materialization_type=MaterializationType.INCREMENTAL,
                        incremental_strategy="append",
                        backfill_action=BackfillAction.FORWARD_ONLY,
                    ),
                    build_model_entry(
                        name="microbatched_orders",
                        action=PlanAction.INCREMENTAL_DELETE_INSERT,
                        reason=PlanReason.QUERY_CHANGED,
                        materialization_type=MaterializationType.INCREMENTAL,
                        incremental_strategy="delete_insert",
                        incremental_mode="microbatch",
                        backfill_action=BackfillAction.BOUNDED,
                        backfill_duration="7d",
                    ),
                ),
            ),
            expected_keys=("models",),
            expected_fragments=(
                '"name": "rebuilt_orders"',
                '"reason": "full_refresh"',
                '"action": "create_table"',
                '"name": "appended_orders"',
                '"reason": "normal_incremental"',
                '"action": "incremental_append"',
                '"name": "microbatched_orders"',
                '"reason": "query_changed"',
                '"incremental_mode": "microbatch"',
            ),
        ),
        JsonOutputTestCase(
            description="plan json includes model action reason and backfill",
            plan_output=build_plan_output(
                model_entries=(
                    build_model_entry(
                        name="fact_orders",
                        action=PlanAction.INCREMENTAL_DELETE_INSERT,
                        reason=PlanReason.QUERY_CHANGED,
                        materialization_type=MaterializationType.INCREMENTAL,
                        incremental_strategy="delete_insert",
                        cursor_type="timestamp",
                        backfill_action=BackfillAction.BOUNDED,
                        backfill_duration="30d",
                    ),
                ),
            ),
            expected_keys=("selected_count", "models", "seeds", "warnings"),
            expected_fragments=(
                '"action": "incremental_delete_insert"',
                '"reason": "query_changed"',
                '"incremental_strategy": "delete_insert"',
                '"cursor_type": "timestamp"',
                '"duration": "30d"',
            ),
        ),
        JsonOutputTestCase(
            description="plan json includes model identity diagnostics",
            plan_output=build_plan_output(
                model_entries=(
                    build_model_entry(
                        name="fact_orders",
                        action=PlanAction.INCREMENTAL_DELETE_INSERT,
                        reason=PlanReason.QUERY_CHANGED,
                        fingerprint_version_hash="expected_hash",
                        previous_version_hash="built_hash",
                    ),
                    build_model_entry(
                        name="dim_customers",
                        action=PlanAction.CREATE_TABLE,
                        reason=PlanReason.FIRST_RUN,
                        fingerprint_version_hash="expected_first_hash",
                    ),
                ),
            ),
            expected_keys=("models",),
            expected_fragments=(
                '"expected_version_hash": "expected_hash"',
                '"built_version_hash": "built_hash"',
                '"built_version_present": true',
                '"identity_status": "stale"',
                '"expected_version_hash": "expected_first_hash"',
                '"built_version_hash": null',
                '"built_version_present": false',
                '"identity_status": "missing"',
            ),
        ),
        JsonOutputTestCase(
            description="plan json exposes deferred runtime-owned cursor work",
            plan_output=build_plan_output(
                model_entries=(
                    build_model_entry(
                        name="clean_events",
                        action=PlanAction.CREATE_TABLE,
                        reason=PlanReason.FIRST_RUN,
                        materialization_type=MaterializationType.INCREMENTAL,
                        incremental_strategy="delete_insert",
                        incremental_mode="microbatch",
                        cursor_column="event_date",
                        cursor_type="timestamp",
                        cursor_grain="day",
                        batch_size="effective",
                        cursor_input_relations=(
                            CursorInputRelation(
                                relation="stg_events",
                                cursor_column="event_date",
                                cursor_grain="month",
                                is_model_backed=True,
                                is_runtime_produced=True,
                            ),
                        ),
                    ),
                ),
            ),
            expected_keys=("models",),
            expected_fragments=(
                '"bounds_owner": "runtime"',
                '"resolution_status": "deferred"',
                '"start": null',
                '"end": null',
                '"resolved_bounds": null',
                '"declared_grain": "day"',
                '"effective_grain": "month"',
                '"declared_batch_size": "effective"',
                '"effective_batch_size": "1mo"',
                '"planned_batch_count": null',
                '"completion_tracking": "universal"',
                '"batch_concurrency": 1',
                '"reconciliation": "runtime"',
            ),
        ),
        JsonOutputTestCase(
            description="plan json preserves fixed batch size at coarser effective grain",
            plan_output=build_plan_output(
                model_entries=(
                    build_model_entry(
                        name="daily_events",
                        action=PlanAction.INCREMENTAL_DELETE_INSERT,
                        reason=PlanReason.NORMAL_INCREMENTAL,
                        materialization_type=MaterializationType.INCREMENTAL,
                        incremental_strategy="delete_insert",
                        incremental_mode="microbatch",
                        cursor_column="event_date",
                        cursor_type="timestamp",
                        cursor_grain="day",
                        batch_size="1d",
                        microbatch_range=CursorBounds(
                            start="2026-01-01",
                            end="2026-04-01",
                        ),
                        cursor_input_relations=(
                            CursorInputRelation(
                                relation="monthly_events",
                                cursor_column="event_date",
                                cursor_grain="month",
                            ),
                        ),
                    ),
                ),
            ),
            expected_keys=("models",),
            expected_fragments=(
                '"declared_grain": "day"',
                '"effective_grain": "month"',
                '"declared_batch_size": "1d"',
                '"effective_batch_size": "1d"',
                '"planned_batch_count": 90',
            ),
        ),
        JsonOutputTestCase(
            description="plan json reports unknown identity when expected hash is unavailable",
            plan_output=build_plan_output(
                model_entries=(
                    build_model_entry(
                        name="orders",
                        action=PlanAction.CREATE_TABLE,
                        reason=PlanReason.NO_CHANGE,
                        previous_version_hash="built_hash",
                    ),
                ),
            ),
            expected_keys=("models",),
            expected_fragments=(
                '"expected_version_hash": null',
                '"built_version_hash": "built_hash"',
                '"built_version_present": true',
                '"identity_status": "unknown"',
            ),
        ),
        JsonOutputTestCase(
            description="plan json lists changed functions on the direct caller",
            plan_output=build_plan_output(
                model_entries=(
                    build_model_entry(
                        name="hourly_order_activity",
                        action=PlanAction.INCREMENTAL_DELETE_INSERT,
                        reason=PlanReason.FUNCTION_CHANGED,
                        changed_functions=("is_completed_order",),
                    ),
                ),
            ),
            expected_keys=("selected_count", "models"),
            expected_fragments=(
                '"reason": "function_changed"',
                '"changed_functions": [\n        "is_completed_order"\n      ]',
            ),
        ),
        JsonOutputTestCase(
            description="plan json marks a renamed model whose query changed",
            plan_output=build_plan_output(
                model_entries=(
                    build_model_entry(
                        name="order_lines",
                        action=PlanAction.CREATE_TABLE,
                        reason=PlanReason.RENAMED,
                        query_changed=True,
                    ),
                ),
            ),
            expected_keys=("models",),
            expected_fragments=('"reason": "renamed"', '"query_changed": true'),
        ),
        JsonOutputTestCase(
            description="plan json includes the query diff a changed model prints",
            plan_output=build_plan_output(
                model_entries=(
                    build_model_entry(
                        name="fact_orders",
                        action=PlanAction.CREATE_TABLE,
                        reason=PlanReason.QUERY_CHANGED,
                        previous_query_sql="SELECT id FROM orders",
                    ),
                ),
            ),
            expected_keys=("models",),
            expected_fragments=(
                '"query_diff": [',
                '"-SELECT id FROM orders"',
                '"+SELECT * FROM fact_orders"',
            ),
        ),
        JsonOutputTestCase(
            description="plan json includes seeds and warnings",
            plan_output=build_plan_output(
                model_entries=(build_model_entry(name="orders", action=PlanAction.CREATE_TABLE),),
                seed_entries=(build_seed_entry(name="country_codes"),),
                warnings=(
                    build_warning(
                        model_name="stg_customers",
                        message="type change detected",
                        severity=WarningSeverity.WARNING,
                    ),
                ),
            ),
            expected_keys=("selected_count", "models", "seeds", "warnings"),
            expected_fragments=(
                '"country_codes"',
                '"reason": "first_run"',
                '"type change detected"',
                '"severity": "warning"',
            ),
        ),
        JsonOutputTestCase(
            description="plan json exposes warning diagnostic codes",
            plan_output=build_plan_output(
                model_entries=(
                    build_model_entry(
                        name="orders",
                        action=PlanAction.CREATE_TABLE,
                        reason=PlanReason.FIRST_RUN,
                        fingerprint_version_hash="expected_hash",
                    ),
                ),
                warnings=(
                    build_warning(
                        model_name="orders",
                        message="relation no longer exists",
                        code="RECORDED_RELATION_MISSING",
                    ),
                ),
            ),
            expected_keys=("models", "warnings"),
            expected_fragments=(
                '"code": "RECORDED_RELATION_MISSING"',
                '"built_version_present": false',
                '"identity_status": "missing"',
            ),
        ),
        JsonOutputTestCase(
            description="plan json includes source load count",
            plan_output=build_plan_output(
                model_entries=(build_model_entry(name="orders", action=PlanAction.CREATE_TABLE),),
                source_load_entries=(build_source_load_entry(name="raw_orders"),),
            ),
            expected_keys=("selected_count", "source_load_count", "source_loads"),
            expected_fragments=(
                '"selected_count": 1',
                '"source_load_count": 1',
                '"name": "raw_orders"',
            ),
        ),
        JsonOutputTestCase(
            description="plan json includes python node lifecycle entries",
            plan_output=build_plan_output(
                model_entries=(build_model_entry(name="orders", action=PlanAction.CREATE_TABLE),),
            ),
            python_plan_entries=(
                PythonPlanEntry(
                    name="prepare_orders",
                    kind=PythonNodeKind.TASK,
                    phase=PythonRunPhase.PRE_SQL_INGRESS,
                ),
                PythonPlanEntry(
                    name="export_orders",
                    kind=PythonNodeKind.ASSET,
                    phase=PythonRunPhase.READ_SIDE,
                ),
            ),
            expected_keys=("python_node_count", "python_nodes"),
            expected_fragments=(
                '"python_node_count": 2',
                '"name": "prepare_orders"',
                '"kind": "task"',
                '"phase": "pre_sql_ingress"',
                '"name": "export_orders"',
                '"kind": "asset"',
                '"phase": "read_side"',
            ),
        ),
        JsonOutputTestCase(
            description="plan json includes changed python identity diffs",
            plan_output=build_plan_output(),
            python_plan_entries=(
                PythonPlanEntry(
                    name="prepare_orders",
                    kind=PythonNodeKind.TASK,
                    phase=PythonRunPhase.PRE_SQL_INGRESS,
                    identity_status=PythonIdentityStatus.CHANGED,
                    previous_definition_json=json.dumps(
                        {"source_text": "def prepare_orders(ctx):\n    return 1\n"},
                        sort_keys=True,
                    ),
                    current_definition_json=json.dumps(
                        {"source_text": "def prepare_orders(ctx):\n    return 2\n"},
                        sort_keys=True,
                    ),
                    previous_metadata_json=json.dumps(
                        {
                            "dependencies": [
                                {
                                    "module": "tasks.helpers",
                                    "qualname": "order_label",
                                    "source_path": "python/tasks/helpers.py",
                                    "source_text": "def order_label():\n    return 'old'\n",
                                }
                            ]
                        },
                        sort_keys=True,
                    ),
                    current_metadata_json=json.dumps(
                        {
                            "dependencies": [
                                {
                                    "module": "tasks.helpers",
                                    "qualname": "order_label",
                                    "source_path": "python/tasks/helpers.py",
                                    "source_text": "def order_label():\n    return 'new'\n",
                                }
                            ]
                        },
                        sort_keys=True,
                    ),
                ),
            ),
            expected_keys=("python_nodes",),
            expected_fragments=(
                '"identity_diff"',
                '"source_diff"',
                '"dependency_diff"',
                '"-    return 1"',
                '"+    return 2"',
                "python/tasks/helpers.py :: tasks.helpers :: order_label",
                "\"-    return 'old'\"",
                "\"+    return 'new'\"",
            ),
        ),
        JsonOutputTestCase(
            description="plan json includes provider usage metadata",
            plan_output=build_plan_output(
                provider_usages=(
                    build_plan_provider_usage(
                        provider_name="marker_provider",
                        consumer_kind="loader",
                        consumer_name="raw_orders",
                    ),
                ),
            ),
            python_plan_entries=(
                PythonPlanEntry(
                    name="publish_orders",
                    kind=PythonNodeKind.TASK,
                    phase=PythonRunPhase.READ_SIDE,
                    provider_usages=(build_discovered_provider_usage(),),
                ),
            ),
            expected_keys=("providers",),
            expected_fragments=(
                '"providers"',
                '"name": "marker_provider"',
                '"used_by"',
                '"kind": "loader"',
                '"name": "raw_orders"',
                '"kind": "task"',
                '"name": "publish_orders"',
                '"parameter": "marker_provider"',
                '"class_name": "MarkerProvider"',
                '"module": "providers.marker"',
            ),
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_plan_output_when_formatting_json_then_produces_valid_json(
    test_case: JsonOutputTestCase,
) -> None:
    result: str = format_plan_json(
        plan=test_case.plan_output,
        python_plan_entries=test_case.python_plan_entries,
    )
    parsed: dict[str, object] = json.loads(result)

    key: str
    for key in test_case.expected_keys:
        assert key in parsed, f"Expected key '{key}' in JSON output"

    fragment: str
    for fragment in test_case.expected_fragments:
        assert fragment in result, f"Expected '{fragment}' in JSON:\n{result}"
