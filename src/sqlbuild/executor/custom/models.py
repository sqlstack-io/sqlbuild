"""Types for user-defined custom materializations."""

from __future__ import annotations

import logging
from collections.abc import Callable
from dataclasses import dataclass, field
from typing import Any

from sqlbuild._native import SchemaFinding
from sqlbuild.adapter.contract.classes.base_adapter import BaseAdapter
from sqlbuild.adapter.contract.classes.node_logger import NodeLogger
from sqlbuild.adapter.contract.classes.statement_recorder import StatementRecorder
from sqlbuild.adapter.contract.models import ColumnInfo, RelationInfo
from sqlbuild.executor.auditing.models import AuditExecutionResult
from sqlbuild.executor.custom.main._execute_sql import execute_sql_with_recording
from sqlbuild.executor.custom.main._qualify_relation import qualify_custom_relation
from sqlbuild.provider.main.runtime import ProviderContainer, _empty_provider_container


@dataclass(frozen=True)
class MaterializationContext:
    """Context provided by the framework to a custom materialize() function."""

    adapter: BaseAdapter
    connection: Any
    destination: str
    destination_database: str | None
    destination_schema: str | None
    destination_name: str
    sql: str
    config: dict[str, Any]
    placeholders: dict[str, str]
    existing_relation: RelationInfo | None
    run_id: str
    build_target: str
    vars: dict[str, object]
    unique_key: tuple[str, ...]
    declared_columns: tuple[ColumnInfo, ...]
    is_first_run: bool
    is_full_refresh: bool
    query_changed: bool
    schema_findings: tuple[SchemaFinding, ...]
    run_audits: Callable[[str], tuple[AuditExecutionResult, ...]]
    on_progress: Callable[[str], None] | None
    logger: logging.Logger
    statement_recorder: StatementRecorder = field(default_factory=StatementRecorder)
    providers: ProviderContainer = field(default_factory=_empty_provider_container)

    def execute_sql(self, sql: str) -> Any:
        """Execute a SQL statement, recording it for runtime artifacts and verbose output."""
        return execute_sql_with_recording(
            adapter=self.adapter,
            connection=self.connection,
            sql=sql,
            statement_recorder=self.statement_recorder,
        )

    @property
    def log(self) -> NodeLogger:
        """`ctx.log.info(...)`, `.warning(...)` or `.error(...)`, kept with the run's events."""

        return NodeLogger(recorder=self.statement_recorder, logger=self.logger)

    def qualify_name(
        self,
        *,
        name: str,
        database: str | None = None,
        schema: str | None = None,
    ) -> str:
        """Return a fully-qualified relation name, preserving already-qualified input."""

        return qualify_custom_relation(
            adapter=self.adapter,
            name=name,
            destination_database=self.destination_database,
            destination_schema=self.destination_schema,
            database=database,
            schema=schema,
        )

    def qualify_in_destination_schema(self, name: str) -> str:
        """Return a relation name qualified into the destination database/schema."""

        return self.qualify_name(name=name)


@dataclass(frozen=True)
class MaterializationResult:
    """Result returned by a custom materialize() function."""

    relation: str
    failed: bool = False
    error: str | None = None
    cleanup_relations: tuple[str, ...] = field(default_factory=tuple)
    audit_results: tuple[AuditExecutionResult, ...] | None = None
