"""Executor pipeline domain models."""

from __future__ import annotations

from collections.abc import Callable
from dataclasses import dataclass, field
from typing import Any

from sqlbuild._native import (
    AuditPlanEntry,
    CompiledObjectKey,
    FunctionPlanEntry,
    SqlTestPlanEntry,
)
from sqlbuild.executor.auditing.models import AuditExecutionResult
from sqlbuild.executor.build.models import (
    BuildCallbacks,
    BuildCustomizations,
    BuildInitialState,
    BuildRuntimeParams,
)
from sqlbuild.executor.testing.models import SqlTestExecutionResult
from sqlbuild.runtime.contracts.types import ConnectionElapsedCallback


@dataclass(frozen=True)
class AuditPipelineCallbacks:
    """Optional connection and item callbacks for standalone audit execution."""

    on_connection_start: Callable[[int], None] | None = None
    on_connection_complete: ConnectionElapsedCallback | None = None
    on_connection_error: ConnectionElapsedCallback | None = None
    on_audit_start: Callable[[AuditPlanEntry], None] | None = None
    on_audit_physical_complete: Callable[[AuditExecutionResult], None] | None = None
    on_audit_complete: Callable[[AuditExecutionResult], None] | None = None
    on_audit_error: Callable[[AuditPlanEntry], None] | None = None


@dataclass(frozen=True)
class TestPipelineCallbacks:
    """Optional connection, setup, and item callbacks for SQL test execution."""

    on_connection_start: Callable[[int], None] | None = None
    on_connection_complete: ConnectionElapsedCallback | None = None
    on_connection_error: ConnectionElapsedCallback | None = None
    on_progress: Callable[[str], None] | None = None
    on_test_start: Callable[[SqlTestPlanEntry], None] | None = None
    on_test_finished: Callable[[SqlTestPlanEntry], None] | None = None
    """Called on the worker once a test has its result, before results are reported in order."""
    on_test_complete: Callable[[SqlTestExecutionResult], None] | None = None


@dataclass(frozen=True)
class TemporaryTestFunctions:
    """Temporary function copies created for one test run and the functions left without one."""

    statements: tuple[tuple[str, str], ...] = ()
    """Each `(function name, statement)` that created a copy, in creation order."""
    created_keys: frozenset[CompiledObjectKey] = frozenset()
    replacements: dict[str, str] = field(default_factory=dict)
    failures: dict[CompiledObjectKey, str] = field(default_factory=dict)
    python_functions: tuple[FunctionPlanEntry, ...] = ()
    """Connection-scoped Python functions the first connection registered, in order."""


@dataclass(frozen=True)
class AuditExecutionFailure:
    """Safe public description of one standalone audit runtime failure."""

    audit_name: str
    resource_id: str
    error_type: str
    message: str


@dataclass(frozen=True)
class ResolvedBuildInputs:
    """Fully-resolved runtime bundles for one build pipeline run."""

    runtime: BuildRuntimeParams
    callbacks: BuildCallbacks
    customizations: BuildCustomizations
    initial_state: BuildInitialState


@dataclass(frozen=True)
class BuildConnectionPreparation:
    """Disjoint connection and schema preparation results."""

    scheduler_connection: Any
    worker_connections: tuple[Any, ...]
    connection_seconds: float
    schema_seconds: float | None
