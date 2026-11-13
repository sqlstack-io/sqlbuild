"""Test case types for native compiled project integration tests."""

from __future__ import annotations

from dataclasses import dataclass, field

from sqlbuild.adapter.contract.classes.base_adapter import BaseAdapter
from sqlbuild.orchestration_declarations.models.authoring import (
    CalendarSchedule,
    Job,
    JobGroup,
    Schedule,
    Sensor,
)


@dataclass(frozen=True)
class HeaderProjectionTestCase:
    """Native header columns projected after real compilation and audit attachment."""

    description: str
    files: dict[str, str]
    model_name: str
    expected_columns: tuple[str, ...]


@dataclass(frozen=True)
class RetainedRulesRowsTestCase:
    """A real compile whose rules request reads model facts the compile-input stage retained."""

    description: str
    expected_codes: tuple[str, ...]


@dataclass(frozen=True)
class ZeroMaterializationTestCase:
    """An ordinary compile that must not construct the legacy Python project representation."""

    description: str
    arguments: tuple[str, ...]
    expected_constructions: dict[str, int]


@dataclass(frozen=True)
class ModelLoopParityTestCase:
    """A project whose model inputs the native loop builds as the attachment recorded them."""

    description: str
    fixtures: tuple[str, ...]
    files: dict[str, str]
    expected_native_models: int
    expected_summary: str


@dataclass(frozen=True)
class ModelLoopErrorTestCase:
    """A model the native loop rejects with the attachment's exact message and code."""

    description: str
    model_sql: str
    expected_message: str
    expected_code: str
    files: dict[str, str] = field(default_factory=dict)


@dataclass(frozen=True)
class ModelPathScopeErrorTestCase:
    """A model path the scope lookup rejects while the loop resolves its declarations."""

    description: str
    model_path: str
    expected_message: str
    expected_code: str


@dataclass(frozen=True)
class ImmediateSyntaxErrorTestCase:
    """A model the native parser cannot read while the loop validates its SQL immediately."""

    description: str
    model_sql: str
    expected_message: str
    expected_help_fragment: str


@dataclass(frozen=True)
class NativeSourceAttachmentTestCase:
    """A source the native loop attaches: its assembled entry and recorded declaration usages."""

    description: str
    source_yml: str
    source_name: str
    expected_entry: dict[str, object]
    expected_usages: tuple[str, ...]


@dataclass(frozen=True)
class NativeSourceKeysTestCase:
    """Sources whose declared keys native views answer as their attached entries do."""

    description: str
    source_yml: str
    expected_keys: dict[str, tuple[tuple[str, ...], ...]]


@dataclass(frozen=True)
class NativeSourceErrorTestCase:
    """A source the native loop rejects while attaching it."""

    description: str
    source_yml: str
    expected_message: str
    expects_help: bool


@dataclass(frozen=True)
class NativeFunctionAttachmentTestCase:
    """A function the native loop attaches and assembly keeps in the native project."""

    description: str
    function_name: str
    expected_summary: tuple[object, ...]
    expected_usages: tuple[str, ...]


@dataclass(frozen=True)
class NativeFunctionErrorTestCase:
    """Function files the native loop rejects while attaching them."""

    description: str
    files: dict[str, str]
    expected_message: str


@dataclass(frozen=True)
class NativeAuditAttachmentTestCase:
    """An audit the native loop attaches and assembly keeps in the native project."""

    description: str
    audit_name: str
    expected_summary: tuple[object, ...]
    expected_usages: tuple[str, ...]


@dataclass(frozen=True)
class NativeAuditErrorTestCase:
    """Audit files the native loop rejects while attaching them."""

    description: str
    files: dict[str, str]
    expected_message: str


@dataclass(frozen=True)
class NativeMeasurementPolicyTestCase:
    """A standalone measurement audit whose header policy the native loop parses."""

    description: str
    files: dict[str, str]
    expected_policy: tuple[object, ...]


@dataclass(frozen=True)
class NativeResourceDiagnosticTestCase:
    """A project's files and the P010, P011 and C216 diagnostics compiling it reports."""

    description: str
    files: dict[str, str]
    expected_diagnostics: tuple[tuple[object, ...], ...]


@dataclass(frozen=True)
class HandBuiltAnalysisTestCase:
    """Model SQL and the analysis facts native and hand-built assembly report for it."""

    description: str
    models: dict[str, str]
    expected_models: tuple[tuple[object, ...], ...]


@dataclass(frozen=True)
class SharedInferredColumnsTestCase:
    description: str
    model_count: int
    expected_columns_per_model: int


@dataclass(frozen=True)
class AdapterParameterBoundaryTestCase:
    description: str
    adapter_name: str
    declaration: str
    literal: str
    expected_sql: str


@dataclass(frozen=True)
class QuotedHookBoundaryTestCase:
    description: str
    argument: str
    expected_sql: str


@dataclass(frozen=True)
class SnapshotCatalogTestCase:
    description: str
    files: dict[str, str]
    expected_kinds: frozenset[str]


@dataclass(frozen=True)
class SnapshotIdentityTestCase:
    description: str
    path: str
    before: str
    after: str
    expected_changed: list[str]


@dataclass(frozen=True)
class SnapshotRejectionTestCase:
    description: str
    files: dict[str, str]
    nodes: dict[str, object]
    expected_compiler_errors: bool
    expected_message: str


@dataclass(frozen=True)
class SnapshotEnvironmentTestCase:
    description: str
    first_lockfile: str
    second_lockfile: str
    expected_environment_count: int


@dataclass(frozen=True)
class SnapshotExpressionRangeTestCase:
    """A published model whose compiled SQL differs from its source, and each column's text."""

    description: str
    adapter: type[BaseAdapter]
    files: dict[str, str]
    model_name: str
    expected_texts: dict[str, str]


@dataclass(frozen=True)
class SnapshotLineageTestCase:
    description: str
    files: dict[str, str]
    expected_edges: list[list[str]]


@dataclass(frozen=True)
class SnapshotProvidersTestCase:
    description: str
    files: dict[str, str]
    environment: dict[str, str]
    expected_providers: dict[str, object] | None


@dataclass(frozen=True)
class SnapshotScheduleTestCase:
    description: str
    schedule: Schedule
    expected_selection: tuple[str, ...]
    expected_controls: dict[str, object] = field(
        default_factory=lambda: {
            "starts_at": None,
            "default_status": "running",
        }
    )


@dataclass(frozen=True)
class SnapshotDeclarationRejectionTestCase:
    description: str
    job_description: str = "Orders."
    select: str = "orders"
    cron: str = "0 2 * * *"
    timezone: str = "UTC"
    settings: dict[str, object] = field(default_factory=dict)
    concurrency: int | None = None
    copies: int = 1
    expected_message: str = ""
    schedule: Schedule | None = None


@dataclass(frozen=True)
class SnapshotDiscoveryTestCase:
    description: str
    files: dict[str, str]
    expected_jobs: tuple[str, ...]
    declarations: tuple[Job | JobGroup | CalendarSchedule | Sensor, ...] | None = None
    expected_groups: dict[str, dict[str, list[str]]] = field(default_factory=dict)
    expected_sensors: dict[str, dict[str, object]] = field(default_factory=dict)
    expected_schedules: tuple[str, ...] = ()
    expected_declared_at: dict[str, dict[str, object] | None] = field(default_factory=dict)
    expected_trigger_declared_at: dict[str, dict[str, object] | None] = field(default_factory=dict)


@dataclass(frozen=True)
class SnapshotDiscoveryRejectionTestCase:
    description: str
    files: dict[str, str]
    expected_error: type[Exception]
    expected_message: str
