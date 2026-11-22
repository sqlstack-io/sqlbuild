"""Internal Python-node execution result models."""

from __future__ import annotations

import logging
from collections.abc import Callable, Mapping
from dataclasses import dataclass, field
from datetime import datetime
from typing import Any

from sqlbuild._native import TargetSettings
from sqlbuild.adapter.contract.classes.base_adapter import BaseAdapter
from sqlbuild.adapter.contract.classes.node_logger import NodeLogger
from sqlbuild.adapter.contract.classes.statement_recorder import StatementRecorder
from sqlbuild.adapter.contract.models import LifeCycleEvent
from sqlbuild.adapter.relations.main.resolve_qualified_name_parts import (
    resolve_qualified_name_parts,
)
from sqlbuild.compiler.python_nodes.types import (
    PythonNodeFanInAction,
    PythonNodeKind,
    PythonNodeStatus,
    SkipMode,
)
from sqlbuild.errors.contracts.exceptions import ExecutorInputError
from sqlbuild.executor.load.models import LoadExecutionResult
from sqlbuild.executor.node_results.classes.dependency_result_reader import DependencyResultReader
from sqlbuild.executor.python_nodes.classes.runtime_relation_guard import RuntimeRelationGuard
from sqlbuild.executor.python_nodes.constants import (
    PYTHON_NODE_RELATION_QUALIFIER_SEPARATOR,
)
from sqlbuild.executor.python_nodes.types import (
    ExecutablePythonNode,
    OwnedResultCallback,
    PythonIdentityRecorder,
)
from sqlbuild.executor.scheduling.classes.session_warehouses import (
    SessionWarehouses,
    configured_warehouse,
)
from sqlbuild.provider.main.runtime import ProviderContainer, _empty_provider_container
from sqlbuild.python_nodes.models import SqlResourceRef
from sqlbuild.python_nodes.types import PythonCheckSeverity
from sqlbuild.runtime.contracts.types import NodeStartCallback


@dataclass(frozen=True)
class PythonNodeResult:
    """User-facing successful result returned by a Python node."""

    payload: object | None = None
    metadata: dict[str, object] = field(default_factory=dict)
    materialized: bool | None = None


@dataclass(frozen=True)
class PythonNodeSkipResult:
    """User-facing skip signal returned by a Python node."""

    reason: str
    mode: SkipMode = SkipMode.SOFT
    metadata: dict[str, object] = field(default_factory=dict)


@dataclass(frozen=True)
class PythonCheckResult:
    """User-facing result returned by a Python check node."""

    passed: bool
    message: str | None = None
    metadata: dict[str, object] = field(default_factory=dict)
    severity: PythonCheckSeverity | None = None


@dataclass(frozen=True)
class PythonCheckExecutionResult:
    """Normalized execution outcome for one Python check node."""

    node_name: str
    passed: bool
    severity: PythonCheckSeverity
    message: str | None = None
    metadata: dict[str, object] = field(default_factory=dict)
    error_message: str | None = None
    warning_messages: tuple[str, ...] = field(default_factory=tuple)

    @property
    def warned(self) -> bool:
        """Return whether this result is a non-failing warning."""

        return not self.passed and self.severity == PythonCheckSeverity.WARN

    @property
    def failed(self) -> bool:
        """Return whether this result is a failing error."""

        return not self.passed and self.severity == PythonCheckSeverity.ERROR


@dataclass(frozen=True)
class PythonNodeExecutionResult:
    """Normalized execution outcome for one Python node."""

    node_name: str
    kind: PythonNodeKind
    status: PythonNodeStatus
    payload: object | None = None
    metadata: dict[str, object] = field(default_factory=dict)
    materialized: bool | None = None
    skip_mode: SkipMode | None = None
    skip_reason: str | None = None
    error_message: str | None = None
    warning_messages: tuple[str, ...] = field(default_factory=tuple)
    lifecycle_events: tuple[LifeCycleEvent, ...] = field(default_factory=tuple)


@dataclass(frozen=True)
class PythonNodeFanInDecision:
    """Scheduler decision for a node after all upstream outcomes are known."""

    action: PythonNodeFanInAction
    reason: str | None = None
    skip_mode: SkipMode | None = None


from sqlbuild.executor.python_nodes.classes.run_state import PythonNodeRunState  # noqa: E402


@dataclass(frozen=True)
class PythonIngressLoaderExecutorResult:
    """Result bundle for pre-SQL Python/loader lifecycle execution."""

    python_results: tuple[PythonNodeExecutionResult, ...]
    load_results: tuple[LoadExecutionResult, ...]
    run_state: PythonNodeRunState


@dataclass(frozen=True)
class PythonNodeRuntime:
    """Runtime-invariant execution inputs shared across Python-node executors."""

    adapter: BaseAdapter
    connection_config: dict[str, object]
    connection: Any
    run_id: str
    target: str | None
    vars: dict[str, object]
    is_reload: bool
    target_config: TargetSettings | None = None
    default_database: str | None = None
    default_schema: str | None = None
    relation_targets: dict[SqlResourceRef, str] | None = None
    project_relations: dict[SqlResourceRef, str] | None = None
    start_cursor_ts: datetime | None = None
    end_cursor_ts: datetime | None = None
    start_cursor_int: int | None = None
    end_cursor_int: int | None = None
    providers: ProviderContainer | None = None
    result_store: Any | None = None
    persist_node_results: bool = True
    node_warehouses: Mapping[str, str] = field(default_factory=dict)
    session_warehouses: SessionWarehouses | None = None
    chain_run_id: str | None = None

    def __post_init__(self) -> None:
        if self.node_warehouses and self.session_warehouses is None:
            object.__setattr__(
                self,
                "session_warehouses",
                SessionWarehouses(
                    adapter=self.adapter,
                    home=configured_warehouse(connection_config=self.connection_config),
                ),
            )

    @property
    def resolved_relation_targets(self) -> dict[SqlResourceRef, str]:
        """Return relation targets, defaulting to an empty mapping."""

        return {} if self.relation_targets is None else self.relation_targets


@dataclass(frozen=True)
class IngressCallbacks:
    """Progress callbacks and display flags for Python ingress execution."""

    use_color: bool = False
    on_node_start: NodeStartCallback | None = None
    on_node_complete: Callable[[object], None] | None = None
    on_node_cancelled: Callable[[str], None] | None = None
    identity_recorder: PythonIdentityRecorder | None = None


@dataclass(frozen=True, kw_only=True)
class BasePythonNodeContext(DependencyResultReader):
    """Shared runtime helpers for framework-owned Python nodes."""

    adapter: BaseAdapter
    connection_config: dict[str, object]
    connection: Any
    run_id: str
    target: str | None
    vars: dict[str, object]
    is_reload: bool
    logger: logging.Logger
    statement_recorder: StatementRecorder
    run_state: PythonNodeRunState | None = None
    result_store: Any | None = None
    target_config: TargetSettings | None = None
    default_database: str | None = None
    default_schema: str | None = None
    relation_targets: dict[SqlResourceRef, str] = field(default_factory=dict)
    allowed_sql_refs: frozenset[SqlResourceRef] = frozenset()
    relation_guard: RuntimeRelationGuard | None = field(default=None, repr=False)
    providers: ProviderContainer = field(default_factory=_empty_provider_container)
    use_color: bool = False
    start_cursor_ts: datetime | None = None
    end_cursor_ts: datetime | None = None
    start_cursor_int: int | None = None
    end_cursor_int: int | None = None

    def execute_sql(self, sql: str) -> Any:
        if self.relation_guard is not None:
            self.relation_guard.check(sql)
        self.statement_recorder.record(sql)
        return self.adapter.execute(connection=self.connection, sql=sql)

    def query(self, sql: str) -> Any:
        if self.relation_guard is not None:
            self.relation_guard.check(sql)
        self.statement_recorder.record(sql)
        return self.adapter.execute(connection=self.connection, sql=sql)

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
        """Return a qualified relation name, preserving already-qualified input."""

        if PYTHON_NODE_RELATION_QUALIFIER_SEPARATOR in name:
            return name
        return resolve_qualified_name_parts(
            adapter=self.adapter,
            database=self.default_database if database is None else database,
            schema=self.default_schema if schema is None else schema,
            name=name,
        )

    def relation(self, ref: SqlResourceRef) -> str:
        """Return the adapter-qualified runtime relation for a declared SQL ref."""

        if ref not in self.allowed_sql_refs:
            raise ExecutorInputError(
                f"SQL relation ref '{ref.name}' must be declared in depends_on before use"
            )
        relation: str | None = self.relation_targets.get(ref)
        if relation is None:
            raise ExecutorInputError(f"No runtime relation found for SQL ref '{ref.name}'")
        if self.relation_guard is not None:
            self.relation_guard.record_resolved(ref)
        return relation

    def skip(
        self,
        *,
        reason: str,
        mode: SkipMode | str = SkipMode.SOFT,
        metadata: dict[str, object] | None = None,
    ) -> PythonNodeSkipResult:
        """Return a skip signal for the current Python node."""

        return PythonNodeSkipResult(
            reason=reason,
            mode=SkipMode(mode),
            metadata={} if metadata is None else metadata,
        )


@dataclass(frozen=True)
class DiscoveredContext:
    """The discovered item an asset node runs for, and the tags its set was saved with."""

    set: str
    item: str
    tags: Mapping[str, object]


@dataclass(frozen=True, kw_only=True)
class AssetContext(BasePythonNodeContext):
    """Runtime context passed to a SQLBuild asset function."""

    discovered: DiscoveredContext | None = None
    chain_run_id: str | None = None

    @property
    def root_run_id(self) -> str:
        """The server run ID of the rerun chain's first run, or the local run's outside a worker."""

        return self.chain_run_id or self.run_id

    def result(
        self,
        *,
        payload: object | None = None,
        metadata: dict[str, object] | None = None,
        materialized: bool | None = None,
    ) -> PythonNodeResult:
        """Return a successful asset result."""

        return PythonNodeResult(
            payload=payload,
            metadata={} if metadata is None else metadata,
            materialized=materialized,
        )


@dataclass(frozen=True, kw_only=True)
class CheckContext(BasePythonNodeContext):
    """Runtime context passed to a SQLBuild check function."""

    def pass_(
        self,
        *,
        message: str | None = None,
        metadata: dict[str, object] | None = None,
    ) -> PythonCheckResult:
        """Return a passing check result."""

        return PythonCheckResult(
            passed=True,
            message=message,
            metadata={} if metadata is None else metadata,
        )

    def fail(
        self,
        *,
        message: str,
        metadata: dict[str, object] | None = None,
    ) -> PythonCheckResult:
        """Return a failing check result using the check's configured severity."""

        return PythonCheckResult(
            passed=False,
            message=message,
            metadata={} if metadata is None else metadata,
        )

    def warn(
        self,
        *,
        message: str,
        metadata: dict[str, object] | None = None,
    ) -> PythonCheckResult:
        """Return a warning check result regardless of decorator severity."""

        return PythonCheckResult(
            passed=False,
            message=message,
            metadata={} if metadata is None else metadata,
            severity=PythonCheckSeverity.WARN,
        )


@dataclass(frozen=True)
class PythonCheckCallbacks:
    """Optional logging, identity, and completion callbacks for Python checks."""

    logger: logging.Logger | None = None
    identity_recorder: PythonIdentityRecorder | None = None
    on_check_complete: Callable[[PythonCheckExecutionResult], None] | None = None
    on_check_start: Callable[[str], None] | None = None


@dataclass(frozen=True)
class ItemSlot:
    """The warehouse connection and result store of one slot of a discovered item pool."""

    connection: Any
    result_store: Any | None


@dataclass(frozen=True)
class RunningItem:
    """A discovered item on a thread, with its set and the slot it holds."""

    node: ExecutablePythonNode
    set_name: str
    slot: ItemSlot


@dataclass(frozen=True)
class DiscoveredItemCallbacks:
    """What a serial path gives its item pool; each is called only on the calling thread."""

    ready: Callable[[], tuple[ExecutablePythonNode, ...]]
    upstream_results: Callable[[str], tuple[PythonNodeExecutionResult, ...]]
    complete: OwnedResultCallback
    cancelled: Callable[[str], None] | None = None
