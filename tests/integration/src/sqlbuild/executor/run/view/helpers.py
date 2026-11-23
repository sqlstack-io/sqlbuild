"""Test helpers for view executor integration tests."""

from __future__ import annotations

from dataclasses import dataclass
from pathlib import Path
from typing import Any, cast

from sqlbuild._native import (
    AuditPlanEntry,
    CompiledObjectKey,
    CompiledRelationLocation,
    CompiledResourceType,
    DiscoveredHookFunction,
    MaterializationType,
    ModelPlanEntry,
    PlanAction,
    PlanReason,
)
from sqlbuild.adapters.duckdb.classes.duckdb_adapter import DuckDbAdapter
from sqlbuild.compiler.auditing.types import (
    AuditAttachmentKind,
    AuditRunScope,
    AuditSeverity,
)
from sqlbuild.executor.run._helpers.materializations.view import execute_view_entry
from sqlbuild.executor.run.models import (
    HookContext,
    ModelExecutionResult,
    ModelMaterializationContext,
)
from sqlbuild.executor.scheduling.types import ExecutionStatus
from tests.integration.src.sqlbuild.executor.run.view._test_types import (
    ViewFailureTestCase,
    ViewSuccessTestCase,
)


@dataclass(frozen=True)
class ViewExtraAuditDefinition:
    name: str
    audit_sql: str
    severity: str


def create_python_view_hook_data(ctx: HookContext, value: int) -> None:
    ctx.execute_sql(
        f"CREATE TABLE {ctx.destination.schema}.python_view_data AS SELECT {value} AS val"
    )
    ctx.log.info(f"python view pre-hook created data for {ctx.model_name}")


def create_python_view_order_step(ctx: HookContext, source: str, target: str) -> None:
    ctx.execute_sql(
        f"CREATE TABLE {ctx.destination.schema}.{target} AS "
        f"SELECT val + 1 AS val FROM {ctx.destination.schema}.{source}"
    )


def fail_python_view_hook(ctx: HookContext, message: str) -> None:
    raise RuntimeError(message)


def build_view_plan_entry(
    *,
    name: str,
    sql: str,
    target_schema: str | None,
    target_name: str,
    pre_hooks: object = None,
    post_hooks: object = None,
) -> ModelPlanEntry:
    """Build a minimal ModelPlanEntry for view execution tests."""

    qualified: str = f"{target_schema or ''}.{target_name}".lstrip(".")
    return ModelPlanEntry(
        key=CompiledObjectKey(resource_type=CompiledResourceType.MODEL, name=name),
        name=name,
        relative_path=Path(f"models/{name}.sql"),
        materialization_type=MaterializationType.VIEW,
        action=PlanAction.CREATE_VIEW,
        reason=PlanReason.FIRST_RUN,
        destination=CompiledRelationLocation(
            database=None,
            schema=target_schema,
            name=target_name,
            qualified_name=qualified,
        ),
        fingerprint_query_sql=sql,
        resolved_sql=sql,
        logical_ddl=f"CREATE OR REPLACE VIEW {qualified} AS {sql}",
        pre_hooks=pre_hooks,
        post_hooks=post_hooks,
    )


def build_view_audit_plan_entry(
    *,
    name: str,
    unresolved_sql: str,
    attached_target_name: str,
    resolved_target_name: str,
    severity: str = "warn",
) -> AuditPlanEntry:
    """Build a minimal AuditPlanEntry for view execution tests."""

    return AuditPlanEntry(
        key=CompiledObjectKey(resource_type=CompiledResourceType.AUDIT, name=name),
        definition_name="test_audit",
        name=name,
        resolved_sql=_resolve_audit_sql(
            unresolved_sql=unresolved_sql,
            attached_target_name=attached_target_name,
            resolved_target_name=resolved_target_name,
        ),
        unresolved_sql=unresolved_sql,
        attachment_kind=AuditAttachmentKind.MODEL,
        severity=AuditSeverity(severity),
        requested_run_scope=AuditRunScope.FINAL,
        effective_run_scope=AuditRunScope.FINAL,
        attached_target_name=attached_target_name,
    )


def _resolve_audit_sql(
    *, unresolved_sql: str, attached_target_name: str, resolved_target_name: str
) -> str:
    return unresolved_sql.replace(f'__ref("{attached_target_name}")', resolved_target_name)


def run_view_success_test(
    *,
    test_case: ViewSuccessTestCase,
    adapter: DuckDbAdapter,
    connection: Any,
) -> ModelExecutionResult:
    """Execute a view success test case and return the result."""

    result: ModelExecutionResult = _execute_view_test(
        test_case=test_case, adapter=adapter, connection=connection
    )
    assert result.status == ExecutionStatus.SUCCESS
    return result


def run_view_failure_test(
    *,
    test_case: ViewFailureTestCase,
    adapter: DuckDbAdapter,
    connection: Any,
) -> ModelExecutionResult:
    """Execute a view failure test case and return the result."""

    result: ModelExecutionResult = _execute_view_test(
        test_case=test_case, adapter=adapter, connection=connection
    )
    assert result.status == ExecutionStatus.FAILED
    return result


def verify_view_success_state(
    *,
    result: ModelExecutionResult,
    test_case: ViewSuccessTestCase,
    connection: Any,
) -> None:
    """Verify warehouse state for a successful view execution."""

    target_qualified: str = _build_target_qualified(
        target_schema=test_case.target_schema, target_name=test_case.target_name
    )
    query_result: Any = connection.execute(f"SELECT * FROM {target_qualified}")
    rows: list[Any] = query_result.fetchall()
    assert len(rows) == test_case.expected_row_count
    assert len(result.audit_results) == test_case.expected_audit_count
    _verify_warning_fragment(result=result, test_case=test_case)
    _verify_lifecycle_event_fragments(result=result, test_case=test_case)


def verify_view_failure_state(
    *,
    result: ModelExecutionResult,
    test_case: ViewFailureTestCase,
) -> None:
    """Verify result fields for a failed view execution."""

    assert result.failed_phase == test_case.expected_failed_phase
    assert len(result.audit_results) == test_case.expected_audit_count
    assert result.promoted_relation == test_case.expected_promoted_relation
    _verify_error_fragment(result=result, test_case=test_case)


def _execute_view_test(
    *,
    test_case: ViewSuccessTestCase | ViewFailureTestCase,
    adapter: DuckDbAdapter,
    connection: Any,
) -> ModelExecutionResult:
    """Set up and execute a view entry test case."""

    sql: str
    for sql in test_case.setup_sql:
        connection.execute(sql)

    entry: ModelPlanEntry = build_view_plan_entry(
        name="dim_view",
        sql=test_case.model_sql,
        target_schema=test_case.target_schema,
        target_name=test_case.target_name,
        pre_hooks=test_case.pre_hook,
        post_hooks=test_case.post_hook,
    )

    target_qualified: str = _build_target_qualified(
        target_schema=test_case.target_schema, target_name=test_case.target_name
    )
    model_audits: tuple[AuditPlanEntry, ...] = _build_model_audits(
        test_case=test_case,
        resolved_target_name=target_qualified,
    )
    model_locations: dict[str, CompiledRelationLocation] = {
        "dim_view": CompiledRelationLocation(
            database=None,
            schema=test_case.target_schema,
            name=test_case.target_name,
            qualified_name=target_qualified,
        ),
    }

    return execute_view_entry(
        context=ModelMaterializationContext(
            entry=entry,
            adapter=adapter,
            connection=connection,
            model_locations=model_locations,
            seed_locations={},
            source_map={},
            python_source_read_map={},
            model_audits=model_audits,
            run_id="test_run",
            query_change_tracking=getattr(test_case, "query_change_tracking", True),
            hook_functions=cast(
                tuple[DiscoveredHookFunction, ...], getattr(test_case, "hook_functions", ())
            ),
        ),
    )


def _build_model_audits(
    *, test_case: ViewSuccessTestCase | ViewFailureTestCase, resolved_target_name: str
) -> tuple[AuditPlanEntry, ...]:
    """Build model audits from test case audit config."""

    audits: list[AuditPlanEntry] = list(
        {
            True: (),
            False: (
                build_view_audit_plan_entry(
                    name="not_null",
                    unresolved_sql=test_case.audit_sql or "",
                    attached_target_name="dim_view",
                    resolved_target_name=resolved_target_name,
                    severity=test_case.audit_severity,
                ),
            ),
        }[test_case.audit_sql is None]
    )
    extra: object
    for extra in test_case.extra_audits:
        assert isinstance(extra, ViewExtraAuditDefinition)
        audits.append(
            build_view_audit_plan_entry(
                name=extra.name,
                unresolved_sql=extra.audit_sql,
                attached_target_name="dim_view",
                resolved_target_name=resolved_target_name,
                severity=extra.severity,
            )
        )
    return tuple(audits)


def _build_target_qualified(*, target_schema: str | None, target_name: str) -> str:
    return f"{target_schema or ''}.{target_name}".lstrip(".")


def _verify_warning_fragment(
    *,
    result: ModelExecutionResult,
    test_case: ViewSuccessTestCase,
) -> None:
    all_warnings: str = " ".join(result.warning_messages)
    assert (test_case.expected_warning_fragment or "") in all_warnings


def _verify_lifecycle_event_fragments(
    *,
    result: ModelExecutionResult,
    test_case: ViewSuccessTestCase,
) -> None:
    fragment: str
    lifecycle_output: str = "\n".join(event.content for event in result.lifecycle_events)
    for fragment in test_case.expected_lifecycle_event_fragments:
        assert fragment in lifecycle_output


def _verify_error_fragment(
    *,
    result: ModelExecutionResult,
    test_case: ViewFailureTestCase,
) -> None:
    assert (test_case.expected_error_fragment or "") in (result.error_message or "")
