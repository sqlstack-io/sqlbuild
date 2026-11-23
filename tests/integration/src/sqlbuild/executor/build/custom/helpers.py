"""Test helpers for custom materialization integration tests."""

from __future__ import annotations

from collections.abc import Callable
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
from sqlbuild.adapter.contract.models import RelationInfo
from sqlbuild.adapters.duckdb.classes.duckdb_adapter import DuckDbAdapter
from sqlbuild.compiler.auditing.types import (
    AuditAttachmentKind,
    AuditRunScope,
    AuditSeverity,
)
from sqlbuild.executor.auditing.models import AuditExecutionResult
from sqlbuild.executor.custom.models import (
    MaterializationContext,
    MaterializationResult,
)
from sqlbuild.executor.run._helpers.materializations.custom import execute_custom_entry
from sqlbuild.executor.run.models import (
    HookContext,
    ModelExecutionResult,
    ModelMaterializationContext,
)
from sqlbuild.observability import LifecycleEvent


def operation_events(events: list[LifecycleEvent]) -> tuple[LifecycleEvent, ...]:
    return tuple(filter(lambda event: event.event_type.startswith("operation_"), events))


def insert_custom_hook_log(ctx: HookContext, phase: str) -> None:
    ctx.execute_sql(f"INSERT INTO {ctx.destination.schema}.hook_marker VALUES ('{phase}')")


def fail_custom_hook(ctx: HookContext, message: str) -> None:
    raise RuntimeError(message)


def build_custom_plan_entry(
    *,
    name: str = "test_model",
    sql: str = "SELECT 1 AS id",
    reason: PlanReason = PlanReason.NO_CHANGE,
    pre_hooks: object = None,
    post_hooks: object = None,
    custom_config: dict[str, object] | None = None,
    custom_placeholders: dict[str, str] | None = None,
) -> ModelPlanEntry:
    """Build a minimal ModelPlanEntry for a custom materialization."""

    return ModelPlanEntry(
        key=CompiledObjectKey(resource_type=CompiledResourceType.MODEL, name=name),
        name=name,
        relative_path=Path(f"sql/models/{name}.sql"),
        materialization_type=MaterializationType.CUSTOM,
        action=PlanAction.CUSTOM,
        reason=reason,
        destination=CompiledRelationLocation(
            database=None, schema="main", name=name, qualified_name=f"main.{name}"
        ),
        fingerprint_query_sql=sql,
        resolved_sql=sql,
        logical_ddl="",
        custom_materialization_name="test_custom",
        custom_config=custom_config or {"test_key": cast(object, "test_value")},
        custom_placeholders=custom_placeholders or {},
        pre_hooks=pre_hooks,
        post_hooks=post_hooks,
    )


def build_passing_audit(*, name: str, target_name: str) -> AuditPlanEntry:
    """Build an audit that returns 0 rows (PASS)."""

    return AuditPlanEntry(
        key=CompiledObjectKey(resource_type=CompiledResourceType.AUDIT, name=name),
        definition_name="test_audit",
        name=name,
        resolved_sql=f"SELECT * FROM {target_name} WHERE 1=0",
        unresolved_sql=f'SELECT * FROM __ref("{target_name}") WHERE 1=0',
        attachment_kind=AuditAttachmentKind.MODEL,
        severity=AuditSeverity.ERROR,
        requested_run_scope=AuditRunScope.FINAL,
        effective_run_scope=AuditRunScope.FINAL,
        attached_target_name=target_name,
    )


def build_failing_audit(*, name: str, target_name: str) -> AuditPlanEntry:
    """Build an audit that returns rows (ERROR)."""

    return AuditPlanEntry(
        key=CompiledObjectKey(resource_type=CompiledResourceType.AUDIT, name=name),
        definition_name="test_audit",
        name=name,
        resolved_sql=f"SELECT * FROM {target_name}",
        unresolved_sql=f'SELECT * FROM __ref("{target_name}")',
        attachment_kind=AuditAttachmentKind.MODEL,
        severity=AuditSeverity.ERROR,
        requested_run_scope=AuditRunScope.FINAL,
        effective_run_scope=AuditRunScope.FINAL,
        attached_target_name=target_name,
    )


def run_custom_entry(
    *,
    adapter: DuckDbAdapter,
    connection: Any,
    entry: ModelPlanEntry,
    materialize_fn: Callable[[MaterializationContext], MaterializationResult],
    model_audits: tuple[AuditPlanEntry, ...] = (),
    model_locations: dict[str, CompiledRelationLocation] | None = None,
    existing_relation: RelationInfo | None = None,
    target: str = "test",
    effective_vars: dict[str, object] | None = None,
    hook_functions: tuple[DiscoveredHookFunction, ...] = (),
) -> ModelExecutionResult:
    """Execute a custom materialization lifecycle with full control over parameters."""

    return execute_custom_entry(
        context=ModelMaterializationContext(
            entry=entry,
            adapter=adapter,
            connection=connection,
            model_locations=model_locations or {},
            seed_locations={},
            source_map={},
            python_source_read_map={},
            model_audits=model_audits,
            run_id="test_run",
            query_change_tracking=True,
            hook_functions=hook_functions,
        ),
        declared_columns=(),
        materialize_fn=materialize_fn,
        target=target,
        effective_vars=effective_vars or {},
        existing_relation=existing_relation,
    )


def relation_exists(connection: Any, *, schema: str, name: str) -> bool:
    """Check if a relation exists in the warehouse."""

    cursor: Any = connection.execute(
        f"SELECT 1 FROM information_schema.tables "
        f"WHERE table_schema = '{schema}' AND table_name = '{name}'"
    )
    return cursor.fetchone() is not None


def row_count(connection: Any, *, qualified_name: str) -> int:
    """Count rows in a relation."""

    cursor: Any = connection.execute(f"SELECT COUNT(*) FROM {qualified_name}")
    return cursor.fetchone()[0]


_FN_REGISTRY: dict[
    str, Callable[[], Callable[[MaterializationContext], MaterializationResult]]
] = {}


def resolve_fn(name: str) -> Callable[[MaterializationContext], MaterializationResult]:
    """Resolve a materialize function builder by name and call it."""

    return _FN_REGISTRY[name]()


def build_simple_fn() -> Callable[[MaterializationContext], MaterializationResult]:
    def materialize(ctx: MaterializationContext) -> MaterializationResult:
        ctx.adapter.create_table_as(
            connection=ctx.connection,
            destination=ctx.destination,
            sql=ctx.sql,
            statement_recorder=ctx.statement_recorder,
        )
        return MaterializationResult(relation=ctx.destination)

    return materialize


def build_failing_fn() -> Callable[[MaterializationContext], MaterializationResult]:
    def materialize(ctx: MaterializationContext) -> MaterializationResult:
        return MaterializationResult(
            relation=ctx.destination, failed=True, error="user-reported failure"
        )

    return materialize


def build_excepting_fn() -> Callable[[MaterializationContext], MaterializationResult]:
    def materialize(ctx: MaterializationContext) -> MaterializationResult:
        raise RuntimeError("materialization crashed")

    return materialize


def build_staging_fn() -> Callable[[MaterializationContext], MaterializationResult]:
    def materialize(ctx: MaterializationContext) -> MaterializationResult:
        staging: str = f"{ctx.destination}__staging"
        ctx.adapter.create_table_as(
            connection=ctx.connection,
            destination=staging,
            sql=ctx.sql,
            statement_recorder=ctx.statement_recorder,
        )
        ctx.adapter.rename(
            connection=ctx.connection,
            origin=staging,
            destination=ctx.destination,
            statement_recorder=ctx.statement_recorder,
        )
        return MaterializationResult(relation=ctx.destination, cleanup_relations=(staging,))

    return materialize


def build_audit_running_fn() -> Callable[[MaterializationContext], MaterializationResult]:
    def materialize(ctx: MaterializationContext) -> MaterializationResult:
        staging: str = f"{ctx.destination}__staging"
        ctx.adapter.create_table_as(
            connection=ctx.connection,
            destination=staging,
            sql=ctx.sql,
            statement_recorder=ctx.statement_recorder,
        )
        audit_results: tuple[AuditExecutionResult, ...] = ctx.run_audits(staging)
        ctx.adapter.rename(
            connection=ctx.connection,
            origin=staging,
            destination=ctx.destination,
            statement_recorder=ctx.statement_recorder,
        )
        return MaterializationResult(
            relation=ctx.destination, cleanup_relations=(staging,), audit_results=audit_results
        )

    return materialize


def build_user_audit_fn(
    *, expect_pass: bool
) -> Callable[[MaterializationContext], MaterializationResult]:
    return {
        True: _build_passing_user_audit_fn,
        False: _build_failing_user_audit_fn,
    }[expect_pass]()


def _build_passing_user_audit_fn() -> Callable[[MaterializationContext], MaterializationResult]:
    def materialize(ctx: MaterializationContext) -> MaterializationResult:
        staging: str = f"{ctx.destination}__staging"
        ctx.adapter.create_table_as(
            connection=ctx.connection,
            destination=staging,
            sql=ctx.sql,
            statement_recorder=ctx.statement_recorder,
        )
        audit_results: tuple[AuditExecutionResult, ...] = ctx.run_audits(staging)
        ctx.adapter.rename(
            connection=ctx.connection,
            origin=staging,
            destination=ctx.destination,
            statement_recorder=ctx.statement_recorder,
        )
        return MaterializationResult(
            relation=ctx.destination, cleanup_relations=(staging,), audit_results=audit_results
        )

    return materialize


def _build_failing_user_audit_fn() -> Callable[[MaterializationContext], MaterializationResult]:
    def materialize(ctx: MaterializationContext) -> MaterializationResult:
        staging: str = f"{ctx.destination}__staging"
        ctx.adapter.create_table_as(
            connection=ctx.connection,
            destination=staging,
            sql=ctx.sql,
            statement_recorder=ctx.statement_recorder,
        )
        audit_results: tuple[AuditExecutionResult, ...] = ctx.run_audits(staging)
        return MaterializationResult(
            relation=ctx.destination,
            failed=True,
            error="audit failed",
            cleanup_relations=(staging,),
            audit_results=audit_results,
        )

    return materialize


def build_cleanup_fn(*, fail: bool) -> Callable[[MaterializationContext], MaterializationResult]:
    return {True: _build_failing_cleanup_fn, False: _build_successful_cleanup_fn}[fail]()


def _create_cleanup_relations(ctx: MaterializationContext) -> str:
    staging: str = f"{ctx.destination}__staging"
    ctx.adapter.create_table_as(
        connection=ctx.connection,
        destination=ctx.destination,
        sql=ctx.sql,
        statement_recorder=ctx.statement_recorder,
    )
    ctx.adapter.create_table_as(
        connection=ctx.connection,
        destination=staging,
        sql="SELECT 1 AS cleanup_marker",
        statement_recorder=ctx.statement_recorder,
    )
    return staging


def _build_successful_cleanup_fn() -> Callable[[MaterializationContext], MaterializationResult]:
    def materialize(ctx: MaterializationContext) -> MaterializationResult:
        staging: str = _create_cleanup_relations(ctx)
        return MaterializationResult(relation=ctx.destination, cleanup_relations=(staging,))

    return materialize


def _build_failing_cleanup_fn() -> Callable[[MaterializationContext], MaterializationResult]:
    def materialize(ctx: MaterializationContext) -> MaterializationResult:
        staging: str = _create_cleanup_relations(ctx)
        return MaterializationResult(
            relation=ctx.destination,
            failed=True,
            error="intentional failure",
            cleanup_relations=(staging,),
        )

    return materialize


def build_partition_tracking_fn() -> Callable[[MaterializationContext], MaterializationResult]:
    """Build a materialize function that does partition-tracked incremental loads."""

    def materialize(ctx: MaterializationContext) -> MaterializationResult:
        tracking_table: str = str(ctx.config["tracking_table"])
        ctx.logger.debug("checking partition state table=%s", tracking_table)
        ctx.log.info("checking partition state")
        ctx.execute_sql(
            f"CREATE TABLE IF NOT EXISTS {tracking_table} (partition_value VARCHAR, run_id VARCHAR)"
        )

        full_sql: str = ctx.sql.replace("@@@partition_start", "'2024-01-01'")
        full_sql = full_sql.replace("@@@partition_end", "'2024-01-04'")
        ctx.log.info("building initial partition range")
        ctx.adapter.create_table_as(
            connection=ctx.connection,
            destination=ctx.destination,
            sql=full_sql,
            statement_recorder=ctx.statement_recorder,
        )
        partition: str
        for partition in ("2024-01-01", "2024-01-02", "2024-01-03"):
            ctx.execute_sql(f"INSERT INTO {tracking_table} VALUES ('{partition}', '{ctx.run_id}')")
        return MaterializationResult(relation=ctx.destination)

    return materialize


def build_existing_relation_capture_fn(
    captured: dict[str, Any],
) -> Callable[[MaterializationContext], MaterializationResult]:
    """Build a materialize function that captures existing_relation state."""

    def materialize(ctx: MaterializationContext) -> MaterializationResult:
        captured["existing_relation"] = ctx.existing_relation
        captured["is_first_run"] = ctx.is_first_run
        ctx.adapter.create_table_as(
            connection=ctx.connection,
            destination=ctx.destination,
            sql=ctx.sql,
            statement_recorder=ctx.statement_recorder,
        )
        return MaterializationResult(relation=ctx.destination)

    return materialize


def build_placeholder_execution_fn(
    substitutions: dict[str, str],
) -> Callable[[MaterializationContext], MaterializationResult]:
    """Build a materialize function that substitutes @@@placeholders and executes."""

    def materialize(ctx: MaterializationContext) -> MaterializationResult:
        sql: str = ctx.sql
        placeholder_name: str
        placeholder_value: str
        for placeholder_name, placeholder_value in substitutions.items():
            sql = sql.replace(f"@@@{placeholder_name}", placeholder_value)
        ctx.adapter.create_table_as(
            connection=ctx.connection,
            destination=ctx.destination,
            sql=sql,
            statement_recorder=ctx.statement_recorder,
        )
        return MaterializationResult(relation=ctx.destination)

    return materialize


def run_scheduler_build(
    *,
    project_files: dict[str, str],
    project_dir: Path,
    db_path: Path,
    adapter: DuckDbAdapter,
    custom_materializations: dict[str, Callable[[MaterializationContext], MaterializationResult]],
) -> tuple[Any, Any]:
    """Run a full build through the scheduler with custom materializations."""

    from sqlbuild._native import (
        CompilePipelineOptions,
        CompilePipelineResult,
        DiscoveredProjectInputs,
        PlanOutput,
        discover_project_inputs,
        run_compile_pipeline,
    )
    from sqlbuild.adapter.contract.types import TablePromotionMode
    from sqlbuild.executor.build.main._execute import execute_build_plan
    from sqlbuild.executor.build.models import (
        BuildCustomizations,
        BuildExecutionResult,
        BuildRuntimeParams,
    )

    config: dict[str, object] = {"database": str(db_path)}
    discovered: DiscoveredProjectInputs = discover_project_inputs(project_dir=project_dir)
    pipeline_result: CompilePipelineResult = run_compile_pipeline(
        discovered_inputs=discovered,
        adapter=adapter,
        options=CompilePipelineOptions(no_sql_validation=True, connection_config=config),
    )
    plan: PlanOutput = pipeline_result.plan_output

    connection: Any = adapter.connect(config)
    try:
        result: BuildExecutionResult = execute_build_plan(
            plan=plan,
            adapter=adapter,
            connection_config=config,
            connections=(connection,),
            scheduler_connection=connection,
            runtime=BuildRuntimeParams(
                promotion_mode=TablePromotionMode.STAGED,
                run_id="test_scheduler",
                query_change_tracking=True,
                run_audits=False,
            ),
            customizations=BuildCustomizations(
                custom_materializations=custom_materializations,
            ),
        )
        return result, connection
    except Exception:
        adapter.close(connection)
        raise


_FN_REGISTRY["simple"] = build_simple_fn
_FN_REGISTRY["failing"] = build_failing_fn
_FN_REGISTRY["excepting"] = build_excepting_fn
_FN_REGISTRY["staging"] = build_staging_fn
_FN_REGISTRY["audit_running"] = build_audit_running_fn
