from collections.abc import Callable
from pathlib import Path
from typing import Any

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
    PythonHookEntry,
    SchemaDynamicColumnFamily,
    SourceEntry,
)
from sqlbuild.adapter.contract.classes.base_adapter import BaseAdapter
from sqlbuild.adapter.contract.classes.statement_recorder import StatementRecorder
from sqlbuild.adapter.contract.models import ColumnInfo
from sqlbuild.adapters.bigquery.classes.bigquery_adapter import BigQueryAdapter
from sqlbuild.adapters.duckdb.classes.duckdb_adapter import DuckDbAdapter
from sqlbuild.adapters.postgres.classes.postgres_adapter import PostgresAdapter
from sqlbuild.adapters.snowflake.classes.snowflake_adapter import SnowflakeAdapter
from sqlbuild.compiler.auditing.types import (
    AuditAttachmentKind,
    AuditOutcome,
    AuditRunScope,
    AuditSeverity,
)
from sqlbuild.errors.contracts.exceptions import ExecutorInputError
from sqlbuild.executor.auditing.models import AuditExecutionResult
from sqlbuild.executor.run._helpers.execution.hooks import execute_hooks
from sqlbuild.executor.run._helpers.validation.configured_columns import require_quoted_spelling
from sqlbuild.executor.run.models import HookContext, HookRelationLookup, HookRunContext
from sqlbuild.executor.run.types import HookPhase
from sqlbuild.python_nodes.models import SqlResourceRef


def insert_snapshot_hook_log(ctx: HookContext, phase: str) -> None:
    ctx.execute_sql(f"INSERT INTO {ctx.destination.schema}.snapshot_hook_log VALUES ('{phase}')")


def fail_snapshot_hook(ctx: HookContext, message: str) -> None:
    raise RuntimeError(message)


def build_result_model_plan_entry() -> ModelPlanEntry:
    return ModelPlanEntry(
        key=CompiledObjectKey(
            resource_type=CompiledResourceType.MODEL,
            name="orders",
        ),
        name="orders",
        relative_path=Path("models/orders.sql"),
        materialization_type=MaterializationType.TABLE,
        action=PlanAction.CREATE_TABLE,
        reason=PlanReason.FIRST_RUN,
        destination=CompiledRelationLocation(
            database=None,
            schema="analytics",
            name="orders",
            qualified_name="analytics.orders",
        ),
        fingerprint_query_sql="SELECT 1 AS id",
        resolved_sql="SELECT 1 AS id",
        logical_ddl="CREATE TABLE analytics.orders AS SELECT 1 AS id",
    )


def build_contract_model_plan_entry(
    *,
    contract_enforced: bool,
    contract_columns: tuple[ColumnInfo, ...],
    contract_dynamic_columns: tuple[SchemaDynamicColumnFamily, ...] = (),
) -> ModelPlanEntry:
    entry: ModelPlanEntry = build_result_model_plan_entry()
    return ModelPlanEntry(
        key=entry.key,
        name=entry.name,
        relative_path=entry.relative_path,
        materialization_type=entry.materialization_type,
        action=entry.action,
        reason=entry.reason,
        destination=entry.destination,
        fingerprint_query_sql=entry.fingerprint_query_sql,
        resolved_sql=entry.resolved_sql,
        logical_ddl=entry.logical_ddl,
        contract_enforced=contract_enforced,
        contract_columns=contract_columns,
        contract_dynamic_columns=contract_dynamic_columns,
    )


def build_snapshot_execution_plan_entry(
    *,
    pre_hooks: object = None,
    post_hooks: object = None,
    contract_enforced: bool = False,
    contract_columns: tuple[ColumnInfo, ...] = (),
) -> ModelPlanEntry:
    return ModelPlanEntry(
        key=CompiledObjectKey(
            resource_type=CompiledResourceType.MODEL,
            name="customer_snapshot",
        ),
        name="customer_snapshot",
        relative_path=Path("models/customer_snapshot.sql"),
        materialization_type=MaterializationType.SNAPSHOT,
        action=PlanAction.SNAPSHOT,
        reason=PlanReason.FIRST_RUN,
        destination=CompiledRelationLocation(
            database=None,
            schema="main",
            name="customer_snapshot",
            qualified_name="main.customer_snapshot",
        ),
        fingerprint_query_sql="SELECT 1 AS customer_id",
        resolved_sql=(
            "SELECT 1 AS customer_id, 'basic' AS plan, "
            "TIMESTAMP '2024-01-01 00:00:00' AS updated_at"
        ),
        logical_ddl="",
        unique_key=("customer_id",),
        snapshot_strategy="timestamp",
        updated_at_column="updated_at",
        contract_enforced=contract_enforced,
        contract_columns=contract_columns,
        pre_hooks=pre_hooks,
        post_hooks=post_hooks,
    )


class FakeCursorAdapter:
    def __init__(self, *, target_relation_exists: bool = False) -> None:
        self.target_relation_exists: bool = target_relation_exists

    def execute(self, connection: Any, sql: str) -> Any:
        return connection.execute(sql)

    def relation_exists(
        self, connection: Any, *, database: str | None, schema: str | None, name: str
    ) -> bool:
        del connection, database, schema, name
        return self.target_relation_exists

    def requires_derived_table_aliases(self) -> bool:
        return False

    def render_cursor_operand(self, *, expression: str, cursor_type: str | None) -> str:
        del cursor_type
        return expression


class RecordingCursorAdapter(FakeCursorAdapter):
    def __init__(self, *, target_relation_exists: bool) -> None:
        super().__init__(target_relation_exists=target_relation_exists)
        self.statements: list[str] = []

    def execute(self, connection: Any, sql: str) -> Any:
        self.statements.append(sql)
        return connection.execute(sql)


def build_name_test_adapter(adapter_name: str) -> DuckDbAdapter | BigQueryAdapter:
    return {"bigquery": BigQueryAdapter, "duckdb": DuckDbAdapter}[adapter_name]()


def build_fingerprint_audit_plan_entry() -> AuditPlanEntry:
    return build_fingerprint_audit_plan_entry_with_options()


def build_fingerprint_audit_plan_entry_with_options(
    *,
    name: str = "not_null_orders",
    severity: str = AuditSeverity.ERROR.value,
    attached_column_name: str | None = "order_id",
    resolved_sql: str = "SELECT order_id FROM analytics.orders WHERE order_id IS NULL",
    always_run: bool = False,
) -> AuditPlanEntry:
    return AuditPlanEntry(
        key=CompiledObjectKey(resource_type=CompiledResourceType.AUDIT, name=name),
        definition_name="test_audit",
        name=name,
        resolved_sql=resolved_sql,
        unresolved_sql='SELECT order_id FROM __ref("orders") WHERE order_id IS NULL',
        attachment_kind=AuditAttachmentKind.MODEL,
        severity=AuditSeverity(severity),
        requested_run_scope=AuditRunScope.FINAL,
        effective_run_scope=AuditRunScope.FINAL,
        attached_target_name="orders",
        attached_column_name=attached_column_name,
        always_run=always_run,
    )


def build_fingerprint_audit_result(
    *,
    outcome: str,
    audit_name: str = "not_null_orders",
    severity: str = AuditSeverity.ERROR.value,
    attached_column_name: str | None = "order_id",
) -> AuditExecutionResult:
    return AuditExecutionResult(
        audit_name=audit_name,
        audit_definition_name="test_audit",
        attachment_kind=AuditAttachmentKind.MODEL,
        severity=AuditSeverity(severity),
        outcome=AuditOutcome(outcome),
        row_count={
            AuditOutcome.PASS.value: 0,
            AuditOutcome.WARN.value: 1,
            AuditOutcome.ERROR.value: 1,
            AuditOutcome.INSUFFICIENT.value: 0,
        }[outcome],
        executed_sql="SELECT order_id FROM analytics.orders WHERE order_id IS NULL",
        run_scope_phase=AuditRunScope.FINAL,
        attached_target_name="orders",
        attached_column_name=attached_column_name,
    )


class FakeRelationReuseAdapter(BaseAdapter):
    adapter_name: str = "fake_relation_reuse"

    def __init__(self, *, supports_zero_copy_clone: bool) -> None:
        self._supports_zero_copy_clone: bool = supports_zero_copy_clone
        self.calls: list[str] = []
        self.sql: str | None = None

    def connect(self, config: dict[str, object]) -> object:
        del config
        return object()

    def _execute(self, connection: Any, sql: str) -> object:
        del connection, sql
        return object()

    def close(self, connection: Any) -> None:
        del connection

    def supports_zero_copy_clone(self) -> bool:
        self.calls.append("supports_zero_copy_clone")
        return self._supports_zero_copy_clone

    def clone(
        self,
        connection: Any,
        *,
        origin: str,
        destination: str,
        hard_copy: bool = False,
        origin_is_transient: bool = False,
        statement_recorder: StatementRecorder,
    ) -> None:
        del connection, origin, destination, hard_copy, origin_is_transient, statement_recorder
        self.calls.append("clone")

    def durable_clone(
        self,
        connection: Any,
        *,
        origin: str,
        destination: str,
        origin_is_transient: bool = False,
        statement_recorder: StatementRecorder,
    ) -> None:
        del connection, origin, destination, origin_is_transient, statement_recorder
        self.calls.append("durable_clone")

    def create_table_as(
        self,
        connection: Any,
        *,
        destination: str,
        sql: str,
        config: dict[str, Any] | None = None,
        statement_recorder: StatementRecorder,
    ) -> None:
        del connection, destination, config, statement_recorder
        self.calls.append("create_table_as")
        self.sql = sql


class RecordingConnection:
    def __init__(self) -> None:
        self.executed: list[str] = []

    def execute(self, sql: str) -> "RecordingConnection":
        self.executed.append(sql)
        return self

    def fetchall(self) -> list[tuple[object, ...]]:
        return []


class RecordingSnowflakeAdapter(SnowflakeAdapter):
    def _execute(self, *, connection: Any, sql: str) -> Any:
        return connection.execute(sql)


class RecordingPostgresAdapter(PostgresAdapter):
    def _execute(self, *, connection: Any, sql: str) -> Any:
        return connection.execute(sql)


class RecordingDuckDbAdapter(DuckDbAdapter):
    def _execute(self, *, connection: Any, sql: str) -> Any:
        return connection.execute(sql)


def build_recording_adapter(
    adapter_name: str,
) -> RecordingSnowflakeAdapter | RecordingPostgresAdapter | RecordingDuckDbAdapter:
    return {
        "snowflake": RecordingSnowflakeAdapter,
        "postgres": RecordingPostgresAdapter,
        "duckdb": RecordingDuckDbAdapter,
    }[adapter_name]()


def run_python_hook_with_relations(
    *,
    function: Callable[..., object],
    reads: tuple[SqlResourceRef, ...],
    enforce_explicit_references: bool = True,
    warnings: list[str] | None = None,
) -> None:
    """Run one post-hook on ``orders`` with ``customers`` and ``raw_orders`` planned."""

    adapter: DuckDbAdapter = DuckDbAdapter()
    connection: Any = adapter.connect({"database": ":memory:"})
    connection.execute("CREATE TABLE customers AS SELECT 1 AS customer_id")
    connection.execute("CREATE SCHEMA raw")
    connection.execute("CREATE TABLE raw.orders AS SELECT 1 AS order_id")
    execute_hooks(
        connection=connection,
        adapter=adapter,
        hooks=[PythonHookEntry(name="refresh_lookup", kwargs={})],
        phase=HookPhase.POST_HOOKS,
        hook_functions=(
            DiscoveredHookFunction(
                file_path=Path(__file__),
                relative_path=Path("hooks/python/lookups.py"),
                name="refresh_lookup",
                function=function,
                reads=reads,
            ),
        ),
        hook_run=HookRunContext(
            model_name="orders",
            destination=CompiledRelationLocation(
                database=None, schema="main", name="orders", qualified_name=None
            ),
            run_id="run-1",
            relation_lookup=HookRelationLookup(
                model_locations={
                    "customers": CompiledRelationLocation(
                        database=None, schema="main", name="customers", qualified_name=None
                    ),
                    "orders": CompiledRelationLocation(
                        database=None, schema="main", name="orders", qualified_name=None
                    ),
                },
                source_map={
                    "raw_orders": SourceEntry(name="raw_orders", schema="raw", table="orders")
                },
            ),
            warnings=warnings,
            enforce_explicit_references=enforce_explicit_references,
        ),
    )


def quoted_spelling_rejection(*, configured: str, listed_names: tuple[str, ...]) -> str | None:
    """The error a unique_key ``configured`` gets against ``listed_names``, or None."""

    try:
        require_quoted_spelling(
            model_name="orders",
            setting="unique_key",
            configured=(configured,),
            listed_columns=tuple(ColumnInfo(name=name, type="VARCHAR") for name in listed_names),
        )
    except ExecutorInputError as error:
        return str(error)
    return None
