"""Public Python-node authoring models."""

from __future__ import annotations

from collections.abc import Callable, Mapping
from dataclasses import dataclass, field

from sqlbuild._native import (
    MeasurementThresholds,
    SourceColumnEntry,
    SourceWriteStrategy,
    retry_policy_exceptions,
    validate_audit_case,
)
from sqlbuild.compiler.auditing.types import AuditSeverity
from sqlbuild.python_nodes.constants import RELATION_RENDERER
from sqlbuild.python_nodes.types import NodeSetKind, PythonCheckSeverity, SqlResourceRefKind


@dataclass(frozen=True)
class SqlResourceRef:
    """Typed dependency reference to a SQLBuild SQL graph resource."""

    kind: SqlResourceRefKind
    name: str

    def __str__(self) -> str:
        """Render as SQL inside a macro invocation, or as the authoring call elsewhere."""

        renderer: Callable[[object], str] | None = RELATION_RENDERER.get()
        if renderer is None:
            return f'{self.kind.value}("{self.name}")'
        return renderer(self)


@dataclass(frozen=True)
class NodeSetRef:
    """A dependency on a job's or tag's end nodes, resolved into node edges at compile."""

    kind: NodeSetKind
    name: str
    exclude: str | None = None

    def __str__(self) -> str:
        """Render as the authoring call."""

        exclude: str = "" if self.exclude is None else f', exclude="{self.exclude}"'
        return f'{self.kind.value}("{self.name}"{exclude})'


@dataclass(frozen=True)
class ColumnLineageRef:
    """Graph-based upstream column reference for declared Python-node lineage."""

    node: str
    column: str


@dataclass(frozen=True, init=False)
class RetryPolicy:
    """Retry policy metadata for Python-node execution."""

    max_attempts: int
    retry_on: tuple[type[BaseException], ...]
    initial_delay_seconds: float
    backoff_multiplier: float
    max_delay_seconds: float | None
    max_elapsed_seconds: float | None
    jitter: bool

    def __init__(
        self,
        *,
        max_attempts: int = 3,
        retry_on: type[BaseException]
        | tuple[type[BaseException], ...]
        | list[type[BaseException]] = Exception,
        initial_delay_seconds: float = 1.0,
        backoff_multiplier: float = 2.0,
        max_delay_seconds: float | None = 30.0,
        max_elapsed_seconds: float | None = None,
        jitter: bool = True,
    ) -> None:
        normalized_retry_on: tuple[type[BaseException], ...] = retry_policy_exceptions(
            max_attempts=max_attempts,
            retry_on=retry_on,
            initial_delay_seconds=initial_delay_seconds,
            backoff_multiplier=backoff_multiplier,
            max_delay_seconds=max_delay_seconds,
            max_elapsed_seconds=max_elapsed_seconds,
        )
        object.__setattr__(self, "max_attempts", max_attempts)
        object.__setattr__(self, "retry_on", normalized_retry_on)
        object.__setattr__(self, "initial_delay_seconds", initial_delay_seconds)
        object.__setattr__(self, "backoff_multiplier", backoff_multiplier)
        object.__setattr__(self, "max_delay_seconds", max_delay_seconds)
        object.__setattr__(self, "max_elapsed_seconds", max_elapsed_seconds)
        object.__setattr__(self, "jitter", jitter)


@dataclass(frozen=True)
class LoaderDefinition:
    """Metadata attached to a decorated source loader function."""

    name: str
    depends_on: tuple[Callable[..., object] | SqlResourceRef | NodeSetRef, ...] = ()
    destination: str | None = None
    write_strategy: SourceWriteStrategy | None = None
    cursor_column: str | None = None
    unique_key: tuple[str, ...] = ()
    columns: tuple[SourceColumnEntry, ...] = ()
    contract: str | None = None
    description: str | None = None


@dataclass(frozen=True)
class FactoryDefinition:
    """Metadata attached to a Python-node factory function."""

    name: str


@dataclass(frozen=True)
class DiscoverDefinition:
    """Metadata attached to a function that discovers a set of items at publish."""

    name: str
    check_every: str | None = None


@dataclass(frozen=True)
class Item:
    """One discovered item: its ID, the items or nodes it waits on, and its nodes' arguments."""

    id: str
    depends_on: tuple[str | SqlResourceRef | NodeSetRef, ...] = ()
    args: Mapping[str, object] = field(default_factory=dict)


@dataclass(frozen=True)
class Discovered:
    """What a discover function returns: its items, and optional set-wide tags and parallelism."""

    items: tuple[Item, ...]
    tags: Mapping[str, object] | None = None
    max_parallel: int | None = None


@dataclass(frozen=True)
class DiscoveredItemRef:
    """A discovered item's dependency on another item of its set, by node name."""

    name: str


@dataclass(frozen=True)
class AuditFactoryDefinition:
    """Metadata attached to an audit-case factory function."""

    name: str


@dataclass(frozen=True)
class HookDefinition:
    """Metadata attached to a decorated model lifecycle hook function."""

    name: str
    description: str | None = None
    reads: tuple[SqlResourceRef, ...] = ()


@dataclass(frozen=True)
class AssetDefinition:
    """Metadata attached to a decorated SQLBuild asset function."""

    name: str
    depends_on: tuple[Callable[..., object] | SqlResourceRef | NodeSetRef, ...] = ()
    tags: tuple[str, ...] = ()
    group: str | None = None
    description: str | None = None
    meta: dict[str, object] | None = None
    columns: tuple[SourceColumnEntry, ...] = ()
    column_lineage: dict[str, tuple[ColumnLineageRef, ...]] | None = None
    retry: RetryPolicy | None = None
    for_each: str | None = None


@dataclass(frozen=True)
class CheckDefinition:
    """Metadata attached to a decorated SQLBuild check function."""

    name: str
    depends_on: tuple[Callable[..., object] | SqlResourceRef | NodeSetRef, ...]
    severity: PythonCheckSeverity = PythonCheckSeverity.ERROR
    tags: tuple[str, ...] = ()
    group: str | None = None
    description: str | None = None
    meta: dict[str, object] | None = None


@dataclass(frozen=True, kw_only=True)
class AuditCase:
    """One named generic-audit attachment generated by an audit factory."""

    name: str
    definition: str
    arguments: Mapping[str, object] = field(default_factory=dict)
    severity: AuditSeverity | None = None
    run_scope: str | None = None
    always_run: bool = False
    description: str | None = None
    thresholds: MeasurementThresholds | None = None
    minimum_samples: int | None = None
    evidence_limit: int | None = None
    meta: dict[str, object] | None = None

    def __post_init__(self) -> None:
        validate_audit_case(
            name=self.name,
            definition=self.definition,
            minimum_samples=self.minimum_samples,
            evidence_limit=self.evidence_limit,
            meta=self.meta,
        )
