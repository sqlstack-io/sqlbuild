"""Test helpers for CLI shared helpers tests."""

from __future__ import annotations

import re
from pathlib import Path

from sqlbuild._native import (
    ChainStep,
    CompiledObjectKey,
    CompiledRelationLocation,
    CompiledResourceType,
    MaterializationType,
    ModelPlanEntry,
    PlanAction,
    PlanOutput,
    PlanReason,
    SqlTestAssertionStep,
    SqlTestPlanEntry,
)
from sqlbuild.compiler.auditing.types import (
    AuditAttachmentKind,
    AuditOutcome,
    AuditRunScope,
    AuditSeverity,
)
from sqlbuild.executor.auditing.models import AuditExecutionResult
from sqlbuild.executor.testing.models import SqlTestExecutionResult, StepResult
from sqlbuild.executor.testing.types import SqlTestOutcome

_STATUS_TOKEN_PATTERN: re.Pattern[str] = re.compile(
    r"^.*? (PASS|FAIL|ERROR|OK)(?= |$)", flags=re.MULTILINE
)


def build_audit_result(
    *,
    name: str,
    outcome: AuditOutcome,
    run_scope_phase: AuditRunScope = AuditRunScope.FINAL,
    row_count: int = 0,
    column_name: str | None = None,
    target_name: str | None = "test_model",
    reused: bool = False,
) -> AuditExecutionResult:
    return AuditExecutionResult(
        audit_name=name,
        audit_definition_name="test_audit",
        attachment_kind=AuditAttachmentKind.MODEL,
        severity=AuditSeverity.ERROR,
        outcome=outcome,
        row_count=row_count,
        executed_sql="SELECT 1",
        run_scope_phase=run_scope_phase,
        attached_target_name=target_name,
        attached_column_name=column_name,
        reused=reused,
    )


def build_progress_snapshot_plan_output(
    *,
    name: str = "customer_snapshot",
    snapshot_strategy: str = "timestamp",
    observed_at_column: str | None = None,
    historical_input: str | None = None,
) -> PlanOutput:
    entry: ModelPlanEntry = ModelPlanEntry(
        key=CompiledObjectKey(resource_type=CompiledResourceType.MODEL, name=name),
        name=name,
        relative_path=Path(f"sql/models/{name}.sql"),
        materialization_type=MaterializationType.SNAPSHOT,
        action=PlanAction.SNAPSHOT,
        reason=PlanReason.FIRST_RUN,
        destination=CompiledRelationLocation(
            database=None,
            schema="main",
            name=name,
            qualified_name=f"main.{name}",
        ),
        fingerprint_query_sql="SELECT 1 AS id",
        resolved_sql="SELECT 1 AS id",
        logical_ddl=f"CREATE TABLE main.{name} AS SELECT 1 AS id",
        snapshot_strategy=snapshot_strategy,
        observed_at_column=observed_at_column,
        historical_input=historical_input,
    )
    return PlanOutput(
        execution_order=(entry.key,),
        model_entries=(entry,),
        selected_keys=frozenset((entry.key,)),
    )


def build_sql_test_plan_entry(
    *, test_name: str, expected_models: tuple[str, ...], assertion_names: tuple[str, ...]
) -> SqlTestPlanEntry:
    return SqlTestPlanEntry(
        key=CompiledObjectKey(resource_type=CompiledResourceType.SQL_TEST, name=test_name),
        name=test_name,
        chain=tuple(
            ChainStep(
                model_name=model_name,
                resolved_sql="SELECT 1 AS order_id",
                expected_cte_sql="SELECT 1 AS order_id",
            )
            for model_name in expected_models
        ),
        assertions=tuple(
            SqlTestAssertionStep(name=assertion_name, resolved_sql="SELECT 1 AS order_id")
            for assertion_name in assertion_names
        ),
    )


def build_sql_test_result(
    *,
    test_name: str,
    expected_models: tuple[str, ...],
    assertion_names: tuple[str, ...],
    outcome: SqlTestOutcome,
) -> SqlTestExecutionResult:
    first_step, *later_steps = (
        *expected_models,
        *(f"assertion {assertion_name}" for assertion_name in assertion_names),
    )
    return SqlTestExecutionResult(
        test_name=test_name,
        outcome=outcome,
        step_results=(
            StepResult(model_name=first_step, outcome=outcome),
            *(
                StepResult(model_name=step_name, outcome=SqlTestOutcome.PASS)
                for step_name in later_steps
            ),
        ),
    )


def status_columns(output: str) -> tuple[int, ...]:
    """Return the column of the first status token on every row that has one."""

    return tuple(match.start(1) - match.start() for match in _STATUS_TOKEN_PATTERN.finditer(output))
