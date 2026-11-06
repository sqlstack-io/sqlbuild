"""Aggregate execution result document coverage."""

from __future__ import annotations

import json
from dataclasses import replace
from decimal import Decimal
from pathlib import Path

import pytest

from sqlbuild._native import (
    CursorInputEvidence,
    FutureCursorAction,
    FutureCursorSafetyEvidence,
    MaximumStartSafetyEvidence,
    MeasurementThresholdBound,
    MeasurementThresholds,
    SqlLogicalType,
    SqlTestParameterDeclaration,
    SqlValue,
    SqlValueKind,
    format_audit_execution_json,
    format_build_footer,
    format_parameterized_test_label,
)
from sqlbuild.adapter.contract.models import LifeCycleEvent
from sqlbuild.adapter.contract.types import LifeCycleEventKind
from sqlbuild.compiler.auditing.types import (
    AuditAttachmentKind,
    AuditEvaluationMode,
    AuditOutcome,
    AuditSeverity,
    ThresholdOperator,
)
from sqlbuild.compiler.compile.types import AttachedAuditTargetKind
from sqlbuild.executor.auditing.models import AuditExecutionResult
from sqlbuild.executor.build.models import BuildExecutionResult
from sqlbuild.executor.build.types import BuildStatus
from sqlbuild.executor.run.models import (
    MicrobatchAccountingInterval,
    ModelExecutionResult,
)
from sqlbuild.executor.run.types import ExecutionPhase
from sqlbuild.executor.scheduling.types import ExecutionStatus
from sqlbuild.executor.testing.models import (
    SqlTestColumnDifference,
    SqlTestDifferenceSample,
    SqlTestExecutionResult,
    StepResult,
)
from sqlbuild.executor.testing.types import SqlTestOutcome
from tests.unit.src.sqlbuild.cli.output._helpers._test_types import (
    AuditExecutionProtocolTestCase,
    FailedModelSqlOutputTestCase,
    FutureCursorExecutionProtocolTestCase,
    MeasurementAuditOutputTestCase,
    MicrobatchExecutionProtocolTestCase,
    SqlTestCaseExecutionProtocolTestCase,
    SqlTestDifferenceOutputTestCase,
)
from tests.unit.src.sqlbuild.cli.output._helpers.helpers import (
    formatted_audit_check,
    formatted_model_asset,
    formatted_sql_test_check,
    formatted_sql_test_step,
)


@pytest.mark.parametrize(
    "test_case",
    (
        FailedModelSqlOutputTestCase(
            description="failed staging statement remains available to integrations",
            expected_recorded_sql="CREATE TABLE staging_orders AS SELECT * FROM raw_orders",
            expected_failed_sql="CREATE TABLE staging_orders AS SELECT invalid FROM raw_orders",
        ),
    ),
    ids=lambda case: case.description,
)
def test_given_failed_model_when_formatting_json_then_last_recorded_sql_is_preserved(
    test_case: FailedModelSqlOutputTestCase,
) -> None:
    result: ModelExecutionResult = ModelExecutionResult(
        model_name="orders",
        status=ExecutionStatus.FAILED,
        failed_phase=ExecutionPhase.STAGING,
        lifecycle_events=(
            LifeCycleEvent(kind=LifeCycleEventKind.SQL, content=test_case.expected_recorded_sql),
            LifeCycleEvent(kind=LifeCycleEventKind.LOG, content="staging failed"),
        ),
        error_message="syntax error",
        failed_sql=test_case.expected_failed_sql,
    )

    assets: tuple[dict[str, object], ...] = (formatted_model_asset(result),)

    assert assets[0]["last_recorded_sql"] == test_case.expected_recorded_sql
    assert assets[0]["failed_sql"] == test_case.expected_failed_sql


@pytest.mark.parametrize(
    "test_case",
    (
        SqlTestDifferenceOutputTestCase(
            description="two-way samples remain structured",
            expected_unexpected_count=1,
            expected_missing_count=1,
            expected_column_differences=(("status", "unexpected", "expected"),),
        ),
    ),
    ids=lambda case: case.description,
)
def test_given_difference_samples_when_formatting_json_then_structured_diagnostics_are_preserved(
    test_case: SqlTestDifferenceOutputTestCase,
) -> None:
    step: StepResult = StepResult(
        model_name="orders",
        outcome=SqlTestOutcome.FAIL,
        unexpected_row_count=test_case.expected_unexpected_count,
        missing_row_count=test_case.expected_missing_count,
        unexpected_samples=(
            SqlTestDifferenceSample(values=(("id", "2"), ("status", "unexpected"))),
        ),
        missing_samples=(SqlTestDifferenceSample(values=(("id", "1"), ("status", "expected"))),),
        column_differences=(
            SqlTestColumnDifference(name="status", actual="unexpected", expected="expected"),
        ),
    )

    payload: dict[str, object] = formatted_sql_test_step(step)

    assert payload["unexpected_row_count"] == test_case.expected_unexpected_count
    assert payload["missing_row_count"] == test_case.expected_missing_count
    assert payload["unexpected_samples"] == [
        [{"name": "id", "value": "2"}, {"name": "status", "value": "unexpected"}]
    ]
    assert payload["missing_samples"] == [
        [{"name": "id", "value": "1"}, {"name": "status", "value": "expected"}]
    ]
    expected_column_differences: list[dict[str, str]] = [
        {"name": name, "actual": actual, "expected": expected}
        for name, actual, expected in test_case.expected_column_differences
    ]
    assert payload["column_differences"] == expected_column_differences


@pytest.mark.parametrize(
    "test_case",
    [
        AuditExecutionProtocolTestCase(
            description="end-scheduled audit retains logical model identity",
            expected_check_id="audit:cross_model_consistency:model:orders",
            expected_attachment_kind="end",
            expected_target_kind="model",
        )
    ],
    ids=lambda case: case.description,
)
def test_given_end_scheduled_model_audit_when_formatting_json_then_logical_identity_is_preserved(
    test_case: AuditExecutionProtocolTestCase,
) -> None:
    result: AuditExecutionResult = AuditExecutionResult(
        audit_name="cross_model_consistency",
        audit_definition_name="test_audit",
        attachment_kind=AuditAttachmentKind.END,
        attached_target_kind=AttachedAuditTargetKind.MODEL,
        attached_target_name="orders",
        severity=AuditSeverity.ERROR,
        outcome=AuditOutcome.PASS,
        row_count=0,
        executed_sql="SELECT 1 WHERE FALSE",
    )

    check: dict[str, object] = formatted_audit_check(result)

    assert check["check_id"] == test_case.expected_check_id
    assert check["attachment_kind"] == test_case.expected_attachment_kind
    assert check["attached_target_kind"] == test_case.expected_target_kind
    assert check["asset_name"] == "orders"


@pytest.mark.parametrize(
    "test_case",
    (
        MeasurementAuditOutputTestCase(
            "insufficient measurement summary",
            AuditOutcome.INSUFFICIENT,
            "insufficient",
            True,
        ),
    ),
    ids=lambda case: case.description,
)
def test_given_measurement_audit_when_formatting_execution_document_then_summary_is_preserved(
    test_case: MeasurementAuditOutputTestCase,
) -> None:
    result: AuditExecutionResult = AuditExecutionResult(
        audit_name="valid_rate",
        audit_definition_name="test_audit",
        attachment_kind=AuditAttachmentKind.END,
        severity=AuditSeverity.WARN,
        outcome=test_case.outcome,
        row_count=0,
        executed_sql="SELECT 99.5 AS valid_rate",
        evaluation_mode=AuditEvaluationMode.MEASUREMENT,
        measured_value=99.5,
        sample_count=42,
        sample_unit="rows",
        minimum_samples=100,
        thresholds=MeasurementThresholds(
            warn=MeasurementThresholdBound(operator=ThresholdOperator.BELOW, limit=100.0)
        ),
        evidence_rows=({"id": 1},),
        evidence_truncated=True,
    )

    check: dict[str, object] = formatted_audit_check(result)

    assert check["passed"] is test_case.expected_passed
    assert check["status"] == test_case.expected_status
    assert check["evaluation_mode"] == "measurement"
    assert check["measured_value"] == 99.5
    assert check["sample_count"] == 42
    assert check["sample_unit"] == "rows"
    assert check["minimum_samples"] == 100
    assert check["thresholds"] == {"warn": {"operator": "below", "limit": 100.0}}
    assert check["evidence_count"] == 1
    assert check["evidence_truncated"] is True


@pytest.mark.parametrize(
    "test_case",
    (
        AuditExecutionProtocolTestCase(
            description="ordered concurrent audit json",
            expected_check_id="audit:first",
            expected_attachment_kind="end",
            expected_target_kind="",
            expected_configured_concurrency=8,
            expected_worker_count=2,
        ),
    ),
    ids=lambda case: case.description,
)
def test_given_ordered_audits_when_formatting_json_then_metadata_and_check_order_are_preserved(
    test_case: AuditExecutionProtocolTestCase,
) -> None:
    results: tuple[AuditExecutionResult, ...] = tuple(
        AuditExecutionResult(
            audit_name=name,
            audit_definition_name="test_audit",
            attachment_kind=AuditAttachmentKind.END,
            severity=AuditSeverity.ERROR,
            outcome=AuditOutcome.PASS,
            row_count=0,
            executed_sql="SELECT 1 WHERE FALSE",
        )
        for name in ("first", "second")
    )

    payload: dict[str, object] = json.loads(
        format_audit_execution_json(
            results=results,
            configured_concurrency=test_case.expected_configured_concurrency,
            worker_count=test_case.expected_worker_count,
        )
    )

    assert payload["version"] == 1
    assert payload["execution"] == {
        "configured_concurrency": test_case.expected_configured_concurrency,
        "worker_count": test_case.expected_worker_count,
    }
    assert [check["name"] for check in payload["checks"]] == ["first", "second"]  # type: ignore[index]


@pytest.mark.parametrize(
    "test_case",
    [
        MicrobatchExecutionProtocolTestCase(
            description="microbatch execution exposes replay accounting",
            expected_run_type="replay_on_change",
            expected_replay_state="complete_with_unknown_fingerprints",
        )
    ],
    ids=lambda case: case.description,
)
def test_given_microbatch_result_when_formatting_json_then_interval_provenance_is_exposed(
    test_case: MicrobatchExecutionProtocolTestCase,
) -> None:
    result: ModelExecutionResult = ModelExecutionResult(
        model_name="orders",
        status=ExecutionStatus.SUCCESS,
        batch_count=2,
        batch_size="1h",
        microbatch_run_type="replay_on_change",
        microbatch_recovery_batch_count=1,
        microbatch_known_gap_count=1,
        microbatch_unaccounted_interval_count=1,
        microbatch_synthetic_completion_count=1,
        microbatch_unknown_fingerprint_count=1,
        microbatch_contiguous_frontier="2026-01-01T01:00:00",
        microbatch_unaccounted_partition_policy="recover_empty",
        microbatch_replay_requirement_id="requirement-1",
        microbatch_required_model_version_hash="F2",
        microbatch_physical_generation_id="generation-1",
        microbatch_concurrent_enabled=True,
        microbatch_batch_concurrency=2,
        microbatch_global_concurrency=4,
        microbatch_replay_requirement_state="complete_with_unknown_fingerprints",
        microbatch_accounting_intervals=(
            MicrobatchAccountingInterval(
                partition_start="2026-01-01T00:00:00",
                partition_end="2026-01-01T01:00:00",
                accounting_status="synthetic",
                fingerprint_status="unknown",
            ),
        ),
    )

    asset: dict[str, object] = formatted_model_asset(result)

    assert asset["microbatch"] == {
        "run_type": test_case.expected_run_type,
        "batch_count": 2,
        "batch_size": "1h",
        "recovery_batch_count": 1,
        "known_gap_count": 1,
        "unaccounted_interval_count": 1,
        "synthetic_completion_count": 1,
        "unknown_fingerprint_count": 1,
        "contiguous_frontier": "2026-01-01T01:00:00",
        "unaccounted_partition_policy": "recover_empty",
        "replay_requirement_id": "requirement-1",
        "required_model_version_hash": "F2",
        "physical_generation_id": "generation-1",
        "concurrent_enabled": True,
        "batch_concurrency": 2,
        "global_concurrency": 4,
        "replay_requirement_state": test_case.expected_replay_state,
        "intervals": [
            {
                "partition_start": "2026-01-01T00:00:00",
                "partition_end": "2026-01-01T01:00:00",
                "accounting_status": "synthetic",
                "fingerprint_status": "unknown",
                "model_version_hash": None,
                "completion_type": None,
                "event_id": None,
            }
        ],
    }


@pytest.mark.parametrize(
    "test_case",
    [FutureCursorExecutionProtocolTestCase("structured future cursor evidence", "cap")],
    ids=lambda case: case.description,
)
def test_given_future_cursor_cap_when_formatting_execution_json_then_structured_evidence_is_exposed(
    test_case: FutureCursorExecutionProtocolTestCase,
) -> None:
    evidence: FutureCursorSafetyEvidence = FutureCursorSafetyEvidence(
        action=FutureCursorAction.CAP,
        max_distance="2d",
        invocation_time="2026-09-01T12:00:00+00:00",
        discovered_start="2500-01-01T00:00:00",
        discovered_end="2500-01-01T00:00:01",
        applied_start="2500-01-01T00:00:00",
        applied_end="2026-09-03T12:00:00",
        maximum_allowed_start="2026-09-03T12:00:00",
        maximum_allowed_end="2026-09-03T12:00:01",
        future_start_detected=True,
        future_end_detected=True,
        determining_relation="raw.events",
        determining_cursor_column="occurred_at",
        inputs=(
            CursorInputEvidence(
                relation="raw.events",
                cursor_column="occurred_at",
                minimum=None,
                maximum="2500-01-01T00:00:00",
            ),
        ),
    )
    result: ModelExecutionResult = ModelExecutionResult(
        model_name="orders",
        status=ExecutionStatus.SUCCESS,
        future_cursor_safety=evidence,
    )

    asset: dict[str, object] = formatted_model_asset(result)

    assert asset["future_cursor_safety"] == {
        "action": test_case.expected_action,
        "max_distance": "2d",
        "invocation_time": "2026-09-01T12:00:00+00:00",
        "discovered_bounds": {
            "start": "2500-01-01T00:00:00",
            "end": "2500-01-01T00:00:01",
        },
        "applied_bounds": {
            "start": "2500-01-01T00:00:00",
            "end": "2026-09-03T12:00:00",
        },
        "maximum_allowed_bounds": {
            "start": "2026-09-03T12:00:00",
            "end": "2026-09-03T12:00:01",
        },
        "future_start_detected": True,
        "future_end_detected": True,
        "determining_input": {
            "relation": "raw.events",
            "cursor_column": "occurred_at",
        },
        "inputs": [
            {
                "relation": "raw.events",
                "cursor_column": "occurred_at",
                "minimum": None,
                "maximum": "2500-01-01T00:00:00",
            }
        ],
    }


@pytest.mark.parametrize(
    "test_case",
    [FutureCursorExecutionProtocolTestCase("structured maximum start evidence", "cap")],
    ids=lambda case: case.description,
)
def test_given_maximum_start_cap_when_formatting_execution_json_then_evidence_is_exposed(
    test_case: FutureCursorExecutionProtocolTestCase,
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
    result: ModelExecutionResult = ModelExecutionResult(
        model_name="events",
        status=ExecutionStatus.SUCCESS,
        maximum_start_safety=evidence,
    )

    asset: dict[str, object] = formatted_model_asset(result)

    maximum_start: object = asset["maximum_start_safety"]
    assert maximum_start == {
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
    [
        SqlTestCaseExecutionProtocolTestCase(
            description="parameterized SQL test emits stable identity and safe decimal value",
            expected_check_id="sql_test:tests/unit/orders.sql:2:large_order",
            expected_decimal_value="12.3400",
            expected_fingerprint="a" * 64,
            expected_text_label=(
                "order totals [large_order] (tests/unit/orders.sql; "
                'amount:decimal="12.3400", note:string?=null)'
            ),
        )
    ],
    ids=lambda case: case.description,
)
def test_given_parameterized_sql_test_result_when_formatting_json_then_case_metadata_is_safe(
    test_case: SqlTestCaseExecutionProtocolTestCase,
) -> None:
    result: SqlTestExecutionResult = SqlTestExecutionResult(
        test_name="order totals [large_order]",
        outcome=SqlTestOutcome.PASS,
        source_path=Path("tests/unit/orders.sql"),
        block_index=2,
        parent_name="order totals",
        case_name="large_order",
        case_index=1,
        case_fingerprint=test_case.expected_fingerprint,
        parameter_schema=(
            SqlTestParameterDeclaration(
                name="amount",
                value_type=SqlValueKind.DECIMAL,
            ),
            SqlTestParameterDeclaration(
                name="note",
                value_type=SqlValueKind.STRING,
                nullable=True,
            ),
        ),
        parameter_values=(
            (
                "amount",
                SqlValue(
                    logical_type=SqlLogicalType(SqlValueKind.DECIMAL),
                    value=Decimal(test_case.expected_decimal_value),
                ),
            ),
            (
                "note",
                SqlValue(
                    logical_type=SqlLogicalType(SqlValueKind.NULL),
                    value=None,
                ),
            ),
        ),
    )

    check: dict[str, object] = formatted_sql_test_check(result)

    assert check["check_id"] == test_case.expected_check_id
    assert check["source_path"] == "tests/unit/orders.sql"
    assert check["block_index"] == 2
    assert check["parent_name"] == "order totals"
    assert check["case_name"] == "large_order"
    assert check["case_index"] == 1
    assert check["case_fingerprint"] == test_case.expected_fingerprint
    assert check["parameter_schema"] == [
        {"name": "amount", "type": "decimal", "nullable": False},
        {"name": "note", "type": "string", "nullable": True},
    ]
    assert check["parameters"] == [
        {
            "name": "amount",
            "type": "decimal",
            "value": test_case.expected_decimal_value,
        },
        {"name": "note", "type": "string", "value": None},
    ]
    assert (
        format_parameterized_test_label(
            name=result.test_name,
            source_path=result.source_path,
            parameter_schema=result.parameter_schema,
            parameter_values=result.parameter_values,
        )
        == test_case.expected_text_label
    )
    footer: str = format_build_footer(
        result=BuildExecutionResult(
            status=BuildStatus.FAILED,
            test_results=(replace(result, outcome=SqlTestOutcome.FAIL),),
        ),
        elapsed=1.0,
        use_color=False,
    )
    assert test_case.expected_text_label in footer
