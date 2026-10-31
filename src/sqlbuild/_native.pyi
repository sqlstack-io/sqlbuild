"""Private SQLBuild native engine bindings."""

import logging
import threading
from collections import Counter
from collections.abc import Callable, Iterable, Iterator, Mapping, Sequence, Set
from contextlib import AbstractContextManager
from contextvars import Token
from dataclasses import Field
from datetime import date, datetime
from decimal import Decimal
from enum import StrEnum
from pathlib import Path, PurePath
from types import TracebackType
from typing import Any, ClassVar, Literal, Protocol, Self, TextIO, TypedDict, overload

from sqlbuild.adapter.contract.classes.base_adapter import (
    BaseAdapter,
)
from sqlbuild.adapter.contract.classes.strict_adapter import (
    StrictAdapter,
)
from sqlbuild.adapter.contract.models import (
    ColumnInfo,
    ExpressionInferenceProfile,
    QueryResult,
    RelationInfo,
    RetentionRequest,
    RowDiffSampling,
    RowDiffTolerances,
)
from sqlbuild.adapter.contract.types import (
    AdapterExecute,
    FrameworkType,
    MigrationTransfer,
    TablePromotionMode,
    TypeDialect,
)
from sqlbuild.adapter.relations.models import InspectionQueryRecord, StatementMetadataEffect
from sqlbuild.compiler.auditing.types import (
    AuditAttachmentKind,
    AuditEvaluationMode,
    AuditOutcome,
    AuditRunScope,
    AuditSeverity,
    ThresholdOperator,
)
from sqlbuild.compiler.compile.types import (
    AttachedAuditTargetKind,
    AuditInput,
    CompiledAuditView,
    CompiledFunctionView,
    CompiledModelView,
    CompiledProjectView,
    CompiledSeedView,
    CompiledSourceView,
    ExternalSqlReferenceResolver,
    FunctionLanguage,
    RelationKeyRow,
    SourceInput,
    SqlTestMode,
    TypedSqlValueRenderer,
)
from sqlbuild.compiler.python_nodes.types import PythonNodeKind
from sqlbuild.compiler.references.models import (
    ProjectRelation,
    ProjectRelationIndex,
    RelationName,
)
from sqlbuild.cost.models import CostResourceContext, RunCostSummary, StatementExecutionTelemetry
from sqlbuild.cost.types import CostStatus
from sqlbuild.cursor_algebra.types import Bound, BoundSentinel, CursorScalar
from sqlbuild.diagnostics.models import ProcessResourceUsage
from sqlbuild.executor.auditing.models import (
    AuditExecutionResult,
)
from sqlbuild.executor.build.models import (
    BuildExecutionResult,
    FunctionExecutionResult,
    SchedulerState,
    SeedExecutionResult,
)
from sqlbuild.executor.clone.models import (
    CloneExecutionResult,
    CloneItemResult,
)
from sqlbuild.executor.diff.models import DiffExecutionResult, RowDiffSamplingOverride
from sqlbuild.executor.janitor.models import (
    JanitorExecutionResult,
    JanitorPlan,
    JanitorRelationKey,
)
from sqlbuild.executor.load.models import LoadExecutionResult
from sqlbuild.executor.python_nodes.models import (
    PythonCheckExecutionResult,
    PythonNodeExecutionResult,
)
from sqlbuild.executor.run.models import (
    MicrobatchBatchProgress,
    ModelExecutionResult,
)
from sqlbuild.executor.testing.models import (
    SqlTestExecutionResult,
    StepResult,
)
from sqlbuild.microbatches.models import (
    MicrobatchCoverageProjection,
    MicrobatchEvent,
    MicrobatchInterval,
    MicrobatchScope,
    ReplayRequirementProjection,
)
from sqlbuild.microbatches.types import MicrobatchRecordType
from sqlbuild.provider.classes.container import ProviderContainer
from sqlbuild.provider.classes.provider import Provider
from sqlbuild.provider.classes.session import ProviderSession
from sqlbuild.python_nodes.models import (
    AuditCase,
    ColumnLineageRef,
    RetryPolicy,
    SqlResourceRef,
)
from sqlbuild.python_nodes.types import PythonCheckSeverity
from sqlbuild.rule_engine.types import RuleCheck, RuleOptionValue
from sqlbuild.runtime.compute_logs.models import (
    CaptureByteCounts,
    CaptureInventory,
    CaptureMetadata,
    ComputeLogReadChunk,
    FinalCaptureMetadata,
    PruneResult,
)
from sqlbuild.runtime.compute_logs.types import ComputeLogStream
from sqlbuild.runtime.contracts.models import ConnectionHooks
from sqlbuild.runtime.contracts.types import ExecutionResourceKind
from sqlbuild.runtime.execution_history.models import (
    EventFilter,
    EventPage,
    RunFilter,
    RunPage,
    RunRecord,
    StoredEvent,
)
from sqlbuild.runtime.execution_history.types import (
    CanonicalLifecycleEvent,
    LifecycleEventLogStorage,
    RunStorage,
)
from sqlbuild.runtime.observability.classes.event_dispatcher import EventDispatcher
from sqlbuild.runtime.observability.models import DiagnosticLog, LifecycleEvent
from sqlbuild.runtime.observability.types import JSONValue
from sqlbuild.runtime.output_capture.models import (
    BoundCommandOutputSink,
    CommandOutputCaptureSummary,
    CommandOutputRecord,
    OutputCaptureContext,
)
from sqlbuild.runtime.output_capture.types import CommandOutputStream

BUILD_IDENTITY: str
HOST_ENTRIES: tuple[tuple[str, str, str], ...]
MICROBATCH_START_SENTINEL: str
MICROBATCH_END_SENTINEL: str
MIGRATION_FINGERPRINT_METADATA_KEY: str
SNAPSHOT_DEFAULT_VALID_FROM_COLUMN: str
SNAPSHOT_DEFAULT_VALID_TO_COLUMN: str
PROJECT_CONFIG_FILENAME: str
PYTHON_INIT_MODULE_STEM: str
PYTHON_NODE_ROOT: str
SETTINGS_SECTION: str
TABLE_PROMOTION_MODE_SETTING_KEY: str
STAGED_PROMOTION_MODE: str
STAGE_CAPTURE_DIR_ENV_VAR: str
COMPILE_REUSE_DISABLE_ENV_VAR: str
DELTA_COMPILE_DISABLE_ENV_VAR: str
DELTA_COMPILE_VERIFY_ENV_VAR: str
SQLBUILD_QUERY_TAG_APP: str
STORE_TRACKED_ENVIRONMENT_PREFIXES: tuple[str, ...]
STORE_UNTRACKED_ENVIRONMENT_NAMES: frozenset[str]
GRAIN_ORDER: dict[CursorGrain, int]
INVOCATION_CONTEXT_ENV: str
COMMAND_OUTPUT_RECORD_TYPE: str
COMMAND_OUTPUT_LOSS_RECORD_TYPE: str
INTEGRATION_RESULT_PATH_ENV: str
FINGERPRINT_TABLE_NAME: str
SOURCE_FRESHNESS_TABLE_NAME: str
MIGRATION_TABLE_NAME: str
COLUMN_MIGRATION_TABLE_NAME: str
OLD_NAME_VIEW_TABLE_NAME: str
COLUMN_NODE_TYPE: str
COLUMN_NODE_NAME: str
COLUMN_TARGET_DATABASE: str
COLUMN_TARGET_SCHEMA: str
COLUMN_TARGET_NAME: str
COLUMN_RUN_ID: str
COLUMN_DEFINITION_HASH: str
COLUMN_VERSION_HASH: str
COLUMN_SCHEMA_FINGERPRINT: str
COLUMN_DEFINITION_B64: str
COLUMN_METADATA_JSON_B64: str
COLUMN_TIMESTAMP: str
NODE_TYPE_MODEL: str
NODE_TYPE_SEED: str
AUDIT_GATE_METADATA_KEY: str
CUSTOM_HOST_RUNTIME_VERSION: str
CUSTOM_HOST_LAUNCH_MODULE: str
CUSTOM_RULE_PROJECT_SUBJECT: str
CONTRACT_ENFORCED: str
LAUNCH_RECORD_OPTION: str
ZERO_DAY_CURSOR_DURATION: str
AST_CTE_KIND: str
SQL_TEXT_FIELD: str
SQL_DIGEST_FIELD: str
SQL_PRIVATE_RECORD_FIELD: str
MIN_CUSTOM_RULE_TEST_CASES: str
DEFAULT_PAGE_LIMIT: int
MAX_PAGE_LIMIT: int
CURRENT_EVENT_LOG_SCHEMA_VERSION: int
CURRENT_RUN_STORAGE_SCHEMA_VERSION: int
COMPUTE_LOG_FORMAT_VERSION: int
DEFAULT_RETENTION_COUNT: int
MAX_READ_BYTES: int
CURRENT_COMMAND_OUTPUT_SCHEMA_VERSION: int
MIGRATION_WRITE_ATTEMPTS: int
CUSTOM_HOST_PROTOCOL_VERSION: int
CUSTOM_HOST_PAYLOAD_MAGIC: bytes

def plan_sql_tests(
    *,
    tests: tuple[CompiledSqlTest, ...],
    project: CompiledProjectView,
    adapter: BaseAdapter,
    sql_analysis_enabled: bool = False,
    validate_fixtures: bool = False,
    fixture_planning_context: RelationFixturePlanningContext | None = None,
) -> tuple[SqlTestPlanResult, ...]: ...
def plan_sql_tests_natively(
    *,
    project: CompiledProjectView,
    tests: tuple[CompiledSqlTest, ...],
    adapter: BaseAdapter,
    sql_analysis_enabled: bool,
    render_sql: bool,
    include_plan: bool = True,
) -> tuple[NativeSqlTestPlan, ...]: ...
def plan_compiled_sql_test_artifacts(
    *,
    project: CompiledProjectView,
    tests: tuple[CompiledSqlTest, ...],
    adapter: BaseAdapter,
    sql_analysis_enabled: bool,
) -> tuple[NativeSqlTestArtifact, ...]: ...
def sql_test_plan_error_messages(*, warnings: tuple[PlanWarning, ...]) -> tuple[str, ...]: ...

STAGE_CAPTURE_OMITTED_ATTRIBUTES: dict[str, frozenset[str]]

class CompileInputReads:
    """Environment, settings, and run-identity reads observed during one compile."""

    def __init__(self) -> None: ...
    def add_environment_name(self, name: str) -> None: ...
    def add_settings_class(self, settings_class: type) -> None: ...
    def mark_run_id_read(self) -> None: ...
    @property
    def environment_names(self) -> tuple[str, ...]: ...
    @property
    def settings_classes(self) -> tuple[type, ...]: ...
    @property
    def read_run_id(self) -> bool: ...

class NativeCompilerError(RuntimeError):
    """An internal native compiler failure: a panic or a broken native invariant."""

class LintPreparationRequest(TypedDict):
    expanded: str
    before_expansion: str
    prior_sites: list[int]
    dialect: str

class NativeRulesRequest:
    """A natively built rules request held until it is evaluated once."""

    def sql_facts_json(self) -> str: ...

def build_rules_request(
    *,
    project: CompiledProjectView,
    config: RulesConfig,
    project_dir: Path,
    dialect: str,
    initial_findings: tuple[Finding, ...],
    custom_payloads: list[dict[str, object]],
    include_type_proof: bool,
    cache_directory: str | None = None,
) -> NativeRulesRequest: ...
def emit_orjson_report(report: object) -> str: ...
def emit_json_report(report: object) -> str: ...

type ColumnRow = tuple[str, str | None, str]

type DiagnosticRow = tuple[str, str, int | None, int | None, int | None, int | None, str]

type ProofRow = tuple[
    bool, list[ColumnRow], list[tuple[str, str | None]], list[str], str | None, bool
]

type QueryColumnsMode = Literal["batch", "reanalysis", "parse", "legacy"]

type SqlTestAssemblyFailureKind = Literal["input", "internal", "decimal_overflow"]

type ProjectProvider = Provider
type SqlScalar = str | int | bool | float | Decimal | None
type SqlValuePayload = SqlScalar | tuple[object, ...]

class TransientLineOwner(Protocol):
    """A renderer that owns the transient line currently drawn in place on a terminal."""

    def clear_transient_line(self) -> None: ...
    def redraw_transient_line(self) -> None: ...

class DbtInvoker(Protocol):
    """Callable dbt process invocation contract."""

    def __call__(self, *, argv: tuple[str, ...], cwd: Path | None) -> object: ...

class FingerprintWriteProgress(Protocol):
    def __call__(self, *, completed: int, total: int) -> None: ...

type JsonValue = None | bool | int | float | str | list[JsonValue] | dict[str, JsonValue]

type ScopeQueryTarget = str | PurePath | ResourceIdentity | DeclarationIdentity

type NativeDiagnosticRow = tuple[str, str, int | None, int | None, int | None, int | None, str]

type NativeNormalizationRequest = tuple[str, dict[str, str], dict[str, str]]

class NativeCompactAnalysisJob(Protocol):
    def run(self) -> bytes: ...

class NativeProjectCatalog(Protocol):
    def inferred_schema(
        self, *, sql: str, columns: Mapping[str, str], inputs: Mapping[str, Mapping[str, str]]
    ) -> list[tuple[str, str]]: ...
    def with_relations(
        self, relations: Mapping[str, Mapping[str, str]]
    ) -> NativeProjectCatalog: ...
    def validation_payload(
        self, requests: list[tuple[str, list[tuple[str, bool]], dict[str, Mapping[str, str]]]]
    ) -> str: ...
    def update_analysis(
        self, relations: Mapping[str, tuple[Mapping[str, str], Mapping[str, str]]]
    ) -> None: ...
    def register_override(self, relations: Mapping[str, Mapping[str, str]]) -> int: ...
    def prepare_compact(self, payload: bytes) -> NativeCompactAnalysisJob: ...
    def update_relations(self, relations: Mapping[str, Mapping[str, str]]) -> None: ...
    def normalize_analysis_sqls(
        self, *, dialect: str, requests: list[NativeNormalizationRequest]
    ) -> list[str | Exception]: ...

class NativeBindingPositions(Protocol):
    def position(
        self, *, start: int | None, line: int | None, column: int | None, message: str
    ) -> tuple[int | None, int | None]: ...

class NativePositionsModule(Protocol):
    def normalize_analysis_sqls(
        self, *, dialect: str, requests: list[NativeNormalizationRequest]
    ) -> list[str | Exception]: ...
    def binding_diagnostics(
        self, *, sql: str, dialect: str, rows: list[NativeDiagnosticRow]
    ) -> list[NativeDiagnosticRow]: ...
    def BindingPositions(
        self,
        request: dict[str, object],
    ) -> NativeBindingPositions: ...
    def normalize_analysis_sql(self, request: dict[str, object]) -> str: ...
    def normalize_dialect_sql(self, *, sql: str, dialect: str) -> str: ...

type RelationKeys = dict[str, tuple[tuple[str, ...], ...]]

type NativePreparedSql = tuple[str, list[tuple[str, int, int, int, int, str]], list[str]]

type ObjectKeyRow = tuple[str, str]

type NamespaceRow = tuple[str | None, str | None, str | None, str | None]

class NativeProjectResources:
    @property
    def model_deps(self) -> tuple[list[tuple[str, str]], ...]: ...
    @property
    def model_syntax_valid(self) -> tuple[bool, ...]: ...
    @property
    def source_namespaces(self) -> tuple[tuple[str | None, str | None] | None, ...]: ...
    @property
    def seed_namespaces(
        self,
    ) -> tuple[tuple[str | None, str | None, str | None, str | None], ...]: ...
    @property
    def function_deps(self) -> tuple[list[tuple[str, str]], ...]: ...
    @property
    def audit_scope_deps(self) -> tuple[list[tuple[str, str]], ...]: ...
    @property
    def failure(self) -> NativeResourceFailure | None: ...

class NativeResourceFailure:
    @property
    def slot(self) -> str: ...
    @property
    def index(self) -> int: ...
    @property
    def error(self) -> Exception: ...

type FixtureKey = tuple[CompiledResourceType, str]
type InputColumns = Callable[[str, str], frozenset[str] | None]
type FixtureGroups = tuple[tuple[CompiledResourceType, dict[str, str]], ...]

class SeedInput(Protocol):
    """One seed paired with its declaration, as the native owner view holds it."""

    @property
    def seed_file(self) -> DiscoveredSeedFile: ...
    @property
    def schema_entry(self) -> SchemaSeedEntry: ...
    @property
    def schema_file(self) -> DiscoveredSchemaFile: ...

class FunctionInput(Protocol):
    """One attached function, as the native owner view holds it."""

    @property
    def function_file(self) -> DiscoveredSqlFunctionFile | DiscoveredPythonFunctionFile: ...
    @property
    def name(self) -> str: ...
    @property
    def arguments(self) -> tuple[FunctionArgument, ...]: ...
    @property
    def returns(self) -> str: ...
    @property
    def body_sql(self) -> str: ...
    @property
    def return_columns(self) -> tuple[FunctionReturnColumn, ...]: ...
    @property
    def references(self) -> tuple[CompileSqlReference, ...]: ...
    @property
    def language(self) -> FunctionLanguage: ...
    @property
    def runtime_version(self) -> str | None: ...
    @property
    def entry_point(self) -> str | None: ...
    @property
    def packages(self) -> tuple[str, ...]: ...
    @property
    def tags(self) -> tuple[str, ...]: ...
    @property
    def description(self) -> str | None: ...
    @property
    def database(self) -> str | None: ...
    @property
    def schema(self) -> str | None: ...
    @property
    def logical_database(self) -> str | None: ...
    @property
    def logical_schema(self) -> str | None: ...
    @property
    def fingerprint_database(self) -> str | None: ...
    @property
    def fingerprint_schema(self) -> str | None: ...
    @property
    def fingerprint_logical_database(self) -> str | None: ...
    @property
    def fingerprint_logical_schema(self) -> str | None: ...
    @property
    def declaration_usages(self) -> tuple[UsageRecord, ...]: ...
    @property
    def assembly_row(
        self,
    ) -> tuple[str, str, list[str], list[tuple[str, str, str | None]]]: ...

class CompileInputsView(Protocol):
    """A compile's attached inputs: a native owner view, or hand-built `CompileProjectInputs`."""

    @property
    def project_config(self) -> ProjectConfig: ...
    @property
    def local_config(self) -> LocalConfig: ...
    @property
    def discovered_inputs(self) -> DiscoveredProjectInputs: ...
    @property
    def sql_lexical_syntax(self) -> SqlLexicalSyntax: ...
    @property
    def run_id(self) -> str: ...
    @property
    def effective_target_name(self) -> str | None: ...
    @property
    def effective_target(self) -> TargetConfig | None: ...
    @property
    def compile_cache_dir(self) -> Path | None: ...
    @property
    def effective_connection(self) -> dict[str, object]: ...
    @property
    def effective_settings(self) -> SettingsConfig: ...
    @property
    def no_sql_validation(self) -> bool: ...
    @property
    def effective_vars(self) -> dict[str, object]: ...
    @property
    def macro_context(self) -> MacroContext | None: ...
    @property
    def loaded_macros(self) -> dict[str, LoadedMacro]: ...
    @property
    def public_enums(self) -> dict[str, EnumDeclaration]: ...
    @property
    def public_constants(self) -> dict[str, ConstantDeclaration]: ...
    @property
    def model_inputs(self) -> tuple[ModelInput, ...]: ...
    @property
    def seed_inputs(self) -> tuple[SeedInput, ...]: ...
    @property
    def source_inputs(self) -> tuple[SourceInput, ...]: ...
    @property
    def sql_function_inputs(self) -> tuple[FunctionInput, ...]: ...
    @property
    def test_inputs(self) -> tuple[CompileSqlTestInput, ...]: ...
    @property
    def scenario_inputs(self) -> tuple[CompileSqlScenarioInput, ...]: ...
    @property
    def audit_inputs(self) -> tuple[AuditInput, ...]: ...
    @property
    def diagnostics(self) -> tuple[CompilerDiagnostic, ...]: ...
    @property
    def external_sql_reference_resolver(self) -> ExternalSqlReferenceResolver | None: ...
    @property
    def scope_index(self) -> ScopeIndex: ...
    @property
    def declaration_scope(self) -> DeclarationScopeBuild | None: ...
    @property
    def native_project(self) -> NativeCompiledProject | None: ...

class ModelInput(Protocol):
    """One model's compile input: a native owner view, or a hand-built `CompileModelInput`."""

    @property
    def model_file(self) -> DiscoveredSqlModelFile: ...
    @property
    def config(self) -> CompileModelConfig: ...
    @property
    def query_sql(self) -> str: ...
    @property
    def macro_source_sql(self) -> str: ...
    @property
    def references(self) -> tuple[CompileSqlReference, ...]: ...
    @property
    def schema_entry(self) -> SchemaModelEntry | None: ...
    @property
    def schema_file(self) -> DiscoveredSchemaFile | None: ...
    @property
    def sql_validation_enabled(self) -> bool: ...
    @property
    def rejected_sql_analysis_opt_out(self) -> SourceLocation | None: ...
    @property
    def enum_declarations(self) -> tuple[EnumDeclaration, ...]: ...
    @property
    def constant_declarations(self) -> tuple[ConstantDeclaration, ...]: ...
    @property
    def enum_columns(self) -> dict[str, EnumDeclaration]: ...
    @property
    def macro_deps(self) -> tuple[str, ...]: ...
    @property
    def macro_usages(self) -> tuple[UsageRecord, ...]: ...
    @property
    def declaration_usages(self) -> tuple[UsageRecord, ...]: ...
    @property
    def sql_expansion(self) -> CompiledSqlExpansion | None: ...
    @property
    def lint_expansion(self) -> CompiledSqlExpansion | None: ...
    @property
    def reference_rows(self) -> list[tuple[str, str, str | None, int | None]]: ...
    @property
    def attached_schema_name(self) -> str | None: ...
    @property
    def relation_key_row(self) -> RelationKeyRow: ...

class CompileInputsReadyCallback(Protocol):
    """Invocation-local observer for the completed pre-semantic input snapshot."""

    def __call__(self, *, inputs: CompileInputsView) -> None: ...

type _GraphKey = tuple[str, str]
type _GraphEdges = list[tuple[_GraphKey, list[_GraphKey]]]
type _GraphIndexes = tuple[
    list[tuple[str, _GraphKey]],
    _GraphEdges,
    _GraphEdges,
    list[tuple[str, list[_GraphKey]]],
    list[tuple[_GraphKey, str]],
]
type _SelectorFailure = tuple[str, str, str | None]
type _SelectorKeys = tuple[list[_GraphKey] | None, _SelectorFailure | None]

class GraphResourceKind(StrEnum):
    MODEL = "model"
    SEED = "seed"
    SOURCE = "source"
    FUNCTION = "function"

class ChangeKind(StrEnum):
    FIRST_RUN = "first_run"
    RENAMED = "renamed"
    QUERY_CHANGED = "query_changed"
    FUNCTION_CHANGED = "function_changed"
    CONFIG_CHANGED = "config_changed"
    SCHEMA_CHANGED = "schema_changed"
    NO_CHANGE = "no_change"

class SchemaChangeKind(StrEnum):
    COLUMN_ADDED = "column_added"
    COLUMN_REMOVED = "column_removed"
    COLUMN_TYPE_CHANGED = "column_type_changed"

class SchemaColumnSource(StrEnum):
    YML = "yml"
    SQL_ANALYSIS = "sql_analysis"

class CursorWatermarkMode(StrEnum):
    ALL = "all"
    ANY = "any"

class SchemaActionKind(StrEnum):
    ADD_COLUMN = "add_column"
    DROP_COLUMN = "drop_column"
    ALTER_COLUMN_TYPE = "alter_column_type"

class WarningSeverity(StrEnum):
    ERROR = "error"
    WARNING = "warning"
    INFO = "info"

class FixtureColumnInferences:
    """Reuse inference for identical fixture SQL under an equal, catalog-free profile."""

    def __init__(self) -> None: ...
    def infer(
        self, *, query_sql: str, inference_profile: ExpressionInferenceProfile
    ) -> FixtureColumnInference | None:
        """Return the inference for one fixture query, computing each distinct query once."""

class NativeSqlTestArtifact(NativeRecord):
    """One native-planned SQL-test artifact and its distinct planning error messages."""

    sql: str
    model_names: tuple[str, ...]
    error_messages: tuple[str, ...]
    def __init__(
        self, sql: str, model_names: tuple[str, ...], error_messages: tuple[str, ...] = ...
    ) -> None: ...
    def __replace__(self, **changes: Any) -> NativeSqlTestArtifact: ...

class RelationFixtureDiagnostic(NativeRecord):
    """One statically provable invalid fixture condition."""

    key: FixtureKey
    message: str
    def __init__(self, key: FixtureKey, message: str) -> None: ...
    def __replace__(self, **changes: Any) -> RelationFixtureDiagnostic: ...

class RelationFixtureCompletion(NativeRecord):
    """Completed fixture SQL plus metadata reused by SQL-test validation."""

    fixture_sql_by_key: dict[FixtureKey, str]
    inferred_by_fixture: dict[FixtureKey, tuple[InferredColumn, ...]]
    expected_types: Mapping[FixtureKey, dict[str, str]]
    diagnostics: tuple[RelationFixtureDiagnostic, ...]
    def __init__(
        self,
        fixture_sql_by_key: dict[FixtureKey, str],
        inferred_by_fixture: dict[FixtureKey, tuple[InferredColumn, ...]],
        expected_types: Mapping[FixtureKey, dict[str, str]],
        diagnostics: tuple[RelationFixtureDiagnostic, ...],
    ) -> None: ...
    def __replace__(self, **changes: Any) -> RelationFixtureCompletion: ...

class RelationFixturePlanningContext:
    """Project-wide fixture metadata shared across test and scenario planning."""

    def __init__(
        self,
        *,
        models_by_name: dict[str, CompiledModelView],
        relations: dict[
            tuple[CompiledResourceType, str],
            tuple[tuple[tuple[str, str | None, bool | None], ...], bool],
        ],
    ) -> None: ...
    def authoritative_names(self, key: tuple[CompiledResourceType, str]) -> bool | None: ...
    def expected_types(self, key: tuple[CompiledResourceType, str]) -> dict[str, str] | None: ...
    def authoritative_columns(
        self, key: tuple[CompiledResourceType, str]
    ) -> frozenset[str] | None: ...

class SelectorExpansion(NativeRecord):
    """One selector split into graph expansion flags and core text."""

    core: str
    upstream: bool
    downstream: bool
    def __init__(self, core: str, upstream: bool = ..., downstream: bool = ...) -> None: ...
    def __replace__(self, **changes: Any) -> SelectorExpansion: ...

class ParsedScenarioArtifactName(NativeRecord):
    """Parsed physical name for one planner-owned scenario artifact."""

    hash_prefix: str
    kind: str
    logical_name: str
    def __init__(self, hash_prefix: str, kind: str, logical_name: str) -> None: ...
    def __replace__(self, **changes: Any) -> ParsedScenarioArtifactName: ...

class MissingUpstream(NativeRecord):
    """One upstream dependency missing from both scope and warehouse."""

    key: CompiledObjectKey
    required_by: tuple[CompiledObjectKey, ...]
    def __init__(
        self, key: CompiledObjectKey, required_by: tuple[CompiledObjectKey, ...]
    ) -> None: ...
    def __replace__(self, **changes: Any) -> MissingUpstream: ...

class WarehouseFingerprints(NativeRecord):
    """Latest direct fingerprints grouped by node type."""

    models: dict[str, Fingerprint]
    functions: dict[str, Fingerprint]
    seeds: dict[str, Fingerprint]
    python_nodes: dict[tuple[str, str], Fingerprint]
    unfiltered_schemas: dict[str, tuple[Fingerprint, ...]]
    unfiltered_database: str | None
    def __init__(
        self,
        models: dict[str, Fingerprint] = ...,
        functions: dict[str, Fingerprint] = ...,
        seeds: dict[str, Fingerprint] = ...,
        python_nodes: dict[tuple[str, str], Fingerprint] = ...,
        unfiltered_schemas: dict[str, tuple[Fingerprint, ...]] = ...,
        unfiltered_database: str | None = ...,
    ) -> None: ...
    def __replace__(self, **changes: Any) -> WarehouseFingerprints: ...

class CursorSnapshotScope(NativeRecord):
    """Execution selection used for cursor validation and runtime producer resolution."""

    model_keys: frozenset[CompiledObjectKey]
    runtime_producer_keys: frozenset[CompiledObjectKey]
    invocation_time: datetime | None
    start_cursor_config: StartCursorsConfig | None
    cursor_overrides: CursorOverrides | None
    def __init__(
        self,
        model_keys: frozenset[CompiledObjectKey],
        runtime_producer_keys: frozenset[CompiledObjectKey],
        invocation_time: datetime | None = ...,
        start_cursor_config: StartCursorsConfig | None = ...,
        cursor_overrides: CursorOverrides | None = ...,
    ) -> None: ...
    def __replace__(self, **changes: Any) -> CursorSnapshotScope: ...

class ReferenceRename(NativeRecord):
    """One model rename that downstream references may have been rewritten for."""

    new_name: str
    origin_name: str
    recorded_at: datetime | None
    def __init__(
        self, new_name: str, origin_name: str, recorded_at: datetime | None = ...
    ) -> None: ...
    def __replace__(self, **changes: Any) -> ReferenceRename: ...

class WarehouseSnapshot(NativeRecord):
    """Frozen point-in-time picture of warehouse state for planning."""

    existing_relations: dict[str, RelationInfo]
    existing_columns: dict[str, tuple[ColumnInfo, ...]]
    fingerprints: WarehouseFingerprints
    cursor_snapshots: dict[str, ModelCursorSnapshot]
    source_freshness_state_schemas: frozenset[str]
    column_dialect: str | None
    renamed_models: frozenset[str]
    reference_renames: tuple[ReferenceRename, ...]
    migration_state_schemas: frozenset[str]
    old_name_view_state_schemas: frozenset[str]
    listed_state_schemas: frozenset[str] | None
    def __init__(
        self,
        existing_relations: dict[str, RelationInfo] = ...,
        existing_columns: dict[str, tuple[ColumnInfo, ...]] = ...,
        fingerprints: WarehouseFingerprints = ...,
        cursor_snapshots: dict[str, ModelCursorSnapshot] = ...,
        source_freshness_state_schemas: frozenset[str] = ...,
        column_dialect: str | None = ...,
        renamed_models: frozenset[str] = ...,
        reference_renames: tuple[ReferenceRename, ...] = ...,
        migration_state_schemas: frozenset[str] = ...,
        old_name_view_state_schemas: frozenset[str] = ...,
        listed_state_schemas: frozenset[str] | None = ...,
    ) -> None: ...
    def __replace__(self, **changes: Any) -> WarehouseSnapshot: ...

class SelectionStalenessNodeKey(NativeRecord):
    """Neutral node identity for shared selection-aware staleness classification."""

    resource_type: str
    name: str
    def __init__(self, resource_type: str, name: str) -> None: ...
    def __replace__(self, **changes: Any) -> SelectionStalenessNodeKey: ...

class SelectionStalenessGraph(NativeRecord):
    """Neutral graph inputs for selected-model stale warning classification."""

    upstream_deps: dict[SelectionStalenessNodeKey, tuple[SelectionStalenessNodeKey, ...]]
    selected_model_names: frozenset[str]
    run_model_names: frozenset[str]
    run_seed_names: frozenset[str]
    run_source_names: frozenset[str]
    changed_model_names: frozenset[str]
    changed_seed_names: frozenset[str]
    changed_source_names: frozenset[str]
    def __init__(
        self,
        upstream_deps: dict[SelectionStalenessNodeKey, tuple[SelectionStalenessNodeKey, ...]],
        selected_model_names: frozenset[str],
        run_model_names: frozenset[str],
        run_seed_names: frozenset[str],
        run_source_names: frozenset[str],
        changed_model_names: frozenset[str],
        changed_seed_names: frozenset[str],
        changed_source_names: frozenset[str],
    ) -> None: ...
    def __replace__(self, **changes: Any) -> SelectionStalenessGraph: ...

class SelectionStalenessWarning(NativeRecord):
    """Neutral stale warning classification for one selected model."""

    model_name: str
    trigger_names: tuple[str, ...]
    def __init__(self, model_name: str, trigger_names: tuple[str, ...]) -> None: ...
    def __replace__(self, **changes: Any) -> SelectionStalenessWarning: ...

class BackfillResult(NativeRecord):
    """Resolved backfill action from a change detection policy."""

    action: BackfillAction
    duration: str | None
    def __init__(self, action: BackfillAction, duration: str | None = ...) -> None: ...
    def __replace__(self, **changes: Any) -> BackfillResult: ...

class ChangeDetectionResult(NativeRecord):
    """Per-model output from change detection and policy resolution."""

    model_name: str
    change_kind: ChangeKind
    query_changed: bool
    config_changed: bool
    fingerprint_metadata_json: str | None
    previous_metadata_json: str | None
    fingerprint_version_hash: str | None
    previous_version_hash: str | None
    schema_findings: tuple[SchemaFinding, ...]
    backfill: BackfillResult
    recorded_build_relation_missing: bool
    changed_functions: tuple[str, ...]
    def __init__(
        self,
        model_name: str,
        change_kind: ChangeKind,
        query_changed: bool = ...,
        config_changed: bool = ...,
        fingerprint_metadata_json: str | None = ...,
        previous_metadata_json: str | None = ...,
        fingerprint_version_hash: str | None = ...,
        previous_version_hash: str | None = ...,
        schema_findings: tuple[SchemaFinding, ...] = ...,
        backfill: BackfillResult = ...,
        recorded_build_relation_missing: bool = ...,
        changed_functions: tuple[str, ...] = ...,
    ) -> None: ...
    def __replace__(self, **changes: Any) -> ChangeDetectionResult: ...

class UnitTestSelectorSplit(NativeRecord):
    """Resource selectors left after unit-test selectors moved into a test selection."""

    select: tuple[str, ...]
    exclude: tuple[str, ...]
    sql_test_selection: SqlTestSelection
    def __init__(
        self,
        select: tuple[str, ...],
        exclude: tuple[str, ...],
        sql_test_selection: SqlTestSelection,
    ) -> None: ...
    def __replace__(self, **changes: Any) -> UnitTestSelectorSplit: ...

class ModelPlanContext:
    """Relation lookups and source columns for building one model plan entry."""

    model_locations: dict[str, CompiledRelationLocation]
    models_by_name: dict[str, CompiledModelView]
    seed_locations: dict[str, CompiledRelationLocation]
    function_locations: dict[str, CompiledRelationLocation]
    source_map: dict[str, SourceEntry]
    source_warehouse_columns: dict[str, tuple[ColumnInfo, ...]]
    star_exclude_keyword: str
    functions_by_name: dict[str, CompiledFunctionView]
    runtime_cursor_producer_names: frozenset[str]
    future_cursor_config: FutureCursorsConfig | None
    start_cursor_config: StartCursorsConfig | None
    invocation_time: datetime | None
    def __init__(
        self,
        *,
        model_locations: dict[str, CompiledRelationLocation],
        models_by_name: dict[str, CompiledModelView],
        seed_locations: dict[str, CompiledRelationLocation],
        function_locations: dict[str, CompiledRelationLocation],
        source_map: dict[str, SourceEntry],
        source_warehouse_columns: dict[str, tuple[ColumnInfo, ...]],
        star_exclude_keyword: str,
        functions_by_name: dict[str, CompiledFunctionView] = ...,
        runtime_cursor_producer_names: frozenset[str] = ...,
        future_cursor_config: FutureCursorsConfig | None = ...,
        start_cursor_config: StartCursorsConfig | None = ...,
        invocation_time: datetime | None = ...,
    ) -> None: ...

class CursorOverridePair(NativeRecord):
    """Resolved per-model cursor start/end overrides for plan entry construction."""

    start_cursor_override: str | None
    end_cursor_override: str | None
    def __init__(
        self, start_cursor_override: str | None = ..., end_cursor_override: str | None = ...
    ) -> None: ...
    def __replace__(self, **changes: Any) -> CursorOverridePair: ...

class PlanEntryBuildInputs:
    """Blocked models and cursor overrides for plan entry building."""

    source_freshness_blocked_model_names: frozenset[str]
    external_blocked_model_names: frozenset[str]
    start_cursor_override: str | None
    end_cursor_override: str | None
    future_cursor_config: FutureCursorsConfig | None
    start_cursor_config: StartCursorsConfig | None
    invocation_time: datetime | None
    max_microbatches: int | None
    max_microbatches_is_override: bool
    microbatch_limit_action: MicrobatchLimitAction
    def __init__(
        self,
        *,
        source_freshness_blocked_model_names: frozenset[str] = ...,
        external_blocked_model_names: frozenset[str] = ...,
        start_cursor_override: str | None = ...,
        end_cursor_override: str | None = ...,
        future_cursor_config: FutureCursorsConfig | None = ...,
        start_cursor_config: StartCursorsConfig | None = ...,
        invocation_time: datetime | None = ...,
        max_microbatches: int | None = ...,
        max_microbatches_is_override: bool = ...,
        microbatch_limit_action: MicrobatchLimitAction = ...,
    ) -> None: ...

class FunctionChangeResult(NativeRecord):
    """Per-function output from change detection."""

    fingerprint_sql: str
    reason: PlanReason
    def __init__(self, fingerprint_sql: str, reason: PlanReason = ...) -> None: ...
    def __replace__(self, **changes: Any) -> FunctionChangeResult: ...

class PlannerChangeResults(NativeRecord):
    """Change detection output for selected planner resources."""

    models: dict[str, ChangeDetectionResult]
    functions: dict[str, FunctionChangeResult]
    def __init__(
        self, models: dict[str, ChangeDetectionResult], functions: dict[str, FunctionChangeResult]
    ) -> None: ...
    def __replace__(self, **changes: Any) -> PlannerChangeResults: ...

class ResolvedModelAction(NativeRecord):
    """Effective model change and backfill decided from the model's own change."""

    change: ChangeDetectionResult
    backfill: BackfillResult
    def __init__(self, change: ChangeDetectionResult, backfill: BackfillResult) -> None: ...
    def __replace__(self, **changes: Any) -> ResolvedModelAction: ...

class PlannerResolvedActions(NativeRecord):
    """Per-model planning decisions keyed by model name."""

    models: dict[str, ResolvedModelAction]
    def __init__(self, models: dict[str, ResolvedModelAction]) -> None: ...
    def __replace__(self, **changes: Any) -> PlannerResolvedActions: ...

class DirectModelVersionIdentities(NativeRecord):
    """Current direct model version identity values by model name."""

    function_local_hashes: dict[str, str]
    seed_version_hashes: dict[str, str]
    seed_metadata_jsons: dict[str, str]
    model_metadata_jsons: dict[str, str]
    model_local_hashes: dict[str, str]
    model_version_hashes: dict[str, str]
    def __init__(
        self,
        function_local_hashes: dict[str, str],
        seed_version_hashes: dict[str, str],
        seed_metadata_jsons: dict[str, str],
        model_metadata_jsons: dict[str, str],
        model_local_hashes: dict[str, str],
        model_version_hashes: dict[str, str],
    ) -> None: ...
    def __replace__(self, **changes: Any) -> DirectModelVersionIdentities: ...

class SchemaAction(NativeRecord):
    """One concrete schema change action to apply to a target table."""

    kind: SchemaActionKind
    column_name: str
    column_type: str | None
    def __init__(
        self, kind: SchemaActionKind, column_name: str, column_type: str | None = ...
    ) -> None: ...
    def __replace__(self, **changes: Any) -> SchemaAction: ...

class PlanWarning(NativeRecord):
    """One warning or error produced during plan resolution."""

    model_name: str | None
    severity: WarningSeverity
    message: str
    code: str | None
    def __init__(
        self,
        model_name: str | None,
        severity: WarningSeverity,
        message: str,
        code: str | None = ...,
    ) -> None: ...
    def __replace__(self, **changes: Any) -> PlanWarning: ...

class ColumnRenameHint(NativeRecord):
    """A removed column that an added column may have been renamed from."""

    model_name: str
    added_column: str
    candidate_columns: tuple[str, ...]
    identical: bool
    def __init__(
        self,
        model_name: str,
        added_column: str,
        candidate_columns: tuple[str, ...],
        identical: bool = ...,
    ) -> None: ...
    def __replace__(self, **changes: Any) -> ColumnRenameHint: ...
    @property
    def message(self) -> str:
        """The one-line hint shown beside the added column."""

class QueryProjection(NativeRecord):
    """One named top-level output column of a query and its canonical expression."""

    name: str
    expression: str
    references: frozenset[str]
    passthrough: bool
    def __init__(
        self, name: str, expression: str, references: frozenset[str], passthrough: bool = ...
    ) -> None: ...
    def __replace__(self, **changes: Any) -> QueryProjection: ...
    @property
    def key(self) -> str:
        """The case-insensitive output name."""

class QueryShape(NativeRecord):
    """Canonical top-level projections and the rest of one query, ignoring formatting."""

    projections: tuple[QueryProjection, ...]
    body: str
    alias_clauses: str
    positional: bool
    input_relations: tuple[tuple[str, str], ...] | None
    local_input_columns: frozenset[str]
    clause_references: frozenset[str]
    def __init__(
        self,
        projections: tuple[QueryProjection, ...],
        body: str,
        alias_clauses: str = ...,
        positional: bool = ...,
        input_relations: tuple[tuple[str, str], ...] | None = ...,
        local_input_columns: frozenset[str] = ...,
        clause_references: frozenset[str] = ...,
    ) -> None: ...
    def __replace__(self, **changes: Any) -> QueryShape: ...

class PlannedSqlTests(NativeRecord):
    """SQL test entries planned for one selection; reusable only for the same selection."""

    selected_keys: frozenset[CompiledObjectKey]
    sql_test_selection: SqlTestSelection
    entries: tuple[SqlTestPlanEntry, ...]
    warnings: tuple[PlanWarning, ...]
    def __init__(
        self,
        selected_keys: frozenset[CompiledObjectKey],
        sql_test_selection: SqlTestSelection = ...,
        entries: tuple[SqlTestPlanEntry, ...] = ...,
        warnings: tuple[PlanWarning, ...] = ...,
    ) -> None: ...
    def __replace__(self, **changes: Any) -> PlannedSqlTests: ...

class NativeSqlTestPlan(NativeRecord):
    """Native chain and assertion steps for one SQL test, with SQL when rendered."""

    chain: tuple[ChainStep, ...]
    assertions: tuple[SqlTestAssertionStep, ...]
    model_names: tuple[str, ...]
    warnings: tuple[PlanWarning, ...]
    error_messages: tuple[str, ...]
    sql: str | None
    def __init__(
        self,
        chain: tuple[ChainStep, ...],
        assertions: tuple[SqlTestAssertionStep, ...],
        model_names: tuple[str, ...],
        warnings: tuple[PlanWarning, ...],
        error_messages: tuple[str, ...],
        sql: str | None = ...,
    ) -> None: ...
    def __replace__(self, **changes: Any) -> NativeSqlTestPlan: ...

class SqlTestPlanResult(NativeRecord):
    """One SQL test's plan entry and warnings, or the fixture diagnostics preventing it."""

    entry: SqlTestPlanEntry | None
    warnings: tuple[PlanWarning, ...]
    fixture_diagnostics: tuple[str, ...]
    def __init__(
        self,
        entry: SqlTestPlanEntry | None,
        warnings: tuple[PlanWarning, ...] = ...,
        fixture_diagnostics: tuple[str, ...] = ...,
    ) -> None: ...
    def __replace__(self, **changes: Any) -> SqlTestPlanResult: ...

class ScenarioGraphPlan(NativeRecord):
    """Inferred graph slice and fixture boundaries for one SQL scenario."""

    key: CompiledObjectKey
    name: str
    target_model_names: tuple[str, ...]
    assertion_target_model_names: tuple[str, ...]
    model_names: tuple[str, ...]
    source_fixture_names: tuple[str, ...]
    ref_fixture_names: tuple[str, ...]
    seed_names: tuple[str, ...]
    seed_fixture_names: tuple[str, ...]
    dbt_ref_fixture_names: tuple[str, ...]
    function_deps: tuple[CompiledObjectKey, ...]
    def __init__(
        self,
        key: CompiledObjectKey,
        name: str,
        target_model_names: tuple[str, ...] = ...,
        assertion_target_model_names: tuple[str, ...] = ...,
        model_names: tuple[str, ...] = ...,
        source_fixture_names: tuple[str, ...] = ...,
        ref_fixture_names: tuple[str, ...] = ...,
        seed_names: tuple[str, ...] = ...,
        seed_fixture_names: tuple[str, ...] = ...,
        dbt_ref_fixture_names: tuple[str, ...] = ...,
        function_deps: tuple[CompiledObjectKey, ...] = ...,
    ) -> None: ...
    def __replace__(self, **changes: Any) -> ScenarioGraphPlan: ...

class ScenarioArtifactIdentity(NativeRecord):
    """Logical identity for one scenario-owned physical artifact."""

    kind: ScenarioArtifactKind | str
    logical_name: str
    def __init__(self, kind: ScenarioArtifactKind | str, logical_name: str) -> None: ...
    def __replace__(self, **changes: Any) -> ScenarioArtifactIdentity: ...

class ScenarioArtifactName(NativeRecord):
    """Resolved physical relation name for one scenario artifact."""

    identity: ScenarioArtifactIdentity
    physical_name: str
    def __init__(self, identity: ScenarioArtifactIdentity, physical_name: str) -> None: ...
    def __replace__(self, **changes: Any) -> ScenarioArtifactName: ...

class PlanProviderUsage(NativeRecord):
    """Provider usage metadata scoped to planned work."""

    provider_name: str
    consumer_kind: str
    consumer_name: str
    parameter_name: str
    annotation_class_name: str | None
    annotation_module: str | None
    def __init__(
        self,
        provider_name: str,
        consumer_kind: str,
        consumer_name: str,
        parameter_name: str,
        annotation_class_name: str | None = ...,
        annotation_module: str | None = ...,
    ) -> None: ...
    def __replace__(self, **changes: Any) -> PlanProviderUsage: ...

class PlannerRuntime:
    """Resolved execution environment shared by planner phases."""

    project: CompiledProjectView
    adapter: BaseAdapter
    connection: Any
    project_config: ProjectConfig | None
    local_config: LocalConfig | None
    on_progress: Callable[[str], None] | None
    invocation_time: datetime
    def __init__(
        self,
        *,
        project: CompiledProjectView,
        adapter: BaseAdapter,
        connection: Any,
        project_config: ProjectConfig | None = ...,
        local_config: LocalConfig | None = ...,
        on_progress: Callable[[str], None] | None = ...,
        invocation_time: datetime = ...,
    ) -> None: ...

class PlannerScopeResolution:
    """Resolved planner scopes for one plan build."""

    selected_scope: PlannerScope
    stale_warning_scope: PlannerScope
    inspection_scope: PlannerScope
    def __init__(
        self,
        *,
        selected_scope: PlannerScope,
        stale_warning_scope: PlannerScope,
        inspection_scope: PlannerScope,
    ) -> None: ...

class PlannerWarehouseState:
    """Warehouse snapshot and inspection relations gathered once per plan."""

    snapshot: WarehouseSnapshot
    inspection_relations: PlannerRelationsContext
    migration_entries: tuple[ModelMigrationPlanEntry, ...]
    migration_warnings: tuple[PlanWarning, ...]
    column_migration_entries: tuple[ColumnMigrationPlanEntry, ...]
    column_rename_hints: tuple[ColumnRenameHint, ...]
    old_name_view_entries: tuple[OldNameViewPlanEntry, ...]
    old_name_views: tuple[OldNameView, ...]
    migration_fingerprints: MigrationFingerprintCache
    def __init__(
        self,
        *,
        snapshot: WarehouseSnapshot,
        inspection_relations: PlannerRelationsContext,
        migration_entries: tuple[ModelMigrationPlanEntry, ...] = ...,
        migration_warnings: tuple[PlanWarning, ...] = ...,
        column_migration_entries: tuple[ColumnMigrationPlanEntry, ...] = ...,
        column_rename_hints: tuple[ColumnRenameHint, ...] = ...,
        old_name_view_entries: tuple[OldNameViewPlanEntry, ...] = ...,
        old_name_views: tuple[OldNameView, ...] = ...,
        migration_fingerprints: MigrationFingerprintCache = ...,
    ) -> None: ...

class PlannerIdentityContext:
    """Expected version identities for inspection and stale-warning scopes."""

    version_identities: DirectModelVersionIdentities
    stale_warning_identities: DirectModelVersionIdentities
    def __init__(
        self,
        *,
        version_identities: DirectModelVersionIdentities,
        stale_warning_identities: DirectModelVersionIdentities,
    ) -> None: ...

class NativeModelInput:
    @property
    def reference_rows(self) -> list[tuple[str, str, str | None, int | None]]: ...
    @property
    def attached_schema_name(self) -> str | None: ...
    @property
    def relation_key_row(self) -> RelationKeyRow: ...
    @property
    def model_file(self) -> DiscoveredSqlModelFile: ...
    @property
    def config(self) -> CompileModelConfig: ...
    @property
    def query_sql(self) -> str: ...
    @property
    def macro_source_sql(self) -> str: ...
    @property
    def references(self) -> tuple[CompileSqlReference, ...]: ...
    @property
    def schema_entry(self) -> SchemaModelEntry | None: ...
    @property
    def schema_file(self) -> DiscoveredSchemaFile | None: ...
    @property
    def sql_validation_enabled(self) -> bool: ...
    @property
    def rejected_sql_analysis_opt_out(self) -> SourceLocation | None: ...
    @property
    def enum_declarations(self) -> tuple[EnumDeclaration, ...]: ...
    @property
    def constant_declarations(self) -> tuple[ConstantDeclaration, ...]: ...
    @property
    def enum_columns(self) -> dict[str, EnumDeclaration]: ...
    @property
    def macro_deps(self) -> tuple[str, ...]: ...
    @property
    def macro_usages(self) -> tuple[UsageRecord, ...]: ...
    @property
    def declaration_usages(self) -> tuple[UsageRecord, ...]: ...
    @property
    def sql_expansion(self) -> CompiledSqlExpansion | None: ...
    @property
    def lint_expansion(self) -> CompiledSqlExpansion | None: ...
    def compiled_model(
        self,
        arguments: tuple[
            list[tuple[str, str]],
            tuple[NativeModelAnalysisSession, int] | None,
            list[tuple[str, str | None, str]] | None,
            tuple[
                bool,
                list[tuple[str, str | None, str]],
                list[tuple[str, str | None]],
                list[str],
                str | None,
                bool,
            ]
            | None,
            list[str],
        ],
    ) -> NativeCompiledModel: ...

class NativeCompiledModel:
    @property
    def reference_rows(self) -> list[tuple[str, str, str | None, int | None]]: ...
    @property
    def relation_key_row(self) -> RelationKeyRow: ...
    @property
    def key_row(self) -> tuple[str, str]: ...
    @property
    def dep_rows(self) -> list[tuple[str, str]]: ...
    @property
    def destination_row(self) -> tuple[str | None, str | None, str]: ...
    @property
    def inferred_rows(self) -> list[tuple[str, str | None, str]] | None: ...
    @property
    def schema_column_rows(self) -> list[tuple[str, str | None]] | None: ...
    @property
    def has_dynamic_columns(self) -> bool: ...
    @property
    def output_column_count(self) -> int: ...
    @property
    def key(self) -> CompiledObjectKey: ...
    @property
    def deps(self) -> tuple[CompiledObjectKey, ...]: ...
    @property
    def name(self) -> str: ...
    @property
    def relative_path(self) -> Path: ...
    @property
    def query_sql(self) -> str: ...
    @property
    def config(self) -> CompileModelConfig: ...
    @property
    def destination(self) -> CompiledRelationLocation: ...
    @property
    def references(self) -> tuple[CompileSqlReference, ...]: ...
    @property
    def schema_entry(self) -> SchemaModelEntry | None: ...
    @property
    def inferred_columns(self) -> tuple[InferredColumn, ...] | None: ...
    @property
    def fast_lineage_columns(self) -> Sequence[CompiledLineageColumnFact] | None: ...
    @property
    def fast_lineage_has_star(self) -> bool: ...
    @property
    def fast_lineage_star_resolved(self) -> bool: ...
    @property
    def authored_sql(self) -> str: ...
    @property
    def authored_query_sql(self) -> str: ...
    @property
    def output_column_locations(self) -> dict[str, SourceLocation]: ...
    @property
    def extract_implicit_alias_columns(self) -> bool: ...
    @property
    def macro_deps(self) -> tuple[str, ...]: ...
    @property
    def enum_declarations(self) -> tuple[EnumDeclaration, ...]: ...
    @property
    def constant_declarations(self) -> tuple[ConstantDeclaration, ...]: ...
    @property
    def enum_columns(self) -> dict[str, EnumDeclaration]: ...
    @property
    def binding_diagnostics(self) -> tuple[CompilerDiagnostic, ...]: ...
    @property
    def binding_validated(self) -> bool: ...
    @property
    def dynamic_column_contract(self) -> DynamicColumnContractProof | None: ...
    @property
    def unchecked_output_columns(self) -> frozenset[str]: ...
    @property
    def rejected_sql_analysis_opt_out(self) -> SourceLocation | None: ...
    def with_binding_diagnostics(
        self, diagnostics: tuple[CompilerDiagnostic, ...]
    ) -> NativeCompiledModel: ...
    def with_type_recovery(
        self, diagnostics: tuple[CompilerDiagnostic, ...], poisoned: list[str]
    ) -> NativeCompiledModel: ...
    def with_target_defaults(
        self, defaults: tuple[str | None, str | None], render: Callable[..., str | None]
    ) -> NativeCompiledModel: ...

class NativeInputsView:
    @property
    def project_config(self) -> ProjectConfig: ...
    @property
    def local_config(self) -> LocalConfig: ...
    @property
    def discovered_inputs(self) -> DiscoveredProjectInputs: ...
    @property
    def sql_lexical_syntax(self) -> SqlLexicalSyntax: ...
    @property
    def run_id(self) -> str: ...
    @property
    def effective_target_name(self) -> str | None: ...
    @property
    def effective_target(self) -> TargetConfig | None: ...
    @property
    def compile_cache_dir(self) -> Path | None: ...
    @property
    def effective_connection(self) -> dict[str, object]: ...
    @property
    def effective_settings(self) -> SettingsConfig: ...
    @property
    def no_sql_validation(self) -> bool: ...
    @property
    def effective_vars(self) -> dict[str, object]: ...
    @property
    def macro_context(self) -> MacroContext | None: ...
    @property
    def loaded_macros(self) -> dict[str, LoadedMacro]: ...
    @property
    def public_enums(self) -> dict[str, EnumDeclaration]: ...
    @property
    def public_constants(self) -> dict[str, ConstantDeclaration]: ...
    @property
    def model_inputs(self) -> tuple[NativeModelInput, ...]: ...
    @property
    def seed_inputs(self) -> tuple[SeedInput, ...]: ...
    @property
    def source_inputs(self) -> tuple[SourceInput, ...]: ...
    @property
    def sql_function_inputs(self) -> tuple[FunctionInput, ...]: ...
    @property
    def test_inputs(self) -> tuple[CompileSqlTestInput, ...]: ...
    @property
    def scenario_inputs(self) -> tuple[CompileSqlScenarioInput, ...]: ...
    @property
    def audit_inputs(self) -> tuple[AuditInput, ...]: ...
    @property
    def diagnostics(self) -> tuple[CompilerDiagnostic, ...]: ...
    @property
    def external_sql_reference_resolver(self) -> ExternalSqlReferenceResolver | None: ...
    @property
    def scope_index(self) -> ScopeIndex: ...
    @property
    def declaration_scope(self) -> DeclarationScopeBuild | None: ...
    @property
    def native_project(self) -> NativeCompiledProject | None: ...
    def with_changes(self, **changes: object) -> NativeInputsView: ...
    def fields(self) -> dict[str, Any]: ...

class NativeSeedInput:
    @property
    def seed_file(self) -> DiscoveredSeedFile: ...
    @property
    def schema_entry(self) -> SchemaSeedEntry: ...
    @property
    def schema_file(self) -> DiscoveredSchemaFile: ...

class NativeCompiledSeed:
    @property
    def key(self) -> CompiledObjectKey: ...
    @property
    def deps(self) -> tuple[CompiledObjectKey, ...]: ...
    @property
    def name(self) -> str: ...
    @property
    def seed_file(self) -> DiscoveredSeedFile: ...
    @property
    def schema_entry(self) -> SchemaSeedEntry: ...
    @property
    def schema_file(self) -> DiscoveredSchemaFile: ...
    @property
    def destination(self) -> CompiledRelationLocation: ...
    @property
    def external(self) -> bool: ...
    def with_target_defaults(
        self,
        defaults: tuple[str | None, str | None],
        render: Callable[..., str | None],
    ) -> NativeCompiledSeed: ...

class NativeSourceInput:
    @property
    def source_entry(self) -> SourceEntry: ...
    @property
    def source_file(self) -> DiscoveredSourceFile: ...
    @property
    def declaration_usages(self) -> tuple[UsageRecord, ...]: ...
    @property
    def assembly_row(self) -> tuple[str, bool, str | None, str | None]: ...

class NativeCompiledSource:
    @property
    def key(self) -> CompiledObjectKey: ...
    @property
    def deps(self) -> tuple[CompiledObjectKey, ...]: ...
    @property
    def name(self) -> str: ...
    @property
    def source_entry(self) -> SourceEntry: ...
    @property
    def source_file(self) -> DiscoveredSourceFile: ...

class NativeFunctionInput:
    @property
    def function_file(self) -> DiscoveredSqlFunctionFile | DiscoveredPythonFunctionFile: ...
    @property
    def name(self) -> str: ...
    @property
    def arguments(self) -> tuple[FunctionArgument, ...]: ...
    @property
    def returns(self) -> str: ...
    @property
    def body_sql(self) -> str: ...
    @property
    def return_columns(self) -> tuple[FunctionReturnColumn, ...]: ...
    @property
    def references(self) -> tuple[CompileSqlReference, ...]: ...
    @property
    def language(self) -> FunctionLanguage: ...
    @property
    def runtime_version(self) -> str | None: ...
    @property
    def entry_point(self) -> str | None: ...
    @property
    def packages(self) -> tuple[str, ...]: ...
    @property
    def tags(self) -> tuple[str, ...]: ...
    @property
    def description(self) -> str | None: ...
    @property
    def database(self) -> str | None: ...
    @property
    def schema(self) -> str | None: ...
    @property
    def logical_database(self) -> str | None: ...
    @property
    def logical_schema(self) -> str | None: ...
    @property
    def fingerprint_database(self) -> str | None: ...
    @property
    def fingerprint_schema(self) -> str | None: ...
    @property
    def fingerprint_logical_database(self) -> str | None: ...
    @property
    def fingerprint_logical_schema(self) -> str | None: ...
    @property
    def declaration_usages(self) -> tuple[UsageRecord, ...]: ...
    @property
    def assembly_row(
        self,
    ) -> tuple[str, str, list[str], list[tuple[str, str, str | None]]]: ...

class NativeCompiledFunction:
    @property
    def key(self) -> CompiledObjectKey: ...
    @property
    def deps(self) -> tuple[CompiledObjectKey, ...]: ...
    @property
    def name(self) -> str: ...
    @property
    def arguments(self) -> tuple[FunctionArgument, ...]: ...
    @property
    def returns(self) -> str: ...
    @property
    def body_sql(self) -> str: ...
    @property
    def return_columns(self) -> tuple[FunctionReturnColumn, ...]: ...
    @property
    def references(self) -> tuple[CompileSqlReference, ...]: ...
    @property
    def language(self) -> FunctionLanguage: ...
    @property
    def runtime_version(self) -> str | None: ...
    @property
    def entry_point(self) -> str | None: ...
    @property
    def packages(self) -> tuple[str, ...]: ...
    @property
    def tags(self) -> tuple[str, ...]: ...
    @property
    def description(self) -> str | None: ...
    @property
    def relative_path(self) -> Path: ...
    @property
    def source_file_path(self) -> Path | None: ...
    @property
    def destination(self) -> CompiledRelationLocation: ...
    @property
    def fingerprint_destination(self) -> CompiledRelationLocation: ...
    def with_target_defaults(
        self,
        defaults: tuple[str | None, str | None],
        render: Callable[..., str | None],
        python_functions_inherit_default_namespace: bool,
    ) -> NativeCompiledFunction: ...

class NativeAuditInput:
    @property
    def audit_file(self) -> DiscoveredAuditFile: ...
    @property
    def audit_block(self) -> DiscoveredAuditBlock: ...
    @property
    def sql_body(self) -> str: ...
    @property
    def evaluation_mode(self) -> AuditEvaluationMode: ...
    @property
    def measurement_contract(self) -> MeasurementContract | None: ...
    @property
    def thresholds(self) -> MeasurementThresholds | None: ...
    @property
    def minimum_samples(self) -> int | None: ...
    @property
    def measure_sql(self) -> str | None: ...
    @property
    def evidence_sql(self) -> str | None: ...
    @property
    def evidence_limit(self) -> int | None: ...
    @property
    def references(self) -> tuple[CompileSqlReference, ...]: ...
    @property
    def attached_target_kind(self) -> AttachedAuditTargetKind | None: ...
    @property
    def attached_target_name(self) -> str | None: ...
    @property
    def attached_column_name(self) -> str | None: ...
    @property
    def severity(self) -> AuditSeverity | None: ...
    @property
    def run_scope(self) -> str | None: ...
    @property
    def always_run(self) -> bool: ...
    @property
    def description(self) -> str | None: ...
    @property
    def meta(self) -> dict[str, object] | None: ...
    @property
    def declaration_usages(self) -> tuple[UsageRecord, ...]: ...
    @property
    def name(self) -> str | None: ...
    @property
    def assembly_row(
        self,
    ) -> tuple[list[tuple[str, str, str | None]], tuple[str, str] | None]: ...

class NativeCompiledAudit:
    @property
    def key(self) -> CompiledObjectKey: ...
    @property
    def scope_deps(self) -> tuple[CompiledObjectKey, ...]: ...
    @property
    def name(self) -> str: ...
    @property
    def definition_name(self) -> str: ...
    @property
    def audit_file(self) -> DiscoveredAuditFile: ...
    @property
    def audit_block(self) -> DiscoveredAuditBlock: ...
    @property
    def sql_body(self) -> str: ...
    @property
    def evaluation_mode(self) -> AuditEvaluationMode: ...
    @property
    def measurement_contract(self) -> MeasurementContract | None: ...
    @property
    def thresholds(self) -> MeasurementThresholds | None: ...
    @property
    def minimum_samples(self) -> int | None: ...
    @property
    def measure_sql(self) -> str | None: ...
    @property
    def evidence_sql(self) -> str | None: ...
    @property
    def evidence_limit(self) -> int | None: ...
    @property
    def references(self) -> tuple[CompileSqlReference, ...]: ...
    @property
    def attached_target_kind(self) -> AttachedAuditTargetKind | None: ...
    @property
    def attached_target_name(self) -> str | None: ...
    @property
    def attached_column_name(self) -> str | None: ...
    @property
    def severity(self) -> AuditSeverity | None: ...
    @property
    def run_scope(self) -> str | None: ...
    @property
    def always_run(self) -> bool: ...
    @property
    def description(self) -> str | None: ...
    @property
    def meta(self) -> dict[str, object] | None: ...

class NativeOrchestrationSnapshot:
    """A compiled data-only publication retained natively until transport."""

    def to_json(self) -> str: ...
    def publish(self, server_url: str, bearer: str) -> str: ...
    def start_worker(
        self, server_url: str, bearer: str, identity: tuple[str, str]
    ) -> NativeOrchestrationWorker: ...

class NativeOrchestrationWorker:
    """A verified claim owner with joined native heartbeat teardown."""

    def execution_json(self) -> str: ...
    def cancel_requested(self) -> bool: ...
    def report(self, event_id: str, change_json: str) -> bool: ...
    def close(self) -> None: ...
    def __enter__(self) -> Self: ...
    def __exit__(
        self,
        kind: type[BaseException] | None,
        value: BaseException | None,
        traceback: TracebackType | None,
    ) -> None: ...

def build_orchestration_snapshot(
    project: CompiledProjectView,
    metadata_json: str,
    project_dir: Path,
    *,
    declarations: Sequence[object] | None = None,
) -> NativeOrchestrationSnapshot: ...
def run_location_command(
    *,
    project_dir: Path,
    location_id: str,
    selected_target: str,
    generation: int,
    server_url: str,
    image_id: str | None = None,
    action: str = "publish",
) -> int: ...

class NativeProjectView:
    @property
    def run_id(self) -> str: ...
    @property
    def effective_target_name(self) -> str | None: ...
    @property
    def effective_connection(self) -> dict[str, object]: ...
    @property
    def effective_vars(self) -> dict[str, object]: ...
    @property
    def binding_catalog(self) -> Any | None: ...
    @property
    def effective_target_database(self) -> str | None: ...
    @property
    def effective_target_schema(self) -> str | None: ...
    @property
    def sql_analysis_dialect(self) -> str | None: ...
    @property
    def sql_lexical_syntax(self) -> SqlLexicalSyntax: ...
    @property
    def compile_cache_dir(self) -> Path | None: ...
    @property
    def settings(self) -> SettingsConfig: ...
    @property
    def scenario(self) -> ScenarioConfig: ...
    @property
    def models(self) -> tuple[CompiledModelView, ...]: ...
    @property
    def sources(self) -> tuple[CompiledSource, ...]: ...
    @property
    def seeds(self) -> tuple[CompiledSeed, ...]: ...
    @property
    def functions(self) -> tuple[CompiledFunction, ...]: ...
    @property
    def audits(self) -> tuple[CompiledAudit, ...]: ...
    @property
    def sql_tests(self) -> tuple[CompiledSqlTest, ...]: ...
    @property
    def sql_scenarios(self) -> tuple[CompiledSqlScenario, ...]: ...
    @property
    def loader_functions(self) -> tuple[DiscoveredLoaderFunction, ...]: ...
    @property
    def hook_functions(self) -> tuple[DiscoveredHookFunction, ...]: ...
    @property
    def enforce_explicit_references(self) -> bool: ...
    @property
    def sql_hook_files(self) -> tuple[DiscoveredSqlHookFile, ...]: ...
    @property
    def materialization_files(self) -> tuple[DiscoveredMaterializationFile, ...]: ...
    @property
    def public_enums(self) -> dict[str, EnumDeclaration]: ...
    @property
    def public_constants(self) -> dict[str, ConstantDeclaration]: ...
    @property
    def loaded_macros(self) -> dict[str, LoadedMacro]: ...
    @property
    def diagnostics(self) -> tuple[CompilerDiagnostic, ...]: ...
    @property
    def unmatched_literal_sql_relations(self) -> tuple[LiteralSqlRelation, ...]: ...
    @property
    def external_sql_reference_resolver(self) -> ExternalSqlReferenceResolver | None: ...
    @property
    def scope_index(self) -> ScopeIndex: ...
    @property
    def sql_expansions(self) -> dict[Path, CompiledSqlExpansion]: ...
    @property
    def native_session(self) -> Any | None: ...
    @property
    def native_project(self) -> NativeCompiledProject | None: ...
    @property
    def lineage_graph(self) -> NativeProjectGraph: ...
    def with_changes(self, **changes: object) -> NativeProjectView: ...
    def fields(self, include_sql_expansions: bool = True) -> dict[str, Any]: ...

class NativeCompiledProject:
    def __init__(self) -> None: ...
    def record_model_analysis(
        self,
        name: str,
        columns: tuple[bool, list[tuple[str, str | None]], list[bool], bool, list[str]],
        dynamic_proof: tuple[
            bool,
            bool,
            list[tuple[str, str | None]],
            str | None,
            list[tuple[str, str | None]],
        ]
        | None,
    ) -> bool: ...
    def record_view_analyses(self, views: list[NativeCompiledModel]) -> int: ...
    def model_names(self) -> list[str]: ...
    def project_view(self, fields: dict[str, object]) -> NativeProjectView: ...
    def inputs_view(self, fields: dict[str, object]) -> NativeInputsView: ...
    def attach_seeds(
        self,
        declarations: list[tuple[SchemaSeedEntry, DiscoveredSchemaFile]],
        seed_files: list[DiscoveredSeedFile],
        compile_error: type[Exception],
    ) -> list[NativeSeedInput]: ...
    def record_seed_namespaces(
        self, namespaces: list[tuple[str | None, str | None, str | None, str | None]]
    ) -> None: ...
    def compiled_seed(self, index: int) -> NativeCompiledSeed: ...
    def attach_sources(
        self,
        rows: list[tuple[str, DiscoveredSourceFile, SourceEntry]],
        settings: tuple[str | None, bool, object],
    ) -> list[NativeSourceInput]: ...
    def record_source_namespaces(
        self, namespaces: list[tuple[str | None, str | None] | None]
    ) -> None: ...
    def compiled_source(self, index: int) -> NativeCompiledSource: ...
    def attach_functions(
        self,
        rows: tuple[list[tuple[object, ...]], list[tuple[object, ...]]],
        settings: tuple[
            str,
            bool,
            tuple[str | None, str | None],
            tuple[str | None, str | None] | None,
            bool,
            object,
        ],
    ) -> list[NativeFunctionInput]: ...
    def record_function_deps(self, deps: list[list[tuple[str, str]]]) -> None: ...
    def compiled_function(self, index: int) -> NativeCompiledFunction: ...
    def resource_diagnostics(
        self, settings: dict[str, object]
    ) -> tuple[list[CompilerDiagnostic], list[CompilerDiagnostic], list[CompilerDiagnostic]]: ...
    def attach_sql_tests(
        self,
        files: list[tuple[object, ...]],
        scenarios: list[tuple[object, ...]],
        settings: dict[str, object],
    ) -> tuple[list[CompileSqlTestInput], list[CompileSqlScenarioInput]]: ...
    def attach_audits(
        self, files: list[tuple[object, ...]], settings: dict[str, object]
    ) -> tuple[list[NativeAuditInput], list[CompilerDiagnostic]]: ...
    def record_audit_scope_deps(self, deps: list[list[tuple[str, str]]]) -> None: ...
    def compiled_audit(self, index: int) -> NativeCompiledAudit: ...
    def graph_builds(self) -> int: ...
    def __eq__(self, other: object) -> bool: ...

def compiled_model_record(model: CompiledModelView) -> CompiledModel: ...
def compiled_audit_record(audit: CompiledAuditView) -> CompiledAudit: ...
def compute_cursor_bounds(
    *,
    cursor_snapshot: ModelCursorSnapshot,
    cursor_type: str | None,
    cursor_start: str | None,
    lookback: str | None,
    backfill_duration: str | None,
    start_cursor_override: str | None,
    end_cursor_override: str | None,
    is_microbatch: bool,
    cursor_grain: str | None = None,
    maximum_start_policy: MaximumStartPolicyInputs | None = None,
) -> CursorBounds | None: ...
def apply_typed_cursor_replay_policy(
    *,
    start: CursorScalar,
    end: CursorScalar,
    cursor_start: CursorScalar | str | None,
    cursor_type: str | None,
    lookback: Duration | str | None,
    backfill_duration: Duration | str | None,
    has_start_override: bool,
) -> CursorScalar: ...
def decide_empty_cursor_inputs(
    *,
    empty_input_names: tuple[str, ...],
    waits_for_every_input: bool,
    inputs_with_rows: bool,
    has_window_start: bool,
) -> EmptyCursorInputDecision: ...
def empty_cursor_window(*, cursor_type: str | None, cursor_start: str | None) -> CursorBounds: ...
def cursor_window_override_flags(*, cursor_type: str | None) -> str: ...
def empty_input_rebuild_refusal(
    *,
    model_name: str,
    input_names: tuple[str, ...],
    cursor_type: str | None,
) -> tuple[str, str]: ...
def resolve_effective_timestamp_grain(
    *,
    cursor_type: str | None,
    downstream_grain: str | None,
    cursor_input_grains: tuple[str | None, ...],
    microbatch_strategy: str | None = None,
) -> str | None: ...
def normalize_cursor_snapshot_grain(
    *,
    cursor_snapshot: ModelCursorSnapshot,
    cursor_type: str | None,
    effective_grain: str | None,
) -> ModelCursorSnapshot: ...
def resolve_model_sql(
    *,
    adapter: BaseAdapter,
    model: CompiledModelView,
    snapshot: WarehouseSnapshot,
    context: ModelPlanContext,
    backfill: BackfillResult,
    full_refresh: bool,
    cursor_overrides: CursorOverridePair,
    external_sql_reference_resolver: ExternalSqlReferenceResolver | None = None,
    replaces_relation: bool = False,
    full_rebuild_cause: str | None = None,
) -> str: ...
def resolve_function_sql(
    *,
    adapter: BaseAdapter,
    function: CompiledFunctionView,
    model_locations: dict[str, CompiledRelationLocation],
    seed_locations: dict[str, CompiledRelationLocation],
    function_locations: dict[str, CompiledRelationLocation],
    source_map: dict[str, SourceEntry],
    source_warehouse_columns: dict[str, tuple[ColumnInfo, ...]],
    star_exclude_keyword: str,
) -> str: ...
def resolve_model_hook_entries(
    *,
    model: CompiledModelView,
    adapter: BaseAdapter,
    context: ModelPlanContext,
    external_sql_reference_resolver: ExternalSqlReferenceResolver | None,
) -> tuple[object, object]: ...
def build_model_locations(
    models: tuple[CompiledModelView, ...],
) -> dict[str, CompiledRelationLocation]: ...
def build_seed_locations(
    seeds: tuple[CompiledSeedView, ...],
) -> dict[str, CompiledRelationLocation]: ...
def build_function_locations(
    functions: tuple[CompiledFunctionView, ...],
) -> dict[str, CompiledRelationLocation]: ...
def compiled_function_record(function: CompiledFunctionView) -> CompiledFunction: ...
def compiled_source_record(source: CompiledSourceView) -> CompiledSource: ...
def compiled_seed_record(seed: CompiledSeedView) -> CompiledSeed: ...
def compiled_project_record(
    project: CompiledProjectView, *, include_sql_expansions: bool = True
) -> CompiledProject: ...
def project_with(*, project: CompiledProjectView, **changes: Any) -> CompiledProjectView: ...
def inputs_with(*, inputs: CompileInputsView, **changes: Any) -> CompileInputsView: ...
def declaration_rows(
    *, enums: Sequence[EnumDeclaration], constants: Sequence[ConstantDeclaration]
) -> tuple[list[tuple[object, ...]], list[tuple[object, ...]]]: ...
def compiled_project_facts(project: CompiledProjectView) -> NativeCompiledProject: ...
def native_model_config_session(
    *, inputs: NativeModelConfigInputs, validator: NativeModelValidator
) -> NativeModelConfigSession: ...
def build_compile_target_context(
    *, discovered_inputs: DiscoveredProjectInputs, selected_target: str | None, no_cache: bool
) -> tuple[str | None, TargetConfig | None, Path | None]: ...
def build_declaration_scope(
    *,
    discovered_inputs: DiscoveredProjectInputs,
    loaded_macros: dict[str, LoadedMacro],
    sql_lexical_syntax: SqlLexicalSyntax,
    compile_cache_dir: Path | None = None,
) -> DeclarationScopeBuild: ...
def rebind_declaration_scope(
    *,
    scope: DeclarationScopeBuild,
    discovered_inputs: DiscoveredProjectInputs,
    loaded_macros: dict[str, LoadedMacro],
) -> DeclarationScopeBuild | None: ...

class NativeMacroExpansion:
    def __init__(
        self,
        file_path: object,
        state: tuple[object, object, object, object, object],
        enforce_explicit: bool,
        classes: dict[str, object],
    ) -> None: ...
    def expand(
        self, sql: str, bridge: object
    ) -> tuple[
        str,
        list[tuple[int, int, int, int]],
        list[tuple[str, str]],
        list[Any],
        list[Any],
    ]: ...
    def render_relation(self, reference: object) -> str: ...
    def declaration_read(self, kind: str, name: str) -> None: ...
    def block_memo(self) -> None: ...

def declared_relation_keys(
    *,
    model_rows: Iterable[RelationKeyRow],
    seeds: Iterable[SchemaSeedEntry],
    sources: Iterable[SourceEntry],
) -> RelationKeys: ...
def compiled_relation_keys(project: CompiledProjectView) -> RelationKeys: ...
def compile_facts(*, project: CompiledProjectView, project_dir: Path) -> CompileFacts: ...
def lint_file_role(
    *, file_path: Path, project_dir: Path, relative_path: PurePath | None = None
) -> LintFileRole: ...
def prepare_lint_body(
    *,
    role: LintFileRole,
    file_path: Path,
    contents: str,
    body_range: tuple[int, int],
    context: SqlExpansionContext,
    dialect: str,
) -> LintBody: ...
def fix_verdict(
    *, before: CompileFacts, after: CompileFacts, edited: frozenset[Path]
) -> FixVerdict: ...
def build_lint_expansion_context(
    *,
    project_dir: Path,
    value_renderer: TypedSqlValueRenderer | None = None,
    discovered_inputs: DiscoveredProjectInputs | None = None,
    declaration_scope: DeclarationScopeBuild | None = None,
    static_declaration_scope: DeclarationScopeBuild | None = None,
) -> SqlExpansionContext: ...
def write_base_definition_snapshot(
    *,
    project_dir: str | Path,
    output_path: str | Path,
    selected_target: str | None = None,
    cli_vars: dict[str, object] | None = None,
) -> None: ...
def build_dag_json(
    *, graph: ProjectGraph, project_name: str, python_graph: PythonNodeGraph | None = None
) -> str: ...

class NativeManifest:
    def contents(self) -> bytes: ...

def build_manifest(
    *,
    project: CompiledProjectView,
    plan_output: PlanOutput | None = None,
    project_name: str,
    adapter_type: str,
    graph: NativeProjectGraph | None = None,
    upstream_deps: Mapping[CompiledObjectKey, tuple[CompiledObjectKey, ...]] | None = None,
    downstream_deps: Mapping[CompiledObjectKey, tuple[CompiledObjectKey, ...]] | None = None,
) -> NativeManifest: ...

class NativeProjectGraph:
    @staticmethod
    def from_resources(
        resources: list[tuple[_GraphKey, list[_GraphKey], list[str], str | None]],
    ) -> NativeProjectGraph: ...
    @staticmethod
    def from_indexes(indexes: _GraphIndexes) -> NativeProjectGraph: ...
    def indexes(self) -> _GraphIndexes: ...
    def upstream(self, key: _GraphKey) -> list[_GraphKey]: ...
    def downstream(self, key: _GraphKey) -> list[_GraphKey]: ...
    def keys(self) -> list[_GraphKey]: ...
    def names(self) -> list[tuple[str, _GraphKey]]: ...
    def edges(self) -> list[tuple[_GraphKey, _GraphKey]]: ...
    def model_layer_count(self) -> int: ...
    def unique_id_maps(
        self, project_name: str, prefixes: dict[str, str]
    ) -> tuple[list[tuple[str, list[str]]], list[tuple[str, list[str]]]]: ...
    def resolve(self, select: list[str], exclude: list[str]) -> _SelectorKeys: ...
    def tokens(self, selectors: list[str]) -> _SelectorKeys: ...
    def matched(self, kind: str, value: str) -> _SelectorKeys: ...
    def build_resources(
        self, selected: list[_GraphKey], include: tuple[bool, bool, bool]
    ) -> list[_GraphKey]: ...

def split_selector_expansion(raw: str) -> SelectorExpansion: ...
def run_compile(request: CompileCommandRequest) -> int: ...

COMPILE_REUSE_RACY_WINDOW_NS: int
COMPILE_REUSE_EXCLUDED_ROOT_DIRECTORIES: frozenset[str]

def format_plan(
    *,
    plan: PlanOutput,
    full_refresh: bool = False,
    use_color: bool = True,
    include_header: bool = True,
    display_options: DisplayOptions | None = None,
    python_plan_entries: tuple[PythonPlanEntry, ...] = (),
    include_direct_freshness_diagnostics: bool = True,
    include_completion: bool = False,
) -> str: ...

class IntegrationAssetResult:
    kind: str
    name: str
    status: str
    action: str | None
    target: str | None
    origin_relation: str | None
    staging_relation: str | None
    failed_phase: str | None
    loader: str | None
    rows_loaded: int | None
    materialized: bool | None
    future_cursor_safety: dict[str, object]
    maximum_start_safety: dict[str, object]
    microbatch: dict[str, object]
    def as_dict(self) -> dict[str, Any]: ...

class IntegrationCheckResult:
    kind: str
    name: str
    check_id: str
    passed: bool
    status: str
    dag_check_id: str | None
    severity: str | None
    asset_name: str | None
    attachment_kind: str | None
    attached_column_name: str | None
    attached_target_name: str | None
    run_scope_phase: str | None
    row_count: int | None
    reused: bool | None
    evaluation_mode: str | None
    measured_value: float | None
    sample_count: int | None
    sample_unit: str | None
    minimum_samples: int | None
    thresholds: dict[str, object] | None
    evidence_count: int | None
    evidence_truncated: bool | None
    audit_definition_name: str | None
    audit_description: str | None
    def as_dict(self) -> dict[str, Any]: ...

class IntegrationResultEnvelope:
    schema_version: int
    record_kind: str
    event_id: str
    event_sequence: int
    event_type: str
    occurred_at: str
    invocation_id: str
    run_id: str
    resource_id: str
    resource_attempt_id: str
    operation_id: str | None
    statement_id: str | None
    resource_kind: str
    resource_name: str
    attempt_number: int
    duration_ms: int | float | None
    output_kind: str
    command: str
    error_code: str | None
    error_type: str | None
    skip_code: str | None
    skip_mode: str | None
    projection_degraded: bool
    @property
    def asset(self) -> IntegrationAssetResult | None: ...
    @property
    def checks(self) -> tuple[IntegrationCheckResult, ...]: ...
    @staticmethod
    def from_json(value: str) -> IntegrationResultEnvelope: ...
    @staticmethod
    def from_json_without_optional_enrichment(value: str) -> IntegrationResultEnvelope: ...
    def to_json(self) -> str: ...

class ExecutionEventWriter:
    def __init__(self, *, path: Path | None = None) -> None: ...
    def write_build_result(
        self, *, result: object, plan: PlanOutput | None, command: str = "build"
    ) -> None: ...
    def write_clone_result(self, *, item: CloneItemResult, resource_type: str) -> None: ...
    def close(self) -> None: ...

def write_cost_run(*, project_dir: Path, record: CostRunRecord) -> Path: ...
def read_cost_run(*, project_dir: Path, run_id: str) -> CostRunRecord | None: ...
def list_cost_runs(*, project_dir: Path) -> tuple[CostRunRecord, ...]: ...
def resolve_cost_run(*, project_dir: Path, selector: str) -> CostRunRecord | None: ...
def resolve_defer_clone_selection(*, scope: PlannerScope) -> tuple[list[str], list[str], int]: ...
def run_check_read_side_dependencies(
    *,
    adapter: BaseAdapter,
    connection_config: dict[str, object],
    connection: Any,
    pipeline_result: CompilePipelineResult,
    python_graph: PythonNodeGraph,
    lifecycle_plan: PythonSqlRunLifecyclePlan,
    relation_targets: dict[SqlResourceRef, str],
    validation_refs: frozenset[SqlResourceRef] | None = None,
    project_relations: dict[SqlResourceRef, str] | None = None,
    providers: ProviderContainer | None = None,
) -> tuple[PythonNodeExecutionResult, ...]: ...
def finalize_build_cost(finalization: BuildCostFinalization) -> CostRunRecord | None: ...
def format_cost_breakdown(
    *,
    record: CostRunRecord,
    use_color: bool,
    limit: int | None = None,
    terminal_width: int | None = None,
    show_run_id: bool = True,
) -> str: ...
def format_summary_footer(
    *, counts: tuple[tuple[str, int], ...], use_color: bool, elapsed: str | None = None
) -> str: ...
def format_count_noun(*, count: int, singular: str, plural: str | None = None) -> str: ...

class CountHeaderStyle:
    def __call__(self, text: str) -> str: ...

def count_header_style(
    *, style: CliStyle, title_style: Callable[[str], str] | None = None
) -> CountHeaderStyle: ...

class CliDocument:
    def __init__(self, style: CliStyle) -> None: ...
    def header(self, *, text: str, suffix: str | None = None) -> None: ...
    def blank(self) -> None: ...
    def field(self, *, label: str, value: str, value_padding: str = " ") -> None: ...
    def fields(
        self, *, rows: tuple[tuple[str, str], ...], label_width: int | None = None
    ) -> None: ...
    def section(self, text: str) -> None: ...
    def title_section(self, text: str) -> None: ...
    def line(self, text: str) -> None: ...
    def items(self, values: tuple[str, ...]) -> None: ...
    def command_line(self, *, prefix: str, command: str, style_command: bool = True) -> None: ...
    def commands(
        self, *, commands: tuple[str, ...], numbered: bool = False, style_command: bool = True
    ) -> None: ...
    def render(self, *, trailing_newline: bool = True) -> str: ...

def update_sqlbuild_skills(
    *,
    project_dir: Path,
    global_install: bool = False,
    requested_targets: tuple[str, ...] = (),
    force: bool = False,
    home_dir: Path | None = None,
) -> SkillUpdateResult: ...
def maintain_sqlbuild_skills(*, project_dir: Path) -> SkillMaintenanceResult: ...
def load_packaged_skill_files() -> dict[str, str]: ...
def run_playground_command(request: PlaygroundCommandRequest) -> int: ...
def describe_connection_warehouse(
    *,
    discovered_inputs: DiscoveredProjectInputs,
    selected_target: str | None,
    connection_config: dict[str, object],
) -> ResolvedWarehouse: ...
def resolve_connection_config(
    *,
    raw_config: dict[str, object],
    project_dir: Path,
    adapter_name: str,
    discovered_inputs: DiscoveredProjectInputs | None = None,
    cli_vars: dict[str, object] | None = None,
    selected_target: str | None = None,
) -> dict[str, object]: ...
def resolve_project_connection_config(
    *,
    discovered_inputs: DiscoveredProjectInputs,
    project_dir: Path,
    selected_target: str | None = None,
    cli_vars: dict[str, object] | None = None,
    require_connection: bool = True,
) -> dict[str, object]: ...
def resolve_target_connection_config(
    *,
    discovered_inputs: DiscoveredProjectInputs,
    project_dir: Path,
    target_name: str,
    cli_vars: dict[str, object] | None = None,
) -> dict[str, object]: ...
def normalize_dbt_profile_connection(
    *, resolved: ResolvedDbtProfileOutput
) -> NormalizedDbtProfileConnection: ...
def coerce_dbt_profile_bool(value: object) -> bool: ...
def coerce_dbt_profile_number(value: object) -> int | float: ...
def build_dbt_manifest_index(*, raw_data: object) -> DbtManifestIndex: ...
def resolve_dbt_manifest_model(
    *, manifest: DbtManifestIndex, name: str, package_name: str | None = None
) -> DbtManifestModel: ...

class DbtCombinedGraph:
    @property
    def nodes(self) -> frozenset[str]: ...
    @property
    def upstream_deps(self) -> dict[str, tuple[str, ...]]: ...
    @property
    def downstream_deps(self) -> dict[str, tuple[str, ...]]: ...
    def downstream_sqlbuild_model_names(self, unique_ids: Iterable[str]) -> tuple[str, ...]: ...

def build_dbt_combined_graph(
    *, manifest: DbtManifestIndex, project: CompiledProjectView
) -> DbtCombinedGraph: ...
def resolve_dbt_interop_sqlbuild_selection(
    *,
    project: CompiledProjectView,
    graph: DbtCombinedGraph,
    select: Sequence[str],
    manifest: DbtManifestIndex | None = None,
    exclude: Sequence[str] | None = None,
    dbt_anchor_unique_ids_by_term: Mapping[str, Sequence[str]] | None = None,
) -> DbtInteropSelectionResult: ...
def dbt_fqn_selector_term(*, fqn: Sequence[str], fallback: str) -> str: ...
def build_merged_dbt_execution_argv(
    *,
    command: DbtInteropCommand,
    options: DbtCliOptions,
    routed_args: Sequence[str],
    plan: DbtInteropPlan,
) -> tuple[str, ...] | None: ...
def build_failed_sqlbuild_model_names(
    *,
    graph: DbtCombinedGraph,
    manifest: DbtManifestIndex,
    node_results: Sequence[DbtNodeExecutionResult],
) -> tuple[str, ...]: ...
def plan_dbt_interop_command(
    *,
    command: DbtInteropCommand | str,
    project: CompiledProjectView,
    manifest: DbtManifestIndex,
    graph: DbtCombinedGraph,
    dbt_runner: DbtRunner,
    dbt_options: DbtCliOptions,
    command_args: DbtInteropCommandArgs,
) -> DbtInteropPlan: ...

class DbtRunner:
    dbt_executable: str
    invoker: DbtInvoker | None
    def __init__(
        self, *, dbt_executable: str | None = None, invoker: DbtInvoker | None = None
    ) -> None: ...
    def compile(
        self, *, options: DbtCliOptions, full_refresh: bool = False
    ) -> DbtCommandResult: ...
    def deps(self, *, options: DbtCliOptions) -> DbtCommandResult: ...
    def debug(self, *, options: DbtCliOptions, args: Sequence[str] = ()) -> DbtCommandResult: ...
    def ls(
        self,
        *,
        options: DbtCliOptions,
        select: Sequence[str] = (),
        exclude: Sequence[str] = (),
        resource_types: Sequence[str] = (),
    ) -> DbtLsResult: ...
    def invoke(self, *, argv: tuple[str, ...], cwd: Path | None) -> DbtCommandResult: ...

def execute_dbt_json_event_stream(
    *,
    argv: tuple[str, ...],
    cwd: Path | None,
    stream: TextIO,
    use_color: bool,
    target_path: Path | None,
    display_total: int | None = None,
    on_node_result: Callable[[DbtNodeExecutionResult], None] | None = None,
    detail_by_unique_id: dict[str, str] | None = None,
    enable_status: bool = True,
) -> tuple[int, tuple[DbtNodeExecutionResult, ...]]: ...
def build_dbt_init_project(*, request: DbtInitRequest) -> DbtInitResult: ...
def build_dbt_compile_argv(
    *, dbt_executable: str, options: DbtCliOptions, full_refresh: bool = False
) -> tuple[str, ...]: ...
def build_dbt_deps_argv(*, dbt_executable: str, options: DbtCliOptions) -> tuple[str, ...]: ...
def build_dbt_debug_argv(
    *, dbt_executable: str, options: DbtCliOptions, args: Sequence[str] | None = None
) -> tuple[str, ...]: ...
def build_dbt_ls_argv(
    *,
    dbt_executable: str,
    options: DbtCliOptions,
    select: Sequence[str] | None = None,
    exclude: Sequence[str] | None = None,
    resource_types: Sequence[str] | None = None,
) -> tuple[str, ...]: ...
def resolve_dbt_execution_invocation(
    request: DbtInteropExecutionRequest,
) -> DbtInteropInvocation: ...
def dbt_interop_plan_safety_gates() -> PlanSafetyGates: ...
def execute_dbt_interop_from_project(request: DbtInteropExecutionRequest) -> int: ...
def plan_dbt_interop_from_project(
    *,
    project_dir: Path,
    args: tuple[str, ...],
    dbt_runner: DbtRunner | None = None,
    dbt_executable: str | None = None,
    sqlbuild_executable: str = "sqb",
    no_sql_validation: bool = False,
    on_progress: Callable[[str], None] | None = None,
    progress_stream: TextIO | None = None,
    use_color: bool = False,
) -> DbtInteropPlan: ...
def debug_dbt_from_project(
    *,
    project_dir: Path,
    args: tuple[str, ...],
    dbt_runner: DbtRunner | None = None,
    dbt_executable: str | None = None,
    stdout_stream: TextIO,
    stderr_stream: TextIO,
) -> int: ...
def write_dbt_execution_plan_text(
    *,
    request: DbtInteropExecutionRequest,
    invocation: DbtInteropInvocation,
    plan: DbtInteropPlan,
    merged_dbt_argv: tuple[str, ...] | None,
) -> None: ...
def execute_dbt_without_state_tracking(
    *,
    request: DbtInteropExecutionRequest,
    invocation: DbtInteropInvocation,
    merged_dbt_argv: tuple[str, ...] | None,
) -> DbtCommandExecutionResult: ...
def write_dbt_execution_summary(
    *,
    request: DbtInteropExecutionRequest,
    invocation: DbtInteropInvocation,
    execution: DbtCommandExecutionResult,
) -> None: ...
def resolve_sqlbuild_execution_plan_output(
    *,
    request: DbtInteropExecutionRequest,
    invocation: DbtInteropInvocation,
    compiled: DbtInteropCompiledProject,
    manifest: DbtManifestIndex,
    plan: DbtInteropPlan,
    failed_sqlbuild_model_names: tuple[str, ...],
) -> PlanOutput | None: ...
def write_sqlbuild_skip_notice(
    *,
    request: DbtInteropExecutionRequest,
    invocation: DbtInteropInvocation,
    message: str,
) -> None: ...
def attach_sqlbuild_plan_output(
    *,
    plan: DbtInteropPlan,
    project_dir: Path,
    discovered_inputs: DiscoveredProjectInputs,
    compiled: DbtInteropCompiledProject,
    sqlbuild_args: tuple[str, ...],
    connection_progress: Any | None,
    on_progress: Callable[[str], None] | None,
) -> DbtInteropPlan: ...
def load_compiled_dbt_manifest(
    *,
    runner: DbtRunner,
    dbt_options: DbtCliOptions,
    full_refresh: bool,
    on_progress: Callable[[str], None] | None,
) -> DbtManifestIndex: ...
def compile_dbt_interop_project(
    *,
    project_dir: Path,
    discovered_inputs: DiscoveredProjectInputs,
    manifest: DbtManifestIndex,
    dbt_vars: dict[str, object],
    no_sql_validation: bool,
    on_progress: Callable[[str], None] | None,
) -> DbtInteropCompiledProject: ...
def resolve_dbt_interop_plan(
    *,
    command: DbtInteropCommand,
    invocation: DbtInteropInvocation,
    compiled: DbtInteropCompiledProject,
    manifest: DbtManifestIndex,
    sqlbuild_executable: str,
    on_progress: Callable[[str], None] | None,
) -> DbtInteropPlanResolution: ...
def execute_dbt_commands(
    *,
    runner: DbtRunner,
    options: DbtCliOptions,
    merged_argv: tuple[str, ...] | None,
    progress_stream: TextIO,
    stdout_stream: TextIO,
    use_color: bool,
    skip_message: str = "Skipping dbt: no dbt work selected.",
    expected_nodes: tuple[DbtLsNode, ...] | None = None,
    on_node_result: Callable[[DbtNodeExecutionResult], None] | None = None,
    on_progress: Callable[[str], None] | None = None,
) -> DbtCommandExecutionResult: ...
def parse_dbt_ls_json_lines(*, stdout: str) -> tuple[DbtLsNode, ...]: ...
def route_dbt_interop_args(
    *, command: DbtInteropCommand | str, parsed: DbtInteropParsedArgs
) -> DbtInteropRoutedArgs: ...
def parse_dbt_execution_args(
    *, command: DbtInteropCommand | str, args: Sequence[str]
) -> DbtInteropParsedArgs: ...
def resolve_dbt_vars_mapping(
    *, project_config: DbtConfig, local_config: LocalDbtConfig, dbt_args: Sequence[str]
) -> dict[str, object]: ...
def resolve_raw_dbt_profile_connection(
    *,
    raw_config: dict[str, object],
    project_dir: Path,
    adapter_name: str,
    project_config: ProjectConfig,
    cli_vars: dict[str, object] | None = None,
) -> NormalizedDbtProfileConnection | None: ...

class DbtCompileReferenceResolver:
    def __init__(self, *, dbt_manifest: DbtManifestIndex | None) -> None: ...
    def validate_model_names(self, *, known_model_names: set[str]) -> None: ...
    def extend_sql_test_model_names(self, *, known_model_names: set[str]) -> set[str]: ...
    def extend_sql_test_source_names(self, *, known_source_names: set[str]) -> set[str]: ...
    def extend_sql_test_seed_names(self, *, known_seed_names: set[str]) -> set[str]: ...
    def validate_reference(
        self,
        *,
        ref_kind: str,
        ref_name: str,
        ref_package: str | None,
        owner_relative_sql_path: Path,
    ) -> None: ...
    def resolve_reference(
        self, *, ref_kind: str, ref_name: str, ref_package: str | None
    ) -> str | None: ...

def build_compile_reference_resolver(
    *, manifest_contents: str | None
) -> ExternalSqlReferenceResolver | None: ...
def resolve_dbt_config(
    *,
    project_root: Path,
    config: DbtConfig,
    overrides: DbtCliConfigOverrides,
    require_project_dir: bool,
    local_config: LocalDbtConfig | None = None,
) -> ResolvedDbtConfig: ...
def build_dbt_interop_plan(
    *,
    command: DbtInteropCommand | str,
    dbt_command_argv: Sequence[str],
    dbt_ls_nodes: Sequence[DbtLsNode],
    sqlbuild_command_argvs: Sequence[Sequence[str]],
    selection: DbtInteropSelectionResult,
    dbt_required_selector_terms: Sequence[str] = (),
    supplemental_dbt_command_argvs: Sequence[Sequence[str]] = (),
    warnings: Sequence[str] = (),
    sqlbuild_plan_output: PlanOutput | None = None,
) -> DbtInteropPlan: ...
def render_dbt_interop_plan(
    *,
    plan: DbtInteropPlan,
    json_output: bool,
    use_color: bool,
    display_options: DisplayOptions | None = None,
) -> str: ...
def run_debug_command(
    *,
    project_dir: Path | None = None,
    no_color: bool = False,
    no_connection: bool = False,
    selected_target: str | None = None,
    json_output: bool = False,
) -> int: ...

class TerminalEventClaim:
    @property
    def terminal(self) -> LifecycleEvent: ...
    @property
    def event_sequence(self) -> int: ...

class TerminalEventIndex:
    def __init__(self) -> None: ...
    def consume(self, event: LifecycleEvent) -> None: ...
    def claim_resource_terminal(
        self,
        *,
        resource_name: str,
        resource_id: str | None = None,
        resource_attempt_id: str | None = None,
        attempt_number: int | None = None,
    ) -> TerminalEventClaim | None: ...
    def resource_terminal(
        self, *, resource_name: str, resource_id: str | None = None
    ) -> LifecycleEvent | None: ...
    def lifecycle_terminal(self) -> LifecycleEvent | None: ...
    def output_serialization_scope(self) -> AbstractContextManager[bool]: ...
    def events(self) -> tuple[LifecycleEvent, ...]: ...

class TerminalEventIndexScope:
    def __enter__(self) -> TerminalEventIndex: ...
    def __exit__(self, *exc_info: object) -> bool: ...

def current_terminal_event_index() -> TerminalEventIndex | None: ...
def terminal_event_index_scope(index: TerminalEventIndex) -> TerminalEventIndexScope: ...

class NativeProgressProjector:
    def __init__(self, *, stream: TextIO, use_color: bool, debug: bool = False) -> None: ...
    def install(self) -> Token[NativeProgressProjector | None]: ...
    def restore(self, token: Token[NativeProgressProjector | None]) -> None: ...
    def configure_resources(self, *, ordinals: Mapping[str, int], total: int) -> None: ...
    def expect_resource_enrichment(self, *, resource_name: str) -> None: ...
    def record_audit_outcome(self, *, resource_attempt_id: str, outcome: str) -> None: ...
    def is_operation_active(self, *, operation_name: str) -> bool: ...
    def consume(self, event: LifecycleEvent) -> None: ...
    def consume_resource_terminal(
        self,
        *,
        resource_name: str,
        resource_id: str | None = None,
        resource_attempt_id: str | None = None,
        attempt_number: int | None = None,
    ) -> float | None: ...
    def resource_terminal_duration(
        self,
        *,
        resource_name: str,
        resource_id: str | None = None,
        resource_attempt_id: str | None = None,
        attempt_number: int | None = None,
    ) -> float | None: ...
    def close(self) -> None: ...

def render_diff_output(
    *,
    result: DiffExecutionResult,
    from_label: str,
    to_label: str,
    mode_label: str,
    use_color: bool,
    verbose: bool,
    max_column_examples: int,
    max_row_only_examples: int,
    max_value_length: int | None = ...,
    suppress_example_values: bool = ...,
    outcome: str | None = ...,
) -> str: ...
def execute_query_diff(
    *, request: DiffCommandRequest, preparation: QueryDiffPreparation
) -> DiffExecutionResult: ...

class CliObservabilityScope:
    def __enter__(self) -> EventDispatcher: ...
    def __exit__(self, *exc_info: object) -> bool: ...

def cli_observability_scope(*, args: CliNamespace, project_dir: Path) -> CliObservabilityScope: ...

class EventExporterCounts:
    """Frozen attempt counters for one exporter or their aggregate."""

    @property
    def accepted(self) -> int: ...
    @property
    def filtered(self) -> int: ...
    @property
    def delivered(self) -> int: ...
    @property
    def failed(self) -> int: ...
    @property
    def dropped(self) -> int: ...

class EventExporterAccounting:
    """Frozen accounting snapshot for one named exporter."""

    @property
    def exporter_name(self) -> str: ...
    @property
    def counts(self) -> EventExporterCounts: ...

class EventExportSummary:
    """Final or point-in-time best-effort delivery accounting."""

    @property
    def aggregate(self) -> EventExporterCounts: ...
    @property
    def per_exporter(self) -> tuple[EventExporterAccounting, ...]: ...
    @property
    def queue_depth(self) -> int: ...
    @property
    def queue_capacity(self) -> int: ...
    @property
    def flush_complete(self) -> bool: ...
    @property
    def accepted(self) -> int: ...
    @property
    def filtered(self) -> int: ...
    @property
    def delivered(self) -> int: ...
    @property
    def failed(self) -> int: ...
    @property
    def dropped(self) -> int: ...

class EventExporterFailure:
    """Sanitized exporter failure information safe for diagnostics."""

    @property
    def exporter_name(self) -> str: ...
    @property
    def error_type(self) -> str: ...
    @property
    def event_kind(self) -> str: ...
    @property
    def event_severity(self) -> EventExportSeverity: ...

class BoundEventExporter:
    """One lifecycle-event sink bound to command-scoped provider instances."""

    def __init__(
        self,
        name: str,
        function: Callable[..., object],
        provider_arguments: Mapping[str, object],
        event_kinds: Iterable[str] | None = None,
        min_severity: EventExportSeverity | str | None = None,
    ) -> None: ...
    @property
    def name(self) -> str: ...
    @property
    def function(self) -> Callable[..., object]: ...
    @property
    def provider_arguments(self) -> Mapping[str, object]: ...
    @property
    def event_kinds(self) -> frozenset[str]: ...
    @property
    def min_severity(self) -> EventExportSeverity: ...

class EventExporterDispatcher:
    """Deliver filtered lifecycle events off execution threads with bounded memory."""

    def __init__(
        self,
        *,
        exporters: Iterable[BoundEventExporter] | None = None,
        queue_capacity: int = 1024,
        shutdown_timeout_seconds: float = 2.0,
        invocation_timeout_seconds: float = 1.0,
        notification_queue_capacity: int = 256,
        health_interval_seconds: float | None = None,
        failure_callback: Callable[[EventExporterFailure], object] | None = None,
        summary_callback: Callable[[EventExportSummary], object] | None = None,
    ) -> None: ...
    @property
    def thread(self) -> threading.Thread: ...
    @property
    def notification_thread(self) -> threading.Thread | None: ...
    def enqueue(self, event: LifecycleEvent, /) -> None: ...
    def bind(self, exporters: Iterable[BoundEventExporter], /) -> None: ...
    def bind_command_output_sinks(self, sinks: Iterable[BoundCommandOutputSink], /) -> None: ...
    def export_output(self, records: Iterable[CommandOutputRecord]) -> None: ...
    def shutdown(self) -> EventExportSummary: ...
    def finalize_when_idle(self, finalizer: Callable[[], object], /) -> None: ...
    def summary(self) -> EventExportSummary: ...

class EventExporterCommandScope:
    """Bind discovered typed sinks and own their command-scoped provider session."""

    def __init__(self, *, dispatcher: EventExporterDispatcher) -> None: ...
    @property
    def dispatcher(self) -> EventExporterDispatcher: ...
    @property
    def provider_session(self) -> ProviderSession | None: ...
    @property
    def project_dir(self) -> Path | None: ...
    @property
    def providers(self) -> tuple[DiscoveredProvider, ...]: ...
    @property
    def event_exporters(self) -> tuple[DiscoveredEventExporter, ...]: ...
    @property
    def command_output_sinks(self) -> tuple[DiscoveredCommandOutputSink, ...]: ...
    def configure(self, discovered_inputs: DiscoveredProjectInputs, /) -> None: ...
    def configure_extensions(
        self,
        *,
        project_dir: Path | None = None,
        providers: tuple[DiscoveredProvider, ...],
        event_exporters: tuple[DiscoveredEventExporter, ...],
        command_output_sinks: tuple[DiscoveredCommandOutputSink, ...] | None = None,
    ) -> None: ...
    def cached_extensions(
        self, *, project_dir: Path
    ) -> (
        tuple[
            tuple[DiscoveredProvider, ...],
            tuple[DiscoveredEventExporter, ...],
            tuple[DiscoveredCommandOutputSink, ...],
        ]
        | None
    ): ...
    def close(self) -> EventExportSummary: ...
    def export_output(self, records: tuple[CommandOutputRecord, ...]) -> None: ...

def current_event_exporter_command_scope() -> EventExporterCommandScope | None: ...
def cached_event_exporter_extensions(
    *, project_dir: Path
) -> (
    tuple[
        tuple[DiscoveredProvider, ...],
        tuple[DiscoveredEventExporter, ...],
        tuple[DiscoveredCommandOutputSink, ...],
    ]
    | None
): ...
def lifecycle_export_policy_catalog() -> dict[str, tuple[str, EventExportSeverity, int]]: ...
def event_export_severity_at_least(
    *, severity: EventExportSeverity | str, minimum: EventExportSeverity | str
) -> bool: ...
def stricter_event_export_severity(
    *, first: EventExportSeverity | str, second: EventExportSeverity | str | None
) -> EventExportSeverity: ...

class OutputCaptureDispatcher:
    """Turn stream fragments into bounded text chunks and export them off execution threads."""

    def __init__(
        self,
        *,
        exporter: object,
        invocation_id: str,
        run_id: str | None = None,
        external_context: Mapping[str, object] | None = None,
        queue_capacity: int = 1024,
        batch_size: int = 64,
        max_record_bytes: int = 65536,
        shutdown_timeout_seconds: float = 2.0,
        clock: Callable[[], datetime] | None = None,
        failure_callback: Callable[[BaseException], object] | None = None,
    ) -> None: ...
    def append(self, *, stream: CommandOutputStream, text: str) -> None: ...
    def close(self) -> CommandOutputCaptureSummary: ...

class CommandOutputContext:
    def __enter__(self) -> None: ...
    def __exit__(self, *exc_info: object) -> bool: ...

def command_output_context(*, external_context: Mapping[str, object]) -> CommandOutputContext: ...
def current_output_capture_context() -> OutputCaptureContext | None: ...
def chunk_command_output_text(*, text: str, max_bytes: int) -> tuple[str, ...]: ...
def freeze_command_output_record(record: CommandOutputRecord, /) -> Mapping[str, object]: ...
def command_output_to_json(record: CommandOutputRecord, /) -> str: ...
def command_output_from_json(raw_json: str, /) -> CommandOutputRecord: ...
def invocation_context_from_environment(
    *, environment: Mapping[str, str] | None = None
) -> Mapping[str, object]: ...

class RecallLifecycleSink:
    """The invocation's Recall session as lifecycle and SQL artifact subscribers."""

    def __call__(self, event: LifecycleEvent) -> None: ...
    def record_sql(
        self, *, sql: str, statement_id: str | None = None, resource_id: str | None = None
    ) -> None: ...

class RecallWriter:
    """Publishes the records a Recall replacement sink keeps; valid only during the sink call."""

    def publish_lifecycle(self, event: LifecycleEvent) -> None: ...
    def publish_command_output(self, record: CommandOutputRecord) -> None: ...

class LocalFilesystemComputeLogStorage:
    """Ordered fsync capture storage subject to underlying filesystem and hardware guarantees."""

    def __init__(
        self,
        *,
        project_dir: Path,
        root: Path | None = None,
        retention_count: int | None = 20,
    ) -> None: ...
    def attach_recall(self, recall: RecallLifecycleSink) -> None: ...
    @property
    def root(self) -> Path: ...
    def start_capture(self, metadata: CaptureMetadata) -> None: ...
    def append(self, *, invocation_id: str, stream: ComputeLogStream, data: bytes) -> None: ...
    def append_diagnostic(self, *, invocation_id: str, record: DiagnosticLog) -> None: ...
    def read(
        self,
        *,
        invocation_id: str,
        stream: ComputeLogStream,
        cursor: int = 0,
        max_bytes: int = 1_048_576,
    ) -> ComputeLogReadChunk: ...
    def get_metadata(self, *, invocation_id: str) -> CaptureMetadata | FinalCaptureMetadata: ...
    def get_byte_count(self, *, invocation_id: str, stream: ComputeLogStream) -> int: ...
    def get_byte_counts(self, *, invocation_id: str) -> CaptureByteCounts: ...
    def is_complete(self, *, invocation_id: str) -> bool: ...
    def mark_complete(self, metadata: FinalCaptureMetadata) -> None: ...
    def delete(self, *, invocation_id: str) -> None: ...
    def prune(self, retain_count: int | None = None) -> PruneResult: ...
    def inventory(self) -> CaptureInventory: ...
    def close(self) -> None: ...
    def dispose(self) -> None: ...
    def __enter__(self) -> Self: ...
    def __exit__(
        self,
        exception_type: type[BaseException] | None,
        exception: BaseException | None,
        traceback: TracebackType | None,
    ) -> bool | None: ...

class _ExecutionHistoryStorage:
    @property
    def project_calls(self) -> int: ...
    def append_event(self, event: CanonicalLifecycleEvent) -> StoredEvent: ...
    def append_events(
        self, events: Iterable[CanonicalLifecycleEvent]
    ) -> tuple[StoredEvent, ...]: ...
    def get_events(
        self,
        *,
        event_filter: EventFilter,
        after_cursor: str | None = None,
        limit: int = 100,
    ) -> EventPage: ...
    def get_run(self, run_id: str) -> RunRecord | None: ...
    def get_runs(
        self,
        *,
        run_filter: RunFilter,
        after_cursor: str | None = None,
        limit: int = 100,
    ) -> RunPage: ...
    def project(self, stored_events: Iterable[StoredEvent]) -> tuple[RunRecord, ...]: ...
    def rebuild_from_events(
        self, stored_events: Iterable[StoredEvent]
    ) -> tuple[RunRecord, ...]: ...
    def append_and_project(
        self, events: Iterable[CanonicalLifecycleEvent]
    ) -> tuple[StoredEvent, ...]: ...
    def reconcile(self) -> tuple[RunRecord, ...]: ...
    def check_health(self) -> bool: ...
    def get_schema_version(self) -> int: ...
    def upgrade_schema(self, *, target_version: int | None = None) -> int: ...
    def close(self) -> None: ...
    def dispose(self) -> None: ...
    def __enter__(self) -> Self: ...
    def __exit__(
        self,
        exception_type: type[BaseException] | None,
        exception: BaseException | None,
        traceback: TracebackType | None,
    ) -> bool | None: ...

class SQLiteExecutionHistory(_ExecutionHistoryStorage):
    """Project-local event-log and run-projection storage with a Rust-owned SQLite driver."""

    # Native connection by default; DB-API overrides retain their existing dispatch boundary.
    _connection: Any
    def __init__(
        self,
        project_dir: Path | str | None = None,
        *,
        path: Path | str | None = None,
        busy_timeout_ms: int = 5_000,
    ) -> None: ...
    @property
    def path(self) -> Path | None: ...
    def _migrate(self) -> None: ...
    def _read_all_events(self) -> tuple[StoredEvent, ...]: ...
    def _publish_projection(self, projected: tuple[RunRecord, ...]) -> None: ...

class PostgresExecutionHistory(_ExecutionHistoryStorage):
    """Deployed PostgreSQL implementation of event-log and run-projection contracts."""

    _connection: Any
    def __init__(
        self,
        dsn: str,
        *,
        connect_timeout_seconds: int = 10,
        statement_timeout_ms: int = 30_000,
        max_transaction_retries: int = 3,
    ) -> None: ...
    def _append_events(
        self, *, cursor: Any, events: tuple[CanonicalLifecycleEvent, ...]
    ) -> tuple[StoredEvent, ...]: ...
    def _replace_projection(self, *, cursor: Any, projected: tuple[RunRecord, ...]) -> None: ...
    def _read_all_events(self, *, cursor: Any) -> tuple[StoredEvent, ...]: ...

def canonical_event_id(event: CanonicalLifecycleEvent) -> str: ...
def canonical_event_content(event: CanonicalLifecycleEvent) -> str: ...
def project_execution_runs(
    *, stored_events: Iterable[StoredEvent], current_runs: Iterable[RunRecord] | None = None
) -> tuple[RunRecord, ...]: ...
def validate_history_page_limit(limit: int) -> None: ...
def append_and_project(
    *,
    event_log: LifecycleEventLogStorage,
    run_storage: RunStorage,
    events: Iterable[CanonicalLifecycleEvent],
) -> tuple[StoredEvent, ...]: ...
def validate_stored_history_event(record: StoredEvent) -> None: ...
def validate_history_event_filter(event_filter: EventFilter) -> None: ...
def validate_history_run_filter(run_filter: RunFilter) -> None: ...
def validate_history_page(*, page: EventPage | RunPage, is_event_page: bool) -> None: ...
def current_native_progress_projector() -> NativeProgressProjector | None: ...

class ConnectionProgressReporter:
    def __init__(
        self,
        *,
        adapter_name: str,
        stream: TextIO,
        blank_line_before_start: bool = False,
        blank_line_after_complete: bool = False,
        use_color: bool = False,
    ) -> None: ...
    def on_connection_start(self, connection_count: int) -> None: ...
    def on_connection_complete(self, *, connection_count: int, elapsed_seconds: float) -> None: ...
    def on_connection_error(self, *, connection_count: int, elapsed_seconds: float) -> None: ...

class DbtEventStreamUpdate:
    @property
    def status_message(self) -> str | None: ...
    @property
    def result(self) -> DbtNodeExecutionResult | None: ...
    @property
    def row(self) -> str | None: ...
    @property
    def finished(self) -> bool: ...

class DbtEventStream:
    def __init__(
        self,
        *,
        display_total: int | None,
        detail_by_unique_id: Mapping[str, str] | None,
        use_color: bool,
    ) -> None: ...
    def consume(self, line: str) -> DbtEventStreamUpdate | None: ...

class CliNamespace:
    command: str | None
    project_dir: str | None
    dbt_project_dir: str | None
    dbt_profiles_dir: str | None
    dbt_target: str | None
    dbt_profile: str | None
    sqb_output_dir: str | None
    dry_run: bool
    overwrite: bool
    skip_dbt_debug: bool
    no_sql_validation: bool
    no_cache: bool
    defer_to: str | None
    defer_clone_from: str | None
    defer_sources_to: str | None
    target: str | None
    warehouse: str | None
    case: str | None
    inspect: bool
    review_against: str | None
    review_downstream: str | None
    review_no_clone: bool
    review_markdown: Path | None
    target_range: str | None
    from_target: str | None
    to_target: str | None
    hard_copy: bool
    json: bool
    json_output: Path | None
    event_output: Path | None
    manifest: bool
    dag: str | None
    compile_lineage_mode: CompileLineageMode
    profile_skip_discovery_sql_analysis: bool
    profile_skip_column_inference: bool
    profile_skip_contracts: bool
    profile_skip_write: bool
    start_cursor: str | None
    end_cursor: str | None
    start_cursor_ts: str | None
    max_microbatches: int | None
    end_cursor_ts: str | None
    start_cursor_int: str | None
    end_cursor_int: str | None
    no_color: bool
    fail_fast: bool
    full_refresh: bool
    as_target: str | None
    selection_diagnostics: bool
    include_python: bool
    load_sources: bool | None
    reload: bool
    run_tests: bool
    run_audits: bool
    allow_snapshot_full_refresh: bool
    allow_table_type_downgrade: bool
    allow_retention_decrease: bool
    allow_missing_migration_origin: bool
    allow_snapshot_schema_change: bool
    concurrency: int | None
    verbose: bool
    debug: bool
    auto_approve: bool
    retention_days: int | None
    direct_state_history_versions: int | None
    drop_old_name_views: list[str]
    keep_in_use: bool
    bounded: str | None
    max_column_examples: int | None
    max_row_only_examples: int | None
    sample_rows: int | None
    sample_seed: int | None
    exhaustive: bool
    max_models: int | None
    max_columns: int | None
    query_sql: str | None
    query_file: str | None
    query_format: str | None
    query_limit: int | None
    query_no_limit: bool
    cost_selector: str
    refactor_target: str | None
    refactor_new_name: str | None
    refactor_destination: str | None
    refactor_cascade: bool
    cost_limit: int | None
    cost_no_limit: bool
    cost_sort: str | None
    cost_order: str | None
    cost_since: str | None
    cost_until: str | None
    lineage_targets: list[str]
    lineage_format: str
    lineage_direction: str | None
    lineage_depth: str
    lineage_mode: ColumnLineageMode
    lineage_include_uses: bool
    no_connection: bool
    fail_on_error: bool
    fail_on_stale: bool
    state: bool
    full: bool
    schema_only: bool
    left_query: str | None
    left_query_file: Path | None
    left_label: str | None
    right_query: str | None
    right_query_file: Path | None
    right_label: str | None
    key: list[str]
    unkeyed: bool
    exclude_column: list[str]
    tolerance: list[str]
    max_value_length: int | None
    no_example_values: bool
    full_example_values: bool
    playground_path: str
    playground_template: str
    scenario_command: str | None
    scenario_selector: list[str]
    scenario_retain: bool
    scenario_namespace: str | None
    scenario_local: bool
    scenario_strict: bool
    scenario_sync_snapshots: bool
    scenario_refresh: bool
    scenario_force: bool
    scenario_max_snapshot_rows: int | None
    scenario_max_snapshot_total_rows: int | None
    scenario_max_snapshot_bytes: int | None
    scenario_max_snapshot_total_bytes: int | None
    skills_global: bool
    skills_target: list[str]
    skills_force: bool
    select: list[str]
    positional_select: list[str]
    select_file: list[str]
    exclude: list[str]
    dbt_command: str | None
    dbt_args: list[str]
    vars: dict[str, object]
    rules_command: str | None
    rules_rule_selector: str | None
    rules_skills_check: bool
    recall_command: str | None
    recall_sql: str | None
    recall_backend: str
    recall_format: str
    recall_limit: int
    recall_no_limit: bool
    recall_max_cell_bytes: int | None
    recall_state: bool
    recall_relation: str | None
    recall_invocation: str
    recall_stream: str
    recall_tail: int | None
    format_check: bool
    format_diff: bool
    format_fixtures_only: bool
    format_fix: bool
    format_paths: list[str]
    scope_target: str | None
    scope_at: str | None
    scope_as_path: str | None
    scope_browse: str | None
    scope_list: str | None
    contract_command: str | None
    contract_from: str | None
    contract_write: bool
    scope_defined_under: str | None
    scope_kind: list[str]
    scope_match: str | None
    scope_used_only: bool
    scope_include_nearby: bool
    scope_nearby_depth: int
    scope_dependency_depth: int
    scope_explain: str | None
    scope_globals: str
    scope_page_size: int
    scope_after: str | None
    scope_paths: str

    def __init__(self) -> None: ...

class PlanningProgressReporter:
    def __init__(self, *, stream: TextIO, use_color: bool = False) -> None: ...
    def on_progress(self, message: str) -> None: ...
    def start(self, message: str) -> None: ...
    def update(self, message: str) -> None: ...
    def complete(self, message: str) -> None: ...
    def complete_styled(self, message: str) -> None: ...
    def error(self, message: str) -> None: ...
    def finish(self, *, blank_line_after: bool = False) -> None: ...

class VerboseInspectionWriter:
    def __init__(self) -> None: ...
    def write(self, record: InspectionQueryRecord) -> None: ...
    def write_total(self) -> None: ...
    @staticmethod
    def format_record(*, record: InspectionQueryRecord) -> str: ...

def write_execution_header(
    *, stream: TextIO, command: str, target: str | None, concurrency: int, use_color: bool
) -> None: ...
def format_expectation_detail(step_result: StepResult) -> str: ...
def format_build_footer(
    *,
    result: BuildExecutionResult,
    elapsed: float,
    use_color: bool,
    python_node_results: tuple[PythonNodeExecutionResult, ...] | None = None,
) -> str: ...
def format_build_execution_json(
    *,
    result: BuildExecutionResult,
    plan: PlanOutput,
    python_node_results: tuple[PythonNodeExecutionResult, ...] = (),
    python_check_results: tuple[PythonCheckExecutionResult, ...] = (),
    command: str = "build",
    run_id: str | None = None,
    cost: dict[str, object] | None = None,
) -> str: ...
def format_seed_execution_json(
    *, results: tuple[SeedExecutionResult, ...], plan: PlanOutput
) -> str: ...
def format_load_execution_json(*, results: tuple[LoadExecutionResult, ...]) -> str: ...
def build_ingestr_command(
    *,
    source_entry: SourceEntry,
    adapter_name: str,
    connection_config: dict[str, object],
    destination_name: str,
    vars: dict[str, object],
    environment: str | None,
    run_id: str,
    is_reload: bool,
) -> tuple[str, ...]: ...
def format_ingestr_command(command: Sequence[str], /) -> str: ...
def dlt_destination_arguments(
    *,
    adapter_name: str,
    connection_config: Mapping[str, object],
    destination_config: Mapping[str, object] | None,
    dataset_name: str | None,
) -> tuple[str, dict[str, Any], dict[str, Any], str | None]: ...
def format_clone_execution_json(
    *,
    result: CloneExecutionResult,
    resource_types_by_name: Mapping[str, str],
    phase_timings: dict[str, float] | None = None,
) -> str: ...
def format_test_execution_json(*, results: tuple[SqlTestExecutionResult, ...]) -> str: ...
def format_audit_execution_json(
    *,
    results: tuple[AuditExecutionResult, ...],
    configured_concurrency: int = 1,
    worker_count: int | None = None,
) -> str: ...
def format_degradable_build_execution_json(
    *,
    result: BuildExecutionResult,
    plan: PlanOutput,
    execution_succeeded: bool,
    python_node_results: tuple[PythonNodeExecutionResult, ...] = (),
    python_check_results: tuple[PythonCheckExecutionResult, ...] = (),
    command: str = "build",
    run_id: str | None = None,
    cost: dict[str, object] | None = None,
) -> str: ...
def format_plan_json(
    *, plan: PlanOutput, python_plan_entries: tuple[PythonPlanEntry, ...] = ()
) -> str: ...
def serialize_maximum_start_safety(
    evidence: MaximumStartSafetyEvidence | None,
) -> dict[str, object] | None: ...
def format_compile_text(
    *,
    graph: ProjectGraph,
    written: WrittenTarget,
    manifest: bool,
    lineage: ProjectColumnLineage | None,
    diagnostics: tuple[CompilerDiagnostic, ...],
    lineage_mode: CompileLineageMode = ...,
    use_color: bool,
    selected_keys: frozenset[CompiledObjectKey] | None = None,
    sql_validation_enabled: bool = True,
) -> str: ...
def format_compile_json(
    *,
    graph: ProjectGraph,
    written: WrittenTarget,
    manifest: bool,
    timings_ms: dict[str, int],
    lineage: ProjectColumnLineage | None,
    diagnostics: tuple[CompilerDiagnostic, ...],
    lineage_mode: CompileLineageMode = ...,
    selected_keys: frozenset[CompiledObjectKey] | None = None,
    sql_validation_enabled: bool = True,
) -> str: ...
def write_static_compile_target(
    *,
    target_dir: Path,
    adapter: BaseAdapter,
    project: CompiledProjectView,
    manifest: NativeManifest | None = None,
) -> WrittenTarget: ...
def write_compile_target(
    *,
    target_dir: Path,
    adapter: BaseAdapter,
    plan_output: PlanOutput,
    manifest: NativeManifest | None = None,
) -> WrittenTarget: ...

class PreparedCompileArtifacts:
    min_prepared_models: int
    max_prepared_models: int
    def __init__(self, *, enabled: bool) -> None: ...
    @property
    def enabled(self) -> bool: ...
    def __enter__(self) -> PreparedCompileArtifacts: ...
    def __exit__(self, *exception: object) -> None: ...
    def remove_abandoned_staging(self, *, target_dir: Path) -> None: ...
    def create_staging_directory(self, staging_dir: Path) -> None: ...
    def start(
        self,
        *,
        project: CompiledProjectView,
        adapter: BaseAdapter,
        target_dir: Path,
        stage_artifacts: bool,
    ) -> None: ...
    def publish(
        self, *, target_dir: Path, manifest: NativeManifest | None
    ) -> WrittenTarget | None: ...
    def planning_diagnostics(self) -> tuple[CompilerDiagnostic, ...] | None: ...

def format_sql_json(request_json: str) -> str: ...
def query_fingerprint(sql: str, dialect: str) -> str: ...
def lint_sql_json(request_json: str) -> str: ...
def prepare_lint_sql(
    request: LintPreparationRequest,
) -> tuple[str, list[tuple[str, int, int, int, int, str]], list[str]] | None: ...
def neutralize_lint_interpolation(
    body: str, dialect: str, for_formatting: bool = False
) -> tuple[str, list[tuple[str, int, int, int, int, str]]]: ...
def restore_lint_interpolation(
    fixed: str, sites: list[tuple[str, int, int, int, int, str]]
) -> tuple[bool, str]: ...
def fixture_read_cache_info(*, reset: bool = False) -> tuple[int, int]: ...
def fixture_planning_work_info(*, reset: bool = False) -> tuple[int, int]: ...
def planner_identity_work_info(*, reset: bool = False) -> tuple[int, dict[str, int]]: ...
def compile_stage_work_info(*, reset: bool = False) -> dict[str, int]: ...
def migration_fingerprint_work_info(*, reset: bool = False) -> tuple[int, int]: ...
def build_relation_fixture_context(
    *, project: CompiledProjectView, models_by_name: dict[str, CompiledModelView] | None = None
) -> RelationFixturePlanningContext: ...
def build_relation_fixture_completion(
    *,
    project: CompiledProjectView,
    adapter: BaseAdapter,
    request: tuple[tuple[str, ...], FixtureGroups],
    planning_context: RelationFixturePlanningContext | None = None,
) -> RelationFixtureCompletion: ...
def format_redundant_fixture_nulls(
    *,
    files: dict[Path, str],
    project_dir: Path,
    discovered_inputs: DiscoveredProjectInputs,
    sql_lexical_syntax: SqlLexicalSyntax,
) -> dict[Path, str]: ...
def format_lint_sql_bodies(
    files: list[tuple[Path, str]], project_dir: Path, config: LintConfig
) -> tuple[dict[Path, str], list[LintViolation]]: ...
def lint_native_sql_bodies(
    bodies: list[LintBody], contents_by_path: dict[Path, str], config: LintConfig
) -> list[LintViolation]: ...
def reject_lint_suppressions(
    violations: list[LintViolation],
    contents_by_path: dict[Path, str],
    project_dir: Path,
) -> list[LintViolation]: ...
def load_rules_config(*, project_dir: Path) -> RulesConfig: ...
def build_rule_catalogue(
    *, config: RulesConfig, project_dir: Path, include_custom: bool | None = None
) -> tuple[Rule, ...]: ...
def select_catalogue_rules(
    *,
    catalogue: tuple[Rule, ...],
    config: RulesConfig,
    project_dir: Path,
    custom_payloads: list[dict[str, object]] | None = None,
) -> tuple[Rule, ...]: ...
def format_rule(*, rule: Rule, config: RulesConfig | None = None) -> str: ...
def install_rules_skills(*, config: RulesConfig, project_dir: Path, check: bool) -> bool: ...
def model_header_column_locations(
    contents: str, header: str, header_start: int
) -> tuple[list[tuple[str, int, int, int, int]] | None, str | None]: ...

class NativeProjectTree:
    def __init__(self, project_dir: str) -> None: ...

class NativeDirectorySnapshot:
    @property
    def project_dir(self) -> Path: ...
    @property
    def memo(self) -> dict[object, object]: ...

class NativeDirectorySnapshotScope:
    def __enter__(self) -> NativeDirectorySnapshot: ...
    def __exit__(self, *exc_info: object) -> bool: ...

def directory_snapshot_scope(*, project_dir: Path) -> NativeDirectorySnapshotScope: ...

class DiscoveredSqlModelFile:
    """A discovered SQL model file and its raw contents."""

    def __init__(
        self,
        file_path: Path,
        relative_path: Path,
        contents: str,
        header_values: dict[str, object],
        header_column_locations: dict[str, SourceLocation],
        output_column_locations: dict[str, SourceLocation],
        query_sql: str,
        enum_declarations: tuple[EnumDeclaration, ...] = (),
        constant_declarations: tuple[ConstantDeclaration, ...] = (),
        extract_implicit_alias_columns: bool = True,
    ) -> None: ...
    @property
    def file_path(self) -> Path: ...
    @property
    def relative_path(self) -> Path: ...
    @property
    def contents(self) -> str: ...
    @property
    def header_values(self) -> dict[str, object]: ...
    @property
    def header_column_locations(self) -> dict[str, SourceLocation]: ...
    @property
    def output_column_locations(self) -> dict[str, SourceLocation]: ...
    @property
    def query_sql(self) -> str: ...
    def with_text(self, contents: str, query_sql: str) -> DiscoveredSqlModelFile: ...
    @property
    def enum_declarations(self) -> tuple[EnumDeclaration, ...]: ...
    @property
    def constant_declarations(self) -> tuple[ConstantDeclaration, ...]: ...
    @property
    def extract_implicit_alias_columns(self) -> bool: ...

def parse_schema_document(loaded: object, path: str, /) -> tuple[SchemaSeedEntry, ...]: ...
def parse_sources_document(loaded: object, path: str, /) -> tuple[SourceEntry, ...]: ...
def build_compiled_project(
    *,
    discovered_inputs: DiscoveredProjectInputs,
    adapter: BaseAdapter,
    selected_target: str | None = None,
    no_sql_validation: bool = False,
    skip_column_inference: bool = False,
    column_lineage_mode: ColumnLineageMode = ...,
    cli_vars: dict[str, object] | None = None,
    external_sql_reference_resolver: ExternalSqlReferenceResolver | None = None,
    resolved_connection: dict[str, object] | None = None,
    analysis_selection: CompileAnalysisSelection | None = None,
) -> CompiledProjectView: ...
def compile_frontier[T](
    *,
    until: CompilerStage,
    python_stage: Callable[[], T],
    native_stage: Callable[[], T] | None = None,
) -> T: ...
def render_stage_capture(value: object) -> str: ...
def claim_first_compile() -> bool: ...
def digest_module_files(paths: Iterable[str], /) -> dict[str, str] | None: ...
def module_digests_metadata(digests: dict[str, str], /) -> bytes: ...
def unchanged_module_digests(
    *, metadata: bytes, known: dict[str, str]
) -> dict[str, str] | None: ...
def macro_store_token(
    *, loaded: LoadedMacro, identity: DeclarationIdentity | None
) -> str | None: ...
def context_store_token(
    *, macro_context: MacroContext, declarations: DeclarationResolutionContext | None
) -> str | None: ...
def mask_capture_noise(text: str) -> str: ...
def assemble_project(
    *,
    inputs: CompileInputsView,
    inference_profile: ExpressionInferenceProfile | None = None,
    skip_column_inference: bool = False,
    column_lineage_mode: ColumnLineageMode = ...,
    analysis_cache_dir: Path | None = None,
    analysis_model_names: frozenset[str] | None = None,
) -> CompiledProjectView: ...
def assembly_stage_inputs(
    stage: Literal["expression_shapes", "model_facts", "semantic_completion"],
    *,
    inputs: CompileInputsView,
    inference_profile: ExpressionInferenceProfile | None = None,
    skip_column_inference: bool = False,
    column_lineage_mode: ColumnLineageMode = ...,
    analysis_cache_dir: Path | None = None,
    analysis_model_names: frozenset[str] | None = None,
) -> dict[str, Any] | None: ...
def build_project_graph(
    *,
    discovered_inputs: DiscoveredProjectInputs,
    adapter: BaseAdapter,
    selected_target: str | None = None,
    no_sql_validation: bool = False,
    skip_column_inference: bool = False,
    column_lineage_mode: ColumnLineageMode = ...,
    cli_vars: dict[str, object] | None = None,
    external_sql_reference_resolver: ExternalSqlReferenceResolver | None = None,
    on_progress: Callable[[str], None] | None = None,
    no_cache: bool = False,
) -> ProjectGraph: ...
def discover_project_inputs(
    *,
    project_dir: Path,
    sql_analysis_enabled_override: bool | None = None,
    extract_output_column_locations: bool = True,
) -> DiscoveredProjectInputs: ...
def build_discovered_project_inputs(
    *,
    project_dir: Path,
    project_config: ProjectConfig,
    local_config: LocalConfig,
    sql_analysis_enabled: bool,
    extract_output_column_locations: bool = True,
) -> DiscoveredProjectInputs: ...
def validate_named_target_schema_strategy(
    *, discovered_inputs: DiscoveredProjectInputs, adapter: BaseAdapter, selected_target: str | None
) -> None: ...
def validate_managed_loader_target_isolation(
    *,
    discovered_inputs: DiscoveredProjectInputs,
    adapter: BaseAdapter,
    resolved_connection: dict[str, object] | None = None,
) -> None: ...
def validate_project_targets(*, adapter_name: str, project: CompiledProjectView) -> None: ...
def validate_managed_write_schemas(
    *, adapter: BaseAdapter, project: CompiledProjectView
) -> None: ...
def model_placeholders(model_input: ModelInput, /) -> dict[str, str] | None: ...
def plan_has_executable_work(
    *, plan: PlanOutput, python_plan_entries: tuple[PythonPlanEntry, ...] = ()
) -> bool: ...
def build_deferred_locations(
    *,
    project: CompiledProjectView,
    deferred_target_config: TargetConfig,
    effective_vars: dict[str, object],
    default_schema: str | None,
    default_database: str | None,
    render_qualified_name: Callable[..., str | None],
) -> dict[str, CompiledRelationLocation]: ...
def build_python_relation_targets(
    *,
    adapter: BaseAdapter,
    project: CompiledProjectView,
    plan_output: PlanOutput,
    required_refs: frozenset[SqlResourceRef] | None = None,
) -> dict[SqlResourceRef, str]: ...
def resolve_diff_model_names(
    *, project: CompiledProjectView, select: tuple[str, ...], exclude: tuple[str, ...]
) -> tuple[str, ...]: ...
def build_python_plan_entries(
    *,
    lifecycle_plan: PythonSqlRunLifecyclePlan,
    python_graph: PythonNodeGraph,
    previous_identities: dict[tuple[str, str], Fingerprint] | None = None,
) -> tuple[PythonPlanEntry, ...]: ...
def run_compile_pipeline(
    *,
    discovered_inputs: DiscoveredProjectInputs,
    adapter: BaseAdapter,
    options: CompilePipelineOptions | None = None,
    hooks: ConnectionHooks | None = None,
) -> CompilePipelineResult: ...
def build_compile_inputs(
    *,
    discovered_inputs: DiscoveredProjectInputs,
    adapter_context: CompileAdapterContext,
    selected_target: str | None = None,
    cli_vars: dict[str, object] | None = None,
    run_id: str | None = None,
    no_sql_validation: bool = False,
    defer_model_sql_validation: bool = False,
    external_sql_reference_resolver: ExternalSqlReferenceResolver | None = None,
    resolved_connection: dict[str, object] | None = None,
    no_cache: bool = False,
) -> CompileInputsView: ...
def defer_compile_teardown_to_exit() -> None: ...
def validate_hook_sql_syntax(
    value: object,
    hook_name: str,
    model: tuple[str, Path],
    parsing: tuple[dict[str, str] | None, str | None] = (None, None),
) -> None: ...
def validate_sql_syntax(
    query_sql: str,
    model: tuple[str, Path],
    parsing: tuple[dict[str, str] | None, str | None] = (None, None),
) -> None: ...
def analyze_resolved_column_reads(
    *, query_sql: str, references: Sequence[CompileSqlReference], dialect: str | None
) -> tuple[CompiledLineageSourceFact, ...]: ...
def build_effective_runtime_config(
    *,
    discovered_inputs: DiscoveredProjectInputs,
    selected_target: str | None = None,
    cli_vars: dict[str, object] | None = None,
) -> tuple[str | None, dict[str, object], str]: ...
def validate_discovered_project_inputs(
    inputs: DiscoveredProjectInputs, nodes: list[dict[str, object]], /
) -> None: ...
def discover_model_files(
    *,
    project_dir: Path,
    extract_implicit_alias_columns: bool,
    extract_output_column_locations: bool,
    selected_model_names: Set[str] | None = None,
    on_fault: Callable[[DiscoveryFileFault], None] | None = None,
) -> tuple[DiscoveredSqlModelFile, ...]: ...
def discover_test_files(
    *,
    project_dir: Path,
    selected_paths: Set[Path] | None = None,
    on_fault: Callable[[DiscoveryFileFault], None] | None = None,
) -> tuple[DiscoveredSqlTestFile, ...]: ...
def discover_scenario_files(
    *, project_dir: Path, on_fault: Callable[[DiscoveryFileFault], None] | None = None
) -> tuple[DiscoveredSqlScenarioFile, ...]: ...
def parse_sql_test_file(
    *, contents: str, file_path: Path
) -> tuple[DiscoveredSqlTestBlock, ...]: ...
def parse_sql_scenario_file(
    *, contents: str, file_path: Path, relative_path: Path
) -> DiscoveredSqlScenarioFile: ...
def discover_source_files(
    *, project_dir: Path, on_fault: Callable[[DiscoveryFileFault], None] | None = None
) -> tuple[DiscoveredSourceFile, ...]: ...
def discover_schema_files(*, project_dir: Path) -> tuple[DiscoveredSchemaFile, ...]: ...
def load_yaml_files(
    request: dict[str, object], relative_paths: list[str], tree: NativeProjectTree
) -> list[tuple[str, object] | Exception]: ...
def load_yaml_document(file_path: str, text: str, kind: str) -> object: ...
def _oracle_read_error(
    errno: int | None, winerror: int | None, message: str, *, file_path: Path
) -> Exception: ...
def discover_enum_files(
    *,
    project_dir: Path,
    on_fault: Callable[[DiscoveryFileFault], None] | None = None,
    isolate_declaration_kind: bool = False,
) -> tuple[DiscoveredEnumFile, ...]: ...
def discover_constant_files(
    *,
    project_dir: Path,
    on_fault: Callable[[DiscoveryFileFault], None] | None = None,
    isolate_declaration_kind: bool = False,
) -> tuple[DiscoveredConstantFile, ...]: ...
def discover_model_schema_files(
    *, project_dir: Path, on_fault: Callable[[DiscoveryFileFault], None] | None = None
) -> tuple[DiscoveredModelSchemaFile, ...]: ...
def discover_sql_function_files(
    *, project_dir: Path, on_fault: Callable[[DiscoveryFileFault], None] | None = None
) -> tuple[DiscoveredSqlFunctionFile, ...]: ...
def discover_sql_hook_files(
    *, project_dir: Path, on_fault: Callable[[DiscoveryFileFault], None] | None = None
) -> tuple[DiscoveredSqlHookFile, ...]: ...
def discover_audit_files(
    *, project_dir: Path, on_fault: Callable[[DiscoveryFileFault], None] | None = None
) -> tuple[DiscoveredAuditFile, ...]: ...
def discover_macro_files(
    *, project_dir: Path, isolate_declaration_kind: bool = False
) -> tuple[DiscoveredMacroFile, ...]: ...
def discover_seed_files(*, project_dir: Path) -> tuple[DiscoveredSeedFile, ...]: ...
def discover_scope_snapshot(*, project_dir: Path) -> TolerantScopeDiscovery: ...
def discover_python_module_files(
    *, project_dir: Path, root: str, public_only: bool
) -> tuple[Path, ...]: ...
def discover_python_function_files(
    *, project_dir: Path
) -> tuple[DiscoveredPythonFunctionFile, ...]: ...

class NativeDiscoverySession: ...

def parse_declaration_contents(
    request: dict[str, object], names: tuple[str, str], contents: str
) -> str: ...
def parse_model_contents(
    request: dict[str, object], contents: str
) -> tuple[dict[str, object], str, dict[str, SourceLocation]]: ...

class NativeDeclarationContexts:
    def __init__(
        self,
        positions: list[tuple[object, ...]],
        table: tuple[list[int], dict[str, list[int]], dict[str, list[int]]],
        classes: dict[str, object],
        lookups: tuple[dict[str, object], dict[str, object]],
    ) -> None: ...
    def resolve(
        self, resolver: object, file_path: object, resource: object | None = None
    ) -> tuple[Any, bool]: ...

def build_scope_index(
    *,
    discovered_inputs: DiscoveredProjectInputs,
    loaded_macros: Mapping[str, LoadedMacro] | None = None,
    static_macros: tuple[StaticMacroExport, ...] = (),
) -> ScopeIndex: ...
def get_placement_validated_scope_index(
    *,
    index: ScopeIndex,
    enforce_placement: bool = True,
    compile_usages: tuple[UsageRecord, ...] | None = None,
) -> ScopeIndex: ...
def relocate_declarations_for_move(
    *, index: ScopeIndex, resource: ResourceIdentity, destination: str
) -> tuple[DeclarationRecord, ...] | None: ...
def validate_scope_index(*, index: ScopeIndex) -> None: ...
def query_scope_target(*, lookup: ScopeLookup, target: ScopeQueryTarget) -> ScopeTargetQuery: ...
def resolve_scope_visibility(
    *, lookup: ScopeLookup, target: ResourceIdentity | str | PurePath
) -> VisibilityResolution: ...
def resolve_scope_declaration_visibility(
    *, lookup: ScopeLookup, target: ResourceIdentity | str | PurePath
) -> DeclarationVisibility: ...
def resolve_scope_path_visibility(
    *, lookup: ScopeLookup, path: str | PurePath
) -> tuple[tuple[DeclarationRecord, ...], tuple[DeclarationRecord, ...]]: ...
def build_scope_lookup(*, index: ScopeIndex) -> ScopeLookup: ...
def query_scope_report(
    *,
    lookup: ScopeLookup,
    target: str | PurePath | ResourceIdentity | DeclarationIdentity | None = None,
    at: str | PurePath | None = None,
    directory: bool = False,
    filters: ScopeReportFilters | None = None,
) -> ScopeReport: ...
def explain_scope_declaration(
    *,
    lookup: ScopeLookup,
    declaration: str | DeclarationIdentity,
    target: str | PurePath | ResourceIdentity | None = None,
    at: str | PurePath | None = None,
    directory: bool = False,
) -> tuple[DeclarationExplanation, tuple[ScopeDiagnostic, ...]]: ...
def browse_scope_folders(
    *,
    lookup: ScopeLookup,
    folder: str = ".",
    target: str | PurePath | ResourceIdentity | None = None,
    target_is_prospective: bool = False,
) -> ScopeBrowseResult: ...
def list_scope_declarations(
    *,
    lookup: ScopeLookup,
    folder: str,
    filters: ScopeReportFilters | None = None,
    target: str | PurePath | ResourceIdentity | None = None,
    target_is_prospective: bool = False,
) -> ScopeListResult: ...
def preview_scope_move(
    *, lookup: ScopeLookup, resource: str | ResourceIdentity, destination: str | PurePath
) -> tuple[MovePreview | None, tuple[ScopeDiagnostic, ...]]: ...
def serialize_scope_report(*, report: ScopeReport | ScopeBrowseResult | ScopeListResult) -> str: ...
def run_scope_command(
    *,
    request: ScopeCommandRequest,
    load_scope_index: ScopeIndexLoader | None = None,
    output_stream: TextIO | None = None,
) -> int: ...
def render_scope_result(
    *,
    result: ScopeReport | ScopeBrowseResult | ScopeListResult,
    request: ScopeCommandRequest,
    use_color: bool = False,
) -> str: ...
def scope_metadata_projection(*, index: ScopeIndex) -> dict[str, JsonValue]: ...
def yaml_entry_line(*, contents: str, name: str) -> int | None: ...

class CollectedCompileDiagnostics:
    def __enter__(self) -> CollectedCompileDiagnostics: ...
    def __exit__(self, *exception: object) -> bool: ...
    def add(self, *, key: tuple[str, ...], diagnostic: CompilerDiagnostic) -> None: ...
    @property
    def diagnostics(self) -> tuple[CompilerDiagnostic, ...]: ...

class CliStyle:
    use_color: bool
    def __init__(self, *, use_color: bool) -> None: ...
    def title(self, text: str) -> str: ...
    def section(self, text: str) -> str: ...
    def label(self, text: str) -> str: ...
    def value(self, text: str) -> str: ...
    def accent(self, text: str) -> str: ...
    def accent_strong(self, text: str) -> str: ...
    def plan_section(self, text: str) -> str: ...
    def object_name(self, text: str) -> str: ...
    def command(self, text: str) -> str: ...
    def muted(self, text: str) -> str: ...
    def success(self, text: str) -> str: ...
    def success_strong(self, text: str) -> str: ...
    def warning(self, text: str) -> str: ...
    def warning_strong(self, text: str) -> str: ...
    def error(self, text: str) -> str: ...
    def error_strong(self, text: str) -> str: ...
    def error_muted(self, text: str) -> str: ...
    def log_label(self, text: str) -> str: ...
    def dbt_section(self, text: str) -> str: ...
    def dbt_label(self, text: str) -> str: ...
    def dbt_object_name(self, text: str) -> str: ...
    def dbt_execution_label(self, text: str) -> str: ...
    def status(self, *, status: str, text: str | None = None) -> str: ...

def collect_compile_diagnostics() -> CollectedCompileDiagnostics: ...

class TransientLineCoordinator:
    lock: threading.RLock
    def __init__(self) -> None: ...
    def claim(self, *, stream: TextIO, owner: TransientLineOwner) -> None: ...
    def release(self, *, owner: TransientLineOwner) -> None: ...
    def clear_active(self) -> None: ...
    def write_persistent(self, *, stream: TextIO, text: str) -> None: ...

def shared_transient_line_coordinator() -> TransientLineCoordinator: ...

class BuildPhaseTimingTracker:
    compile_seconds: float | None
    planning_seconds: float | None
    connection_preparation_seconds: float | None
    schema_preparation_seconds: float | None
    execution_seconds: float | None
    cost_collection_seconds: float | None
    def __init__(self, *, monotonic: Callable[[], float] | None = None) -> None: ...
    def snapshot(self) -> PartialBuildPhaseTimings: ...
    @classmethod
    def current(cls) -> BuildPhaseTimingTracker | None: ...
    def scope(self) -> PhaseTimingScope: ...

class PhaseTimingScope:
    def __enter__(self) -> None: ...
    def __exit__(self, *exception: object) -> bool: ...

class ProcessResourceTracker:
    def __init__(
        self,
        *,
        monotonic: Callable[[], float] | None = None,
        resource_reader: Callable[[], tuple[float, float, int | None]] | None = None,
    ) -> None: ...
    def finish(self) -> ProcessResourceUsage: ...

def error_code(*, error: BaseException, fallback_code: str) -> str: ...
def error_help(error: BaseException, /) -> str | None: ...
def error_message(error: BaseException, /) -> str: ...
def setting_help(
    *, purpose: str, file_name: str, section: str, key: str, value: bool | int | str
) -> str: ...
def snippet_help(*, purpose: str, target: str, lines: tuple[str, ...]) -> str: ...
def open_connection_with_hooks(
    *, adapter: BaseAdapter, connection_config: dict[str, object], hooks: ConnectionHooks
) -> Any: ...

class CompileInputRecording:
    def __enter__(self) -> CompileInputReads: ...
    def __exit__(self, *exception: object) -> bool: ...

def record_compile_input_reads() -> CompileInputRecording: ...
def compiler_cache_directory(project_dir: Path, /) -> Path: ...
def project_relative_path(*, path: Path, project_dir: Path) -> Path: ...
def resolve_target_config(
    *, project_config: ProjectConfig, local_config: LocalConfig, target_name: str
) -> TargetConfig: ...
def resolve_adapter(*, adapter_name: str, project_dir: Path | None = None) -> BaseAdapter: ...

class CommandWarehouseScopeInstall:
    def __enter__(self) -> CommandWarehouseScope: ...
    def __exit__(self, *exc_info: object) -> bool: ...

def command_warehouse_scope(
    *,
    command: CliCommand | str | None,
    cli_warehouse: str | None,
    cli_vars: Mapping[str, object] | None = None,
) -> CommandWarehouseScopeInstall: ...
def current_command_warehouse_scope() -> CommandWarehouseScope: ...
def resolve_effective_adapter_name(
    *, project_config: ProjectConfig, local_config: LocalConfig
) -> str: ...
def resolve_effective_collection_rendering(
    *, project_config: ProjectConfig, declaration_override: CollectionRendering | None
) -> CollectionRendering: ...
def resolve_effective_scenario_config(
    *, project_config: ProjectConfig, local_config: LocalConfig
) -> ScenarioConfig: ...
def scenario_local_type_overrides_for_dialect(
    *, scenario_config: ScenarioConfig, sql_analysis_dialect: str | None
) -> dict[str, str]: ...
def run_clone_pipeline_with_options(
    *,
    discovered_inputs: DiscoveredProjectInputs,
    adapter: BaseAdapter,
    origin_target_name: str,
    destination_target_name: str,
    destination_connection: ClonePipelineConnection,
    options: ClonePipelineOptions,
    external_sql_reference_resolver: ExternalSqlReferenceResolver | None = None,
) -> ClonePipelineResult: ...
def loader_destination_parts(
    *, destination: str, default_database: str | None, default_schema: str | None
) -> LoaderDestinationParts: ...
def matching_dynamic_families(
    *, families: tuple[SchemaDynamicColumnFamily, ...], column_name: str
) -> tuple[SchemaDynamicColumnFamily, ...]: ...
def assert_no_unresolved_sql_markers(*, sql: str, context: str) -> None: ...
def normalize_sql_value(
    *,
    raw_value: object,
    context: str,
    explicit_type: str | None = None,
    limits: SqlValueLimits | None = None,
) -> SqlValue: ...
def render_state_sql_literal(
    *, value: object | None, declared_type: StateSqlValueType, timestamp_type: str | None = None
) -> str: ...
def format_diagnostics_console_record(record: logging.LogRecord, *, use_color: bool) -> str: ...
def interpreter_identity() -> tuple[str, str, str, str]: ...
def project_microbatch_coverage(
    *,
    events: tuple[MicrobatchEvent, ...],
    expected_intervals: tuple[MicrobatchInterval, ...],
    cursor_type: str,
) -> MicrobatchCoverageProjection: ...
def project_replay_requirement(
    *,
    requirement: MicrobatchEvent,
    current_model_version_hash: str,
    expected_intervals: tuple[MicrobatchInterval, ...],
    coverage: MicrobatchCoverageProjection,
    cursor_type: str,
) -> ReplayRequirementProjection: ...
def latest_active_replay_requirement(
    *, events: tuple[MicrobatchEvent, ...], current_model_version_hash: str
) -> MicrobatchEvent | None: ...
def deterministic_microbatch_event_id(
    *,
    scope: MicrobatchScope,
    record_type: MicrobatchRecordType,
    partition_start: str | None,
    partition_end: str | None,
    completion_reason: str,
) -> str: ...
def build_microbatch_insert_many_sql(
    *,
    events: tuple[MicrobatchEvent, ...],
    render_qualified_name: Callable[..., str | None],
    timestamp_type: str | None = None,
) -> str: ...
def build_microbatch_existing_event_ids_sql(
    *, events: tuple[MicrobatchEvent, ...], render_qualified_name: Callable[..., str | None]
) -> str: ...
def build_microbatch_read_scope_sql(
    *, scope: MicrobatchScope, render_qualified_name: Callable[..., str | None]
) -> str: ...
def build_microbatch_create_table_sql(
    *,
    database: str | None,
    schema: str,
    render_qualified_name: Callable[..., str | None],
    render_framework_type: Callable[[FrameworkType], str],
) -> str: ...
def build_microbatch_create_index_sqls(
    *,
    database: str | None,
    schema: str,
    render_identifier: Callable[[str], str],
    render_qualified_name: Callable[..., str | None],
) -> tuple[str, ...]: ...

CURRENT_LIFECYCLE_EVENT_SCHEMA_VERSION: int
CURRENT_DIAGNOSTIC_LOG_SCHEMA_VERSION: int
LIFECYCLE_EVENT_SCHEMA_VERSIONS: frozenset[int]
OPERATION_ADAPTERS: frozenset[str]
OPERATION_KINDS: frozenset[str]
OPERATION_NAMES: frozenset[str]
OPERATION_PHASES: frozenset[str]
OPERATION_SCOPES: frozenset[str]
OPERATION_STRATEGIES: frozenset[str]
OPERATION_TARGET_KINDS: frozenset[str]
RESOURCE_SKIP_CODES: frozenset[str]
RESOURCE_SKIP_MODES: frozenset[str]

def lifecycle_event_catalogued(*, schema_version: object, event_type: object) -> bool: ...
def lifecycle_event_terminal(*, schema_version: int, event_type: str) -> bool: ...
def validate_lifecycle_schema_version(*, value: object) -> None: ...
def freeze_lifecycle_json(*, value: object, path: str) -> JSONValue: ...
def validate_lifecycle_required_text(*, value: object, field_name: str) -> None: ...
def validate_lifecycle_optional_text(*, value: object, field_name: str) -> None: ...
def validate_lifecycle_event(*, event: LifecycleEvent) -> None: ...
def validate_lifecycle_invocation_sequence(*, value: object) -> None: ...
def validate_diagnostic_log(*, record: DiagnosticLog) -> None: ...
def validate_run_bounds(
    *, selected_count: int, configured_concurrency: int, worker_count: int
) -> None: ...
def validated_operation_metadata(*, metadata: Mapping[str, int] | None) -> dict[str, int]: ...
def validated_operation_hook_fields(
    *,
    hook_phase: str | None,
    hook_index: int | None,
    hook_type: str | None,
    hook_name: str | None,
) -> dict[str, JSONValue]: ...
def validate_lifecycle_producer(*, producer: str, producer_version: str | None) -> None: ...
def render_state_table_create_sql(
    *,
    qualified_name: str,
    columns: tuple[str, ...],
    column_types: Mapping[str, StateSqlValueType],
    required_columns: frozenset[str],
    render_framework_type: Callable[[FrameworkType], str],
) -> str: ...
def render_insert_source_freshness_records_sql(
    *,
    database: str | None,
    schema: str,
    records: tuple[SourceFreshnessRecord, ...],
    render_qualified_name: Callable[..., str | None],
) -> str: ...
def model_header_declarations(
    header: tuple[object | None, object | None], model_name: str, relative_path: Path, /
) -> tuple[tuple[object, ...], tuple[object, ...]]: ...
def semantic_help_catalogue() -> list[tuple[str, str]]: ...
def compute_query_hash(*, query_sql: str, dialect: str | None) -> str: ...
def compute_function_definition_hash(
    *, fingerprint_sql: str, language: str, dialect: str | None
) -> str: ...
def normalize_query_sql(query_sql: str, /) -> str: ...
def build_create_table_sql(
    *,
    database: str | None,
    schema: str,
    render_qualified_name: Callable[..., str | None],
    render_framework_type: Callable[[FrameworkType], str],
) -> str: ...
def build_read_latest_sql(
    *, database: str | None, schema: str, render_qualified_name: Callable[..., str | None]
) -> str: ...
def read_latest_fingerprints(
    *,
    connection: Any,
    execute: AdapterExecute[Any, Any],
    table_exists: bool,
    database: str | None,
    schema: str,
    render_qualified_name: Callable[..., str | None],
    render_read_latest_sql: Callable[..., str],
    require_table: bool = False,
    node_names: tuple[str, ...] | None = None,
    filtered_node_types: tuple[str, ...] = (),
) -> FingerprintSet: ...
def write_fingerprint(
    *,
    connection: Any,
    execute: AdapterExecute[Any, Any],
    database: str | None,
    schema: str,
    fingerprint: Fingerprint,
    render_qualified_name: Callable[..., str | None],
    render_framework_type: Callable[[FrameworkType], str],
    render_create_table_sql: Callable[..., str] | None = None,
    render_create_index_sqls: Callable[..., tuple[str, ...]] | None = None,
    ensure_table: bool = True,
) -> bool: ...
def write_fingerprints(
    *,
    connection: Any,
    execute: AdapterExecute[Any, Any],
    database: str | None,
    schema: str,
    fingerprints: Sequence[Fingerprint],
    render_qualified_name: Callable[..., str | None],
    render_framework_type: Callable[[FrameworkType], str],
    render_create_table_sql: Callable[..., str] | None = None,
    render_create_index_sqls: Callable[..., tuple[str, ...]] | None = None,
    on_progress: FingerprintWriteProgress | None = None,
) -> None: ...
def select_latest_fingerprints(
    *,
    fingerprint_set: FingerprintSet,
    node_names: tuple[str, ...],
    filtered_node_types: tuple[str, ...] = (),
) -> FingerprintSet: ...
def validate_resource_identity(
    *, name: str, kind: str, path: Path, private_identity: bool = False
) -> None: ...
def function_node_type(*, return_columns: tuple[object, ...]) -> str: ...
def load_project_config_file(project_dir: Path, /) -> ProjectConfig: ...
def load_local_config_file(project_dir: Path, /) -> LocalConfig: ...
def normalize_scope_path(*, path: str | PurePath) -> str: ...
def scope_index_fingerprint(*, project_dir: Path) -> str | None: ...
def load_or_build_scope_index(*, project_dir: Path, no_cache: bool = False) -> ScopeIndex: ...

class NativeConfigError:
    class_name: str
    message: str
    code: str | None
    help: str | None
    key: str | None
    def exception(self, values: Mapping[str, object] | None = None) -> Exception: ...

def parse_schema_columns(
    raw_columns: object,
    site: tuple[str, str],
    options: tuple[bool, bool, bool],
    column_locations: dict[str, SourceLocation],
) -> tuple[SchemaColumn, ...]: ...
def expand_template_data(
    *,
    value: object,
    variables: dict[str, object],
    context_values: dict[str, str | None],
    context_label: str,
    allow_context: bool,
    preserve_context_tokens: bool,
    preserve_unknown_context: bool,
) -> object: ...
def expand_effective_vars(raw_values: dict[str, object]) -> dict[str, object]: ...
def record_template_reads(reads: Sequence[tuple[str, str]]) -> None: ...
def resolve_run_id(*, selected_run_id: str | None) -> str: ...

class NativeModelConfigBuilder:
    def __init__(
        self,
        layers: tuple[DefaultsConfig, dict[str, dict[str, object]], tuple[type, ...]],
        sources: tuple[dict[str, object], object],
        target: tuple[tuple[str | None, str], tuple[str | None, str | None] | None],
    ) -> None: ...
    def model_config(
        self,
        header: dict[str, object],
        model: tuple[str | None, str],
        classes: dict[str, object],
    ) -> object | NativeConfigError: ...

class NativeModelValidator:
    def __init__(
        self,
        names: tuple[set[str], set[str], set[str], set[str], set[str]],
        custom_materializations: set[str],
        microbatch_concurrency: bool,
    ) -> None: ...
    @staticmethod
    def from_inputs(
        inputs: DiscoveredProjectInputs, microbatch_concurrency: bool
    ) -> NativeModelValidator: ...
    def model_names(self) -> set[str]: ...
    def validate(
        self,
        values: dict[str, object],
        model: tuple[str, str, str],
        facts: tuple[list[tuple[str, str, bool]], Sequence[str] | None, bool, bool],
    ) -> int | NativeConfigError | None: ...
    def references(
        self, model: tuple[str, str], references: list[tuple[str, str, bool]]
    ) -> int | NativeConfigError | None: ...

class SqlCodeScanner:
    def __init__(self, sql: str, syntax: dict[str, object] | None, context: str) -> None: ...
    def non_code_end(self, index: int) -> int | None: ...
    def matching_paren(self, open: int) -> int: ...

def scan_sql_reference_calls(*, sql: str, syntax: SqlLexicalSyntax) -> SqlReferenceScan: ...
def extract_sql_references(
    *, sql: str, syntax: SqlLexicalSyntax
) -> tuple[CompileSqlReference, ...]: ...

class SqlReferenceScanner:
    def __init__(self, syntax: dict[str, object]) -> None: ...
    def extract(
        self, sql: str
    ) -> (
        tuple[
            tuple[
                list[tuple[str, str, str | None, int | None]],
                list[tuple[str, str, int, str, str, str]],
            ],
            None,
        ]
        | tuple[None, tuple[str, int]]
    ): ...

def scope_expected_model_names(
    texts: list[tuple[str, str]], scenario: bool, syntax: dict[str, object]
) -> list[tuple[str | None, list[str]]]: ...
def supports_color() -> bool: ...
def parse_cli_invocation(argv: Sequence[str], *, use_color: bool) -> ParsedCliInvocation: ...
def run_cli(argv: Sequence[str] | None = None, handlers: object | None = None) -> int: ...
def termination_requested() -> bool: ...
def scope_test_ctes(
    texts: list[tuple[str, str]], syntax: dict[str, object]
) -> list[tuple[str | None, list[tuple[str, str]]]]: ...
def omitted_ceremonial_select(sql: str, syntax: dict[str, object]) -> int | None: ...
def extract_sql_tests_json(request_json: str) -> str: ...
def parse_row_diff_tolerances(
    *, raw: object, label: str = "row_diff_tolerances"
) -> RowDiffTolerances: ...
def parse_cli_tolerance_overrides(*, values: Sequence[str]) -> RowDiffTolerances: ...
def resolve_row_diff_sampling(
    *, raw_row_limit: object, raw_seed: object, override: RowDiffSamplingOverride, label: str
) -> RowDiffSampling | None: ...
def merge_row_diff_tolerances(
    *, base: RowDiffTolerances, override: RowDiffTolerances | None
) -> RowDiffTolerances: ...
def aggregate_build_result(
    *,
    model_results: tuple[ModelExecutionResult, ...],
    seed_results: tuple[SeedExecutionResult, ...],
    function_results: tuple[FunctionExecutionResult, ...],
    load_results: tuple[LoadExecutionResult, ...],
    test_results: tuple[SqlTestExecutionResult, ...],
    source_audit_results: tuple[AuditExecutionResult, ...],
    end_audit_results: tuple[AuditExecutionResult, ...],
) -> BuildExecutionResult: ...
def dagster_asset_plans(
    dag: Mapping[str, Any], /
) -> list[tuple[dict[str, Any], list[dict[str, Any]], set[str]]]: ...
def dagster_check_plans(
    dag: Mapping[str, Any], /, *, include_scenarios: bool, only_scenarios: bool
) -> list[tuple[Mapping[str, Any], bool, list[Mapping[str, Any]], str | None]]: ...
def dagster_selected_asset_nodes(
    dag: Mapping[str, Any], /, *, select: str, exclude: str | None = None
) -> list[dict[str, Any]]: ...
def rivers_asset_plans(
    dag: Mapping[str, Any], /
) -> list[tuple[dict[str, Any], list[Mapping[str, Any]]]]: ...
def load_sqlbuild_dag(
    dag: Mapping[str, Any] | str | Path, *, error_class: type[Exception]
) -> Mapping[str, Any]: ...
def build_python_node_graph(*, discovered_inputs: DiscoveredProjectInputs) -> PythonNodeGraph: ...
def inventory_sql_macros(
    sources: list[tuple[str, str, str]], /
) -> list[tuple[str, list[str], list[str]]]: ...
def macro_module_load_plan(
    modules: Sequence[tuple[str, str, str, Sequence[str]]],
) -> tuple[str, list[str], list[str]]: ...
def fit_artifact_logical_name(
    *, logical_name: str, fixed_prefix: str, identifier_limit: int, artifact_label: str
) -> str: ...
def fit_auxiliary_relation_name(*, base_name: str, suffix: str, identifier_limit: int) -> str: ...
def merge_microbatch_intervals(
    *, intervals: Sequence[MicrobatchInterval], cursor_type: str
) -> tuple[MicrobatchInterval, ...]: ...
def execution_limit_seconds(text: str) -> int | None: ...
def render_sql_test_comparison_sql(
    test_entries: Sequence[SqlTestPlanEntry],
    *,
    set_difference_operator: str = "EXCEPT",
    sql_analysis_dialect: str | None = None,
    probe_step_index: int | None = None,
    output_step_index: int | None = None,
    actual_relations: Mapping[int, str] | None = None,
) -> list[str]: ...
def render_sql_test_difference_sample_sql(
    *,
    test_entry: SqlTestPlanEntry,
    step: ChainStep,
    direction: str,
    sample_limit: int,
    set_difference_operator: str = "EXCEPT",
    sql_analysis_dialect: str | None = None,
    actual_relation: str | None = None,
) -> str: ...
def describe_missing_expected_columns(
    *, model_name: str, expected_columns: Sequence[str], available_columns: Iterable[str]
) -> str | None: ...
def build_scenario_expected_comparison_sql(
    *,
    actual_sql: str,
    expected_sql: str,
    set_difference_operator: str,
    compared_columns: tuple[str, ...] | None = None,
) -> str: ...
def sql_test_resource_id(
    *, test_name: str, source_path: PurePath | None, block_index: int | None, case_name: str | None
) -> str: ...
def audit_resource_id(
    *,
    audit_name: str,
    attachment_kind: AuditAttachmentKind,
    attached_target_kind: AttachedAuditTargetKind | None = None,
    attached_target_name: str | None,
    attached_column_name: str | None,
) -> str: ...
def validated_model_cursor_intrinsics(
    *, sql: str, config_values: Mapping[str, object], model_name: str
) -> str: ...
def cursor_intrinsics_analysis_sql(*, sql: str, cursor_type: object) -> str: ...
def resolve_cursor_intrinsics(
    *, sql: str, start_sql: str | None = None, end_sql: str | None = None
) -> tuple[str, bool]: ...
def scan_macro_call_sites(
    sql: str,
) -> tuple[
    list[tuple[int, int, int, str, list[str] | None, bool]],
    tuple[int | None, str | None, str] | None,
]: ...
def extract_relation_names(
    *, sql: str, dialect: str | None
) -> tuple[tuple[RelationName, ...], frozenset[str]] | None: ...
def match_project_relation(
    *,
    index: ProjectRelationIndex,
    relation: RelationName,
    default_database: str | None = None,
    default_schema: str | None = None,
) -> ProjectRelation | None: ...
def runtime_project_relations(
    *, relations: Mapping[SqlResourceRef, str], dialect: str | None
) -> ProjectRelationIndex: ...
def render_source_relation(*, entry: SourceEntry, adapter: StrictAdapter | None = None) -> str: ...
def render_source_read_relation(*, adapter: BaseAdapter, source_entry: SourceEntry) -> str: ...
def resolve_ref_references(
    *,
    query_sql: str,
    model_locations: dict[str, CompiledRelationLocation],
    seed_locations: dict[str, CompiledRelationLocation],
    cursor_bounds: CursorBounds | None,
    cursor_filter_inputs: dict[str, str],
    adapter: BaseAdapter,
    cursor_type: str | None,
    lower_bound_inclusive: bool,
) -> str: ...
def resolve_source_references(
    *,
    query_sql: str,
    source_map: dict[str, SourceEntry],
    source_warehouse_columns: dict[str, tuple[ColumnInfo, ...]],
    star_exclude_keyword: str,
    cursor_bounds: CursorBounds | None,
    cursor_filter_inputs: dict[str, str],
    adapter: BaseAdapter,
    cursor_type: str | None,
    lower_bound_inclusive: bool,
) -> str: ...
def resolve_dbt_ref_references(
    *,
    query_sql: str,
    external_sql_reference_resolver: ExternalSqlReferenceResolver | None = None,
) -> str: ...
def resolve_udf_references(
    *,
    query_sql: str,
    function_locations: dict[str, CompiledRelationLocation],
    adapter: BaseAdapter,
) -> str: ...
def resolve_table_function_references(
    *,
    query_sql: str,
    function_locations: dict[str, CompiledRelationLocation],
    adapter: BaseAdapter,
) -> str: ...
def apply_deferred_locations(
    *,
    model_locations: dict[str, CompiledRelationLocation],
    seed_locations: dict[str, CompiledRelationLocation],
    deferred_locations: dict[str, CompiledRelationLocation],
    selected_keys: frozenset[CompiledObjectKey],
) -> tuple[dict[str, CompiledRelationLocation], dict[str, CompiledRelationLocation]]: ...
def substitute_placeholder_defaults(sql: str, placeholders: list[tuple[str, str]], /) -> str: ...
def statement_metadata_effect(sql: str, /) -> StatementMetadataEffect: ...
def relation_name_key(name: str, /) -> str: ...
def qualified_relation_name_key(qualified: str, /) -> str | None: ...
def fixture_projection_facts(
    sql: str, dialect: str | None, /
) -> tuple[list[tuple[str, str]], list[str], list[str]]: ...
def build_python_sql_run_lifecycle(
    *, selection: PythonSqlRunSelection, python_graph: PythonNodeGraph
) -> PythonSqlRunLifecyclePlan: ...
def map_expanded_offset(
    *, offset: int, passes: Sequence[Sequence[ExpansionSpan]]
) -> MappedOffset: ...
def infer_fixture_column_facts(
    *, query_sql: str, inference_profile: ExpressionInferenceProfile
) -> FixtureColumnInference | None: ...
def fixture_inference_count(*, reset: bool = False) -> int: ...
def find_macro_call_names(sql: str, /) -> tuple[str, ...]: ...
def build_native_declaration_contexts(
    *,
    lookup: ScopeLookup,
    declarations: Mapping[DeclarationIdentity, EnumDeclaration | ConstantDeclaration | LoadedMacro],
) -> NativeDeclarationContexts: ...
def resolve_declaration_context(
    *,
    resolver: DeclarationScopeResolver,
    file_path: Path,
    resource: ResourceIdentity | None = None,
) -> DeclarationResolutionContext: ...
def substitute_sql_vars_with_spans(
    *,
    sql: str,
    file_path: Path,
    effective_vars: dict[str, object],
    context_values: Mapping[str, str | None] | None = None,
) -> tuple[str, tuple[ExpansionSpan, ...]]: ...
def expand_authored_sql_with_spans(
    *,
    sql: str,
    file_path: Path,
    effective_vars: dict[str, object],
    loaded_macros: dict[str, LoadedMacro],
    macro_context: MacroContext,
    value_renderer: TypedSqlValueRenderer,
    collection_rendering: CollectionRendering,
    context_values: Mapping[str, str | None] | None = None,
    declarations: DeclarationResolutionContext | None = None,
    declaration_resolver: DeclarationScopeResolver | None = None,
) -> tuple[str, tuple[tuple[ExpansionSpan, ...], ...]]: ...
def expand_sql_with_spans(
    *, sql: str, file_path: Path, context: SqlExpansionContext
) -> tuple[str, tuple[tuple[ExpansionSpan, ...], ...]]: ...
def expand_sql_macros(
    *,
    sql: str,
    file_path: Path,
    loaded_macros: dict[str, LoadedMacro],
    macro_context: MacroContext,
    macro_overrides: dict[str, str] | None = None,
    declaration_resolver: DeclarationScopeResolver | None = None,
    declarations: DeclarationResolutionContext | None = None,
    consumer: ResourceIdentity | DeclarationIdentity | None = None,
) -> MacroExpansionResult: ...
def inventory_macro_modules(
    sources: list[
        tuple[str, str, list[str], list[str] | None, list[str] | None, str, str, str | None]
    ],
    /,
) -> tuple[
    list[tuple[str, int, list[str], int, str, list[str]]],
    list[tuple[int, str]],
    list[tuple[int, str, list[str]]] | None,
]: ...

class MacroCallMemo:
    def __init__(self) -> None: ...
    def reset_call_classes(self, store_tokens: bool) -> None: ...
    def observe_modules(self, count: int | None) -> None: ...
    def stats(self) -> tuple[int, int, int]: ...
    def attach_store(self, path: str, environment: str) -> tuple[bytes, int]: ...
    def discard_store(self) -> None: ...
    def store_allowed(self) -> bool: ...
    def store_needs_save(self) -> bool: ...
    def store_stats(self) -> tuple[int, int]: ...
    def save_store(self, path: str, metadata: bytes) -> int | None: ...

class SqlTestScanStore:
    @staticmethod
    def open_in(
        cache_dir: str, code_identity: str
    ) -> tuple[SqlTestScanStore | None, str | None]: ...
    def counts(self) -> tuple[int, int]: ...
    def save(self) -> tuple[int | None, str | None]: ...

class UnicodeEnvironment(Mapping[str, str]):
    def __init__(self) -> None: ...
    def __getitem__(self, name: str) -> str: ...
    def __iter__(self) -> Iterator[str]: ...
    def __len__(self) -> int: ...

class SqlTestScanCache:
    def __init__(self, *, cache_dir: Path | None) -> None: ...
    def opened(self) -> SqlTestScanStore | None: ...
    def save(self) -> None: ...

SQL_TEST_SCAN_STORE_FILE_NAME: str
FORMAT_RESULT_STORE_FILE_NAME: str

def content_digest(parts: list[str]) -> str: ...
def compiled_code_identity() -> str: ...
def fingerprint_project_files(root: str, excluded_files: list[str]) -> str: ...
def digest_files(paths: list[str]) -> list[str | None]: ...

# Native analysis: type system.
def normalize_type(
    type_sql: str, dialect: str
) -> tuple[tuple[str, str, str | None, str | None, str | None], str | None]: ...
def infer_first_arg_nullability(
    args: Sequence[InferredNullability | str],
) -> InferredNullability: ...
def infer_conditional_result_nullability(
    args: Sequence[InferredNullability | str],
) -> InferredNullability: ...
def infer_safe_cast_nullability(
    args: Sequence[InferredNullability | str],
) -> InferredNullability: ...

# Native analysis: model analysis session.
type _AnalysisDiagnosticRow = tuple[str, str, int | None, int | None, int | None, int | None, str]

type _PivotContractRow = tuple[
    str,
    tuple[
        bool,
        list[tuple[str, str | None, str]],
        list[tuple[str, str | None]],
        list[str],
        str | None,
        bool,
    ]
    | None,
]

class NativeModelAnalysisSession:
    @property
    def fact_models(self) -> list[str]: ...
    def binding_rows(self) -> list[list[_AnalysisDiagnosticRow]]: ...
    def outcome_rows(self, index: int) -> tuple[Any, Any]: ...
    def prove_dynamic_contracts(
        self, models: list[tuple[str, list[tuple[str, str, str, str, str, str | None]]]], /
    ) -> list[_PivotContractRow]: ...
    @property
    def sharing(self) -> tuple[int, int]: ...
    @property
    def cache_stats(self) -> tuple[int, int, int, str | None] | None: ...

def session_outcome_analysis(
    *, session: Any, position: int, placeholders: dict[str, str] | None
) -> Any: ...
def dynamic_contract_proof(row: Any, /) -> Any: ...
def infer_native_query_columns(
    *,
    queries: Sequence[Any],
    profile: Any,
    column_types_by_table: Mapping[str, Mapping[str, str]] | None = None,
    column_nullability_by_table: Mapping[str, Mapping[str, Any]] | None = None,
) -> tuple[Any, ...]: ...
def infer_native_expression_source_shapes(
    *, expressions: tuple[str, ...], profile: Any
) -> tuple[dict[str, str] | None, ...]: ...
def prove_native_dynamic_contracts(
    *,
    session: NativeModelAnalysisSession | None,
    tables: Any,
    models: tuple[tuple[str, tuple[Any, ...]], ...],
) -> tuple[Any, ...]: ...
def _oracle_names_set_operation(sql: str, /) -> bool: ...
def _oracle_cte_fact_recovery(
    request: tuple[
        str,
        str,
        list[tuple[str, list[tuple[str, str]]]],
        list[tuple[str, str]],
        list[tuple[str, str]] | None,
        bool,
        bool,
    ],
    /,
) -> tuple[str | None, list[tuple[str, str]], list[tuple[str, str]], list[str], list[str]]: ...

# Native analysis: semantic completion.
def semantic_shapes(*, project: Any, profile: Any | None = None) -> dict[str, dict[str, str]]: ...
def source_binding_diagnostics(
    *, project: Any, columns: Mapping[str, Any], profile: Any, selected_keys: frozenset[Any]
) -> tuple[Any, ...]: ...
def complete_semantic_project(
    *,
    project: Any,
    profile: Any,
    binding_results: dict[
        str, list[tuple[str, str, int | None, int | None, int | None, int | None, str]]
    ],
    resource_sql_analysis: bool,
    session: NativeModelAnalysisSession | None = None,
) -> Any: ...
def semantic_metadata_diagnostics(
    *, project: Any, profile: Any, resource_sql_analysis: bool
) -> tuple[Any, ...]: ...

# Native analysis: contracts.
def build_audit_gate_identity(*, audits: tuple[AuditPlanEntry, ...]) -> AuditGateIdentity: ...
def evaluate_measurement(
    *,
    measured_value: float,
    sample_count: int | None,
    minimum_samples: int | None,
    thresholds: MeasurementThresholds,
) -> AuditOutcome: ...
def render_audit_sql(
    *,
    unresolved_sql: str,
    model_locations: Mapping[str, CompiledRelationLocation],
    seed_locations: Mapping[str, CompiledRelationLocation],
    source_map: Mapping[str, SourceEntry],
    adapter: BaseAdapter | None = None,
    relation_overrides: Mapping[str, str] | None = None,
) -> str: ...
def parse_audit_instances(
    *,
    raw_audits: object | None,
    file_path: Path,
    label: str,
    error_class: type[Exception],
    null_as_empty: bool,
) -> tuple[SchemaAuditInstance, ...]: ...
def retry_policy_exceptions(
    *,
    max_attempts: int,
    retry_on: type[BaseException] | Iterable[type[BaseException]],
    initial_delay_seconds: float,
    backoff_multiplier: float,
    max_delay_seconds: float | None,
    max_elapsed_seconds: float | None,
) -> tuple[type[BaseException], ...]: ...
def retry_delay_seconds(*, retry_policy: RetryPolicy, retry_index: int) -> float: ...
def validate_audit_case(
    *,
    name: str,
    definition: str,
    minimum_samples: int | None,
    evidence_limit: int | None,
    meta: object = ...,
) -> None: ...
def normalize_hook_reads(
    value: SqlResourceRef | tuple[SqlResourceRef, ...] | list[SqlResourceRef],
) -> tuple[SqlResourceRef, ...]: ...
def optional_named_string(
    *,
    raw_value: object | None,
    file_path: Path,
    label: str,
    key: str,
    error_class: type[Exception],
) -> str | None: ...
def optional_named_bool[D: (bool, None)](
    *,
    raw_value: object | None,
    file_path: Path,
    label: str,
    key: str,
    error_class: type[Exception],
    default: D,
) -> bool | D: ...
def compare_contract_evidence(
    *,
    item: ContractEvidence,
    physical_columns: tuple[ColumnInfo, ...] | None,
    adapter: BaseAdapter,
) -> ContractEvidence: ...
def edit_contract_model(
    *, model: CompiledModelView, physical_columns: tuple[ColumnInfo, ...], overwrite: bool
) -> tuple[str | None, str | None]: ...
def evaluate_model_contracts(
    *, project: CompiledProjectView, dialect: TypeDialect | str | None = None
) -> ContractValidationResult: ...
def promotion_conflict_diagnostics(
    *, project: CompiledProjectView, adapter_default: TablePromotionMode, settings_file: str
) -> tuple[CompilerDiagnostic, ...]: ...
def build_project_column_lineage(
    project: CompiledProjectView,
    dialect: str | None,
    mode: ColumnLineageMode,
    model_names: frozenset[str] | None,
    /,
) -> ProjectColumnLineage | None: ...

class ProjectColumnLineage:
    """Project-level column lineage graph with lazily materialized public views."""

    def __init__(
        self,
        *,
        models: dict[str, ModelColumnLineage],
        edges: tuple[ColumnLineageEdge, ...],
    ) -> None: ...
    @classmethod
    def from_fast_facts(
        cls,
        *,
        models: dict[str, ModelColumnLineage],
        compact_models: dict[str, tuple[Sequence[CompiledLineageColumnFact], bool]],
        model_order: tuple[str, ...],
    ) -> ProjectColumnLineage: ...
    @property
    def models(self) -> dict[str, ModelColumnLineage]: ...
    @property
    def edges(self) -> tuple[ColumnLineageEdge, ...]: ...
    def has_model(self, model_name: str, /) -> bool: ...
    def model_has_star(self, model_name: str, /) -> bool: ...
    def edge_count_targeting(self, model_name: str, /) -> int: ...
    def edges_targeting(self, model_name: str, /) -> tuple[ColumnLineageEdge, ...]: ...
    def edges_sourced_from(self, resource_name: str, /) -> tuple[ColumnLineageEdge, ...]: ...
    def column_consumers(
        self, *, resource_name: str, column_name: str
    ) -> tuple[ColumnLineageEdge, ...]: ...

def lineage_cache_fingerprint(
    *, project_dir: Path, cli_vars: dict[str, object] | None
) -> str | None: ...
def run_refactor_command(*, request: RefactorCommandRequest) -> int: ...
def run_load_command(*, request: LoadCommandRequest) -> int: ...
def write_check_results(
    *,
    stream: TextIO,
    results: tuple[PythonCheckExecutionResult, ...],
    use_color: bool,
    check_functions: tuple[DiscoveredCheckFunction, ...] = (),
    python_graph: PythonNodeGraph | None = None,
) -> None: ...
def format_check_json(*, results: tuple[PythonCheckExecutionResult, ...]) -> str: ...
def run_test_command(*, request: TestCommandRequest) -> int: ...
def run_build_command(*, request: BuildCommandRequest) -> int: ...
def enforce_plan_safety_policies(
    *,
    plan: PlanOutput,
    gates: PlanSafetyGates,
    snapshots_config: SnapshotsConfig,
    target_name: str | None,
    execution_limits: ExecutionLimitsConfig,
    streams: tuple[TextIO, TextIO],
) -> None: ...
def enforce_model_execution_limit(
    *,
    model_count: int,
    target_name: str | None,
    limits: ExecutionLimitsConfig,
    refusal_note: str,
) -> None: ...
def run_audit_command(*, request: AuditCommandRequest) -> int: ...
def run_init_command(project_dir: Path | None = None) -> int: ...
def run_cost_command(request: CostCommandRequest) -> int: ...
def run_dbt_command(
    *, command: DbtInteropCommand, project_dir: Path | None, args: tuple[str, ...], no_color: bool
) -> int: ...
def run_dbt_init_command(request: DbtInitCommandRequest) -> int: ...
def refresh_pending_cost_run(request: CostCommandRequest) -> None: ...
def parse_cost_since_bound(*, value: str | None) -> datetime | None: ...
def run_query_command(
    *,
    project_dir: Path | None,
    sql: str | None,
    query_file: Path | None = None,
    selected_target: str | None = None,
    output_format: str = "long",
    limit: int | None = 20,
) -> int: ...
def render_query_result(
    *, result: QueryResult, output_format: str | None, limit: int | None
) -> str: ...
def run_dag_command(
    *,
    project_dir: Path | None,
    no_sql_validation: bool = False,
    json_output: bool = False,
    cli_vars: dict[str, object] | None = None,
) -> int: ...
def plan_scenario_graph(
    *,
    scenario: CompiledSqlScenario,
    project: CompiledProjectView,
    sql_analysis_enabled: bool = True,
    sql_analysis_dialect: str | None = None,
) -> tuple[ScenarioGraphPlan, tuple[PlanWarning, ...]]: ...
def build_scenario_relation_plan(
    *,
    project: CompiledProjectView,
    graph_plan: ScenarioGraphPlan,
    relation_map: ScenarioRelationMap,
    render_qualified_name: Callable[..., str | None],
    database: str | None = None,
    schema: str | None = None,
) -> ScenarioRelationPlan: ...
def build_cli_scenario_plan(
    *,
    scenario: CompiledSqlScenario,
    pipeline_result: CompilePipelineResult,
    adapter: BaseAdapter,
    project_name: str,
    source_lexical_syntax: SqlLexicalSyntax,
) -> ScenarioExecutionPlan: ...
def run_scenario_test_command(*, request: ScenarioTestCommandRequest) -> int: ...
def compute_scenario_hash_prefix(
    *,
    project_name: str,
    scenario_name: str,
    run_namespace: str | None = None,
    prefix_length: int = 12,
) -> str: ...
def build_scenario_hash_index(
    *,
    project_name: str,
    scenarios: tuple[CompiledSqlScenario, ...],
    run_namespace: str | None = None,
    prefix_length: int = 12,
) -> dict[str, str]: ...
def build_scenario_artifact_name(
    *, hash_prefix: str, kind: str, logical_name: str, identifier_limit: int = 63
) -> str: ...
def parse_scenario_artifact_physical_name(name: str) -> ParsedScenarioArtifactName | None: ...
def is_scenario_artifact_physical_name(name: str) -> bool: ...
def build_scenario_relation_map(
    *,
    scenario_name: str,
    hash_prefix: str,
    artifacts: tuple[ScenarioArtifactIdentity, ...],
    identifier_limit: int = 63,
    normalize_identifier: Callable[[str], str] | None = None,
) -> ScenarioRelationMap: ...
def resolve_scenario_check_sql(
    *,
    sql: str,
    relation_plan: ScenarioRelationPlan,
    adapter: BaseAdapter,
    source_label: str,
    source_lexical_syntax: SqlLexicalSyntax,
) -> str: ...
def build_scenario_fixture_plans(
    *,
    scenario: CompiledSqlScenario,
    graph_plan: ScenarioGraphPlan,
    relation_plan: ScenarioRelationPlan,
    adapter: BaseAdapter,
    source_lexical_syntax: SqlLexicalSyntax,
    fixture_sql_overrides: dict[tuple[CompiledResourceType, str], str] | None = None,
) -> tuple[ScenarioFixturePlan, ...]: ...
def build_scenario_execution_plan(
    *,
    scenario: CompiledSqlScenario,
    project: CompiledProjectView,
    adapter: BaseAdapter,
    graph_plan: ScenarioGraphPlan,
    relation_plan: ScenarioRelationPlan,
    source_lexical_syntax: SqlLexicalSyntax,
    snapshot: WarehouseSnapshot | None = None,
    source_warehouse_columns: dict[str, tuple[ColumnInfo, ...]] | None = None,
    sql_analysis_enabled: bool = True,
) -> tuple[ScenarioExecutionPlan, tuple[PlanWarning, ...]]: ...
def serialize_hook_entries(
    *,
    value: object,
    sql_fields: tuple[str, ...],
    python_hook_fields: Mapping[str, Mapping[str, object]] | None = None,
) -> list[dict[str, object]]: ...
def discover_model_description_inputs(*, project_dir: Path) -> DiscoveredProjectInputs: ...
def microbatch_guidance() -> tuple[str, str]: ...
def sql_analysis_opt_out_help(*, model_owned: bool = True) -> str: ...
def explicit_references_help(*, allowed: str) -> str: ...
def format_parameterized_test_label(
    *,
    name: str,
    source_path: Path | None,
    parameter_schema: tuple[SqlTestParameterDeclaration, ...],
    parameter_values: tuple[tuple[str, SqlValue], ...],
) -> str: ...
def resolve_test_name_width(test_entries: tuple[SqlTestPlanEntry, ...]) -> int: ...
def build_test_expectation_rows(
    result: SqlTestExecutionResult,
) -> tuple[NestedProgressChildRow, ...]: ...
def select_load_entries(
    *,
    discovered_inputs: DiscoveredProjectInputs,
    select: tuple[str, ...],
    exclude: tuple[str, ...],
    target_config: TargetConfig | None,
    loader_default_database: str | None = None,
    loader_default_schema: str | None = None,
) -> tuple[SourceEntry, ...]: ...
def select_load_reference_entries(
    *,
    discovered_inputs: DiscoveredProjectInputs,
    selected_sources: tuple[SourceEntry, ...],
    target_config: TargetConfig | None,
    loader_default_database: str | None = None,
    loader_default_schema: str | None = None,
) -> tuple[SourceEntry, ...]: ...
def resolve_loader_defaults(
    *,
    discovered_inputs: DiscoveredProjectInputs,
    selected_target: str | None = None,
    target_config: TargetConfig | None = None,
    cli_vars: dict[str, object] | None = None,
) -> tuple[str | None, str | None]: ...
def write_runtime_target(
    *, target_dir: Path, plan_output: PlanOutput, result: BuildExecutionResult
) -> None: ...
def run_plan_command(*, request: PlanCommandRequest) -> int: ...

class TransientStatusReporter:
    def __init__(
        self, *, stream: TextIO, use_color: bool = False, enabled: bool = True
    ) -> None: ...
    def start(self, message: str) -> None: ...
    def update(self, message: str) -> None: ...
    def complete(self, *, message: str, blank_line_after: bool = False) -> None: ...
    def complete_styled(self, *, message: str, blank_line_after: bool = False) -> None: ...
    def error(self, message: str) -> None: ...
    def close(self) -> None: ...
    def clear_transient_line(self) -> None: ...
    def redraw_transient_line(self) -> None: ...
    def report_preflight_progress(self, message: str) -> None: ...
    def write_blank_line(self) -> None: ...

class LoadProgressReporter:
    def __init__(
        self, *, stream: TextIO, use_color: bool, source_order: dict[str, int], total_count: int
    ) -> None: ...
    def on_start(self, source: SourceEntry) -> None: ...
    def on_progress(self, source: SourceEntry, message: str) -> None: ...
    def on_complete(self, result: LoadExecutionResult) -> None: ...
    def clear_transient_line(self) -> None: ...
    def redraw_transient_line(self) -> None: ...

class BuildProgressCallbacks:
    def __init__(
        self,
        *,
        plan: PlanOutput,
        use_color: bool,
        verbose: bool = False,
        debug: bool = False,
        event_output_path: Path | None = None,
    ) -> None: ...
    @property
    def elapsed(self) -> float: ...
    def on_node_start(self, *, name: str, resource_kind: ExecutionResourceKind) -> None: ...
    def on_sub_progress(self, message: str) -> None: ...
    def on_batch_complete(self, progress: MicrobatchBatchProgress) -> None: ...
    def on_scheduler_state(self, state: SchedulerState) -> None: ...
    def on_statement_complete(self, telemetry: StatementExecutionTelemetry) -> None: ...
    def clear_transient_line(self) -> None: ...
    def redraw_transient_line(self) -> None: ...
    def on_node_complete(self, node_result: object) -> None: ...
    def write_execution_event(self, result: object) -> None: ...
    def close(self) -> None: ...

class AuditProgressReporter:
    def __init__(
        self,
        *,
        entries: tuple[AuditPlanEntry, ...],
        worker_limit: int,
        stream: TextIO,
        use_color: bool,
    ) -> None: ...
    def set_result_callback(self, callback: Callable[[AuditExecutionResult], None]) -> None: ...
    def on_item_start(self, entry: AuditPlanEntry) -> None: ...
    def on_item_complete(self, result: AuditExecutionResult) -> None: ...
    def on_item_physical_complete(self, result: AuditExecutionResult) -> None: ...
    def on_item_error(self, entry: AuditPlanEntry) -> None: ...
    def close(self) -> None: ...
    def clear_transient_line(self) -> None: ...
    def redraw_transient_line(self) -> None: ...

class NestedCommandProgressCallbacks:
    def __init__(
        self,
        *,
        total: int,
        label: str,
        stream: TextIO,
        use_color: bool,
        name_width: int = 50,
        concurrency: int = 1,
    ) -> None: ...
    def on_item_start(
        self, *, group_name: str, item_name: str, canonical_resource_name: str | None = None
    ) -> None: ...
    def on_item_complete(
        self,
        *,
        group_name: str,
        item_name: str,
        status_text: str,
        detail: str = "",
        error_code: str | None = None,
        error_help: str | None = None,
        error_message: str | None = None,
        child_rows: tuple[NestedProgressChildRow, ...] = (),
        canonical_resource_name: str | None = None,
        canonical_resource_id: str | None = None,
    ) -> None: ...
    def clear_transient_line(self) -> None: ...
    def redraw_transient_line(self) -> None: ...

def observe_source_freshness_json(
    *,
    adapter: object,
    connection: object,
    sources: Sequence[SourceEntry],
    observed_at: datetime,
    select: Sequence[str] = (),
    exclude: Sequence[str] = (),
    run_id: str | None = None,
    previous_records: Mapping[SourceFreshnessIdentity, SourceFreshnessRecord] | None = None,
    previous_records_by_source_name: Mapping[str, SourceFreshnessRecord] | None = None,
) -> str: ...
def render_janitor_plan(
    *, plan: JanitorPlan, use_color: bool = False, verbose: bool = False
) -> str: ...
def discover_python_node_functions(
    *, project_dir: Path, providers: tuple[DiscoveredProvider, ...] | None = None
) -> DiscoveredPythonNodeFunctions: ...
def discover_hook_functions(
    *, project_dir: Path, providers: tuple[DiscoveredProvider, ...] | None = None
) -> tuple[DiscoveredHookFunction, ...]: ...
def discover_materialization_files(
    *, project_dir: Path, providers: tuple[DiscoveredProvider, ...] | None = None
) -> tuple[DiscoveredMaterializationFile, ...]: ...
def discover_provider_classes(*, project_dir: Path) -> tuple[DiscoveredProvider, ...]: ...
def discover_event_exporter_declarations(
    *, project_dir: Path
) -> tuple[DiscoveredEventExporterDeclaration, ...]: ...
def bind_event_exporter_declarations(
    *,
    declarations: tuple[DiscoveredEventExporterDeclaration, ...],
    providers: tuple[DiscoveredProvider, ...],
    project_dir: Path,
) -> tuple[DiscoveredEventExporter, ...]: ...
def discover_event_exporter_functions(
    *, project_dir: Path, providers: tuple[DiscoveredProvider, ...] | None = None
) -> tuple[DiscoveredEventExporter, ...]: ...
def dagster_cli_arguments(
    *,
    args: list[str],
    dag: Mapping[str, Any] | None,
    has_context: bool,
    read_selection: Callable[[], tuple[list[tuple[str, ...]], list[tuple[tuple[str, ...], str]]]],
) -> dict[str, Any]: ...
def dagster_selector_log_lines(*, selectors: list[str]) -> list[str]: ...
def dagster_payload_results(
    *,
    dag: Mapping[str, Any],
    payload: Mapping[str, Any],
    command: list[str],
    selected_asset_paths: list[tuple[str, ...]],
    selected_check_keys: list[tuple[tuple[str, ...], str]],
    check_selection_is_explicit: bool,
) -> list[tuple[Any, ...]]: ...
def dagster_integration_results(
    *,
    dag: Mapping[str, Any],
    envelope: IntegrationResultEnvelope,
    command: list[str],
    selected_asset_paths: list[tuple[str, ...]],
    selected_check_keys: list[tuple[tuple[str, ...], str]],
    check_selection_is_explicit: bool,
    emitted_asset_paths: list[tuple[str, ...]],
) -> list[tuple[Any, ...]]: ...
def dagster_selected_asset_results(
    *, dag: Mapping[str, Any], command: list[str], selected_asset_paths: list[tuple[str, ...]]
) -> list[tuple[Any, ...]]: ...
def dagster_execution_payload(*, text: str) -> Mapping[str, Any] | None: ...
def dagster_failure(
    *,
    command: list[str],
    project_dir: Path,
    selection: list[str],
    selector_file: str,
    returncode: int,
    execution_payload: Mapping[str, Any] | None,
    stdout: str,
    stderr: str,
) -> tuple[str, dict[str, Any]]: ...
def dagster_live_failure(
    *, asset: IntegrationAssetResult, envelope: IntegrationResultEnvelope, command: list[str]
) -> tuple[list[str], dict[str, Any]]: ...
def dagster_default_asset_key_path(node: Mapping[str, Any]) -> list[str]: ...
def dagster_default_group_name(node: Mapping[str, Any]) -> str: ...
def dagster_default_tags(node: Mapping[str, Any]) -> dict[str, str]: ...
def dagster_default_metadata(node: Mapping[str, Any]) -> dict[str, Any]: ...
def dagster_default_description(node: Mapping[str, Any]) -> str: ...
def dagster_default_check_name(check: Mapping[str, Any]) -> str: ...
def dagster_default_check_metadata(check: Mapping[str, Any]) -> dict[str, Any]: ...
def build_janitor_plan(
    *,
    project: CompiledProjectView,
    adapter: BaseAdapter,
    connection: Any,
    retention_days: int,
    delete_tracked_only: bool = True,
    exclude_patterns: Sequence[str] = (),
    direct_mode: bool = True,
    state_history_versions: int = 20,
    archive_retention_days: int = 14,
    early_old_name_view_drops: Sequence[str] = (),
    scan_relation_keys: Iterable[JanitorRelationKey] | None = None,
    protected_relation_keys: Iterable[JanitorRelationKey] | None = None,
    protected_relation_reasons: Mapping[JanitorRelationKey, str] | None = None,
) -> JanitorPlan: ...
def build_janitor_archive_name(
    *, original_name: str, archived_at: datetime, identifier_limit: int, kind: str | None = None
) -> str: ...
def build_query_diff_artifact_name(*, run_id: str, side: str) -> str: ...
def parse_query_diff_artifact_name(name: str, /) -> tuple[str, str] | None: ...
def parse_review_schema_name(name: str, /) -> datetime | None: ...
def render_janitor_completion(
    *,
    result: JanitorExecutionResult,
    elapsed_seconds: float | None = None,
    use_color: bool = False,
) -> str: ...

# Native analysis: SQL test planning glue.
def assemble_native_sql_tests(
    *, inputs: CompileInputsView
) -> tuple[NativeSqlTestAssembly, ...]: ...

# Native analysis: compiled project assembly.
def assemble_native_project_resources(
    *,
    inputs: CompileInputsView,
    dialect: str | None,
    syntax_checks: tuple[tuple[tuple[str, dict[str, str] | None], ...], ...],
) -> NativeProjectResources: ...
def hard_coded_relation_remedy(
    *,
    owner_kind: HardCodedRelationOwnerKind,
    ref: SqlResourceRef,
    upstream_loader_by_source: Mapping[str, str] | None = None,
) -> str: ...
def transitive_closure[K](
    *, start: K, edges: Mapping[K, Iterable[K]], max_depth: int | None = None
) -> frozenset[K]: ...
def transitive_closure_many[K](
    *,
    starts: Iterable[K],
    edges: Mapping[K, Iterable[K]],
    include_starts: bool,
    max_depth: int | None = None,
) -> frozenset[K]: ...
def invert_edges[K](
    *, edges: Mapping[K, Iterable[K]], sort_key: Callable[[K], Any] | None = None
) -> dict[K, tuple[K, ...]]: ...
def path_nodes[K](
    *, start: K, end: K, downstream: Mapping[K, Iterable[K]]
) -> frozenset[K] | None: ...
def resolve_python_node_selection(
    *, graph: PythonNodeGraph, select: Sequence[str], exclude: Sequence[str]
) -> frozenset[str]: ...
def resolve_python_sql_run_selection(
    *,
    project_graph: ProjectGraph,
    python_graph: PythonNodeGraph,
    select: Sequence[str],
    exclude: Sequence[str],
) -> tuple[list[tuple[str, str]], list[str]]: ...
def compiled_graph_keys(keys: Iterable[tuple[str, str]], /) -> frozenset[CompiledObjectKey]: ...
def project_lineage_views(project: CompiledProjectView, /) -> LineageGraphViews: ...
def native_graph_from_views(
    *,
    all_keys: Mapping[str, CompiledObjectKey],
    upstream: Mapping[CompiledObjectKey, Iterable[CompiledObjectKey]],
    downstream: Mapping[CompiledObjectKey, Iterable[CompiledObjectKey]],
    tag_index: Mapping[str, Iterable[CompiledObjectKey]],
    path_index: Mapping[CompiledObjectKey, str],
) -> NativeProjectGraph: ...

class BindingCatalog:
    schemas: dict[str, Mapping[str, str]]
    dialect: str
    quoted_ignore_case: bool
    analysis_shapes: dict[str, tuple[Mapping[str, str], Mapping[str, str]]]
    expression_shapes: dict[str, dict[str, str] | None]
    native: NativeProjectCatalog
    def __init__(
        self,
        *,
        dialect: str,
        quoted_ignore_case: bool,
        known_functions: Sequence[str],
        known_types: Sequence[str],
        relations: Mapping[str, Mapping[str, str]],
    ) -> None: ...
    def prepare_analysis(
        self,
        *,
        types: Mapping[str, Mapping[str, str]],
        nullability: Mapping[str, Mapping[str, str]],
    ) -> None: ...
    def with_relations(self, relations: Mapping[str, Mapping[str, str]]) -> BindingCatalog: ...
    def inferred_schema(
        self, *, sql: str, columns: dict[str, str], inputs: Mapping[str, Mapping[str, str]]
    ) -> dict[str, str]: ...

def schema_validations(
    *, requests: Sequence[SqlSchemaValidationRequest]
) -> tuple[SqlBindingResult, ...]: ...
def _oracle_json_dumps(dialect_json: str, value_json: str) -> str: ...
def _oracle_text_positions(
    data: bytes, byte_offsets: list[int]
) -> tuple[str, list[tuple[int, int, int] | None]]: ...
def _oracle_yaml_load(text: str) -> str: ...
def _oracle_toml_load(text: str) -> str: ...

# Discovery: header values and authored locations.
def deterministic_migration_event_id(
    *,
    run_id: str,
    target_name: str | None,
    origin: MigrationRelation,
    destination: MigrationRelation,
) -> str: ...
def deterministic_column_migration_event_id(
    *,
    run_id: str,
    target_name: str | None,
    relation: MigrationRelation,
    origin_column: str,
    destination_column: str,
) -> str: ...
def deterministic_old_name_view_event_id(
    *, event_type: OldNameViewEventType, migration_event_id: str
) -> str: ...
def render_migration_state_create_sql(
    *,
    kind: Literal["model", "column", "old_name"],
    database: str | None,
    schema: str,
    render_qualified_name: Callable[..., str | None],
    render_framework_type: Callable[[FrameworkType], str],
) -> str: ...
def read_migration_state_events(
    *,
    kind: Literal["model", "column", "old_name"],
    connection: Any,
    execute: AdapterExecute[Any, Any],
    database: str | None,
    schema: str,
    render_qualified_name: Callable[..., str | None],
    stored_columns: Iterable[str] | None = None,
) -> tuple[Any, ...]: ...
def read_recorded_column_renames(
    *,
    connection: Any,
    execute: AdapterExecute[Any, Any],
    relation: MigrationRelation,
    since: datetime,
    render_qualified_name: Callable[..., str | None],
) -> tuple[ColumnMigrationEvent, ...]: ...
def stored_old_name_view_columns(
    *, adapter: BaseAdapter, connection: Any, database: str | None, schema: str
) -> tuple[str, ...]: ...
def write_migration_state_event(
    *,
    kind: Literal["model", "column", "old_name"],
    connection: Any,
    execute: AdapterExecute[Any, Any],
    event: MigrationEvent | ColumnMigrationEvent | OldNameViewEvent,
    render_qualified_name: Callable[..., str | None],
    create_table_sql: str | None,
    attempts: int = 3,
) -> None: ...

class MigrationStateInspection:
    """Relations, fingerprints, and migration events for one planning database."""

    def __init__(
        self,
        *,
        adapter: BaseAdapter,
        connection: Any,
        database: str | None,
        known_fingerprints: WarehouseFingerprints | None = None,
    ) -> None: ...
    @property
    def database(self) -> str | None: ...
    @property
    def events(self) -> tuple[MigrationEvent, ...]: ...
    @property
    def column_events(self) -> tuple[ColumnMigrationEvent, ...]: ...
    @property
    def old_name_histories(self) -> tuple[OldNameViewHistory, ...]: ...
    def inspect_column_events(self, *, schemas: set[str]) -> None: ...
    def inspect_old_name_views(self, *, schemas: set[str]) -> None: ...
    def inspect_migration_events(self, *, schemas: set[str]) -> None: ...
    def inspect_schemas(self, *, schemas: set[str]) -> None: ...
    def inspect_relations(self, *, locations: tuple[CompiledRelationLocation, ...]) -> None: ...
    def relation(self, location: CompiledRelationLocation) -> RelationInfo | None: ...
    def relation_columns(self, location: CompiledRelationLocation) -> tuple[ColumnInfo, ...]: ...
    def model_fingerprint(
        self, *, location: CompiledRelationLocation, model_name: str | None
    ) -> Fingerprint | None: ...
    def named_fingerprint(self, *, model_name: str) -> Fingerprint | None: ...
    def orphan_fingerprints(
        self, *, project_model_names: frozenset[str]
    ) -> tuple[Fingerprint, ...]: ...

def newest_migration_event_mentioning(
    *, events: Iterable[MigrationEvent], relation: MigrationRelation
) -> MigrationEvent | None: ...
def newest_column_migration_event_mentioning(
    *,
    events: Iterable[ColumnMigrationEvent],
    relation: MigrationRelation,
    origin_column: str,
    destination_column: str,
) -> ColumnMigrationEvent | None: ...
def project_old_name_views(
    *, moves: Iterable[MigrationEvent], facts: Iterable[OldNameViewEvent]
) -> tuple[OldNameViewHistory, ...]: ...
def source_freshness_data_version_hash(
    *,
    source_name: str,
    strategy: SourceFreshnessStrategy | str,
    value_kind: SourceFreshnessValueKind | str,
    data_version: str,
) -> str: ...
def normalize_source_freshness_data_version(
    *, value: object, value_kind: SourceFreshnessValueKind
) -> str: ...
def source_freshness_records_equivalent(
    *,
    previous_record: SourceFreshnessComparableRecord,
    current_record: SourceFreshnessComparableRecord,
    lag_tolerance: str | None = None,
) -> bool: ...
def evaluate_source_freshness_age_policy(
    *,
    policy: SourceFreshnessAgePolicy | None,
    data_version: object,
    observed_at: datetime,
) -> SourceFreshnessAgeStatus | None: ...
def build_source_freshness_read_latest_sql(
    *, database: str | None, schema: str, render_qualified_name: Callable[..., str | None]
) -> str: ...
def build_source_freshness_create_table_sql(
    *,
    database: str | None,
    schema: str,
    render_qualified_name: Callable[..., str | None],
    render_framework_type: Callable[[FrameworkType], str],
) -> str: ...
def read_latest_source_freshness(
    *,
    connection: object,
    execute: AdapterExecute[Any, Any],
    table_exists: bool,
    database: str | None,
    schema: str,
    render_qualified_name: Callable[..., str | None],
    render_read_latest_sql: Callable[..., str],
    source_names: tuple[str, ...] | None = None,
) -> SourceFreshnessSet: ...
def write_source_freshness_records(
    *,
    connection: object,
    execute: AdapterExecute[Any, Any],
    database: str | None,
    schema: str,
    records: tuple[SourceFreshnessRecord, ...],
    renderers: SourceFreshnessRenderers,
) -> None: ...
def append_eligible_direct_source_freshness_records(
    *,
    plan: PlanOutput,
    result: object,
    adapter: object,
    connection_config: dict[str, object],
    run_id: str,
) -> None: ...
def observe_configured_source_freshness(
    *, adapter: StrictAdapter, connection: object, source: SourceEntry, observed_at: datetime
) -> SourceFreshnessObservation: ...
def observe_adapter_sources_freshness(
    *,
    adapter: StrictAdapter,
    connection: object,
    sources: tuple[SourceEntry, ...],
    observed_at: datetime,
) -> AdapterSourceFreshnessBatch: ...
def build_direct_source_freshness_planning_result(
    *,
    adapter: StrictAdapter,
    connection: object,
    sources: tuple[SourceEntry, ...],
    state_database: str | None,
    state_schemas: tuple[str, ...],
    observed_at: datetime,
    run_id: str,
    render_qualified_name: Callable[..., str | None],
    state_table_exists_by_schema: Mapping[str, bool],
    filter_previous_to_sources: bool = False,
) -> DirectSourceFreshnessPlanningResult: ...
def build_direct_source_freshness_propagation_result(
    *, source_freshness: DirectSourceFreshnessPlanningResult, scope: PlannerScope
) -> DirectSourceFreshnessPropagationResult: ...
def redact_diagnostic_text(value: str) -> str: ...
def redact_diagnostic_value(*, name: str, value: object) -> object: ...
def sql_log_record(
    *,
    context: Mapping[str, object] | None,
    identity: Mapping[str, str | None] | None,
    sql: str,
    action: str,
    intent: str | None,
    query_id: str | None,
) -> tuple[str, dict[str, object]]: ...
def debug_event_record_fields(
    *,
    context: Mapping[str, object] | None,
    event_fields: Mapping[str, object],
    identity: Mapping[str, str | None] | None,
) -> dict[str, object]: ...
def route_diagnostic_log_record(
    *,
    record: logging.LogRecord,
    internal_only: bool,
    allow_internal: bool,
    include_sql_text: bool,
    summarize_third_party: bool = False,
    project_roots: tuple[str, ...] = (),
) -> dict[str, object] | None: ...
def safe_diagnostic_extra_fields(record: logging.LogRecord) -> dict[str, object]: ...
def compute_diagnostic_log_fields(
    *, record: logging.LogRecord, invocation_id: str, include_sql_text: bool
) -> dict[str, object] | None: ...
def load_sql_rule_cache(*, path: Path) -> dict[str, tuple[str, str]]: ...
def encode_sql_rule_findings(*, findings: list[Finding]) -> str: ...
def cursor_parse(*, raw: object, cursor_type: str) -> CursorScalar: ...
def cursor_render(*, value: CursorScalar) -> str: ...
def cursor_sentinel_to_token(*, sentinel: Bound) -> str: ...
def cursor_sentinel_from_token(*, token: str) -> BoundSentinel | None: ...
def cursor_compare(*, left: CursorScalar, right: CursorScalar) -> int: ...
def type_cursor_overrides(
    *, cursor_overrides: CursorOverrides | None, selected_models: Iterable[CompiledModelView] = ()
) -> CursorOverrides | None: ...
def maximum_allowed_start(
    *, discovered_value: str, cursor_grain: str | None, invocation_time: datetime, max_ahead: str
) -> str: ...
def apply_maximum_start_policy(
    *,
    bounds: CursorBounds,
    snapshot: ModelCursorSnapshot,
    cursor_type: str | None,
    cursor_grain: str | None,
    cursor_start: CursorScalar | str | None,
    lookback: Duration | str | None,
    backfill_duration: Duration | str | None,
    policy: MaximumStartPolicyInputs,
    has_start_override: bool,
) -> CursorBounds: ...
def build_selected_test_entries(
    *,
    project: CompiledProjectView,
    adapter: BaseAdapter,
    selected_keys: frozenset[CompiledObjectKey],
    sql_test_selection: SqlTestSelection,
    case_name: str | None = None,
) -> tuple[list[SqlTestPlanEntry], list[PlanWarning]]: ...
def resolve_static_command_scope(
    *, project: CompiledProjectView, selection: PlannerSelection, auto_load_sources: bool = False
) -> PlannerScope: ...
def resolve_static_relation_context(
    *,
    project: CompiledProjectView,
    adapter: BaseAdapter,
    scope: PlannerScope,
    deferral: DeferralInputs | None = None,
    project_config: ProjectConfig | None = None,
    local_config: LocalConfig | None = None,
    relation_keys: frozenset[CompiledObjectKey] | None = None,
) -> PlannerRelationsContext: ...
def build_audit_command_plan(
    *,
    project: CompiledProjectView,
    adapter: BaseAdapter,
    scope: PlannerScope,
    relations: PlannerRelationsContext,
) -> PlanOutput: ...
def build_test_command_plan(
    *,
    project: CompiledProjectView,
    adapter: BaseAdapter,
    scope: PlannerScope,
    relations: PlannerRelationsContext,
    case_name: str | None = None,
) -> PlanOutput: ...
def build_seed_command_plan(
    *,
    project: CompiledProjectView,
    scope: PlannerScope,
    relations: PlannerRelationsContext,
    fingerprints: dict[str, Fingerprint] | None = None,
) -> PlanOutput: ...
def read_selected_seed_fingerprints(
    *, adapter: BaseAdapter, connection: Any, seeds: tuple[CompiledSeedView, ...]
) -> dict[str, Fingerprint]: ...
def resolve_clone_boundary[K](
    *,
    selected: frozenset[K],
    upstream: Mapping[K, tuple[K, ...]],
    is_clonable: Callable[[K], bool],
    is_view: Callable[[K], bool],
) -> frozenset[K]: ...
def resolve_skipped_view_chain[K](
    *,
    selected: frozenset[K],
    upstream: Mapping[K, tuple[K, ...]],
    is_clonable: Callable[[K], bool],
    is_view: Callable[[K], bool],
) -> frozenset[K]: ...
def sql_test_model_chain_names_by_key(
    *, project: CompiledProjectView, tests: tuple[CompiledSqlTest, ...]
) -> dict[CompiledObjectKey, tuple[str, ...]]: ...
def build_execution_plan(
    *,
    project: CompiledProjectView,
    adapter: BaseAdapter,
    connection: Any,
    selection: PlannerSelection,
    overrides: PlannerOverrides,
    deferral: DeferralInputs,
    policies: PlannerPolicies,
    on_progress: Callable[[str], None] | None = None,
    project_config: ProjectConfig | None = None,
    local_config: LocalConfig | None = None,
) -> PlanOutput: ...

class SqlTestPlanning:
    """The planned SQL tests of one plan, planned on a native thread when enabled."""

    def result(self) -> PlannedSqlTests: ...

def start_sql_test_planning(
    *,
    project: CompiledProjectView,
    adapter: BaseAdapter,
    selected_keys: frozenset[CompiledObjectKey],
    sql_test_selection: SqlTestSelection,
    enabled: bool,
) -> SqlTestPlanning: ...
def build_display_only_sqlbuild_plan(
    *, project: CompiledProjectView, selected_model_names: tuple[str, ...], full_refresh: bool
) -> PlanOutput: ...
def format_pre_build_work(
    *, lines: list[str], plan: PlanOutput, display_options: DisplayOptions
) -> list[str]: ...
def future_cursor_cap_warning(bounds: CursorBounds | None, /) -> str | None: ...
def maximum_start_cap_warning(bounds: CursorBounds | None, /) -> str | None: ...
def compute_plan_cursor_bounds(
    *,
    model: CompiledModelView,
    snapshot: WarehouseSnapshot,
    backfill: BackfillResult,
    full_refresh: bool,
    start_cursor_override: str | None,
    end_cursor_override: str | None,
    runtime_owned_cursor_bounds: bool,
    invocation_time: datetime | None = None,
    future_cursor_config: FutureCursorsConfig | None = None,
    start_cursor_config: StartCursorsConfig | None = None,
    replaces_relation: bool = False,
    cursor_input_relations: tuple[CursorInputRelation, ...] = (),
) -> CursorBounds | None: ...
def compute_microbatch_range(
    *,
    model: CompiledModelView,
    snapshot: WarehouseSnapshot,
    backfill: BackfillResult,
    full_refresh: bool,
    start_cursor_override: str | None,
    end_cursor_override: str | None,
    runtime_owned_cursor_bounds: bool,
    invocation_time: datetime | None = None,
    future_cursor_config: FutureCursorsConfig | None = None,
    start_cursor_config: StartCursorsConfig | None = None,
    replaces_relation: bool = False,
    cursor_input_relations: tuple[CursorInputRelation, ...] = (),
) -> CursorBounds | None: ...
def resolve_cursor_overrides(
    *,
    model: CompiledModelView,
    cursor_overrides: CursorOverrides | None,
    start_cursor_override: str | None,
    end_cursor_override: str | None,
) -> tuple[str | None, str | None]: ...
def build_planner_relations_context(
    *,
    project: CompiledProjectView,
    adapter: BaseAdapter,
    connection: Any,
    scope: PlannerScope,
    deferral: DeferralInputs | None = None,
    project_config: ProjectConfig | None = None,
    local_config: LocalConfig | None = None,
    known_source_columns: dict[str, tuple[ColumnInfo, ...]] | None = None,
    relation_keys: frozenset[CompiledObjectKey] | None = None,
) -> PlannerRelationsContext: ...
def scope_overlaps(
    *, scope_deps: tuple[CompiledObjectKey, ...], selected_keys: frozenset[CompiledObjectKey]
) -> bool: ...
def resolve_future_cursor_safety(
    *,
    bounds: CursorBounds,
    cursor_type: str | None,
    cursor_grain: str | None,
    config: FutureCursorsConfig | None,
    invocation_time: datetime | None,
    has_complete_override: bool,
    input_evidence: tuple[CursorInputEvidence, ...] = (),
) -> CursorBounds: ...
def cursor_max_bound[T](*, values: tuple[T, ...] | list[T], cursor_type: str) -> T: ...
def cursor_min_bound[T](*, values: tuple[T, ...] | list[T], cursor_type: str) -> T: ...
def cursor_floor_to_grain(*, value: CursorScalar, grain: CursorGrain) -> CursorScalar: ...
def cursor_next_boundary(*, value: CursorScalar, grain: CursorGrain) -> CursorScalar: ...
def cursor_inclusive_to_exclusive(
    *, value: CursorScalar, grain: CursorGrain | None
) -> CursorScalar: ...
def cursor_exclusive_to_inclusive(
    *, value: CursorScalar, grain: CursorGrain | None
) -> CursorScalar: ...
def cursor_exclusive_end_from_observed_max(
    *, value: CursorScalar, grain: CursorGrain | None
) -> CursorScalar: ...
def cursor_observed_partition(
    *, value: CursorScalar, grain: CursorGrain | None
) -> AlignedInterval: ...
def cursor_clamp(
    *, interval: AlignedInterval, bounds: AlignedInterval
) -> AlignedInterval | None: ...
def cursor_merge(*, intervals: tuple[AlignedInterval, ...]) -> tuple[AlignedInterval, ...]: ...
def cursor_split_into_batches(
    *, interval: AlignedInterval, step: int
) -> tuple[AlignedInterval, ...]: ...
def cursor_cap_from_start[T](*, intervals: tuple[T, ...], count: int) -> tuple[T, ...]: ...
def cursor_cap_from_end[T](*, intervals: tuple[T, ...], count: int) -> tuple[T, ...]: ...
def get_config_str(*, values: Mapping[str, object], key: str) -> str | None: ...
def get_config_bool(*, values: Mapping[str, object], key: str) -> bool | None: ...
def get_config_int(*, values: Mapping[str, object], key: str) -> int | None: ...
def get_config_string_tuple(
    *, values: Mapping[str, object], key: str
) -> tuple[str, ...] | None: ...
def get_config_cursor_bound(*, values: Mapping[str, object], key: str) -> str | None: ...
def custom_rule_import_closure(*, rule: Rule, project_dir: Path) -> tuple[Path, ...]: ...
def allowed_custom_rule_import(module: str) -> bool: ...
def verify_custom_rules(*, rules: Iterable[Rule], project_dir: Path) -> None: ...
def finding_position(
    *, path: Path, line: int | None, column: int | None, node: object
) -> tuple[int, int]: ...
def rule_option_toml_value(value: object) -> str: ...
def project_tree_paths(root: Path) -> tuple[ProjectPath, ...]: ...
def project_tree_read_text(root: Path, path: str) -> str: ...
def fact_value_digest(value: object) -> str | None: ...
def fact_error_digest(error: BaseException) -> str | None: ...
def project_tree_child_nodes(
    nodes: Iterable[ProjectPath], path: str | PurePath
) -> tuple[ProjectPath, ...]: ...
def project_tree_descendant_nodes(
    nodes: Iterable[ProjectPath], path: str | PurePath
) -> tuple[ProjectPath, ...]: ...
def project_tree_glob_nodes(
    nodes: Iterable[ProjectPath], pattern: str
) -> tuple[ProjectPath, ...]: ...
def project_tree_relative_parts(
    *, root: Path, path: str | PurePath, under: str | PurePath
) -> tuple[str, ...]: ...
def startup_input_stamps(
    *, directories: Iterable[str], files: Iterable[str], prefixes: Iterable[str]
) -> list[tuple[str, int | None, int | None]]: ...
def custom_rule_implementation_fingerprint(
    *, rule: Rule, project_dir: Path, import_closure: tuple[Path, ...] | None = None
) -> str: ...
def evaluate_project_rules(
    *,
    project: CompiledProjectView,
    config: RulesConfig,
    project_dir: Path,
    dialect: str = "generic",
    verify_determinism: bool = False,
) -> RulesResult: ...
def evaluate_native_rules(
    *,
    project: CompiledProjectView,
    config: RulesConfig,
    project_dir: Path,
    catalogue: tuple[Rule, ...],
    dialect: str = "generic",
    initial_findings: tuple[Finding, ...] | None = None,
    defer_suppressions: bool = False,
    verify_determinism: bool = False,
) -> RulesResult: ...
def evaluate_custom_rules(
    *,
    project: CompiledProjectView,
    config: RulesConfig,
    project_dir: Path,
    rules: tuple[Rule, ...],
    dialect: str,
    verify_determinism: bool = False,
    implementation_fingerprints: dict[str, str] | None = None,
) -> CustomRulesOutcome: ...
def custom_host_payload(project: CompiledProjectView, config: object) -> bytes: ...
def custom_host_declared_columns(data: bytes) -> tuple[SchemaColumn, ...]: ...
def custom_rule_fact_digest(
    *, project: CompiledProjectView, project_dir: Path, key: tuple[str, ...]
) -> str | None: ...

class CustomHostObserver:
    def __init__(
        self,
        *,
        cores: int | None = None,
        min_invocations_per_host: int | None = None,
        publish_inputs: Callable[[Path], object] | None = None,
    ) -> None: ...
    def launches(self) -> tuple[str, ...]: ...

custom_host_observer: CustomHostObserver | None

def custom_rule_test_evidence(
    *, rule: Rule, project_dir: Path
) -> list[tuple[str, int, int, str]]: ...
def format_rules_result(*, result: RulesResult, json_output: bool) -> str: ...
def run_rules(
    *,
    graph: ProjectGraph,
    discovered_inputs: DiscoveredProjectInputs,
    config: RulesConfig,
    project_dir: Path,
    dialect: str,
    selected_keys: frozenset[CompiledObjectKey] | None = None,
    early_lint: EarlySqlLint | None = None,
    no_sql_analysis: bool = False,
) -> RulesRunResult: ...
def rules_bulk_cache_path(*, project_dir: Path, file_name: str) -> Path: ...
def run_planned_sql_rules(
    *,
    plan: RuleRunPlan,
    graph: ProjectGraph,
    discovered_inputs: DiscoveredProjectInputs,
    dialect: str,
    early_lint: EarlySqlLint | None,
) -> SqlRulesEvaluation: ...

class EarlySqlLint:
    """Own speculative SQL checks for one compilation invocation."""

    def __init__(self, *, enabled: bool, no_cache: bool = False) -> None: ...
    @property
    def enabled(self) -> bool: ...
    def __enter__(self) -> EarlySqlLint: ...
    def __exit__(self, *exception: object) -> None: ...
    def start(self, *, inputs: CompileInputsView, dialect: str) -> None: ...
    def stop(self) -> int: ...

def scan_headers(*, contents: str, first_only: bool = False) -> tuple[HeaderSpan, ...]: ...
def sql_body_ranges(
    *, contents: str, headers: tuple[HeaderSpan, ...]
) -> tuple[tuple[int, int], ...]: ...
def run_lint(
    *,
    project_dir: Path,
    config: LintConfig,
    value_renderer: TypedSqlValueRenderer | None = None,
    selected_paths: frozenset[Path] | None = None,
    discovered_inputs: DiscoveredProjectInputs | None = None,
    dynamic_output_paths: frozenset[Path] = frozenset(),
    compiled_expansions: dict[Path, CompiledSqlExpansion] | None = None,
    expansion_context: SqlExpansionContext | None = None,
    source_files: dict[Path, str] | None = None,
    skip_unexpandable: bool = False,
) -> LintRunResult: ...
def collect_lint_project_files(
    *, project_dir: Path, selected_paths: frozenset[Path] | None = None
) -> dict[Path, str]: ...
def lint_native_headers(
    *, contents: str, file_path: Path, headers: tuple[HeaderSpan, ...], config: LintConfig
) -> tuple[LintViolation, ...]: ...
def format_native_headers(
    *, contents: str, file_path: Path, config: LintConfig
) -> tuple[str, tuple[LintViolation, ...]]: ...
def parse_header_values(
    *,
    header: str,
    file_path: object,
    statement_name: str,
    header_line: int | None,
    error_class: type[Exception] | None = None,
) -> dict[str, object]: ...
def model_output_column_locations(
    *, contents: str, relative_path: object, extract_implicit_alias_columns: bool = True
) -> dict[str, Any]: ...
def inclusive_cursor_end(*, end: str, cursor_type: str | None, cursor_grain: str | None) -> str: ...
def cursor_bound_display(
    *, value: str, cursor_type: str | None, cursor_grain: str | None
) -> str: ...
def detect_changes(
    *,
    project: CompiledProjectView,
    scope: PlannerScope,
    snapshot: WarehouseSnapshot,
    full_refresh: bool,
    expected_version_hashes: dict[str, str] | None = None,
    expected_metadata_jsons: dict[str, str] | None = None,
) -> PlannerChangeResults: ...
def parse_query_shape(*, query_sql: str, dialect: str | None) -> QueryShape | None: ...
def renames_explain_change(
    *,
    previous: QueryShape,
    current: QueryShape,
    renames: Mapping[str, str],
    input_columns: InputColumns,
) -> bool: ...
def build_migration_fingerprint(
    *, query_sql: str, metadata_json: str, ref_identities: Mapping[str, str], dialect: str | None
) -> str | None: ...
def plan_column_migrations(
    *,
    runtime: PlannerRuntime,
    scope: PlannerScope,
    snapshot: WarehouseSnapshot,
    full_refresh_model_names: frozenset[str],
    physical_relations: Mapping[str, str],
    overrides: PlannerOverrides,
    deferral: DeferralInputs,
    source_columns: Mapping[str, tuple[ColumnInfo, ...]],
) -> None: ...
def identical_renames(
    *,
    previous: QueryShape,
    current: QueryShape,
    excluded: frozenset[str],
    declared: Mapping[str, str],
    input_columns: InputColumns,
) -> tuple[tuple[str, str], ...]: ...
def rename_hints(
    *,
    model_name: str,
    previous: QueryShape,
    current: QueryShape,
    excluded: frozenset[str],
    live_columns: frozenset[str],
) -> tuple[ColumnRenameHint, ...]: ...
def detect_model_changes(
    *,
    model: CompiledModelView,
    snapshot: WarehouseSnapshot,
    sql_analysis_enabled: bool,
    query_change_tracking: bool,
    full_refresh: bool,
    function_local_hashes: dict[str, str] | None = None,
    hook_version_hashes: dict[str, str] | None = None,
    expected_version_hash: str | None = None,
    expected_metadata_json: str | None = None,
) -> ChangeDetectionResult: ...
def replaces_incremental_relation(
    *, materialization_type: MaterializationType, action: PlanAction
) -> bool: ...
def version_identity_metadata_payload(metadata_json: str | None) -> Any: ...
def get_materialization_type(model: CompiledModelView) -> MaterializationType: ...
def resolve_model_plan_action(
    *, model: CompiledModelView, change_result: ChangeDetectionResult, full_refresh: bool
) -> tuple[PlanAction, PlanReason]: ...
def resolve_schema_actions(
    *, schema_findings: tuple[SchemaFinding, ...], on_schema_change: OnSchemaChange | None
) -> tuple[SchemaAction, ...]: ...
def build_model_warnings(
    *,
    model_name: str,
    materialization_type: MaterializationType,
    change_result: ChangeDetectionResult,
    schema_actions: tuple[SchemaAction, ...],
    on_schema_change: OnSchemaChange | None,
    type_enforcement: bool,
) -> tuple[PlanWarning, ...]: ...
def validate_source_cursor_input_columns(
    *,
    model: CompiledModelView,
    cursor_column: str | None,
    models_by_name: dict[str, CompiledModelView] | None = None,
    functions_by_name: dict[str, CompiledFunctionView] | None = None,
    source_map: dict[str, SourceEntry] | None = None,
    source_warehouse_columns: dict[str, tuple[ColumnInfo, ...]],
) -> None: ...
def build_cursor_input_relations(
    *,
    model: CompiledModelView,
    adapter: BaseAdapter,
    model_locations: dict[str, CompiledRelationLocation],
    models_by_name: dict[str, CompiledModelView],
    functions_by_name: dict[str, CompiledFunctionView] | None = None,
    seed_locations: dict[str, CompiledRelationLocation],
    source_map: dict[str, SourceEntry],
    cursor_column: str | None,
    runtime_cursor_producer_names: frozenset[str],
) -> tuple[CursorInputRelation, ...]: ...
def runtime_cursor_producer_names(
    *, scope: PlannerScope, source_map: dict[str, SourceEntry]
) -> frozenset[str]: ...
def resolve_microbatch_lookback(
    *, model: CompiledModelView, effective_grain: str | None = None
) -> str | None: ...
def plan_audit(
    *,
    audit: CompiledAuditView,
    model_locations: dict[str, CompiledRelationLocation],
    seed_locations: dict[str, CompiledRelationLocation],
    source_map: dict[str, SourceEntry],
    adapter: BaseAdapter,
    lineage_upstream_deps: dict[CompiledObjectKey, tuple[CompiledObjectKey, ...]],
    lineage_downstream_deps: dict[CompiledObjectKey, tuple[CompiledObjectKey, ...]],
    model_materializations: dict[str, str],
) -> AuditPlanEntry: ...
def resolve_attachment_kind(
    *,
    audit: CompiledAuditView,
    downstream_deps: Mapping[CompiledObjectKey, tuple[CompiledObjectKey, ...]],
) -> tuple[AuditAttachmentKind, str | None]: ...
def reads_outside_target_lineage(
    *,
    audit: CompiledAuditView,
    upstream_deps: Mapping[CompiledObjectKey, tuple[CompiledObjectKey, ...]],
) -> bool: ...
def resolve_effective_run_scope(
    *, requested_run_scope: AuditRunScope, attached_model_materialization: str | None
) -> AuditRunScope: ...
def check_cursor_type_consistency(
    *,
    model_name: str,
    cursor_column: str | None,
    cursor_type: str | None,
    warehouse_columns: tuple[ColumnInfo, ...],
    sql_analysis_enabled: bool,
) -> PlanWarning | None: ...
def apply_microbatch_limit(
    *, entry: ModelPlanEntry, max_batches: int | None, action: MicrobatchLimitAction
) -> tuple[ModelPlanEntry, PlanWarning | None]: ...
def apply_configured_microbatch_limits(
    *, entry: ModelPlanEntry, inputs: PlanEntryBuildInputs
) -> tuple[ModelPlanEntry, tuple[PlanWarning, ...]]: ...
def microbatch_limit_warning(
    *, model_name: str, max_batches: int | None, batch_count: int, action: MicrobatchLimitAction
) -> str | None: ...
def detect_schema_changes(
    *,
    yml_columns: tuple[ColumnInfo, ...],
    inferred_columns: tuple[InferredColumn, ...] | None,
    warehouse_columns: tuple[ColumnInfo, ...],
    type_enforcement: bool,
    inferred_schema_complete: bool,
    dynamic_columns: tuple[SchemaDynamicColumnFamily, ...] | None = None,
    dialect: str | None = None,
) -> tuple[SchemaFinding, ...]: ...
def origin_reference_names(
    *,
    query_sql: str,
    recorded: Fingerprint,
    renames: tuple[ReferenceRename, ...],
    dialect: str | None,
) -> dict[str, str] | None: ...
def with_declared_columns_hash(*, metadata_json: str, declared_columns_hash: str) -> str: ...
def without_declared_columns_hash(metadata_json: str) -> str: ...
def model_definition_hash(*, model_name: str, query_sql: str, dialect: str | None) -> str: ...
def build_version_identity_metadata_json(
    *,
    model_name: str,
    config_values: Mapping[str, object],
    local_function_hashes: Mapping[str, str] | None = None,
    execution_signature: Mapping[str, object] | None = None,
) -> str: ...
def build_model_version_identity_metadata_json(
    *,
    model: CompiledModelView,
    function_local_hashes: Mapping[str, str] | None = None,
    hook_version_hashes: Mapping[str, str] | None = None,
) -> str: ...
def declared_columns_hash(*, model: CompiledModelView) -> str: ...
def build_function_local_hashes(
    *, functions: Iterable[CompiledFunctionView], dialect: str | None
) -> dict[str, str]: ...
def build_compiled_function_fingerprint_sql(function: CompiledFunctionView) -> str: ...
def build_seed_identity(seed: CompiledSeedView) -> tuple[str, str]: ...
def build_direct_model_version_identities(
    *,
    functions: Iterable[CompiledFunctionView],
    scope: PlannerScope,
    dialect: str | None,
    seeds: Iterable[CompiledSeedView] = (),
    source_version_hashes: Mapping[str, str] | None = None,
    hook_functions: Iterable[DiscoveredHookFunction] = (),
) -> DirectModelVersionIdentities: ...
def merge_recomputed_model_changes(
    *, resolved_actions: PlannerResolvedActions, changes: PlannerChangeResults
) -> PlannerResolvedActions: ...
def build_stale_out_of_selection_warnings(
    *,
    original_scope: PlannerScope,
    execution_scope: PlannerScope,
    changes: PlannerChangeResults,
    snapshot: WarehouseSnapshot,
    version_identities: DirectModelVersionIdentities,
    source_freshness: DirectSourceFreshnessPlanningResult | None,
    include_sources: bool = True,
    model_changes_complete: bool = False,
) -> tuple[PlanWarning, ...]: ...
def classify_selection_staleness_warnings(
    graph: SelectionStalenessGraph,
) -> tuple[SelectionStalenessWarning, ...]: ...
def build_execution_upstream_deps(
    project: CompiledProjectView,
) -> dict[CompiledObjectKey, tuple[CompiledObjectKey, ...]]: ...
def build_execution_edge_origins(
    project: CompiledProjectView,
) -> dict[tuple[CompiledObjectKey, CompiledObjectKey], str]: ...
def build_downstream_deps(
    upstream: Mapping[CompiledObjectKey, tuple[CompiledObjectKey, ...]],
) -> dict[CompiledObjectKey, tuple[CompiledObjectKey, ...]]: ...
def topologically_order_keys(
    *,
    upstream: Mapping[CompiledObjectKey, tuple[CompiledObjectKey, ...]],
    injected_edge_origins: Mapping[tuple[CompiledObjectKey, CompiledObjectKey], str] | None = None,
) -> tuple[CompiledObjectKey, ...]: ...
def build_downstream_model_name_closure(
    *,
    start_keys: Iterable[CompiledObjectKey],
    downstream_deps: Mapping[CompiledObjectKey, tuple[CompiledObjectKey, ...]],
) -> frozenset[str]: ...
def check_buildability(
    *,
    selected_keys: Set[CompiledObjectKey],
    upstream_deps: Mapping[CompiledObjectKey, tuple[CompiledObjectKey, ...]],
    snapshot: WarehouseSnapshot,
    deferred_relations: Mapping[str, RelationInfo] | None = None,
    satisfied_keys: Set[CompiledObjectKey] | None = None,
) -> tuple[MissingUpstream, ...]: ...
def missing_upstream_message(
    *,
    missing: tuple[MissingUpstream, ...],
    edge_origins: Mapping[tuple[CompiledObjectKey, CompiledObjectKey], str],
) -> str: ...
def format_waiting_on_empty_inputs(input_names: tuple[str, ...]) -> str: ...
def resolve_effective_microbatch_batch_size(*, batch_size: str, effective_grain: str) -> str: ...
def aligned_timestamp_bounds(
    *, start: DateValue | TimestampValue, end: DateValue | TimestampValue
) -> tuple[datetime, datetime]: ...
def expand_selected_loader_dependencies(
    *,
    project: CompiledProjectView,
    selected_keys: frozenset[CompiledObjectKey],
    upstream_deps: dict[CompiledObjectKey, tuple[CompiledObjectKey, ...]],
    executable_dependency_source_keys: frozenset[CompiledObjectKey] = frozenset(),
) -> tuple[
    frozenset[CompiledObjectKey], dict[CompiledObjectKey, tuple[CompiledObjectKey, ...]]
]: ...
def upstream_loader_dependency_names(
    *,
    loader_function: DiscoveredLoaderFunction,
    loader_functions: tuple[DiscoveredLoaderFunction, ...],
) -> tuple[str, ...]: ...
def build_intermediate_source_map(
    *,
    project: CompiledProjectView,
    selected_keys: frozenset[CompiledObjectKey],
) -> dict[str, SourceEntry]: ...
def build_source_load_map(
    *,
    project: CompiledProjectView,
    selected_keys: frozenset[CompiledObjectKey],
    upstream_deps: dict[CompiledObjectKey, tuple[CompiledObjectKey, ...]] | None = None,
) -> dict[str, SourceEntry]: ...
def build_source_load_entries(
    *,
    execution_order: tuple[CompiledObjectKey, ...],
    selected_keys: frozenset[CompiledObjectKey],
    source_map: dict[str, SourceEntry],
    is_reload: bool,
) -> tuple[SourceLoadPlanEntry, ...]: ...
def old_name_rows(*, entry: OldNameViewPlanEntry, style: CliStyle) -> list[str]: ...

class MigrationFingerprintCache:
    def __init__(self, *, root: Path | None = None) -> None: ...
    def fingerprint(
        self,
        *,
        query_sql: str,
        metadata_json: str,
        ref_identities: Mapping[str, str],
        dialect: str | None,
    ) -> str | None: ...
    def persist(self) -> None: ...

def gather_warehouse_snapshot(
    *,
    project: CompiledProjectView,
    adapter: BaseAdapter,
    connection: Any,
    execute: AdapterExecute[Any, Any],
    selected_keys: frozenset[CompiledObjectKey] | None = None,
    full_refresh: bool = False,
    full_refresh_model_names: frozenset[str] | None = None,
    on_progress: Callable[[str], None] | None = None,
    deferred_locations: dict[str, CompiledRelationLocation] | None = None,
    cursor_scope: CursorSnapshotScope | None = None,
) -> WarehouseSnapshot: ...
def watermark_type_is_compatible(*, declared_type: str, cursor_type: str | None) -> bool: ...
def gather_source_columns(
    *,
    project: CompiledProjectView,
    adapter: BaseAdapter,
    connection: Any,
    source_entries: tuple[SourceEntry, ...] | None = None,
) -> dict[str, tuple[ColumnInfo, ...]]: ...
def get_semantic_source_columns(
    *,
    project: CompiledProjectView,
    adapter: BaseAdapter,
    connection: Any,
    source_read_map: dict[str, SourceEntry],
    columns: dict[str, tuple[ColumnInfo, ...]],
    selected_keys: frozenset[CompiledObjectKey],
) -> dict[str, tuple[ColumnInfo, ...]]: ...
def check_selected_source_tables_exist(
    *,
    project: CompiledProjectView,
    adapter: BaseAdapter,
    connection: Any,
    scope: PlannerScope,
    relations: PlannerRelationsContext,
) -> None: ...
def build_planner_source_freshness_result(
    *,
    project: CompiledProjectView,
    adapter: BaseAdapter,
    connection: Any,
    scope: PlannerScope,
    relations: PlannerRelationsContext,
    freshness_state_schemas: frozenset[str] | None = None,
) -> DirectSourceFreshnessPlanningResult: ...
def resolve_planner_scopes(
    *,
    project: CompiledProjectView,
    selection: PlannerSelection,
    policies: PlannerPolicies,
) -> PlannerScopeResolution: ...
def detect_stale_warning_changes(
    *,
    project: CompiledProjectView,
    scopes: PlannerScopeResolution,
    snapshot: WarehouseSnapshot,
    identities: PlannerIdentityContext,
    execution_changes: PlannerChangeResults,
) -> PlannerChangeResults: ...
def describe_full_rebuild_cause(
    *, change_result: ChangeDetectionResult, full_refresh: bool
) -> str: ...
def check_full_rebuild_allowed(
    *, model: CompiledModelView, change_result: ChangeDetectionResult, full_rebuild_cause: str
) -> None: ...
def table_type_copy_name(*, target_name: str, identifier_limit: int) -> str: ...
def plan_table_types(
    *, runtime: PlannerRuntime, warehouse: PlannerWarehouseState, scope: PlannerScope
) -> tuple[TableTypePlanEntry, ...]: ...
def plan_retention(
    *, runtime: PlannerRuntime, warehouse: PlannerWarehouseState, scope: PlannerScope
) -> tuple[RetentionPlanEntry, ...]: ...
def planner_metadata_name_filter(
    *, project: CompiledProjectView, selected_keys: frozenset[CompiledObjectKey] | None = None
) -> tuple[str, ...] | None: ...
def list_source_candidates(
    *,
    adapter: BaseAdapter,
    connection: Any,
    database: str | None,
    entries: tuple[SourceEntry, ...],
) -> tuple[RelationInfo, ...]: ...

class CostArtifactError(ValueError): ...
class CommandTerminated(KeyboardInterrupt): ...
class SourceFreshnessInputError(ValueError): ...
class SourceFreshnessObservationError(ValueError): ...

class CostTelemetryHealth:
    def mark_ledger_failure(self, *, run_id: str, error: BaseException) -> None: ...
    def consume_ledger_failure(self, *, run_id: str) -> str | None: ...

COST_TELEMETRY_HEALTH: CostTelemetryHealth

class CostScope:
    def __enter__(self) -> None: ...
    def __exit__(self, *exception: object) -> None: ...

class CostContext:
    @staticmethod
    def current() -> CostResourceContext | None: ...
    @staticmethod
    def scope(
        *,
        run_id: str,
        resource_type: str,
        resource_name: str,
        ledger_path: Path | None = None,
        phase: str = "execute",
        attempt: int = 1,
        on_statement_complete: Callable[[StatementExecutionTelemetry], None] | None = None,
    ) -> CostScope: ...
    @staticmethod
    def resource_scope(
        *,
        resource_type: str,
        resource_name: str,
        phase: str | None = None,
        attempt: int = 1,
    ) -> CostScope: ...

def allocate_run_cost(
    *,
    observations: Iterable[QueryCostObservation],
    run_id: str,
    usd_per_credit: Decimal,
    rate_source: str,
    result_limit_reached: bool = False,
) -> RunCostSummary: ...
def record_statement(
    *,
    context: CostResourceContext,
    statement_id: str,
    sql: str,
    query_id: object,
    status: str,
    started_at: datetime,
    completed_at: datetime,
    error: BaseException | None = None,
) -> None: ...
def read_statement_ledger(*, path: Path, run_id: str) -> tuple[StatementLedgerEntry, ...]: ...
def collect_snowflake_cost(
    *,
    connection: Any,
    run_id: str,
    started_at: datetime,
    completed_at: datetime,
    statement_ledger_path: Path,
    usd_per_credit: Decimal,
    rate_source: str,
    database: str | None = None,
    attempts: int = 3,
    retry_delay_seconds: float = 1.0,
    result_limit: int = 10000,
    budget_seconds: float | None = None,
) -> RunCostSummary: ...
def snowflake_query_observations(
    *,
    connection: Any,
    started_at: datetime,
    completed_at: datetime,
    expected_query_ids: Iterable[str],
    classification_query_ids: Iterable[str],
    deadline: float,
    database: str | None = None,
    result_limit: int | None = None,
    run_id: str | None = None,
) -> tuple[tuple[QueryCostObservation, ...], frozenset[str], dict[str, str], bool]: ...
def render_snowflake_query_history_sql(
    *,
    started_at: datetime,
    database: str | None = None,
    completed_at: datetime | None = None,
    result_limit: int = 10000,
    run_id: str | None = None,
) -> str: ...
def snowflake_statement_is_metadata_only(*, query_type: object, query_text: object) -> bool: ...
def parse_sqlbuild_query_tag(value: object) -> dict[str, str | None] | None: ...

class NestedProgressChildRow(NativeRecord):
    """One child row rendered below a completed nested progress item."""

    label: str
    name: str
    status_text: str
    detail: str
    def __init__(
        self,
        label: str,
        name: str,
        status_text: str,
        detail: str = "",
    ) -> None: ...

class AuditCommandRequest(NativeRecord):
    """CLI inputs for one audit command invocation."""

    project_dir: Path | None
    no_sql_validation: bool
    defer_to: str | None
    no_color: bool
    selected_target: str | None
    concurrency: int | None
    select: tuple[str, ...]
    exclude: tuple[str, ...]
    cli_vars: dict[str, object] | None
    json_output: bool
    json_output_path: Path | None
    def __init__(
        self,
        project_dir: Path | None = None,
        no_sql_validation: bool = False,
        defer_to: str | None = None,
        no_color: bool = False,
        selected_target: str | None = None,
        concurrency: int | None = None,
        select: tuple[str, ...] = (),
        exclude: tuple[str, ...] = (),
        cli_vars: dict[str, object] | None = None,
        json_output: bool = False,
        json_output_path: Path | None = None,
    ) -> None: ...

class BuildCommandRequest(NativeRecord):
    """CLI inputs for one build command invocation."""

    project_dir: Path | None
    no_sql_validation: bool
    defer_to: str | None
    defer_clone_from: str | None
    defer_sources_to: str | None
    selected_target: str | None
    cursor_overrides: CursorOverrides | None
    no_color: bool
    fail_fast: bool
    full_refresh: bool
    load_sources: bool | None
    reload_sources: bool
    include_python: bool
    allow_snapshot_full_refresh: bool
    allow_table_type_downgrade: bool
    allow_retention_decrease: bool
    allow_missing_migration_origin: bool
    allow_snapshot_schema_change: bool
    concurrency: int | None
    select: tuple[str, ...]
    exclude: tuple[str, ...]
    verbose: bool
    debug: bool
    cli_vars: dict[str, object] | None
    run_tests: bool
    run_audits: bool
    manifest: bool
    json_output: bool
    json_output_path: Path | None
    event_output_path: Path | None
    no_cache: bool
    max_microbatches: int | None
    selection_diagnostics: bool
    selector_files: tuple[SelectorFileSummary, ...]
    test_timeout: str | None
    discovered_inputs: DiscoveredProjectInputs | None
    def __init__(
        self,
        project_dir: Path | None = None,
        no_sql_validation: bool = False,
        defer_to: str | None = None,
        defer_clone_from: str | None = None,
        defer_sources_to: str | None = None,
        selected_target: str | None = None,
        cursor_overrides: CursorOverrides | None = None,
        no_color: bool = False,
        fail_fast: bool = False,
        full_refresh: bool = False,
        load_sources: bool | None = None,
        reload_sources: bool = False,
        include_python: bool = True,
        allow_snapshot_full_refresh: bool = False,
        allow_table_type_downgrade: bool = False,
        allow_retention_decrease: bool = False,
        allow_missing_migration_origin: bool = False,
        allow_snapshot_schema_change: bool = False,
        concurrency: int | None = None,
        select: tuple[str, ...] = (),
        exclude: tuple[str, ...] = (),
        verbose: bool = False,
        debug: bool = False,
        cli_vars: dict[str, object] | None = None,
        run_tests: bool = True,
        run_audits: bool = True,
        manifest: bool = False,
        json_output: bool = False,
        json_output_path: Path | None = None,
        event_output_path: Path | None = None,
        no_cache: bool = False,
        max_microbatches: int | None = None,
        selection_diagnostics: bool = False,
        selector_files: tuple[SelectorFileSummary, ...] = (),
        test_timeout: str | None = None,
        discovered_inputs: DiscoveredProjectInputs | None = None,
    ) -> None: ...

class CloneCommandRequest(NativeRecord):
    """CLI inputs for one clone command invocation."""

    project_dir: Path | None
    no_color: bool
    no_sql_validation: bool
    origin_target_name: str
    destination_target_name: str | None
    hard_copy: bool
    select: tuple[str, ...]
    exclude: tuple[str, ...]
    verbose: bool
    cli_vars: dict[str, object] | None
    json_output_path: Path | None
    event_output_path: Path | None
    discovered_inputs: DiscoveredProjectInputs | None
    def __init__(
        self,
        project_dir: Path | None,
        no_color: bool,
        no_sql_validation: bool,
        origin_target_name: str,
        destination_target_name: str | None,
        hard_copy: bool,
        select: tuple[str, ...] = (),
        exclude: tuple[str, ...] = (),
        verbose: bool = False,
        cli_vars: dict[str, object] | None = None,
        json_output_path: Path | None = None,
        event_output_path: Path | None = None,
        discovered_inputs: DiscoveredProjectInputs | None = None,
    ) -> None: ...

class PlanSafetyGates(NativeRecord):
    """Pre-confirmed plan safety gates and the guidance shown when one needs a terminal."""

    allow_missing_migration_origin: bool
    allow_snapshot_full_refresh: bool
    allow_table_type_downgrade: bool
    allow_retention_decrease: bool
    missing_origin_help: str
    snapshot_full_refresh_help: str
    table_type_downgrade_help: str
    retention_decrease_help: str
    execution_limit_note: str
    def __init__(
        self,
        allow_missing_migration_origin: bool,
        allow_snapshot_full_refresh: bool,
        allow_table_type_downgrade: bool,
        allow_retention_decrease: bool,
        missing_origin_help: str,
        snapshot_full_refresh_help: str,
        table_type_downgrade_help: str,
        retention_decrease_help: str,
        execution_limit_note: str,
    ) -> None: ...

class DbtInitCommandRequest(NativeRecord):
    """CLI inputs for one dbt init command invocation."""

    cwd: Path
    dbt_project_dir: str | None
    profiles_dir: str | None
    profile_name: str | None
    target_name: str | None
    sqb_output_dir: str | None
    dry_run: bool
    overwrite: bool
    skip_dbt_debug: bool
    def __init__(
        self,
        cwd: Path,
        dbt_project_dir: str | None,
        profiles_dir: str | None,
        profile_name: str | None,
        target_name: str | None,
        sqb_output_dir: str | None,
        dry_run: bool,
        overwrite: bool,
        skip_dbt_debug: bool,
    ) -> None: ...

class DiffCommandRequest(NativeRecord):
    """CLI inputs for one diff command invocation."""

    project_dir: Path | None
    no_color: bool
    no_sql_validation: bool
    from_name: str | None
    to_name: str | None
    full: bool
    schema_only: bool
    bounded: str | None
    max_column_examples: int | None
    max_row_only_examples: int | None
    sample_rows: int | None
    sample_seed: int | None
    exhaustive: bool
    json_output: bool
    json_output_path: Path | None
    max_models: int | None
    max_columns: int | None
    select: tuple[str, ...]
    exclude: tuple[str, ...]
    verbose: bool
    cli_vars: dict[str, object] | None
    selected_target: str | None
    left_query: str | None
    left_query_file: Path | None
    left_label: str | None
    right_query: str | None
    right_query_file: Path | None
    right_label: str | None
    unique_key_override: tuple[str, ...]
    unkeyed: bool
    excluded_columns_override: tuple[str, ...]
    tolerance_overrides: tuple[str, ...]
    max_value_length: int | None
    suppress_example_values: bool
    full_example_values: bool
    def __init__(
        self,
        project_dir: Path | None,
        no_color: bool,
        no_sql_validation: bool,
        from_name: str | None,
        to_name: str | None,
        full: bool,
        schema_only: bool,
        bounded: str | None,
        max_column_examples: int | None = None,
        max_row_only_examples: int | None = None,
        sample_rows: int | None = None,
        sample_seed: int | None = None,
        exhaustive: bool = False,
        json_output: bool = False,
        json_output_path: Path | None = None,
        max_models: int | None = None,
        max_columns: int | None = None,
        select: tuple[str, ...] = (),
        exclude: tuple[str, ...] = (),
        verbose: bool = False,
        cli_vars: dict[str, object] | None = None,
        selected_target: str | None = None,
        left_query: str | None = None,
        left_query_file: Path | None = None,
        left_label: str | None = None,
        right_query: str | None = None,
        right_query_file: Path | None = None,
        right_label: str | None = None,
        unique_key_override: tuple[str, ...] = (),
        unkeyed: bool = False,
        excluded_columns_override: tuple[str, ...] = (),
        tolerance_overrides: tuple[str, ...] = (),
        max_value_length: int | None = None,
        suppress_example_values: bool = False,
        full_example_values: bool = False,
    ) -> None: ...

class ReviewCommandRequest(NativeRecord):
    """CLI inputs for one `sqb review` invocation."""

    project_dir: Path | None
    no_color: bool
    no_sql_validation: bool
    selected_target: str | None
    select: tuple[str, ...]
    cli_vars: dict[str, object] | None
    json_output: bool
    since: str | None
    against: str | None
    downstream: str | None
    dry_run: bool
    no_clone: bool
    bounded: str | None
    markdown_path: Path | None
    concurrency: int | None
    verbose: bool
    debug: bool
    def __init__(
        self,
        project_dir: Path | None = None,
        no_color: bool = False,
        no_sql_validation: bool = False,
        selected_target: str | None = None,
        select: tuple[str, ...] = (),
        cli_vars: dict[str, object] | None = None,
        json_output: bool = False,
        since: str | None = None,
        against: str | None = None,
        downstream: str | None = None,
        dry_run: bool = False,
        no_clone: bool = False,
        bounded: str | None = None,
        markdown_path: Path | None = None,
        concurrency: int | None = None,
        verbose: bool = False,
        debug: bool = False,
    ) -> None: ...

class QueryDiffPreparation(NativeRecord):
    """Resolved raw-query diff adapter, connection, SQL, and limits."""

    adapter: BaseAdapter
    connection_config: dict[str, object]
    left_sql: str
    right_sql: str
    left_label: str
    right_label: str
    selected_target: str | None
    database: str | None
    schema: str
    run_id: str
    artifact_ttl: str
    effective_max_column_examples: int
    effective_max_row_only_examples: int
    def __init__(
        self,
        adapter: BaseAdapter,
        connection_config: dict[str, object],
        left_sql: str,
        right_sql: str,
        left_label: str,
        right_label: str,
        selected_target: str | None,
        database: str | None,
        schema: str,
        run_id: str,
        artifact_ttl: str,
        effective_max_column_examples: int,
        effective_max_row_only_examples: int,
    ) -> None: ...

class FreshnessCommandRequest(NativeRecord):
    """CLI inputs for one `sqb freshness` invocation."""

    project_dir: Path | None
    no_sql_validation: bool
    no_color: bool
    selected_target: str | None
    select: tuple[str, ...]
    exclude: tuple[str, ...]
    cli_vars: dict[str, object] | None
    json_output: bool
    json_output_path: Path | None
    fail_on_error: bool
    compare_state: bool
    fail_on_stale: bool
    def __init__(
        self,
        project_dir: Path | None = None,
        no_sql_validation: bool = False,
        no_color: bool = False,
        selected_target: str | None = None,
        select: tuple[str, ...] = (),
        exclude: tuple[str, ...] = (),
        cli_vars: dict[str, object] | None = None,
        json_output: bool = False,
        json_output_path: Path | None = None,
        fail_on_error: bool = False,
        compare_state: bool = False,
        fail_on_stale: bool = False,
    ) -> None: ...

class JanitorCommandRequest(NativeRecord):
    """CLI inputs for one janitor command invocation."""

    project_dir: Path | None
    no_color: bool
    auto_approve: bool
    retention_days: int | None
    direct_state_history_versions: int | None
    drop_old_name_views: tuple[str, ...]
    keep_in_use: bool
    selected_target: str | None
    as_target: str | None
    verbose: bool
    json_output: bool
    def __init__(
        self,
        project_dir: Path | None,
        no_color: bool = False,
        auto_approve: bool = False,
        retention_days: int | None = None,
        direct_state_history_versions: int | None = None,
        drop_old_name_views: tuple[str, ...] = (),
        keep_in_use: bool = False,
        selected_target: str | None = None,
        as_target: str | None = None,
        verbose: bool = False,
        json_output: bool = False,
    ) -> None: ...

class LoadCommandRequest(NativeRecord):
    """CLI inputs for one load command invocation."""

    project_dir: Path | None
    no_color: bool
    selected_target: str | None
    select: tuple[str, ...]
    exclude: tuple[str, ...]
    reload: bool
    concurrency: int | None
    cursor_overrides: CursorOverrides | None
    cli_vars: dict[str, object] | None
    json_output: bool
    json_output_path: Path | None
    def __init__(
        self,
        project_dir: Path | None,
        no_color: bool = False,
        selected_target: str | None = None,
        select: tuple[str, ...] = (),
        exclude: tuple[str, ...] = (),
        reload: bool = False,
        concurrency: int | None = None,
        cursor_overrides: CursorOverrides | None = None,
        cli_vars: dict[str, object] | None = None,
        json_output: bool = False,
        json_output_path: Path | None = None,
    ) -> None: ...

class PlanCommandRequest(NativeRecord):
    """CLI inputs for one plan command invocation."""

    project_dir: Path | None
    no_sql_validation: bool
    defer_to: str | None
    defer_sources_to: str | None
    selected_target: str | None
    as_target: str | None
    cursor_overrides: CursorOverrides | None
    json_output: bool
    full_refresh: bool
    load_sources: bool | None
    include_python: bool
    no_color: bool
    select: tuple[str, ...]
    exclude: tuple[str, ...]
    verbose: bool
    cli_vars: dict[str, object] | None
    no_cache: bool
    max_microbatches: int | None
    selection_diagnostics: bool
    def __init__(
        self,
        project_dir: Path | None = None,
        no_sql_validation: bool = False,
        defer_to: str | None = None,
        defer_sources_to: str | None = None,
        selected_target: str | None = None,
        as_target: str | None = None,
        cursor_overrides: CursorOverrides | None = None,
        json_output: bool = False,
        full_refresh: bool = False,
        load_sources: bool | None = None,
        include_python: bool = True,
        no_color: bool = False,
        select: tuple[str, ...] = (),
        exclude: tuple[str, ...] = (),
        verbose: bool = False,
        cli_vars: dict[str, object] | None = None,
        no_cache: bool = False,
        max_microbatches: int | None = None,
        selection_diagnostics: bool = False,
    ) -> None: ...

class PlaygroundCommandRequest(NativeRecord):
    """CLI inputs for one playground command invocation."""

    project_dir: Path | None
    target_path: str
    template: str
    def __init__(
        self,
        project_dir: Path | None,
        target_path: str,
        template: str = ...,
    ) -> None: ...

class ScenarioSnapshotLimitInputs(NativeRecord):
    """CLI snapshot capture-limit overrides for one scenario run."""

    max_snapshot_rows: int | None
    max_snapshot_total_rows: int | None
    max_snapshot_bytes: int | None
    max_snapshot_total_bytes: int | None
    force: bool
    def __init__(
        self,
        max_snapshot_rows: int | None = None,
        max_snapshot_total_rows: int | None = None,
        max_snapshot_bytes: int | None = None,
        max_snapshot_total_bytes: int | None = None,
        force: bool = False,
    ) -> None: ...

class ScenarioTestCommandRequest(NativeRecord):
    """CLI inputs for one `sqb scenario test` invocation."""

    project_dir: Path | None
    scenario_namespace: str | None
    no_sql_validation: bool
    no_color: bool
    selectors: tuple[str, ...]
    exclude: tuple[str, ...]
    retain: bool
    local: bool
    strict: bool
    concurrency: int | None
    sync_snapshots: bool
    refresh: bool
    limit_inputs: ScenarioSnapshotLimitInputs
    json_output: bool
    json_output_path: Path | None
    scenario_timeout: str | None
    since: str | None
    explain: bool
    def __init__(
        self,
        project_dir: Path | None = None,
        scenario_namespace: str | None = None,
        no_sql_validation: bool = False,
        no_color: bool = False,
        selectors: tuple[str, ...] = (),
        exclude: tuple[str, ...] = (),
        retain: bool = False,
        local: bool = False,
        strict: bool = False,
        concurrency: int | None = None,
        sync_snapshots: bool = False,
        refresh: bool = False,
        limit_inputs: ScenarioSnapshotLimitInputs = ...,
        json_output: bool = False,
        json_output_path: Path | None = None,
        scenario_timeout: str | None = None,
        since: str | None = None,
        explain: bool = False,
    ) -> None: ...

class ScenarioCaptureCommandRequest(NativeRecord):
    """CLI inputs for one `sqb scenario capture` invocation."""

    project_dir: Path | None
    scenario_namespace: str | None
    no_sql_validation: bool
    no_color: bool
    selectors: tuple[str, ...]
    exclude: tuple[str, ...]
    retain: bool
    limit_inputs: ScenarioSnapshotLimitInputs
    def __init__(
        self,
        project_dir: Path | None = None,
        scenario_namespace: str | None = None,
        no_sql_validation: bool = False,
        no_color: bool = False,
        selectors: tuple[str, ...] = (),
        exclude: tuple[str, ...] = (),
        retain: bool = False,
        limit_inputs: ScenarioSnapshotLimitInputs = ...,
    ) -> None: ...

class TestCommandRequest(NativeRecord):
    """CLI inputs for one test command invocation."""

    project_dir: Path | None
    no_sql_validation: bool
    no_color: bool
    selected_target: str | None
    select: tuple[str, ...]
    exclude: tuple[str, ...]
    cli_vars: dict[str, object] | None
    json_output: bool
    json_output_path: Path | None
    concurrency: int | None
    case_name: str | None
    inspect: bool
    test_timeout: str | None
    since: str | None
    explain: bool
    def __init__(
        self,
        project_dir: Path | None = None,
        no_sql_validation: bool = False,
        no_color: bool = False,
        selected_target: str | None = None,
        select: tuple[str, ...] = (),
        exclude: tuple[str, ...] = (),
        cli_vars: dict[str, object] | None = None,
        json_output: bool = False,
        json_output_path: Path | None = None,
        concurrency: int | None = None,
        case_name: str | None = None,
        inspect: bool = False,
        test_timeout: str | None = None,
        since: str | None = None,
        explain: bool = False,
    ) -> None: ...

class RefactorCommandRequest(NativeRecord):
    """CLI inputs for one `sqb rename` or `sqb mv` invocation."""

    command: str
    target: str | None
    new_name: str | None
    destination: str | None
    more_moves: tuple[str, ...]
    from_file: str | None
    project_dir: Path | None
    cascade: bool
    dry_run: bool
    json_output: bool
    no_color: bool
    def __init__(
        self,
        command: str,
        target: str | None = None,
        new_name: str | None = None,
        destination: str | None = None,
        more_moves: tuple[str, ...] = (),
        from_file: str | None = None,
        project_dir: Path | None = None,
        cascade: bool = False,
        dry_run: bool = False,
        json_output: bool = False,
        no_color: bool = False,
    ) -> None: ...

class ScopeCommandRequest(NativeRecord):
    """CLI inputs for one offline declaration-scope inspection."""

    project_dir: Path | None
    target: str | None
    at: str | None
    as_path: str | None
    browse: str | None
    list_path: str | None
    defined_under: str | None
    kinds: tuple[str, ...]
    match: str | None
    used_only: bool
    include_nearby: bool
    nearby_depth: int
    dependency_depth: int
    explain: str | None
    globals: str
    page_size: int
    after: str | None
    paths: str
    json_output: bool
    no_cache: bool
    verbose: bool
    no_color: bool
    def __init__(
        self,
        project_dir: Path | None = None,
        target: str | None = None,
        at: str | None = None,
        as_path: str | None = None,
        browse: str | None = None,
        list_path: str | None = None,
        defined_under: str | None = None,
        kinds: tuple[str, ...] = (),
        match: str | None = None,
        used_only: bool = False,
        include_nearby: bool = False,
        nearby_depth: int = 1,
        dependency_depth: int = 0,
        explain: str | None = None,
        globals: str = "summary",
        page_size: int = 100,
        after: str | None = None,
        paths: str = "relative",
        json_output: bool = False,
        no_cache: bool = False,
        verbose: bool = False,
        no_color: bool = False,
    ) -> None: ...

class RulesCommandRequest(NativeRecord):
    """Inputs for focused rule execution, inspection, or skill generation."""

    project_dir: Path | None
    json_output: bool
    action: str
    rule_selector: str | None
    skills_check: bool
    select: tuple[str, ...]
    exclude: tuple[str, ...]
    def __init__(
        self,
        project_dir: Path | None,
        json_output: bool,
        action: str,
        rule_selector: str | None,
        skills_check: bool,
        select: tuple[str, ...] = (),
        exclude: tuple[str, ...] = (),
    ) -> None: ...

class CostCommandRequest(NativeRecord):
    """Inputs for local run-cost history and drill-down."""

    project_dir: Path | None
    selector: str
    no_color: bool
    limit: int | None
    no_limit: bool
    sort: str | None
    order: str | None
    since: str | None
    until: str | None
    json_output: bool
    json_output_path: Path | None
    def __init__(
        self,
        project_dir: Path | None,
        selector: str,
        no_color: bool,
        limit: int | None,
        no_limit: bool,
        sort: str | None,
        order: str | None,
        since: str | None,
        until: str | None,
        json_output: bool,
        json_output_path: Path | None,
    ) -> None: ...

class BuildCostFinalization(NativeRecord):
    """Inputs for nonfatal automatic build-cost finalization."""

    project_dir: Path
    adapter_name: str
    adapter: object
    connection_config: dict[str, object]
    target_name: str | None
    target_database: str | None
    run_id: str
    build_status: str
    started_at: datetime
    completed_at: datetime
    config: CostConfig
    output_stream: TextIO
    use_color: bool
    collect: bool
    render: bool
    cost_status: CostStatus
    cost_message: str
    had_executable_work: bool | None
    def __init__(
        self,
        project_dir: Path,
        adapter_name: str,
        adapter: object,
        connection_config: dict[str, object],
        target_name: str | None,
        target_database: str | None,
        run_id: str,
        build_status: str,
        started_at: datetime,
        completed_at: datetime,
        config: CostConfig,
        output_stream: TextIO,
        use_color: bool,
        collect: bool = True,
        render: bool = True,
        cost_status: CostStatus = ...,
        cost_message: str = ...,
        had_executable_work: bool | None = True,
    ) -> None: ...

class LineageCommandRequest(NativeRecord):
    """CLI inputs for dependency and column lineage inspection."""

    project_dir: Path | None
    no_sql_validation: bool
    targets: tuple[str, ...]
    output_format: str
    direction: str | None
    depth: str
    select: tuple[str, ...]
    exclude: tuple[str, ...]
    lineage_mode: ColumnLineageMode
    include_uses: bool
    cli_vars: dict[str, object] | None
    def __init__(
        self,
        project_dir: Path | None,
        no_sql_validation: bool = False,
        targets: tuple[str, ...] = (),
        output_format: str = "tree",
        direction: str | None = None,
        depth: str = "all",
        select: tuple[str, ...] = (),
        exclude: tuple[str, ...] = (),
        lineage_mode: ColumnLineageMode = ...,
        include_uses: bool = False,
        cli_vars: dict[str, object] | None = None,
    ) -> None: ...

class CompileProfileFlags(NativeRecord):
    """Profiling toggles that skip compile phases for benchmarking."""

    skip_discovery_sql_analysis: bool
    skip_column_inference: bool
    skip_contracts: bool
    skip_write: bool
    def __init__(
        self,
        skip_discovery_sql_analysis: bool = False,
        skip_column_inference: bool = False,
        skip_contracts: bool = False,
        skip_write: bool = False,
    ) -> None: ...

class CompileCommandRequest(NativeRecord):
    """CLI inputs for one `sqb compile` invocation."""

    project_dir: Path | None
    no_sql_validation: bool
    defer_to: str | None
    selected_target: str | None
    json_output: bool
    manifest: bool
    dag_path: str | None
    no_color: bool
    lineage_mode: CompileLineageMode
    select: tuple[str, ...]
    exclude: tuple[str, ...]
    cli_vars: dict[str, object] | None
    profile_flags: CompileProfileFlags
    no_cache: bool
    debug: bool
    def __init__(
        self,
        project_dir: Path | None = None,
        no_sql_validation: bool = False,
        defer_to: str | None = None,
        selected_target: str | None = None,
        json_output: bool = False,
        manifest: bool = False,
        dag_path: str | None = None,
        no_color: bool = False,
        lineage_mode: CompileLineageMode = ...,
        select: tuple[str, ...] = (),
        exclude: tuple[str, ...] = (),
        cli_vars: dict[str, object] | None = None,
        profile_flags: CompileProfileFlags = ...,
        no_cache: bool = False,
        debug: bool = False,
    ) -> None: ...

class SelectorFileSummary(NativeRecord):
    """Non-expanded provenance for one selector file."""

    path: Path
    selector_count: int
    def __init__(
        self,
        path: Path,
        selector_count: int,
    ) -> None: ...

class ParsedCliInvocation(NativeRecord):
    """Outcome of parsing CLI arguments: either a namespace or an exit code."""

    args: CliNamespace | None
    exit_code: int | None
    def __init__(
        self,
        args: CliNamespace | None,
        exit_code: int | None,
    ) -> None: ...

class CliEntrypointHandlers(NativeRecord):
    """Injected command handlers for the CLI entrypoint."""

    run_compile: Callable[[CompileCommandRequest], int]
    run_cost: Callable[..., int]
    run_dag: DagCommandHandler
    run_plan: Callable[..., int]
    run_dbt_plan: Callable[[Path | None, tuple[str, ...], bool], int]
    run_dbt_run: Callable[[Path | None, tuple[str, ...], bool], int]
    run_dbt_build: Callable[[Path | None, tuple[str, ...], bool], int]
    run_dbt_debug: Callable[[Path | None, tuple[str, ...], bool], int]
    run_dbt_init: Callable[..., int]
    run_build: Callable[..., int]
    run_freshness: Callable[..., int]
    run_test: Callable[..., int]
    run_check: Callable[..., int]
    run_audit: Callable[..., int]
    run_seed: Callable[..., int]
    run_load: Callable[..., int]
    run_clone: Callable[..., int]
    run_diff: Callable[..., int]
    run_query: QueryCommandHandler
    run_debug: DebugCommandHandler
    run_lineage: LineageCommandHandler
    run_janitor: Callable[..., int]
    run_init: Callable[[Path | None], int]
    run_playground: Callable[..., int]
    run_skills_update: SkillsUpdateCommandHandler
    run_format: FormatCommandHandler
    run_scenario: Callable[..., int]
    run_scenario_capture: Callable[..., int]
    run_rules: Callable[..., int]
    run_scope: ScopeCommandHandler
    run_contract: Callable[..., int] | None
    run_refactor: Callable[..., int] | None
    run_recall: Callable[..., int] | None
    run_review: Callable[..., int] | None
    run_location: Callable[..., int] | None
    def __init__(
        self,
        run_compile: Callable[[CompileCommandRequest], int],
        run_cost: Callable[..., int],
        run_dag: DagCommandHandler,
        run_plan: Callable[..., int],
        run_dbt_plan: Callable[[Path | None, tuple[str, ...], bool], int],
        run_dbt_run: Callable[[Path | None, tuple[str, ...], bool], int],
        run_dbt_build: Callable[[Path | None, tuple[str, ...], bool], int],
        run_dbt_debug: Callable[[Path | None, tuple[str, ...], bool], int],
        run_dbt_init: Callable[..., int],
        run_build: Callable[..., int],
        run_freshness: Callable[..., int],
        run_test: Callable[..., int],
        run_check: Callable[..., int],
        run_audit: Callable[..., int],
        run_seed: Callable[..., int],
        run_load: Callable[..., int],
        run_clone: Callable[..., int],
        run_diff: Callable[..., int],
        run_query: QueryCommandHandler,
        run_debug: DebugCommandHandler,
        run_lineage: LineageCommandHandler,
        run_janitor: Callable[..., int],
        run_init: Callable[[Path | None], int],
        run_playground: Callable[..., int],
        run_skills_update: SkillsUpdateCommandHandler,
        run_format: FormatCommandHandler,
        run_scenario: Callable[..., int],
        run_scenario_capture: Callable[..., int],
        run_rules: Callable[..., int],
        run_scope: ScopeCommandHandler,
        run_contract: Callable[..., int] | None = None,
        run_refactor: Callable[..., int] | None = None,
        run_recall: Callable[..., int] | None = None,
        run_review: Callable[..., int] | None = None,
        run_location: Callable[..., int] | None = None,
    ) -> None: ...

class CommandWarehouseScope(NativeRecord):
    """The warehouse group and CLI override active for one command invocation."""

    group: WarehouseGroup | None
    cli_warehouse: str | None
    cli_vars: dict[str, object]
    def __init__(
        self,
        group: WarehouseGroup | None,
        cli_warehouse: str | None = None,
        cli_vars: dict[str, object] = ...,
    ) -> None: ...

class ResolvedWarehouse(NativeRecord):
    """The warehouse one command connects with and where that value came from."""

    warehouse: str | None
    source: str
    def __init__(
        self,
        warehouse: str | None,
        source: str,
    ) -> None: ...

class WrittenTarget(NativeRecord):
    """Result of writing compiled output to target/."""

    model_count: int
    seed_count: int
    function_count: int
    audit_count: int
    test_count: int
    target_dir: Path
    diagnostics: tuple[CompilerDiagnostic, ...]
    failed_compile: bool
    failed_model_sql: tuple[str, ...]
    def __init__(
        self,
        model_count: int,
        seed_count: int,
        function_count: int,
        audit_count: int,
        test_count: int,
        target_dir: Path,
        diagnostics: tuple[CompilerDiagnostic, ...] = ...,
        failed_compile: bool = ...,
        failed_model_sql: tuple[str, ...] = ...,
    ) -> None: ...

class SkillMaintenanceResult(NativeRecord):
    """Non-blocking generated-skill maintenance result."""

    message: str
    def __init__(
        self,
        message: str = "",
    ) -> None: ...

class SkillUpdateResult(NativeRecord):
    """Result of installing or updating skill files."""

    written_paths: tuple[Path, ...]
    def __init__(
        self,
        written_paths: tuple[Path, ...],
    ) -> None: ...

class DbtManifestModel(NativeRecord):
    """One dbt model node needed for SQLBuild dbt_ref resolution."""

    unique_id: str
    package_name: str
    name: str
    relation_name: str
    database: str | None
    schema: str | None
    alias: str | None
    node_checksum: str | None
    fqn: tuple[str, ...]
    query_sql: str
    depends_on_nodes: tuple[str, ...]
    payload: dict[str, object]
    def __init__(
        self,
        unique_id: str,
        package_name: str,
        name: str,
        relation_name: str,
        database: str | None = None,
        schema: str | None = None,
        alias: str | None = None,
        node_checksum: str | None = None,
        fqn: tuple[str, ...] = ...,
        query_sql: str = "",
        depends_on_nodes: tuple[str, ...] = ...,
        payload: dict[str, object] = ...,
    ) -> None: ...

class DbtManifestSource(NativeRecord):
    """One dbt source node needed for graph and lineage resolution."""

    unique_id: str
    package_name: str
    source_name: str
    name: str
    relation_name: str
    database: str | None
    schema: str | None
    identifier: str | None
    payload: dict[str, object]
    def __init__(
        self,
        unique_id: str,
        package_name: str,
        source_name: str,
        name: str,
        relation_name: str,
        database: str | None = None,
        schema: str | None = None,
        identifier: str | None = None,
        payload: dict[str, object] = ...,
    ) -> None: ...

class DbtManifestSeed(NativeRecord):
    """One dbt seed node, a leaf data relation used for column lineage."""

    unique_id: str
    package_name: str
    name: str
    relation_name: str
    database: str | None
    schema: str | None
    alias: str | None
    payload: dict[str, object]
    def __init__(
        self,
        unique_id: str,
        package_name: str,
        name: str,
        relation_name: str,
        database: str | None = None,
        schema: str | None = None,
        alias: str | None = None,
        payload: dict[str, object] = ...,
    ) -> None: ...

class DbtManifestIndex(NativeRecord):
    """Lookup indexes for dbt model and source nodes in a manifest."""

    models_by_unique_id: dict[str, DbtManifestModel]
    models_by_name: dict[str, tuple[DbtManifestModel, ...]]
    models_by_package_and_name: dict[tuple[str, str], DbtManifestModel]
    sources_by_unique_id: dict[str, DbtManifestSource]
    seeds_by_unique_id: dict[str, DbtManifestSeed]
    validation_depends_on_nodes_by_unique_id: dict[str, tuple[str, ...]]
    def __init__(
        self,
        models_by_unique_id: dict[str, DbtManifestModel],
        models_by_name: dict[str, tuple[DbtManifestModel, ...]],
        models_by_package_and_name: dict[tuple[str, str], DbtManifestModel],
        sources_by_unique_id: dict[str, DbtManifestSource] = ...,
        seeds_by_unique_id: dict[str, DbtManifestSeed] = ...,
        validation_depends_on_nodes_by_unique_id: dict[str, tuple[str, ...]] = ...,
    ) -> None: ...

class DbtCliConfigOverrides(NativeRecord):
    """dbt config values supplied by CLI flags."""

    project_dir: str | None
    profiles_dir: str | None
    target: str | None
    target_path: str | None
    def __init__(
        self,
        project_dir: str | None = None,
        profiles_dir: str | None = None,
        target: str | None = None,
        target_path: str | None = None,
    ) -> None: ...

class ResolvedDbtConfig(NativeRecord):
    """Resolved dbt config after applying CLI overrides."""

    project_dir: Path | None
    profiles_dir: Path | None
    target: str | None
    target_path: Path | None
    def __init__(
        self,
        project_dir: Path | None,
        profiles_dir: Path | None,
        target: str | None,
        target_path: Path | None,
    ) -> None: ...

class DbtNodeMessage(NativeRecord):
    """One dbt log message attached to a dbt node."""

    level: str
    message: str
    def __init__(
        self,
        level: str,
        message: str,
    ) -> None: ...

class DbtCommandExecutionResult(NativeRecord):
    """dbt command execution output from streamed JSON events."""

    returncode: int
    node_results: tuple[DbtNodeExecutionResult, ...]
    def __init__(
        self,
        returncode: int,
        node_results: tuple[DbtNodeExecutionResult, ...] = ...,
    ) -> None: ...

class DbtLsNode(NativeRecord):
    """One node returned by `dbt ls --output json`."""

    unique_id: str
    resource_type: str | None
    package_name: str | None
    name: str | None
    fqn: tuple[str, ...]
    original_file_path: str | None
    def __init__(
        self,
        unique_id: str,
        resource_type: str | None = None,
        package_name: str | None = None,
        name: str | None = None,
        fqn: tuple[str, ...] = ...,
        original_file_path: str | None = None,
    ) -> None: ...

class DbtInteropParsedArgs(NativeRecord):
    """Declared `sqb dbt` flags parsed by argparse, before routing to buckets."""

    select: tuple[str, ...]
    exclude: tuple[str, ...]
    vars: str | None
    threads: str | None
    full_refresh: bool
    event_time_start: str | None
    event_time_end: str | None
    project_dir: str | None
    profiles_dir: str | None
    profile: str | None
    target: str | None
    target_path: str | None
    state: str | None
    indirect_selection: str | None
    defer: bool
    start_cursor_ts: str | None
    end_cursor_ts: str | None
    start_cursor_int: str | None
    end_cursor_int: str | None
    defer_to: str | None
    fail_fast: bool
    hard_copy: bool
    dbt_passthrough: tuple[str, ...]
    def __init__(
        self,
        select: tuple[str, ...] = ...,
        exclude: tuple[str, ...] = ...,
        vars: str | None = None,
        threads: str | None = None,
        full_refresh: bool = False,
        event_time_start: str | None = None,
        event_time_end: str | None = None,
        project_dir: str | None = None,
        profiles_dir: str | None = None,
        profile: str | None = None,
        target: str | None = None,
        target_path: str | None = None,
        state: str | None = None,
        indirect_selection: str | None = None,
        defer: bool = False,
        start_cursor_ts: str | None = None,
        end_cursor_ts: str | None = None,
        start_cursor_int: str | None = None,
        end_cursor_int: str | None = None,
        defer_to: str | None = None,
        fail_fast: bool = False,
        hard_copy: bool = False,
        dbt_passthrough: tuple[str, ...] = ...,
    ) -> None: ...

class DbtInteropRoutedArgs(NativeRecord):
    """Arguments split for a future `sqb dbt` command execution."""

    command: DbtInteropCommand
    select: tuple[str, ...]
    exclude: tuple[str, ...]
    dbt_args: tuple[str, ...]
    sqlbuild_args: tuple[str, ...]
    def __init__(
        self,
        command: DbtInteropCommand,
        select: tuple[str, ...] = ...,
        exclude: tuple[str, ...] = ...,
        dbt_args: tuple[str, ...] = ...,
        sqlbuild_args: tuple[str, ...] = ...,
    ) -> None: ...

class DbtInteropSelectionResult(NativeRecord):
    """SQLBuild-side selection produced from `sqb dbt` selectors."""

    sqlbuild_model_names: tuple[str, ...]
    dbt_required_unique_ids: tuple[str, ...]
    dbt_anchor_terms: tuple[str, ...]
    dbt_anchor_unique_ids_by_term: dict[str, tuple[str, ...]]
    path_translations: tuple[tuple[str, str], ...]
    def __init__(
        self,
        sqlbuild_model_names: tuple[str, ...] = ...,
        dbt_required_unique_ids: tuple[str, ...] = ...,
        dbt_anchor_terms: tuple[str, ...] = ...,
        dbt_anchor_unique_ids_by_term: dict[str, tuple[str, ...]] = ...,
        path_translations: tuple[tuple[str, str], ...] = ...,
    ) -> None: ...

class DbtInteropPlan(NativeRecord):
    """Plan output for one future `sqb dbt` command."""

    command: DbtInteropCommand
    dbt_command_argv: tuple[str, ...]
    dbt_selected_nodes: tuple[DbtLsNode, ...]
    dbt_selected_unique_ids: tuple[str, ...]
    sqlbuild_command_argvs: tuple[tuple[str, ...], ...]
    selection: DbtInteropSelectionResult
    sqlbuild_plan_output: PlanOutput | None
    dbt_required_selector_terms: tuple[str, ...]
    supplemental_dbt_command_argvs: tuple[tuple[str, ...], ...]
    dbt_skip_reason: DbtInteropSkipReason | None
    sqlbuild_skip_reason: DbtInteropSkipReason | None
    warnings: tuple[str, ...]
    def __init__(
        self,
        command: DbtInteropCommand,
        dbt_command_argv: tuple[str, ...],
        dbt_selected_nodes: tuple[DbtLsNode, ...],
        dbt_selected_unique_ids: tuple[str, ...],
        sqlbuild_command_argvs: tuple[tuple[str, ...], ...],
        selection: DbtInteropSelectionResult,
        sqlbuild_plan_output: PlanOutput | None = None,
        dbt_required_selector_terms: tuple[str, ...] = ...,
        supplemental_dbt_command_argvs: tuple[tuple[str, ...], ...] = ...,
        dbt_skip_reason: DbtInteropSkipReason | None = None,
        sqlbuild_skip_reason: DbtInteropSkipReason | None = None,
        warnings: tuple[str, ...] = ...,
    ) -> None: ...

class DbtInteropExecutionRequest(NativeRecord):
    """Caller inputs for one dbt interop execution command."""

    command: DbtInteropCommand
    project_dir: Path
    args: tuple[str, ...]
    dbt_runner: Any | None
    dbt_executable: str | None
    sqlbuild_executable: str
    no_sql_validation: bool
    fail_fast: bool
    on_progress: Callable[[str], None] | None
    progress_stream: TextIO | None
    dbt_stdout_stream: TextIO | None
    use_color: bool
    verbose: bool
    json_output: bool
    def __init__(
        self,
        command: DbtInteropCommand,
        project_dir: Path,
        args: tuple[str, ...],
        dbt_runner: Any | None = None,
        dbt_executable: str | None = None,
        sqlbuild_executable: str = "sqb",
        no_sql_validation: bool = False,
        fail_fast: bool = False,
        on_progress: Callable[[str], None] | None = None,
        progress_stream: TextIO | None = None,
        dbt_stdout_stream: TextIO | None = None,
        use_color: bool = False,
        verbose: bool = False,
        json_output: bool = False,
    ) -> None: ...

class DbtInteropInvocation(NativeRecord):
    """Resolved execution environment for one dbt interop command."""

    dbt_executable: str
    output_stream: TextIO
    dbt_output_stream: TextIO
    routed: DbtInteropRoutedArgs
    discovered_inputs: DiscoveredProjectInputs
    effective_sqlbuild_args: tuple[str, ...]
    dbt_options: DbtCliOptions
    dbt_vars: dict[str, object]
    runner: Any
    def __init__(
        self,
        dbt_executable: str,
        output_stream: TextIO,
        dbt_output_stream: TextIO,
        routed: DbtInteropRoutedArgs,
        discovered_inputs: DiscoveredProjectInputs,
        effective_sqlbuild_args: tuple[str, ...],
        dbt_options: DbtCliOptions,
        dbt_vars: dict[str, object],
        runner: Any,
    ) -> None: ...

class DbtInteropCompiledProject(NativeRecord):
    """Adapter and compiled SQLBuild project for dbt interop planning."""

    adapter_name: str
    adapter: BaseAdapter
    project: CompiledProjectView
    def __init__(
        self,
        adapter_name: str,
        adapter: BaseAdapter,
        project: CompiledProjectView,
    ) -> None: ...

class DbtInteropPlanResolution(NativeRecord):
    """Combined dbt graph and the resolved interop plan."""

    graph: DbtCombinedGraph
    plan: DbtInteropPlan
    def __init__(
        self,
        graph: DbtCombinedGraph,
        plan: DbtInteropPlan,
    ) -> None: ...

class DbtInteropCommandArgs(NativeRecord):
    """Selection, passthrough args, and executables for a dbt interop command."""

    select: tuple[str, ...]
    exclude: tuple[str, ...]
    dbt_command_args: tuple[str, ...]
    sqlbuild_command_args: tuple[str, ...]
    dbt_executable: str
    sqlbuild_executable: str
    def __init__(
        self,
        select: tuple[str, ...],
        exclude: tuple[str, ...] = (),
        dbt_command_args: tuple[str, ...] = (),
        sqlbuild_command_args: tuple[str, ...] = (),
        dbt_executable: str = "dbt",
        sqlbuild_executable: str = "sqb",
    ) -> None: ...

class SourceFreshnessComparableRecord(Protocol):
    @property
    def data_version_hash(self) -> str: ...
    @property
    def value_kind(self) -> str: ...
    @property
    def data_version(self) -> str | None: ...

class SourceFreshnessAgeStatus(StrEnum):
    PASS = "pass"
    WARN = "warn"
    ERROR = "error"
    UNKNOWN = "unknown"

class SourceFreshnessUnknownReason(StrEnum):
    """Why one source's freshness could not be observed."""

    UNAVAILABLE = "unavailable"
    ERROR = "error"
    NO_FRESHNESS_CONFIG = "no_freshness_config"

class SourceFreshnessRenderers(NativeRecord):
    """Adapter SQL renderers used when writing source freshness rows."""

    render_qualified_name: Callable[..., str | None]
    render_framework_type: Callable[[FrameworkType], str]
    render_insert_records_sql: Callable[..., str]
    render_create_table_sql: Callable[..., str] | None
    render_create_index_sqls: Callable[..., tuple[str, ...]] | None
    def __init__(
        self,
        render_qualified_name: Callable[..., str | None],
        render_framework_type: Callable[[FrameworkType], str],
        render_insert_records_sql: Callable[..., str],
        render_create_table_sql: Callable[..., str] | None = None,
        render_create_index_sqls: Callable[..., tuple[str, ...]] | None = None,
    ) -> None: ...

class SourceFreshnessObservation(NativeRecord):
    """One comparable observation whose datetime values are always aware UTC."""

    source_name: str
    strategy: SourceFreshnessStrategy
    data_version: object
    value_kind: SourceFreshnessValueKind
    observed_at: datetime
    def __init__(
        self,
        source_name: str,
        strategy: SourceFreshnessStrategy,
        data_version: object,
        value_kind: SourceFreshnessValueKind,
        observed_at: datetime,
    ) -> None: ...

class SourceFreshnessUnknown(NativeRecord):
    """One source whose freshness could not be observed, with the reason."""

    source_name: str
    reason: SourceFreshnessUnknownReason
    message: str
    def __init__(
        self,
        source_name: str,
        reason: SourceFreshnessUnknownReason,
        message: str,
    ) -> None: ...

class AdapterSourceFreshnessBatch(NativeRecord):
    """Per-source outcomes of one batched adapter metadata observation."""

    observations: dict[str, SourceFreshnessObservation]
    unknown: dict[str, SourceFreshnessUnknown]
    def __init__(
        self,
        observations: dict[str, SourceFreshnessObservation] = ...,
        unknown: dict[str, SourceFreshnessUnknown] = ...,
    ) -> None: ...

class SourceFreshnessIdentity(NativeRecord):
    """Stable identity for one source freshness stream in direct state."""

    source_name: str
    target_database: str | None
    target_schema: str | None
    target_name: str | None
    def __init__(
        self,
        source_name: str,
        target_database: str | None,
        target_schema: str | None,
        target_name: str | None,
    ) -> None: ...

class SourceFreshnessSet(NativeRecord):
    """Latest direct source freshness records for one target schema."""

    schema: str
    records: dict[SourceFreshnessIdentity, SourceFreshnessRecord]
    def __init__(
        self,
        schema: str,
        records: dict[SourceFreshnessIdentity, SourceFreshnessRecord],
    ) -> None: ...

class DirectSourceFreshnessPropagationResult(NativeRecord):
    """Downstream model impact derived from direct source freshness roots."""

    changed_source_model_names: dict[SourceFreshnessIdentity, frozenset[str]]
    unknown_source_model_names: dict[str, frozenset[str]]
    error_source_model_names: dict[SourceFreshnessIdentity, frozenset[str]]
    stale_model_names: frozenset[str]
    blocked_model_names: frozenset[str]
    def __init__(
        self,
        changed_source_model_names: dict[SourceFreshnessIdentity, frozenset[str]] = ...,
        unknown_source_model_names: dict[str, frozenset[str]] = ...,
        error_source_model_names: dict[SourceFreshnessIdentity, frozenset[str]] = ...,
        stale_model_names: frozenset[str] = ...,
        blocked_model_names: frozenset[str] = ...,
    ) -> None: ...

class QueryCostObservation(NativeRecord):
    query_id: str
    warehouse_name: str
    warehouse_size: str
    warehouse_type: str | None
    cluster_number: int | None
    started_at: datetime
    completed_at: datetime
    execution_ms: int
    bytes_scanned: int
    execution_status: str
    run_id: str | None
    resource_type: str | None
    resource_name: str | None
    def __init__(
        self,
        query_id: str,
        warehouse_name: str,
        warehouse_size: str,
        warehouse_type: str | None,
        cluster_number: int | None,
        started_at: datetime,
        completed_at: datetime,
        execution_ms: int,
        bytes_scanned: int,
        execution_status: str,
        run_id: str | None = None,
        resource_type: str | None = None,
        resource_name: str | None = None,
    ) -> None: ...

class ResourceCost(NativeRecord):
    resource_type: str
    resource_name: str
    warehouse_name: str
    warehouse_size: str
    warehouse_type: str | None
    cluster_number: int | None
    query_count: int
    attributed_seconds: Decimal
    bytes_scanned: int
    estimated_compute_credits: Decimal
    estimated_usd: Decimal
    def __init__(
        self,
        resource_type: str,
        resource_name: str,
        warehouse_name: str,
        warehouse_size: str,
        warehouse_type: str | None,
        cluster_number: int | None,
        query_count: int,
        attributed_seconds: Decimal,
        bytes_scanned: int,
        estimated_compute_credits: Decimal,
        estimated_usd: Decimal,
    ) -> None: ...

class CostRunRecord(NativeRecord):
    run_id: str
    adapter_name: str
    target_name: str | None
    build_status: str
    started_at: datetime
    completed_at: datetime
    cost: RunCostSummary
    version: int
    had_executable_work: bool | None
    def __init__(
        self,
        run_id: str,
        adapter_name: str,
        target_name: str | None,
        build_status: str,
        started_at: datetime,
        completed_at: datetime,
        cost: RunCostSummary,
        version: int = 1,
        had_executable_work: bool | None = None,
    ) -> None: ...

class StatementLedgerEntry(NativeRecord):
    statement_id: str
    run_id: str
    resource_type: str
    resource_name: str
    phase: str
    attempt: int
    query_id: str | None
    status: str
    started_at: datetime
    completed_at: datetime
    def __init__(
        self,
        statement_id: str,
        run_id: str,
        resource_type: str,
        resource_name: str,
        phase: str,
        attempt: int,
        query_id: str | None,
        status: str,
        started_at: datetime,
        completed_at: datetime,
    ) -> None: ...

class NativeRecord:
    """Base of every native frozen record, built, compared, shown and pickled as a dataclass."""

    __dataclass_fields__: ClassVar[dict[str, Field[Any]]]
    __match_args__: ClassVar[tuple[str, ...]]

class CompileLineageMode(StrEnum):
    """Column lineage mode for compile output."""

    FAST = "fast"
    RICH = "rich"
    NONE = "none"

class CliCommand(StrEnum):
    COMPILE = "compile"
    DAG = "dag"
    PLAN = "plan"
    FRESHNESS = "freshness"
    BUILD = "build"
    TEST = "test"
    CHECK = "check"
    AUDIT = "audit"
    LOAD = "load"
    SEED = "seed"
    CLONE = "clone"
    DIFF = "diff"
    REVIEW = "review"
    DEBUG = "debug"
    LOCATION = "location"
    LINEAGE = "lineage"
    QUERY = "query"
    COST = "cost"
    CLEAN = "clean"
    JANITOR = "janitor"
    INIT = "init"
    PLAYGROUND = "playground"
    SCENARIO = "scenario"
    DBT = "dbt"
    SKILLS = "skills"
    FORMAT = "format"
    RULES = "rules"
    SCOPE = "scope"
    CONTRACT = "contract"
    RENAME = "rename"
    MV = "mv"
    RECALL = "recall"

class ScopeCommandHandler(Protocol):
    def __call__(self, *, request: ScopeCommandRequest) -> int: ...

class ScopeIndexLoader(Protocol):
    def __call__(self, *, project_dir: Path, no_cache: bool = False) -> ScopeIndex: ...

class DagCommandHandler(Protocol):
    def __call__(
        self,
        project_dir: Path | None,
        *,
        no_sql_validation: bool,
        json_output: bool,
        cli_vars: dict[str, object] | None,
    ) -> int: ...

class QueryCommandHandler(Protocol):
    def __call__(
        self,
        project_dir: Path | None,
        *,
        sql: str | None,
        query_file: Path | None,
        selected_target: str | None,
        output_format: str,
        limit: int | None,
    ) -> int: ...

class DebugCommandHandler(Protocol):
    def __call__(
        self,
        project_dir: Path | None,
        *,
        no_color: bool,
        no_connection: bool,
        selected_target: str | None,
        json_output: bool,
    ) -> int: ...

class LineageCommandHandler(Protocol):
    def __call__(self, request: LineageCommandRequest) -> int: ...

class SkillsUpdateCommandHandler(Protocol):
    def __call__(
        self,
        project_dir: Path | None,
        *,
        global_install: bool,
        targets: tuple[str, ...],
        force: bool,
    ) -> int: ...

class FormatCommandHandler(Protocol):
    def __call__(
        self,
        project_dir: Path | None,
        *,
        select: tuple[str, ...],
        exclude: tuple[str, ...],
        check: bool,
        diff: bool,
        fixtures_only: bool,
        fix: bool,
        json_output: bool,
        no_color: bool,
    ) -> int: ...

class PlaygroundTemplate(StrEnum):
    """Available `sqb playground` templates."""

    WAFFLE_SHOP = "waffle_shop"
    LOADER_WAFFLE_SHOP = "loader_waffle_shop"
    DAGSTER = "dagster"
    RIVERS = "rivers"
    PYTHON_NODES = "python_nodes"

class DbtInteropCommand(StrEnum):
    """dbt interop commands with SQLBuild participation."""

    PLAN = "plan"
    RUN = "run"
    BUILD = "build"
    DEBUG = "debug"

class DbtInteropSkipReason(StrEnum):
    """Reason one side of a dbt interop plan has no work."""

    NO_DBT_WORK = "no_dbt_work"
    NO_SQLBUILD_WORK = "no_sqlbuild_work"

class AuthoredSqlSet(NativeRecord):
    """Set literal entries retained in authored order until validation."""

    values: tuple[object, ...]
    def __init__(
        self,
        values: tuple[object, ...],
    ) -> None: ...

class AuthoredSqlValueCall(NativeRecord):
    """Source-preserving representation of an authored ``constant(...)`` call."""

    arguments: tuple[tuple[str, object], ...]
    def __init__(
        self,
        arguments: tuple[tuple[str, object], ...],
    ) -> None: ...

class SqlValueLimits(NativeRecord):
    """Safety limits applied while normalizing recursively authored values."""

    max_depth: int
    max_elements: int
    max_size: int
    def __init__(
        self,
        max_depth: int = ...,
        max_elements: int = ...,
        max_size: int = ...,
    ) -> None: ...

class SqlValueValidationError(ValueError):
    """An authored value cannot be represented by the typed SQL value domain."""

class PartialBuildPhaseTimings(NativeRecord):
    """Available disjoint phase durations after an exceptional build."""

    compile_seconds: float | None
    planning_seconds: float | None
    connection_preparation_seconds: float | None
    schema_preparation_seconds: float | None
    execution_seconds: float | None
    cost_collection_seconds: float | None
    total_seconds: float | None
    def __init__(
        self,
        compile_seconds: float | None = None,
        planning_seconds: float | None = None,
        connection_preparation_seconds: float | None = None,
        schema_preparation_seconds: float | None = None,
        execution_seconds: float | None = None,
        cost_collection_seconds: float | None = None,
        total_seconds: float | None = None,
    ) -> None: ...

class CliUserError(Exception):
    """Expected CLI-facing error that should be rendered without a traceback."""

    code: str
    message: str
    help: str | None
    def __init__(
        self, message: str, *, code: str | None = None, help: str | None = None
    ) -> None: ...

class StagedArtifactsChangedError(CliUserError):
    """Staged compile artifacts changed before publication, so nothing stale is deleted."""

class QueryExecutionError(CliUserError):
    """The warehouse rejected or failed an ad hoc query."""

    exit_code: int

class ContractFindingKind(StrEnum):
    """Stable physical-to-authored contract difference categories."""

    MISSING_DECLARATION = "missing_declaration"
    MISSING_PHYSICAL_COLUMN = "missing_physical_column"
    TYPE_MISMATCH = "type_mismatch"
    RELATION_MISSING = "relation_missing"
    INSPECTION_UNAVAILABLE = "inspection_unavailable"
    OWNERSHIP_CONFLICT = "ownership_conflict"

class CompilerStage(StrEnum):
    """Frontier objects handed from one compiler stage to the next, in pipeline order."""

    DISCOVERED_PROJECT_INPUTS = "discovered_project_inputs"
    COMPILE_PROJECT_INPUTS = "compile_project_inputs"
    COMPILED_PROJECT = "compiled_project"

class LintSeverity(StrEnum):
    """Severity assigned to one lint violation."""

    FAULT = "fault"
    WARNING = "warning"

class DiscoveryError(ValueError):
    """Base error for expected discovery failures."""

    code: str
    message: str
    help: str | None
    def __init__(
        self, message: str, *, code: str | None = None, help: str | None = None
    ) -> None: ...

class ProjectConfigError(DiscoveryError):
    """Raised when shared or local project config files are invalid."""

class ModelSqlParseError(DiscoveryError):
    """Raised when an authored SQL model file has an invalid shape."""

class SqlTestParseError(DiscoveryError):
    """Raised when an authored SQL-native test file has an invalid shape."""

class SqlAuditParseError(DiscoveryError):
    """Raised when an authored SQL audit file has an invalid shape."""

class SchemaParseError(DiscoveryError):
    """Raised when a schema.yml file has an invalid shape."""

class SourceParseError(DiscoveryError):
    """Raised when a sources/*.yml file has an invalid shape."""

class DiscoveryConflictError(DiscoveryError):
    """Raised when discovered project inputs conflict across files."""

class SeedDiscoveryError(DiscoveryError):
    """Raised when declared seed metadata does not match local seed files."""

class SqlScenarioParseError(DiscoveryError):
    """Raised when an authored SQL scenario file has an invalid shape."""

class PythonNodeDiscoveryError(DiscoveryError):
    """Raised when project Python nodes cannot be discovered."""

class ProviderDiscoveryError(DiscoveryError):
    """Raised when project providers cannot be discovered."""

class DeclarationParseError(DiscoveryError):
    """Raised when an authored public SQL declaration is invalid."""

class SqlHookParseError(DiscoveryError):
    """Raised when an authored SQL hook resource has an invalid shape."""

class EventExporterDiscoveryError(DiscoveryError):
    """Raised when project lifecycle event exporters are invalid."""

class ProjectPythonPathError(DiscoveryError):
    """Raised when project-owned Python lives outside a supported extension root."""

class ProjectPathError(DiscoveryError):
    """Raised when an authored project path cannot be read as valid UTF-8."""

class ResourceIdentityError(DiscoveryError):
    """Raised when an authored resource identity is not canonical snake_case."""

class FingerprintInputError(ValueError):
    """Raised when fingerprint storage inputs or rows are invalid."""

    code: str
    message: str
    help: str | None
    def __init__(
        self, message: str, *, code: str | None = None, help: str | None = None
    ) -> None: ...

class QueryFingerprintError(ValueError):
    """Raised when SQL cannot be tokenized into a change fingerprint."""

class NativeStageMismatchError(RuntimeError):
    """Raised when native state the compiler replays is inconsistent with its inputs."""

class ScopeError(ValueError):
    """Base error for invalid scope-domain input."""

    code: str
    message: str
    help: str | None
    def __init__(
        self, message: str, *, code: str | None = None, help: str | None = None
    ) -> None: ...

class InvalidScopePathError(ScopeError):
    """Raised when a project-relative scope path is invalid."""

class InvalidQualifiedIdentityError(ScopeError):
    """Raised when a qualified scope identity cannot be parsed."""

class ScopeValidationError(ScopeError):
    """Aggregate error raised only when strict scope validation is requested."""

    diagnostics: Any

class LintError(Exception):
    """A lint or format run could not proceed."""

    code: str
    message: str
    help: str | None
    def __init__(
        self, message: str, *, code: str | None = None, help: str | None = None
    ) -> None: ...

class ProjectCompileError(LintError):
    """The project could not be compiled, so its SQL cannot be linted."""

class SpecConfigError(RuntimeError):
    """Raised when project specification configuration is invalid."""

class ConfigValueTypeError(ValueError):
    """Raised when an authored config value has the wrong runtime type."""

    code: str
    message: str
    help: str | None
    def __init__(
        self, message: str, *, code: str | None = None, help: str | None = None
    ) -> None: ...
    key: Any
    expected: Any
    actual_type: Any

class NativePivotTables(NativeRecord):
    """The relation facts Python's dynamic pivot proofs read, for proofs outside a session."""

    dialect: str | None
    column_types_by_table: dict[str, dict[str, str]]
    authoritative_column_types_by_table: dict[str, dict[str, str]]
    column_nullability_by_table: dict[str, dict[str, InferredNullability]]
    dynamic_families_by_table: dict[str, tuple[SchemaDynamicColumnFamily, ...]]
    def __init__(
        self,
        dialect: str | None,
        column_types_by_table: dict[str, dict[str, str]],
        authoritative_column_types_by_table: dict[str, dict[str, str]],
        column_nullability_by_table: dict[str, dict[str, InferredNullability]],
        dynamic_families_by_table: dict[str, tuple[SchemaDynamicColumnFamily, ...]],
    ) -> None: ...

class NativeColumnQuery(NativeRecord):
    """One query outside the session whose output columns Python's analysis inferred."""

    sql: str
    mode: Literal["batch", "reanalysis", "parse", "legacy"]
    placeholders: dict[str, str] | None
    references: tuple[CompileSqlReference, ...]
    recover_cte_facts: bool
    def __init__(
        self,
        sql: str,
        mode: Literal["batch", "reanalysis", "parse", "legacy"],
        placeholders: dict[str, str] | None = ...,
        references: tuple[CompileSqlReference, ...] = ...,
        recover_cte_facts: bool = ...,
    ) -> None: ...

class NativeQueryColumns(NativeRecord):
    """One query's analysis: whether it succeeded, its columns and whether it projects a star."""

    succeeded: bool
    columns: tuple[InferredColumn, ...] | None
    has_star: bool
    def __init__(
        self,
        succeeded: bool,
        columns: tuple[InferredColumn, ...] | None,
        has_star: bool,
    ) -> None: ...

class ContractFinding(NativeRecord):
    resource_type: CompiledResourceType
    resource_name: str
    kind: ContractFindingKind
    message: str
    column_name: str | None
    declared_type: str | None
    physical_type: str | None
    def __init__(
        self,
        resource_type: CompiledResourceType,
        resource_name: str,
        kind: ContractFindingKind,
        message: str,
        column_name: str | None = ...,
        declared_type: str | None = ...,
        physical_type: str | None = ...,
    ) -> None: ...

class ContractEvidence(NativeRecord):
    resource_type: CompiledResourceType
    resource_name: str
    database: str | None
    schema: str | None
    relation: str
    declared_columns: tuple[ColumnInfo, ...]
    physical_columns: tuple[ColumnInfo, ...] | None
    source_path: Path | None
    dynamic_columns: tuple[SchemaDynamicColumnFamily, ...]
    findings: tuple[ContractFinding, ...]
    def __init__(
        self,
        resource_type: CompiledResourceType,
        resource_name: str,
        database: str | None,
        schema: str | None,
        relation: str,
        declared_columns: tuple[ColumnInfo, ...],
        physical_columns: tuple[ColumnInfo, ...] | None,
        source_path: Path | None,
        dynamic_columns: tuple[SchemaDynamicColumnFamily, ...] = ...,
        findings: tuple[ContractFinding, ...] = ...,
    ) -> None: ...

class ContractValidationResult(NativeRecord):
    """Diagnostics emitted by model contract validation."""

    diagnostics: tuple[CompilerDiagnostic, ...]
    def __init__(
        self,
        diagnostics: tuple[CompilerDiagnostic, ...] = ...,
    ) -> None: ...

class LineageGraphViews(NativeRecord):
    """A native project graph's lineage edges and selector indexes as Python dicts."""

    upstream_deps: dict[CompiledObjectKey, tuple[CompiledObjectKey, ...]]
    downstream_deps: dict[CompiledObjectKey, tuple[CompiledObjectKey, ...]]
    tag_index: dict[str, frozenset[CompiledObjectKey]]
    path_index: dict[CompiledObjectKey, str]
    all_keys: dict[str, CompiledObjectKey]
    def __init__(
        self,
        upstream_deps: dict[CompiledObjectKey, tuple[CompiledObjectKey, ...]],
        downstream_deps: dict[CompiledObjectKey, tuple[CompiledObjectKey, ...]],
        tag_index: dict[str, frozenset[CompiledObjectKey]],
        path_index: dict[CompiledObjectKey, str],
        all_keys: dict[str, CompiledObjectKey],
    ) -> None: ...

class QualifiedLineageColumn(NativeRecord):
    """A resource-qualified column in the collapsed SQLBuild lineage graph."""

    resource_type: CompiledResourceType | str
    resource_name: str
    column_name: str
    def __init__(
        self,
        resource_type: CompiledResourceType | str,
        resource_name: str,
        column_name: str,
    ) -> None: ...

class ColumnLineageSource(NativeRecord):
    """One upstream column dependency for an output column."""

    resource_type: CompiledResourceType | str
    resource_name: str
    column_name: str
    def __init__(
        self,
        resource_type: CompiledResourceType | str,
        resource_name: str,
        column_name: str,
    ) -> None: ...

class ColumnLineageEdge(NativeRecord):
    """One collapsed SQLBuild lineage graph edge."""

    source: QualifiedLineageColumn
    target: QualifiedLineageColumn
    transform_kind: ColumnTransformKind
    confidence: ColumnLineageConfidence
    def __init__(
        self,
        source: QualifiedLineageColumn,
        target: QualifiedLineageColumn,
        transform_kind: ColumnTransformKind = ...,
        confidence: ColumnLineageConfidence = ...,
    ) -> None: ...

class ColumnLineage(NativeRecord):
    """Lineage for one output column in one model."""

    output_column: str
    transform_kind: ColumnTransformKind
    expression_sql: str | None
    upstream_columns: tuple[ColumnLineageSource, ...]
    nullability: InferredNullability
    confidence: ColumnLineageConfidence
    def __init__(
        self,
        output_column: str,
        transform_kind: ColumnTransformKind,
        expression_sql: str | None,
        upstream_columns: tuple[ColumnLineageSource, ...],
        nullability: InferredNullability = ...,
        confidence: ColumnLineageConfidence = ...,
    ) -> None: ...

class ModelColumnLineage(NativeRecord):
    """Column lineage extracted for one compiled model."""

    model_name: str
    columns: tuple[ColumnLineage, ...]
    has_star: bool
    def __init__(
        self,
        model_name: str,
        columns: tuple[ColumnLineage, ...],
        has_star: bool = ...,
    ) -> None: ...

class NativeSqlTestFailure(NativeRecord):
    """The error one test's assembly raises: its kind and message."""

    kind: Literal["input", "internal", "decimal_overflow"]
    message: str
    def __init__(
        self,
        kind: Literal["input", "internal", "decimal_overflow"],
        message: str,
    ) -> None: ...

class NativeSqlTestAssembly(NativeRecord):
    """One natively assembled test, its keyed diagnostics, and the error its assembly raises."""

    test: CompiledSqlTest | None
    diagnostics: tuple[tuple[tuple[str, ...], CompilerDiagnostic], ...]
    failure: NativeSqlTestFailure | None
    def __init__(
        self,
        test: CompiledSqlTest | None,
        diagnostics: tuple[tuple[tuple[str, ...], CompilerDiagnostic], ...],
        failure: NativeSqlTestFailure | None = ...,
    ) -> None: ...

class HeaderSpan(NativeRecord):
    """A located DSL header region inside one file."""

    kind: str
    start: int
    end: int
    def __init__(
        self,
        kind: str,
        start: int,
        end: int,
    ) -> None: ...

class LintFileRole(NativeRecord):
    """Path-derived classification of one lint input, derived once per file."""

    declaration_kind: DeclarationKind | None
    in_project: bool
    in_hook_directory: bool
    def __init__(
        self,
        declaration_kind: DeclarationKind | None = ...,
        in_project: bool = ...,
        in_hook_directory: bool = ...,
    ) -> None: ...

class InterpolationSite(NativeRecord):
    """One sqlbuild interpolation occurrence replaced by a unique sentinel."""

    sentinel: str
    neutralized_start: int
    neutralized_end: int
    original_start: int
    original_end: int
    original_text: str
    def __init__(
        self,
        sentinel: str,
        neutralized_start: int,
        neutralized_end: int,
        original_start: int,
        original_end: int,
        original_text: str,
    ) -> None: ...

class LintBody(NativeRecord):
    """One authored SQL body prepared for linting, with its expansion spans."""

    file_path: Path
    body_start: int
    body_end: int
    lint_text: str
    passes: tuple[tuple[ExpansionSpan, ...], ...]
    external_identifiers: tuple[str, ...]
    dependency_identifiers: tuple[str, ...]
    externally_referenced_ctes: tuple[str, ...]
    allows_ceremonial_select: bool
    allows_dynamic_output_star: bool
    allows_empty_fixture_star: bool
    dependency_relations: tuple[tuple[str, str], ...]
    def __init__(
        self,
        file_path: Path,
        body_start: int,
        body_end: int,
        lint_text: str,
        passes: tuple[tuple[ExpansionSpan, ...], ...],
        external_identifiers: tuple[str, ...] = ...,
        dependency_identifiers: tuple[str, ...] = ...,
        externally_referenced_ctes: tuple[str, ...] = ...,
        allows_ceremonial_select: bool = ...,
        allows_dynamic_output_star: bool = ...,
        allows_empty_fixture_star: bool = ...,
        dependency_relations: tuple[tuple[str, str], ...] = ...,
    ) -> None: ...

class LintEdit(NativeRecord):
    """One deterministic authored-source edit proposed by a lint rule."""

    file_path: Path
    code: str
    start: int
    end: int
    replacement: str
    def __init__(
        self,
        file_path: Path,
        code: str,
        start: int,
        end: int,
        replacement: str,
    ) -> None: ...

class LintViolation(NativeRecord):
    """One lint diagnostic reported against an authored file."""

    file_path: Path
    line: int
    column: int
    code: str
    message: str
    severity: LintSeverity
    engine: str
    end_line: int | None
    end_column: int | None
    remediation: str | None
    fix: LintEdit | None
    fix_unavailable_reason: str | None
    def __init__(
        self,
        file_path: Path,
        line: int,
        column: int,
        code: str,
        message: str,
        severity: LintSeverity,
        engine: str,
        end_line: int | None = ...,
        end_column: int | None = ...,
        remediation: str | None = ...,
        fix: LintEdit | None = ...,
        fix_unavailable_reason: str | None = ...,
    ) -> None: ...

class LintConfig(NativeRecord):
    """Resolved lint and format configuration for one run."""

    native_enabled: bool
    max_description_lines: int
    line_width: int
    dialect: str
    enabled_native_rules: tuple[str, ...] | None
    ignored_native_rules: tuple[str, ...]
    header_rules_enabled: bool
    max_literal_length: int
    max_ranking_order_by: int
    relation_keys: Mapping[str, tuple[tuple[str, ...], ...]]
    def __init__(
        self,
        native_enabled: bool = ...,
        max_description_lines: int = ...,
        line_width: int = ...,
        dialect: str = ...,
        enabled_native_rules: tuple[str, ...] | None = ...,
        ignored_native_rules: tuple[str, ...] = ...,
        header_rules_enabled: bool = ...,
        max_literal_length: int = ...,
        max_ranking_order_by: int = ...,
        relation_keys: Mapping[str, tuple[tuple[str, ...], ...]] = ...,
    ) -> None: ...

class LintRunResult(NativeRecord):
    """Aggregated outcome of one lint or format run."""

    files_checked: int
    violations: tuple[LintViolation, ...]
    formatted_files: tuple[Path, ...]
    source_texts: Mapping[Path, str]
    unexpandable: Mapping[Path, str]
    def __init__(
        self,
        files_checked: int,
        violations: tuple[LintViolation, ...],
        formatted_files: tuple[Path, ...],
        source_texts: Mapping[Path, str] = ...,
        unexpandable: Mapping[Path, str] = ...,
    ) -> None: ...

class CompileFacts(NativeRecord):
    """Per-model outputs and per-owner diagnostics of one compilation."""

    models: dict[CompiledObjectKey, CompiledModelView]
    model_paths: dict[CompiledObjectKey, Path]
    diagnostics: dict[Path, Counter[DiagnosticIdentity]]
    def __init__(
        self,
        models: dict[CompiledObjectKey, CompiledModelView],
        model_paths: dict[CompiledObjectKey, Path],
        diagnostics: dict[Path, Counter[DiagnosticIdentity]],
    ) -> None: ...

class FixVerdict(NativeRecord):
    """Edited files that fail verification, and whether any change is unattributable."""

    failing: dict[Path, str]
    unattributed: bool
    def __init__(
        self,
        failing: dict[Path, str],
        unattributed: bool,
    ) -> None: ...

class DisplayOptions(NativeRecord):
    """Options controlling bounded human output."""

    max_entries_per_section: int | None
    overflow_flag: str
    def __init__(
        self,
        max_entries_per_section: int | None = ...,
        overflow_flag: str = ...,
    ) -> None: ...

type RuleFixStatus = Literal["applied", "refused", "unavailable"]
type DiagnosticIdentity = tuple[str, str, str, str]
type NativeLintPreparationRequest = LintPreparationRequest

class DiagnosticPhase(StrEnum):
    """Phase that produced a compiler diagnostic."""

    COMPILE = "compile"
    CONTRACT = "contract"
    RULE = "rule"
    PLAN = "plan"
    BUILD = "build"
    AUDIT = "audit"
    TEST = "test"
    CONNECTION = "connection"

class DiagnosticSeverity(StrEnum):
    """Severity for a compiler diagnostic."""

    ERROR = "error"
    WARNING = "warning"
    INFO = "info"

class CompiledResourceType(StrEnum):
    MODEL = "model"
    SOURCE = "source"
    SEED = "seed"
    UDF = "udf"
    TABLE_FN = "table_fn"
    DBT_REF = "dbt_ref"
    AUDIT = "audit"
    SQL_TEST = "sql_test"
    SQL_SCENARIO = "sql_scenario"

class InferredNullability(StrEnum):
    """Conservative nullability state inferred for an output column."""

    NON_NULL = "non_null"
    NULLABLE = "nullable"
    UNKNOWN = "unknown"

class ColumnTransformKind(StrEnum):
    """High-level transform classification for one output column."""

    DIRECT = "direct"
    CAST = "cast"
    EXPRESSION = "expression"
    AGGREGATION = "aggregation"
    STAR = "star"
    CONSTANT = "constant"
    UNKNOWN = "unknown"

class ColumnLineageConfidence(StrEnum):
    """Coarse confidence that a lineage edge is fully understood."""

    HIGH = "high"
    MEDIUM = "medium"
    LOW = "low"
    UNKNOWN = "unknown"

class ColumnLineageMode(StrEnum):
    """Column lineage analyzer mode."""

    RICH = "rich"
    FAST = "fast"

class MigrationDiscovery(StrEnum):
    """How a migration was requested."""

    MANUAL = "manual"
    AUTOMATIC = "automatic"

class MigrationCompatibility(StrEnum):
    """Outcome of checking the origin relation against the destination model."""

    COMPATIBLE = "compatible"
    INCOMPATIBLE = "incompatible"
    NOT_CHECKED = "not_checked"

class OldNameViewAction(StrEnum):
    """What a build does at a migrated model's old name."""

    ARCHIVE_AND_VIEW = "archive_and_view"
    VIEW_ONLY = "view_only"
    LIVE = "live"
    NONE = "none"

class SelectorKind(StrEnum):
    NAME = "name"
    SEED = "seed"
    SOURCE = "source"
    TASK = "task"
    ASSET = "asset"
    LOADER = "loader"
    CHECK = "check"
    TAG = "tag"
    PATH = "path"
    TEST = "test"

class PythonRunPhase(StrEnum):
    """Lifecycle-aware Python execution phase."""

    PRE_SQL_INGRESS = "pre_sql_ingress"
    READ_SIDE = "read_side"

class PythonIdentityStatus(StrEnum):
    """Read-only identity diff status for Python plan display."""

    NEW = "new"
    CHANGED = "changed"
    UNCHANGED = "unchanged"
    UNKNOWN = "unknown"

class CollectionRendering(StrEnum):
    """Physical rendering choices for homogeneous SQL collections."""

    VALUE_LIST = "value_list"
    ARRAY = "array"

class DbtInteropError(ValueError):
    """Base class for expected dbt interop errors."""

    code: str
    message: str
    help: str | None
    def __init__(
        self, message: str, *, code: str | None = None, help: str | None = None
    ) -> None: ...

class DbtInteropConfigError(DbtInteropError):
    """Raised when dbt project configuration is missing or invalid."""

class DbtInteropRuntimeError(DbtInteropError):
    """Raised when dbt command execution fails."""

class DbtInteropArgumentError(DbtInteropError):
    """Raised when `sqb dbt` arguments cannot be routed safely."""

class RelatedLocation(NativeRecord):
    """A secondary authored location that adds context to a diagnostic."""

    label: str
    location: SourceLocation
    message: str | None
    def __init__(
        self,
        label: str,
        location: SourceLocation,
        message: str | None = ...,
    ) -> None: ...

class FixtureColumnInference(NativeRecord):
    """Inferred fixture columns plus outputs that are explicitly bare NULL literals."""

    columns: tuple[InferredColumn, ...]
    null_literal_names: frozenset[str]
    quoted_names: frozenset[str]
    def __init__(
        self,
        columns: tuple[InferredColumn, ...],
        null_literal_names: frozenset[str],
        quoted_names: frozenset[str],
    ) -> None: ...

class DynamicColumnFamilyProof(NativeRecord):
    """Compiler evidence for one declared runtime-generated column family."""

    name: str
    inferred_type: str | None
    def __init__(
        self,
        name: str,
        inferred_type: str | None = ...,
    ) -> None: ...

class DynamicColumnContractProof(NativeRecord):
    """Closed output-shape evidence for a dynamic column contract."""

    output_proven: bool
    fixed_columns: tuple[InferredColumn, ...]
    families: tuple[DynamicColumnFamilyProof, ...]
    input_relations: tuple[str, ...]
    failure_reason: str | None
    bare_dynamic_pivot: bool
    def __init__(
        self,
        output_proven: bool,
        fixed_columns: tuple[InferredColumn, ...] = ...,
        families: tuple[DynamicColumnFamilyProof, ...] = ...,
        input_relations: tuple[str, ...] = ...,
        failure_reason: str | None = ...,
        bare_dynamic_pivot: bool = ...,
    ) -> None: ...

class CompileModelConfig(NativeRecord):
    """Pre-semantic effective config layers attached to a model input."""

    values: dict[str, object]
    model_header_keys: tuple[str, ...]
    matched_path_default: str | None
    logical_schema: str | None
    layer_schema: str | None
    logical_database: str | None
    time_travel_retention: ResolvedTimeTravelRetention
    table_type: ResolvedTableType
    def __init__(
        self,
        values: dict[str, object] = ...,
        model_header_keys: tuple[str, ...] = ...,
        matched_path_default: str | None = ...,
        logical_schema: str | None = ...,
        layer_schema: str | None = ...,
        logical_database: str | None = ...,
        time_travel_retention: ResolvedTimeTravelRetention = ...,
        table_type: ResolvedTableType = ...,
    ) -> None: ...

class DeclarationRuntimeProjection(NativeRecord):
    """Process-only declaration values keyed by canonical static identity."""

    declarations: Mapping[DeclarationIdentity, EnumDeclaration | ConstantDeclaration | LoadedMacro]
    def __init__(
        self,
        declarations: Mapping[
            DeclarationIdentity, EnumDeclaration | ConstantDeclaration | LoadedMacro
        ] = ...,
    ) -> None: ...

class DeclarationScopeResolver(NativeRecord):
    """Process-only lookup state used to project visible runtime declarations."""

    project_dir: Path | None
    lookup: ScopeLookup
    projection: DeclarationRuntimeProjection
    native_contexts: NativeDeclarationContexts
    resource_specific: frozenset[ResourceIdentity] | None
    contexts_by_directory: dict[tuple[str, str], DeclarationResolutionContext]
    def __init__(
        self,
        project_dir: Path | None,
        lookup: ScopeLookup,
        projection: DeclarationRuntimeProjection,
        native_contexts: NativeDeclarationContexts,
        resource_specific: frozenset[ResourceIdentity] | None = ...,
        contexts_by_directory: dict[tuple[str, str], DeclarationResolutionContext] = ...,
    ) -> None: ...

class DeclarationScopeBuild(NativeRecord):
    """Static artifact and process-local values shared by compile attachment phases."""

    loaded_macros: dict[str, LoadedMacro]
    index: ScopeIndex
    resolver: DeclarationScopeResolver
    sql_test_scans: SqlTestScanCache
    def __init__(
        self,
        loaded_macros: dict[str, LoadedMacro],
        index: ScopeIndex,
        resolver: DeclarationScopeResolver,
        sql_test_scans: SqlTestScanCache,
    ) -> None: ...

class MacroExpansionResult(NativeRecord):
    """Expanded SQL and resolved macro usage facts from one authored string."""

    sql: str
    spans: tuple[ExpansionSpan, ...]
    dependencies: tuple[DeclarationIdentity, ...]
    usages: tuple[UsageRecord, ...]
    argument_references: tuple[CompileSqlReference, ...]
    def __init__(
        self,
        sql: str,
        spans: tuple[ExpansionSpan, ...] = ...,
        dependencies: tuple[DeclarationIdentity, ...] = ...,
        usages: tuple[UsageRecord, ...] = ...,
        argument_references: tuple[CompileSqlReference, ...] = ...,
    ) -> None: ...

class DeclarationExpansionContext(NativeRecord):
    """Declarations and adapter rendering used by one authored SQL owner."""

    declarations: DeclarationResolutionContext
    value_renderer: TypedSqlValueRenderer
    collection_rendering: CollectionRendering
    resolver: DeclarationScopeResolver | None
    def __init__(
        self,
        declarations: DeclarationResolutionContext,
        value_renderer: TypedSqlValueRenderer,
        collection_rendering: CollectionRendering,
        resolver: DeclarationScopeResolver | None = ...,
    ) -> None: ...

class CompileAdapterContext(NativeRecord):
    """Cycle-free adapter behavior required while building compile inputs."""

    value_renderer: TypedSqlValueRenderer
    collection_rendering: CollectionRendering
    python_functions_inherit_default_namespace: bool
    sql_lexical_syntax: SqlLexicalSyntax
    def __init__(
        self,
        value_renderer: TypedSqlValueRenderer,
        collection_rendering: CollectionRendering,
        python_functions_inherit_default_namespace: bool,
        sql_lexical_syntax: SqlLexicalSyntax,
    ) -> None: ...

class InvalidSqlReferenceCall(NativeRecord):
    """One reference call whose arguments compile could not replace with a relation."""

    ref_kind: SqlReferenceKind
    call: str
    start: int
    message: str
    help: str
    corrected_call: str
    def __init__(
        self,
        ref_kind: SqlReferenceKind,
        call: str,
        start: int,
        message: str,
        help: str,
        corrected_call: str,
    ) -> None: ...

class SqlReferenceScan(NativeRecord):
    """Valid references and rejected reference calls found in one SQL text."""

    references: tuple[CompileSqlReference, ...]
    invalid_calls: tuple[InvalidSqlReferenceCall, ...]
    def __init__(
        self,
        references: tuple[CompileSqlReference, ...],
        invalid_calls: tuple[InvalidSqlReferenceCall, ...] = ...,
    ) -> None: ...

class CompiledLineageColumnFact(NativeRecord):
    """Compact output column lineage fact extracted during SQL analysis."""

    output_column: str
    upstream_columns: tuple[CompiledLineageSourceFact, ...]
    transform_kind: ColumnTransformKind
    confidence: ColumnLineageConfidence
    def __init__(
        self,
        output_column: str,
        upstream_columns: tuple[CompiledLineageSourceFact, ...] = ...,
        transform_kind: ColumnTransformKind = ...,
        confidence: ColumnLineageConfidence = ...,
    ) -> None: ...

class PolyglotAnalysisResult(NativeRecord):
    """Outcome of one Polyglot column and lineage analysis pass."""

    analysis_succeeded: bool
    columns: tuple[InferredColumn, ...] | None
    lineage_columns: Sequence[CompiledLineageColumnFact]
    has_star: bool
    star_resolved: bool
    binding_diagnostics: tuple[SqlBindingDiagnostic, ...]
    binding_validated: bool
    def __init__(
        self,
        analysis_succeeded: bool,
        columns: tuple[InferredColumn, ...] | None = ...,
        lineage_columns: Sequence[CompiledLineageColumnFact] = ...,
        has_star: bool = ...,
        star_resolved: bool = ...,
        binding_diagnostics: tuple[SqlBindingDiagnostic, ...] = ...,
        binding_validated: bool = ...,
    ) -> None: ...

class CompileAnalysisSelection(NativeRecord):
    """Deep-analysis selection and invocation-local preparation observer."""

    select: tuple[str, ...]
    exclude: tuple[str, ...]
    auto_load_sources: bool
    no_cache: bool
    on_inputs_ready: CompileInputsReadyCallback | None
    def __init__(
        self,
        select: tuple[str, ...] = ...,
        exclude: tuple[str, ...] = ...,
        auto_load_sources: bool = ...,
        no_cache: bool = ...,
        on_inputs_ready: CompileInputsReadyCallback | None = ...,
    ) -> None: ...

class CompileSqlScenarioCte(NativeRecord):
    """One top-level SQL-native scenario CTE extracted after macro expansion."""

    name: str
    sql_body: str
    def __init__(
        self,
        name: str,
        sql_body: str,
    ) -> None: ...

class FunctionArgument(NativeRecord):
    """One SQL function argument with an adapter-native type string."""

    name: str
    type: str
    def __init__(
        self,
        name: str,
        type: str,
    ) -> None: ...

class FunctionReturnColumn(NativeRecord):
    """One named table-function return column with an adapter-native type string."""

    name: str
    type: str
    def __init__(
        self,
        name: str,
        type: str,
    ) -> None: ...

class ModelSqlAnalysis(NativeRecord):
    """Completed model analysis and its in-memory binding proof."""

    polyglot_analysis: PolyglotAnalysisResult
    placeholders: dict[str, str] | None
    cached: bool
    fused_binding_validated: bool
    cleaned_sql: str | None
    validated_schema: dict[str, dict[str, str]] | None
    dynamic_column_contract: DynamicColumnContractProof | None
    def __init__(
        self,
        polyglot_analysis: PolyglotAnalysisResult,
        placeholders: dict[str, str] | None,
        cached: bool = ...,
        fused_binding_validated: bool = ...,
        cleaned_sql: str | None = ...,
        validated_schema: dict[str, dict[str, str]] | None = ...,
        dynamic_column_contract: DynamicColumnContractProof | None = ...,
    ) -> None: ...

class CompileSqlScenarioInput(NativeRecord):
    """One discovered SQL-native scenario with compile-time SQL expansion applied."""

    scenario_file: DiscoveredSqlScenarioFile
    sql_body: str
    authored_ctes: tuple[CompileSqlScenarioCte, ...]
    expected_ctes: tuple[CompileSqlScenarioCte, ...]
    assertion_ctes: tuple[CompileSqlScenarioCte, ...]
    source_fixture_names: tuple[str, ...]
    ref_fixture_names: tuple[str, ...]
    seed_fixture_names: tuple[str, ...]
    dbt_ref_fixture_names: tuple[str, ...]
    expected_model_names: tuple[str, ...]
    assertion_names: tuple[str, ...]
    assertion_target_model_names: tuple[str, ...]
    target_model_names: tuple[str, ...]
    declaration_usages: tuple[UsageRecord, ...]
    def __init__(
        self,
        scenario_file: DiscoveredSqlScenarioFile,
        sql_body: str,
        authored_ctes: tuple[CompileSqlScenarioCte, ...] = ...,
        expected_ctes: tuple[CompileSqlScenarioCte, ...] = ...,
        assertion_ctes: tuple[CompileSqlScenarioCte, ...] = ...,
        source_fixture_names: tuple[str, ...] = ...,
        ref_fixture_names: tuple[str, ...] = ...,
        seed_fixture_names: tuple[str, ...] = ...,
        dbt_ref_fixture_names: tuple[str, ...] = ...,
        expected_model_names: tuple[str, ...] = ...,
        assertion_names: tuple[str, ...] = ...,
        assertion_target_model_names: tuple[str, ...] = ...,
        target_model_names: tuple[str, ...] = ...,
        declaration_usages: tuple[UsageRecord, ...] = ...,
    ) -> None: ...

class CompileProjectInputs(NativeRecord):
    """Attached project view used as the first pre-semantic compile input snapshot."""

    project_config: ProjectConfig
    local_config: LocalConfig
    discovered_inputs: DiscoveredProjectInputs
    sql_lexical_syntax: SqlLexicalSyntax
    run_id: str
    effective_target_name: str | None
    effective_target: TargetConfig | None
    compile_cache_dir: Path | None
    effective_connection: dict[str, object]
    effective_settings: SettingsConfig
    no_sql_validation: bool
    effective_vars: dict[str, object]
    macro_context: MacroContext | None
    loaded_macros: dict[str, LoadedMacro]
    public_enums: dict[str, EnumDeclaration]
    public_constants: dict[str, ConstantDeclaration]
    model_inputs: tuple[ModelInput, ...]
    seed_inputs: tuple[SeedInput, ...]
    source_inputs: tuple[SourceInput, ...]
    sql_function_inputs: tuple[FunctionInput, ...]
    test_inputs: tuple[CompileSqlTestInput, ...]
    scenario_inputs: tuple[CompileSqlScenarioInput, ...]
    audit_inputs: tuple[AuditInput, ...]
    diagnostics: tuple[CompilerDiagnostic, ...]
    external_sql_reference_resolver: ExternalSqlReferenceResolver | None
    scope_index: ScopeIndex
    declaration_scope: DeclarationScopeBuild | None
    native_project: NativeCompiledProject | None
    def __init__(
        self,
        project_config: ProjectConfig,
        local_config: LocalConfig,
        discovered_inputs: DiscoveredProjectInputs,
        sql_lexical_syntax: SqlLexicalSyntax,
        run_id: str = ...,
        effective_target_name: str | None = ...,
        effective_target: TargetConfig | None = ...,
        compile_cache_dir: Path | None = ...,
        effective_connection: dict[str, object] = ...,
        effective_settings: SettingsConfig = ...,
        no_sql_validation: bool = ...,
        effective_vars: dict[str, object] = ...,
        macro_context: MacroContext | None = ...,
        loaded_macros: dict[str, LoadedMacro] = ...,
        public_enums: dict[str, EnumDeclaration] = ...,
        public_constants: dict[str, ConstantDeclaration] = ...,
        model_inputs: tuple[ModelInput, ...] = ...,
        seed_inputs: tuple[SeedInput, ...] = ...,
        source_inputs: tuple[SourceInput, ...] = ...,
        sql_function_inputs: tuple[FunctionInput, ...] = ...,
        test_inputs: tuple[CompileSqlTestInput, ...] = ...,
        scenario_inputs: tuple[CompileSqlScenarioInput, ...] = ...,
        audit_inputs: tuple[AuditInput, ...] = ...,
        diagnostics: tuple[CompilerDiagnostic, ...] = ...,
        external_sql_reference_resolver: ExternalSqlReferenceResolver | None = ...,
        scope_index: ScopeIndex = ...,
        declaration_scope: DeclarationScopeBuild | None = ...,
        native_project: NativeCompiledProject | None = ...,
    ) -> None: ...

class CompiledSource(NativeRecord):
    """Planner-ready compiled source metadata."""

    key: CompiledObjectKey
    deps: tuple[CompiledObjectKey, ...]
    name: str
    source_entry: SourceEntry
    source_file: DiscoveredSourceFile
    def __init__(
        self,
        key: CompiledObjectKey,
        deps: tuple[CompiledObjectKey, ...],
        name: str,
        source_entry: SourceEntry,
        source_file: DiscoveredSourceFile,
    ) -> None: ...

class CompiledSeed(NativeRecord):
    """Planner-ready compiled seed metadata."""

    key: CompiledObjectKey
    deps: tuple[CompiledObjectKey, ...]
    name: str
    seed_file: DiscoveredSeedFile
    schema_entry: SchemaSeedEntry
    schema_file: DiscoveredSchemaFile
    destination: CompiledRelationLocation
    external: bool
    def __init__(
        self,
        key: CompiledObjectKey,
        deps: tuple[CompiledObjectKey, ...],
        name: str,
        seed_file: DiscoveredSeedFile,
        schema_entry: SchemaSeedEntry,
        schema_file: DiscoveredSchemaFile,
        destination: CompiledRelationLocation,
        external: bool = ...,
    ) -> None: ...

class CompiledFunction(NativeRecord):
    """Planner-ready compiled SQL function metadata."""

    key: CompiledObjectKey
    deps: tuple[CompiledObjectKey, ...]
    name: str
    relative_path: Path
    arguments: tuple[FunctionArgument, ...]
    returns: str
    body_sql: str
    destination: CompiledRelationLocation
    fingerprint_destination: CompiledRelationLocation
    return_columns: tuple[FunctionReturnColumn, ...]
    references: tuple[CompileSqlReference, ...]
    language: FunctionLanguage
    source_file_path: Path | None
    runtime_version: str | None
    entry_point: str | None
    packages: tuple[str, ...]
    tags: tuple[str, ...]
    description: str | None
    def __init__(
        self,
        key: CompiledObjectKey,
        deps: tuple[CompiledObjectKey, ...],
        name: str,
        relative_path: Path,
        arguments: tuple[FunctionArgument, ...],
        returns: str,
        body_sql: str,
        destination: CompiledRelationLocation,
        fingerprint_destination: CompiledRelationLocation,
        return_columns: tuple[FunctionReturnColumn, ...] = ...,
        references: tuple[CompileSqlReference, ...] = ...,
        language: FunctionLanguage = ...,
        source_file_path: Path | None = ...,
        runtime_version: str | None = ...,
        entry_point: str | None = ...,
        packages: tuple[str, ...] = ...,
        tags: tuple[str, ...] = ...,
        description: str | None = ...,
    ) -> None: ...

class CompileSqlTestCte(NativeRecord):
    """One top-level SQL-native test CTE extracted after macro expansion."""

    name: str
    sql_body: str
    def __init__(
        self,
        name: str,
        sql_body: str,
    ) -> None: ...

class CompileModelSqlTestInputPayload(NativeRecord):
    """Model-mode SQL test compile payload."""

    authored_ctes: tuple[CompileSqlTestCte, ...]
    macro_mocks: dict[str, str]
    mock_model_names: tuple[str, ...]
    mock_source_names: tuple[str, ...]
    mock_seed_names: tuple[str, ...]
    mock_dbt_ref_names: tuple[str, ...]
    mock_table_function_names: tuple[str, ...]
    expected_ctes: tuple[CompileSqlTestCte, ...]
    expected_model_names: tuple[str, ...]
    assertion_ctes: tuple[CompileSqlTestCte, ...]
    assertion_names: tuple[str, ...]
    assertion_target_model_names: tuple[str, ...]
    read_helper_names: tuple[str, ...]
    reference_target_model_names: tuple[str, ...]
    def __init__(
        self,
        authored_ctes: tuple[CompileSqlTestCte, ...] = ...,
        macro_mocks: dict[str, str] = ...,
        mock_model_names: tuple[str, ...] = ...,
        mock_source_names: tuple[str, ...] = ...,
        mock_seed_names: tuple[str, ...] = ...,
        mock_dbt_ref_names: tuple[str, ...] = ...,
        mock_table_function_names: tuple[str, ...] = ...,
        expected_ctes: tuple[CompileSqlTestCte, ...] = ...,
        expected_model_names: tuple[str, ...] = ...,
        assertion_ctes: tuple[CompileSqlTestCte, ...] = ...,
        assertion_names: tuple[str, ...] = ...,
        assertion_target_model_names: tuple[str, ...] = ...,
        read_helper_names: tuple[str, ...] = ...,
        reference_target_model_names: tuple[str, ...] = ...,
    ) -> None: ...

class CompileDirectLogicSqlTestInputPayload(NativeRecord):
    """Direct-logic SQL test compile payload."""

    actual_cte: CompileSqlTestCte
    expected_cte: CompileSqlTestCte
    mode: SqlTestMode
    helper_ctes: tuple[CompileSqlTestCte, ...]
    tested_resource_names: tuple[str, ...]
    def __init__(
        self,
        actual_cte: CompileSqlTestCte,
        expected_cte: CompileSqlTestCte,
        mode: SqlTestMode,
        helper_ctes: tuple[CompileSqlTestCte, ...] = ...,
        tested_resource_names: tuple[str, ...] = ...,
    ) -> None: ...

class CompileSqlTestInput(NativeRecord):
    """One discovered SQL-native test block with compile-time SQL expansion applied."""

    test_file: DiscoveredSqlTestFile
    test_block: DiscoveredSqlTestBlock
    sql_body: str
    mode: SqlTestMode
    payload: CompileModelSqlTestInputPayload | CompileDirectLogicSqlTestInputPayload
    declaration_usages: tuple[UsageRecord, ...]
    parent_name: str | None
    case_name: str | None
    case_index: int | None
    parameter_schema: tuple[SqlTestParameterDeclaration, ...]
    parameter_values: tuple[tuple[str, SqlValue], ...]
    def __init__(
        self,
        test_file: DiscoveredSqlTestFile,
        test_block: DiscoveredSqlTestBlock,
        sql_body: str,
        mode: SqlTestMode = ...,
        payload: CompileModelSqlTestInputPayload | CompileDirectLogicSqlTestInputPayload = ...,
        declaration_usages: tuple[UsageRecord, ...] = ...,
        parent_name: str | None = ...,
        case_name: str | None = ...,
        case_index: int | None = ...,
        parameter_schema: tuple[SqlTestParameterDeclaration, ...] = ...,
        parameter_values: tuple[tuple[str, SqlValue], ...] = ...,
    ) -> None: ...

class CompiledModelSqlTestPayload(NativeRecord):
    """Compiled model-mode SQL test payload."""

    authored_ctes: tuple[CompileSqlTestCte, ...]
    macro_mocks: dict[str, str]
    model_query_overrides: dict[str, str]
    mock_model_names: tuple[str, ...]
    mock_source_names: tuple[str, ...]
    mock_seed_names: tuple[str, ...]
    mock_dbt_ref_names: tuple[str, ...]
    mock_table_function_names: tuple[str, ...]
    expected_ctes: tuple[CompileSqlTestCte, ...]
    expected_model_names: tuple[str, ...]
    assertion_ctes: tuple[CompileSqlTestCte, ...]
    assertion_names: tuple[str, ...]
    def __init__(
        self,
        authored_ctes: tuple[CompileSqlTestCte, ...] = ...,
        macro_mocks: dict[str, str] = ...,
        model_query_overrides: dict[str, str] = ...,
        mock_model_names: tuple[str, ...] = ...,
        mock_source_names: tuple[str, ...] = ...,
        mock_seed_names: tuple[str, ...] = ...,
        mock_dbt_ref_names: tuple[str, ...] = ...,
        mock_table_function_names: tuple[str, ...] = ...,
        expected_ctes: tuple[CompileSqlTestCte, ...] = ...,
        expected_model_names: tuple[str, ...] = ...,
        assertion_ctes: tuple[CompileSqlTestCte, ...] = ...,
        assertion_names: tuple[str, ...] = ...,
    ) -> None: ...

class SqlExpansionContext(NativeRecord):
    """Everything needed to expand one project's authored SQL bodies."""

    effective_vars: dict[str, object]
    loaded_macros: dict[str, LoadedMacro]
    macro_context: MacroContext
    enums: dict[str, EnumDeclaration]
    constants: dict[str, ConstantDeclaration]
    value_renderer: TypedSqlValueRenderer
    collection_rendering: CollectionRendering
    local_declarations: dict[Path, DeclarationResolutionContext]
    declaration_resolver: DeclarationScopeResolver | None
    def __init__(
        self,
        effective_vars: dict[str, object],
        loaded_macros: dict[str, LoadedMacro],
        macro_context: MacroContext,
        enums: dict[str, EnumDeclaration],
        constants: dict[str, ConstantDeclaration],
        value_renderer: TypedSqlValueRenderer,
        collection_rendering: CollectionRendering,
        local_declarations: dict[Path, DeclarationResolutionContext] = ...,
        declaration_resolver: DeclarationScopeResolver | None = ...,
    ) -> None: ...

class ExpansionSpan(NativeRecord):
    """One substituted region, pairing its source range with its rendered range."""

    source_start: int
    source_end: int
    output_start: int
    output_end: int
    def __init__(
        self,
        source_start: int,
        source_end: int,
        output_start: int,
        output_end: int,
    ) -> None: ...

class CompiledSqlExpansion(NativeRecord):
    """Process-local authored-to-expanded SQL evidence shared with compiler checks."""

    authored_sql: str
    expanded_sql: str
    passes: tuple[tuple[ExpansionSpan, ...], ...]
    def __init__(
        self,
        authored_sql: str,
        expanded_sql: str,
        passes: tuple[tuple[ExpansionSpan, ...], ...],
    ) -> None: ...

class MappedOffset(NativeRecord):
    """An offset in rendered SQL resolved back onto the text that produced it."""

    offset: int
    generated: bool
    def __init__(
        self,
        offset: int,
        generated: bool,
    ) -> None: ...

class CompiledDirectLogicSqlTestPayload(NativeRecord):
    """Compiled direct-logic SQL test payload."""

    actual_cte: CompileSqlTestCte
    expected_cte: CompileSqlTestCte
    mode: SqlTestMode
    helper_ctes: tuple[CompileSqlTestCte, ...]
    tested_resource_names: tuple[str, ...]
    def __init__(
        self,
        actual_cte: CompileSqlTestCte,
        expected_cte: CompileSqlTestCte,
        mode: SqlTestMode = ...,
        helper_ctes: tuple[CompileSqlTestCte, ...] = ...,
        tested_resource_names: tuple[str, ...] = ...,
    ) -> None: ...

class CompiledSqlTestResource(NativeRecord):
    """One direct SQL test resource with its authored resource kind."""

    kind: SqlTestMode
    name: str
    def __init__(
        self,
        kind: SqlTestMode,
        name: str,
    ) -> None: ...

class NativeModelConfigSession(NativeRecord):
    """The native config builder, validator, and inherited storage policies for one compile."""

    path_defaults: dict[str, dict[str, object]]
    builder: NativeModelConfigBuilder
    validator: NativeModelValidator
    inherited_storage: dict[str | None, tuple[ResolvedTimeTravelRetention, ResolvedTableType]]
    def __init__(
        self,
        path_defaults: dict[str, dict[str, object]],
        builder: NativeModelConfigBuilder,
        validator: NativeModelValidator,
        inherited_storage: dict[str | None, tuple[ResolvedTimeTravelRetention, ResolvedTableType]],
    ) -> None: ...

class NativeModelConfigInputs(NativeRecord):
    """The project and run inputs every model's native config build reads."""

    project_config: ProjectConfig
    target_config: TargetConfig | None
    effective_vars: dict[str, object]
    effective_target_name: str | None
    run_id: str
    microbatch_concurrency: bool
    def __init__(
        self,
        project_config: ProjectConfig,
        target_config: TargetConfig | None,
        effective_vars: dict[str, object],
        effective_target_name: str | None,
        run_id: str,
        microbatch_concurrency: bool,
    ) -> None: ...

class ModelValidationRequest(NativeRecord):
    """One model's effective config and the facts its validators read."""

    model_file: DiscoveredSqlModelFile
    config: CompileModelConfig
    references: tuple[CompileSqlReference, ...]
    declared_columns: tuple[SchemaColumn, ...] | None
    query_sql: str
    def __init__(
        self,
        model_file: DiscoveredSqlModelFile,
        config: CompileModelConfig,
        references: tuple[CompileSqlReference, ...],
        declared_columns: tuple[SchemaColumn, ...] | None,
        query_sql: str,
    ) -> None: ...

class NamedSqlHookEntry(NativeRecord):
    """An unresolved invocation of a discovered SQL hook resource."""

    name: str
    kwargs: dict[str, object]
    def __init__(
        self,
        name: str,
        kwargs: dict[str, object],
    ) -> None: ...

class DiscoveredSqlFunctionFile(NativeRecord):
    """A discovered SQL function file and its raw contents."""

    file_path: Path
    relative_path: Path
    contents: str
    header_values: dict[str, object]
    body_sql: str
    def __init__(
        self,
        file_path: Path,
        relative_path: Path,
        contents: str,
        header_values: dict[str, object],
        body_sql: str,
    ) -> None: ...

class DiscoveredSqlHookFile(NativeRecord):
    """A discovered named SQL lifecycle hook resource."""

    file_path: Path
    relative_path: Path
    contents: str
    header_values: dict[str, object]
    sql_body: str
    name: str
    description: str | None
    scope_kind: ScopeKind
    ownership_root: Path | None
    owning_path: Path | None
    declaration_root: Path | None
    def __init__(
        self,
        file_path: Path,
        relative_path: Path,
        contents: str,
        header_values: dict[str, object],
        sql_body: str,
        name: str,
        description: str | None = ...,
        scope_kind: ScopeKind = ...,
        ownership_root: Path | None = ...,
        owning_path: Path | None = ...,
        declaration_root: Path | None = ...,
    ) -> None: ...

class EnumMember(NativeRecord):
    """One named member in a SQLBuild enum declaration."""

    name: str
    value: str | int
    def __init__(
        self,
        name: str,
        value: str | int,
    ) -> None: ...

class DiscoveredEnumFile(NativeRecord):
    """A public SQL file containing one or more enum declarations."""

    file_path: Path
    relative_path: Path
    contents: str
    declarations: tuple[EnumDeclaration, ...]
    scope_kind: ScopeKind
    ownership_root: Path | None
    owning_path: Path | None
    declaration_root: Path | None
    def __init__(
        self,
        file_path: Path,
        relative_path: Path,
        contents: str,
        declarations: tuple[EnumDeclaration, ...],
        scope_kind: ScopeKind = ...,
        ownership_root: Path | None = ...,
        owning_path: Path | None = ...,
        declaration_root: Path | None = ...,
    ) -> None: ...

class DiscoveredConstantFile(NativeRecord):
    """A public SQL file containing one or more constant declarations."""

    file_path: Path
    relative_path: Path
    contents: str
    declarations: tuple[ConstantDeclaration, ...]
    scope_kind: ScopeKind
    ownership_root: Path | None
    owning_path: Path | None
    declaration_root: Path | None
    def __init__(
        self,
        file_path: Path,
        relative_path: Path,
        contents: str,
        declarations: tuple[ConstantDeclaration, ...],
        scope_kind: ScopeKind = ...,
        ownership_root: Path | None = ...,
        owning_path: Path | None = ...,
        declaration_root: Path | None = ...,
    ) -> None: ...

class ModelSchemaDeclaration(NativeRecord):
    """One public reusable model column schema declaration."""

    name: str
    columns: tuple[SchemaColumn, ...]
    relative_path: Path
    extends: str | None
    description: str | None
    def __init__(
        self,
        name: str,
        columns: tuple[SchemaColumn, ...],
        relative_path: Path,
        extends: str | None = ...,
        description: str | None = ...,
    ) -> None: ...

class DiscoveredModelSchemaFile(NativeRecord):
    """A public SQL file containing reusable model schemas."""

    file_path: Path
    relative_path: Path
    contents: str
    declarations: tuple[ModelSchemaDeclaration, ...]
    scope_kind: ScopeKind
    ownership_root: Path | None
    owning_path: Path | None
    declaration_root: Path | None
    def __init__(
        self,
        file_path: Path,
        relative_path: Path,
        contents: str,
        declarations: tuple[ModelSchemaDeclaration, ...],
        scope_kind: ScopeKind = ...,
        ownership_root: Path | None = ...,
        owning_path: Path | None = ...,
        declaration_root: Path | None = ...,
    ) -> None: ...

class DiscoveredPythonFunctionFile(NativeRecord):
    """A discovered Python function file and its parsed UDF metadata."""

    file_path: Path
    relative_path: Path
    contents: str
    header_values: dict[str, object]
    entry_point: str
    body_python: str
    def __init__(
        self,
        file_path: Path,
        relative_path: Path,
        contents: str,
        header_values: dict[str, object],
        entry_point: str,
        body_python: str,
    ) -> None: ...

class DiscoveredSchemaFile(NativeRecord):
    """A discovered schema.yml file and its raw contents."""

    file_path: Path
    relative_path: Path
    contents: str
    model_entries: tuple[SchemaModelEntry, ...]
    seed_entries: tuple[SchemaSeedEntry, ...]
    def __init__(
        self,
        file_path: Path,
        relative_path: Path,
        contents: str,
        model_entries: tuple[SchemaModelEntry, ...],
        seed_entries: tuple[SchemaSeedEntry, ...],
    ) -> None: ...

class DiscoveredSeedFile(NativeRecord):
    """A discovered seed file."""

    file_path: Path
    relative_path: Path
    def __init__(
        self,
        file_path: Path,
        relative_path: Path,
    ) -> None: ...

class DiscoveredSqlTestCase(NativeRecord):
    """One ordered named case belonging to a SQL-native test template."""

    name: str
    values: tuple[tuple[str, SqlValue], ...]
    case_index: int
    def __init__(
        self,
        name: str,
        values: tuple[tuple[str, SqlValue], ...],
        case_index: int,
    ) -> None: ...

class DiscoveredSqlTestBlock(NativeRecord):
    """One raw TEST(...) block discovered from a SQL-native test file."""

    test_index: int
    header_values: dict[str, object]
    sql_body: str
    name: str | None
    mode: SqlTestMode
    parameters: tuple[SqlTestParameterDeclaration, ...]
    cases: tuple[DiscoveredSqlTestCase, ...]
    cursor_start: str | None
    cursor_end: str | None
    sql_body_span: tuple[int, int] | None
    def __init__(
        self,
        test_index: int,
        header_values: dict[str, object],
        sql_body: str,
        name: str | None = ...,
        mode: SqlTestMode = ...,
        parameters: tuple[SqlTestParameterDeclaration, ...] = ...,
        cases: tuple[DiscoveredSqlTestCase, ...] = ...,
        cursor_start: str | None = ...,
        cursor_end: str | None = ...,
        sql_body_span: tuple[int, int] | None = ...,
    ) -> None: ...

class DiscoveredAuditFile(NativeRecord):
    """A discovered audit SQL file and its raw contents."""

    file_path: Path
    relative_path: Path
    contents: str
    blocks: tuple[DiscoveredAuditBlock, ...]
    declaration_kind: DeclarationKind
    scope_kind: ScopeKind
    ownership_root: Path | None
    owning_path: Path | None
    declaration_root: Path | None
    def __init__(
        self,
        file_path: Path,
        relative_path: Path,
        contents: str,
        blocks: tuple[DiscoveredAuditBlock, ...],
        declaration_kind: DeclarationKind = ...,
        scope_kind: ScopeKind = ...,
        ownership_root: Path | None = ...,
        owning_path: Path | None = ...,
        declaration_root: Path | None = ...,
    ) -> None: ...

class DiscoveredAuditBlock(NativeRecord):
    """One raw AUDIT(...) block discovered from a SQL audit file."""

    audit_index: int
    header_values: dict[str, object]
    sql_body: str
    name: str | None
    evaluation_mode: AuditEvaluationMode
    measurement_contract: MeasurementContract | None
    measure_sql: str | None
    evidence_sql: str | None
    def __init__(
        self,
        audit_index: int,
        header_values: dict[str, object],
        sql_body: str,
        name: str | None = ...,
        evaluation_mode: AuditEvaluationMode = ...,
        measurement_contract: MeasurementContract | None = ...,
        measure_sql: str | None = ...,
        evidence_sql: str | None = ...,
    ) -> None: ...

class DiscoveredAdapterFile(NativeRecord):
    """A detected project adapter Python file."""

    file_path: Path
    relative_path: Path
    def __init__(
        self,
        file_path: Path,
        relative_path: Path,
    ) -> None: ...

class DiscoveryFileFault(NativeRecord):
    """One project-relative authored file fault captured during tolerant discovery."""

    path: Path | None
    message: str
    def __init__(
        self,
        path: Path | None,
        message: str,
    ) -> None: ...

class TolerantScopeDiscovery(NativeRecord):
    """Parsed scope inputs and independently categorized tolerant discovery faults."""

    discovered_inputs: DiscoveredProjectInputs
    resource_faults: tuple[DiscoveryFileFault, ...]
    declaration_faults: tuple[DiscoveryFileFault, ...]
    relationship_faults: tuple[DiscoveryFileFault, ...]
    config_faults: tuple[DiscoveryFileFault, ...]
    def __init__(
        self,
        discovered_inputs: DiscoveredProjectInputs,
        resource_faults: tuple[DiscoveryFileFault, ...] = ...,
        declaration_faults: tuple[DiscoveryFileFault, ...] = ...,
        relationship_faults: tuple[DiscoveryFileFault, ...] = ...,
        config_faults: tuple[DiscoveryFileFault, ...] = ...,
    ) -> None: ...

class CompilePipelineOptions(NativeRecord):
    """Selection, deferral, and planning options for one compile pipeline run."""

    selected_target: str | None
    no_sql_validation: bool
    defer_to: str | None
    defer_sources_to: str | None
    source_deferral_enabled: bool
    select: tuple[str, ...]
    exclude: tuple[str, ...]
    cursor_overrides: CursorOverrides | None
    full_refresh: bool
    auto_load_sources: bool
    reload_sources: bool
    connection_config: dict[str, object] | None
    cli_vars: dict[str, object] | None
    external_sql_reference_resolver: ExternalSqlReferenceResolver | None
    resolve_python_run_selectors: bool
    no_cache: bool
    max_microbatches: int | None
    concurrency: int | None
    selection_diagnostics: bool
    plan_sql_tests: bool
    record_migration_fingerprints: bool
    accepts_unit_test_selectors: bool
    sql_test_selection: SqlTestSelection
    source_lexical_syntax: SqlLexicalSyntax | None
    def __init__(
        self,
        selected_target: str | None = ...,
        no_sql_validation: bool = ...,
        defer_to: str | None = ...,
        defer_sources_to: str | None = ...,
        source_deferral_enabled: bool = ...,
        select: tuple[str, ...] = ...,
        exclude: tuple[str, ...] = ...,
        cursor_overrides: CursorOverrides | None = ...,
        full_refresh: bool = ...,
        auto_load_sources: bool = ...,
        reload_sources: bool = ...,
        connection_config: dict[str, object] | None = ...,
        cli_vars: dict[str, object] | None = ...,
        external_sql_reference_resolver: ExternalSqlReferenceResolver | None = ...,
        resolve_python_run_selectors: bool = ...,
        no_cache: bool = ...,
        max_microbatches: int | None = ...,
        concurrency: int | None = ...,
        selection_diagnostics: bool = ...,
        plan_sql_tests: bool = ...,
        record_migration_fingerprints: bool = ...,
        accepts_unit_test_selectors: bool = ...,
        sql_test_selection: SqlTestSelection = ...,
        source_lexical_syntax: SqlLexicalSyntax | None = ...,
    ) -> None: ...

class PythonPlanEntry(NativeRecord):
    """Display-ready Python node entry for plan output."""

    name: str
    kind: PythonNodeKind
    phase: PythonRunPhase
    identity_status: PythonIdentityStatus
    current_definition_json: str | None
    previous_definition_json: str | None
    current_metadata_json: str | None
    previous_metadata_json: str | None
    provider_usages: tuple[DiscoveredProviderUsage, ...]
    def __init__(
        self,
        name: str,
        kind: PythonNodeKind,
        phase: PythonRunPhase,
        identity_status: PythonIdentityStatus = ...,
        current_definition_json: str | None = ...,
        previous_definition_json: str | None = ...,
        current_metadata_json: str | None = ...,
        previous_metadata_json: str | None = ...,
        provider_usages: tuple[DiscoveredProviderUsage, ...] = ...,
    ) -> None: ...

class ClonePipelineOptions(NativeRecord):
    """Compilation and selection options for one clone pipeline."""

    no_sql_validation: bool
    select: tuple[str, ...]
    exclude: tuple[str, ...]
    cli_vars: dict[str, object] | None
    no_cache: bool
    def __init__(
        self,
        no_sql_validation: bool = ...,
        select: tuple[str, ...] = ...,
        exclude: tuple[str, ...] = ...,
        cli_vars: dict[str, object] | None = ...,
        no_cache: bool = ...,
    ) -> None: ...

class ClonePipelineConnection(NativeRecord):
    """Resolved destination connection used throughout clone compilation and planning."""

    config: dict[str, object]
    handle: Any
    def __init__(
        self,
        config: dict[str, object],
        handle: Any,
    ) -> None: ...

class ClonePipelineResult(NativeRecord):
    """Prepared clone inputs for origin and destination target environments."""

    origin_project: CompiledProjectView
    destination_project: CompiledProjectView
    clone_plan: PlanOutput
    destination_source_entries: tuple[CloneSourcePlanEntry, ...]
    destination_model_entries: tuple[ModelPlanEntry, ...]
    destination_seed_entries: tuple[SeedPlanEntry, ...]
    destination_function_entries: tuple[FunctionPlanEntry, ...]
    origin_model_entries: tuple[ModelPlanEntry, ...]
    origin_seed_entries: tuple[SeedPlanEntry, ...]
    origin_source_entries: tuple[CloneSourcePlanEntry, ...]
    def __init__(
        self,
        origin_project: CompiledProjectView,
        destination_project: CompiledProjectView,
        clone_plan: PlanOutput,
        destination_source_entries: tuple[CloneSourcePlanEntry, ...] = ...,
        destination_model_entries: tuple[ModelPlanEntry, ...] = ...,
        destination_seed_entries: tuple[SeedPlanEntry, ...] = ...,
        destination_function_entries: tuple[FunctionPlanEntry, ...] = ...,
        origin_model_entries: tuple[ModelPlanEntry, ...] = ...,
        origin_seed_entries: tuple[SeedPlanEntry, ...] = ...,
        origin_source_entries: tuple[CloneSourcePlanEntry, ...] = ...,
    ) -> None: ...

class ParsedSelector(NativeRecord):
    """One parsed selector token before graph resolution."""

    kind: SelectorKind
    value: str
    upstream: bool
    downstream: bool
    def __init__(
        self,
        kind: SelectorKind,
        value: str,
        upstream: bool = ...,
        downstream: bool = ...,
    ) -> None: ...

class PathSelector(NativeRecord):
    """A directed path selector between two model names with optional endpoint expansion."""

    start_name: str
    end_name: str
    upstream: bool
    downstream: bool
    def __init__(
        self,
        start_name: str,
        end_name: str,
        upstream: bool = ...,
        downstream: bool = ...,
    ) -> None: ...

class SqlTestSelection(NativeRecord):
    """Unit tests selected or excluded by name, beside the tests of the selected resources."""

    names: frozenset[str]
    excluded_names: frozenset[str]
    models_selected: bool
    def __init__(
        self,
        names: frozenset[str] = ...,
        excluded_names: frozenset[str] = ...,
        models_selected: bool = ...,
    ) -> None: ...

class PlannerScope(NativeRecord):
    """Resolved graph scope for one planner invocation."""

    upstream_deps: dict[CompiledObjectKey, tuple[CompiledObjectKey, ...]]
    downstream_deps: dict[CompiledObjectKey, tuple[CompiledObjectKey, ...]]
    all_keys: dict[str, CompiledObjectKey]
    models_by_name: dict[str, CompiledModelView]
    selected_keys: frozenset[CompiledObjectKey]
    execution_order: tuple[CompiledObjectKey, ...]
    user_selected_keys: frozenset[CompiledObjectKey]
    python_read_source_names: frozenset[str]
    sql_test_selection: SqlTestSelection
    load_sources: bool
    end_audit_source_keys: frozenset[CompiledObjectKey]
    def __init__(
        self,
        upstream_deps: dict[CompiledObjectKey, tuple[CompiledObjectKey, ...]],
        downstream_deps: dict[CompiledObjectKey, tuple[CompiledObjectKey, ...]],
        all_keys: dict[str, CompiledObjectKey],
        models_by_name: dict[str, CompiledModelView],
        selected_keys: frozenset[CompiledObjectKey],
        execution_order: tuple[CompiledObjectKey, ...],
        user_selected_keys: frozenset[CompiledObjectKey] = ...,
        python_read_source_names: frozenset[str] = ...,
        sql_test_selection: SqlTestSelection = ...,
        load_sources: bool = ...,
        end_audit_source_keys: frozenset[CompiledObjectKey] = ...,
    ) -> None: ...

class PlannerRelationsContext:
    """Resolved relation and source inputs for plan entry construction."""

    model_locations: dict[str, CompiledRelationLocation]
    seed_locations: dict[str, CompiledRelationLocation]
    function_locations: dict[str, CompiledRelationLocation]
    source_map: dict[str, SourceEntry]
    source_read_map: dict[str, SourceEntry]
    python_source_read_map: dict[str, SourceEntry]
    source_warehouse_columns: dict[str, tuple[ColumnInfo, ...]]
    star_exclude_keyword: str
    listed_source_names: frozenset[str]
    def __init__(
        self,
        *,
        model_locations: dict[str, CompiledRelationLocation],
        seed_locations: dict[str, CompiledRelationLocation],
        function_locations: dict[str, CompiledRelationLocation],
        source_map: dict[str, SourceEntry],
        source_read_map: dict[str, SourceEntry],
        python_source_read_map: dict[str, SourceEntry],
        source_warehouse_columns: dict[str, tuple[ColumnInfo, ...]],
        star_exclude_keyword: str,
        listed_source_names: frozenset[str] = ...,
    ) -> None: ...

class SqlTestAssertionStep(NativeRecord):
    """One zero-row assertion step in a SQL-native unit test."""

    name: str
    resolved_sql: str
    lifted_ctes: tuple[tuple[str, str], ...]
    comparison_body_sql: str | None
    def __init__(
        self,
        name: str,
        resolved_sql: str,
        lifted_ctes: tuple[tuple[str, str], ...] = ...,
        comparison_body_sql: str | None = ...,
    ) -> None: ...

class PlannerSelection(NativeRecord):
    """User-facing selection inputs for one planner invocation."""

    select: tuple[str, ...]
    exclude: tuple[str, ...]
    selected_keys: frozenset[CompiledObjectKey] | None
    python_read_source_names: frozenset[str]
    sql_test_selection: SqlTestSelection
    def __init__(
        self,
        select: tuple[str, ...] = ...,
        exclude: tuple[str, ...] = ...,
        selected_keys: frozenset[CompiledObjectKey] | None = ...,
        python_read_source_names: frozenset[str] = ...,
        sql_test_selection: SqlTestSelection = ...,
    ) -> None: ...

class PlannerOverrides(NativeRecord):
    """Explicit user overrides that force or bound planner decisions."""

    cursor_overrides: CursorOverrides | None
    full_refresh: bool
    reload_sources: bool
    forced_stale_model_names: tuple[str, ...]
    external_blocked_model_names: tuple[str, ...]
    max_microbatches: int | None
    concurrency: int | None
    def __init__(
        self,
        cursor_overrides: CursorOverrides | None = ...,
        full_refresh: bool = ...,
        reload_sources: bool = ...,
        forced_stale_model_names: tuple[str, ...] = ...,
        external_blocked_model_names: tuple[str, ...] = ...,
        max_microbatches: int | None = ...,
        concurrency: int | None = ...,
    ) -> None: ...

class DeferralInputs(NativeRecord):
    """Deferred relation and source-deferral inputs for planning."""

    deferred_locations: dict[str, CompiledRelationLocation] | None
    deferred_relations: dict[str, RelationInfo] | None
    defer_sources_to: str | None
    source_deferral_enabled: bool
    def __init__(
        self,
        deferred_locations: dict[str, CompiledRelationLocation] | None = ...,
        deferred_relations: dict[str, RelationInfo] | None = ...,
        defer_sources_to: str | None = ...,
        source_deferral_enabled: bool = ...,
    ) -> None: ...

class PlannerPolicies(NativeRecord):
    """Behavior policies selected by the caller for one planner invocation."""

    auto_load_sources: bool
    selection_diagnostics: bool
    plan_sql_tests: bool
    record_migration_fingerprints: bool
    def __init__(
        self,
        auto_load_sources: bool = ...,
        selection_diagnostics: bool = ...,
        plan_sql_tests: bool = ...,
        record_migration_fingerprints: bool = ...,
    ) -> None: ...

class DiscoveredPythonLoaderMetadata(NativeRecord):
    """Loader-specific metadata carried by an internal Python loader node."""

    destination: str | None
    write_strategy: SourceWriteStrategy | None
    cursor_column: str | None
    unique_key: tuple[str, ...]
    columns: tuple[SourceColumnEntry, ...]
    contract: str | None
    connection_mode: LoaderConnectionMode
    def __init__(
        self,
        destination: str | None = ...,
        write_strategy: SourceWriteStrategy | None = ...,
        cursor_column: str | None = ...,
        unique_key: tuple[str, ...] = ...,
        columns: tuple[SourceColumnEntry, ...] = ...,
        contract: str | None = ...,
        connection_mode: LoaderConnectionMode = ...,
    ) -> None: ...

class DiscoveredPythonTaskMetadata(NativeRecord):
    """Task-specific metadata carried by an internal Python task node."""

    retry: RetryPolicy | None
    def __init__(
        self,
        retry: RetryPolicy | None = ...,
    ) -> None: ...

class DiscoveredPythonAssetMetadata(NativeRecord):
    """Asset-specific metadata carried by an internal Python asset node."""

    columns: tuple[SourceColumnEntry, ...]
    column_lineage: dict[str, tuple[ColumnLineageRef, ...]] | None
    retry: RetryPolicy | None
    def __init__(
        self,
        columns: tuple[SourceColumnEntry, ...] = ...,
        column_lineage: dict[str, tuple[ColumnLineageRef, ...]] | None = ...,
        retry: RetryPolicy | None = ...,
    ) -> None: ...

class DiscoveredPythonCheckMetadata(NativeRecord):
    """Check-specific metadata carried by an internal Python check node."""

    severity: PythonCheckSeverity
    def __init__(
        self,
        severity: PythonCheckSeverity = ...,
    ) -> None: ...

class PythonNodeDependencyEdge(NativeRecord):
    """Internal dependency edge between two discovered Python nodes."""

    upstream_name: str
    downstream_name: str
    def __init__(
        self,
        upstream_name: str,
        downstream_name: str,
    ) -> None: ...

class PythonSqlRunSelection(NativeRecord):
    """Unified run-command selection excluding Python checks."""

    sql_keys: frozenset[CompiledObjectKey]
    python_node_names: frozenset[str]
    def __init__(
        self,
        sql_keys: frozenset[CompiledObjectKey],
        python_node_names: frozenset[str],
    ) -> None: ...

class LiteralSqlRelation(NativeRecord):
    """A relation named in literal SQL in Python code that matched no project relation."""

    owner_label: str
    owner_kind: HardCodedRelationOwnerKind
    relative_path: Path
    line: int
    method: str
    relation: RelationName
    def __init__(
        self,
        owner_label: str,
        owner_kind: HardCodedRelationOwnerKind,
        relative_path: Path,
        line: int,
        method: str,
        relation: RelationName,
    ) -> None: ...

class DirectSourceFreshnessPlanningResult(NativeRecord):
    """Direct planning-time source freshness observations and comparisons."""

    observed_records: tuple[SourceFreshnessRecord, ...]
    previous_records: tuple[SourceFreshnessRecord, ...]
    changed_identities: frozenset[SourceFreshnessIdentity]
    unchanged_identities: frozenset[SourceFreshnessIdentity]
    unknown_source_names: tuple[str, ...]
    unknown_sources: dict[str, SourceFreshnessUnknown]
    age_statuses: dict[SourceFreshnessIdentity, SourceFreshnessAgeStatus]
    propagation: DirectSourceFreshnessPropagationResult | None
    def __init__(
        self,
        observed_records: tuple[SourceFreshnessRecord, ...] = ...,
        previous_records: tuple[SourceFreshnessRecord, ...] = ...,
        changed_identities: frozenset[SourceFreshnessIdentity] = ...,
        unchanged_identities: frozenset[SourceFreshnessIdentity] = ...,
        unknown_source_names: tuple[str, ...] = ...,
        unknown_sources: dict[str, SourceFreshnessUnknown] = ...,
        age_statuses: dict[SourceFreshnessIdentity, SourceFreshnessAgeStatus] = ...,
        propagation: DirectSourceFreshnessPropagationResult | None = ...,
    ) -> None: ...

class SqlBindingDiagnostic(NativeRecord):
    """One native binding error or non-blocking runtime-conversion warning."""

    code: str
    message: str
    line: int | None
    column: int | None
    start: int | None
    end: int | None
    severity: str
    def __init__(
        self,
        code: str,
        message: str,
        line: int | None = ...,
        column: int | None = ...,
        start: int | None = ...,
        end: int | None = ...,
        severity: str = ...,
    ) -> None: ...

class SqlBindingResult(NativeRecord):
    """Stable result of schema-aware semantic SQL validation."""

    diagnostics: tuple[SqlBindingDiagnostic, ...]
    def __init__(
        self,
        diagnostics: tuple[SqlBindingDiagnostic, ...] = ...,
    ) -> None: ...

class SqlSchemaValidationRequest(NativeRecord):
    """One expanded SQL query and its complete relation schemas."""

    sql: str
    dialect: str | None
    schema: Mapping[str, Mapping[str, str]]
    known_functions: tuple[str, ...]
    known_types: tuple[str, ...]
    quoted_identifiers_ignore_case: bool
    catalog: Any | None
    def __init__(
        self,
        sql: str,
        dialect: str | None,
        schema: Mapping[str, Mapping[str, str]],
        known_functions: tuple[str, ...] = ...,
        known_types: tuple[str, ...] = ...,
        quoted_identifiers_ignore_case: bool = ...,
        catalog: Any | None = ...,
    ) -> None: ...

class DbtCliOptions(NativeRecord):
    """Resolved dbt CLI options shared by dbt commands."""

    project_dir: Path | None
    profiles_dir: Path | None
    target: str | None
    target_path: Path | None
    vars: str | None
    state: Path | None
    defer: bool
    def __init__(
        self,
        project_dir: Path | None = ...,
        profiles_dir: Path | None = ...,
        target: str | None = ...,
        target_path: Path | None = ...,
        vars: str | None = ...,
        state: Path | None = ...,
        defer: bool = ...,
    ) -> None: ...

class NormalizedDbtProfileConnection(NativeRecord):
    """Rendered dbt profile output normalized for SQLBuild."""

    adapter: str
    connection: dict[str, object]
    target_schema: str | None
    target_database: str | None
    warnings: tuple[str, ...]
    def __init__(
        self,
        adapter: str,
        connection: dict[str, object],
        target_schema: str | None = ...,
        target_database: str | None = ...,
        warnings: tuple[str, ...] = ...,
    ) -> None: ...

class DbtInitProgressCallbacks(NativeRecord):
    """Optional progress hooks for `sqb dbt init` phases."""

    start: Callable[[str], None] | None
    complete: Callable[[str], None] | None
    def __init__(
        self,
        start: Callable[[str], None] | None = ...,
        complete: Callable[[str], None] | None = ...,
    ) -> None: ...

class DbtInitRequest(NativeRecord):
    """Request to initialize a SQLBuild project from a dbt project."""

    cwd: Path
    dbt_project_dir: Path
    profiles_dir: Path | None
    profile_name: str | None
    target_name: str | None
    sqb_output_dir: Path | None
    dry_run: bool
    overwrite: bool
    skip_dbt_debug: bool
    progress_callbacks: DbtInitProgressCallbacks
    def __init__(
        self,
        cwd: Path,
        dbt_project_dir: Path,
        profiles_dir: Path | None,
        profile_name: str | None,
        target_name: str | None,
        sqb_output_dir: Path | None,
        dry_run: bool = ...,
        overwrite: bool = ...,
        skip_dbt_debug: bool = ...,
        progress_callbacks: DbtInitProgressCallbacks = ...,
    ) -> None: ...

class DbtInitResult(NativeRecord):
    """Result from `sqb dbt init`."""

    output_dir: Path
    project_file: Path
    project_name: str
    adapter: str
    target_name: str
    profile_name: str
    toml: str
    warnings: tuple[str, ...]
    dry_run: bool
    def __init__(
        self,
        output_dir: Path,
        project_file: Path,
        project_name: str,
        adapter: str,
        target_name: str,
        profile_name: str,
        toml: str,
        warnings: tuple[str, ...] = ...,
        dry_run: bool = ...,
    ) -> None: ...

class DbtCommandResult(NativeRecord):
    """Completed dbt command output."""

    argv: tuple[str, ...]
    returncode: int
    stdout: str
    stderr: str
    def __init__(
        self,
        argv: tuple[str, ...],
        returncode: int,
        stdout: str = ...,
        stderr: str = ...,
    ) -> None: ...

class DbtNodeExecutionResult(NativeRecord):
    """One dbt node execution result parsed from JSON logs."""

    unique_id: str
    resource_type: str
    node_name: str
    status: str
    index: int | None
    total: int | None
    execution_time: float | None
    materialized: str | None
    relation_name: str | None
    database: str | None
    schema: str | None
    node_checksum: str | None
    messages: tuple[DbtNodeMessage, ...]
    def __init__(
        self,
        unique_id: str,
        resource_type: str,
        node_name: str,
        status: str,
        index: int | None,
        total: int | None,
        execution_time: float | None,
        materialized: str | None = ...,
        relation_name: str | None = ...,
        database: str | None = ...,
        schema: str | None = ...,
        node_checksum: str | None = ...,
        messages: tuple[DbtNodeMessage, ...] = ...,
    ) -> None: ...

class DbtLsResult(NativeRecord):
    """Parsed result from one dbt ls invocation."""

    nodes: tuple[DbtLsNode, ...]
    command: DbtCommandResult
    def __init__(
        self,
        nodes: tuple[DbtLsNode, ...],
        command: DbtCommandResult,
    ) -> None: ...

class RuleGuidance(NativeRecord):
    """Structured authoring guidance shared by diagnostics and generated documentation."""

    good_example: str
    anti_tautology: str
    mutation_check: str
    def __init__(
        self,
        good_example: str,
        anti_tautology: str,
        mutation_check: str,
    ) -> None: ...

class RuleExemption(NativeRecord):
    rule: str
    path: str
    reason: str
    relation: str | None
    cte: str | None
    def __init__(
        self,
        rule: str,
        path: str,
        reason: str,
        relation: str | None = ...,
        cte: str | None = ...,
    ) -> None: ...

class GraphEdgeExclusion(NativeRecord):
    """One exact, temporary internal-layer dependency exception."""

    consumer: str
    dependency: str
    reason: str
    def __init__(
        self,
        consumer: str,
        dependency: str,
        reason: str,
    ) -> None: ...

class RuleIgnore(NativeRecord):
    rules: tuple[str, ...]
    paths: tuple[str, ...]
    reason: str
    selectors: tuple[str, ...]
    def __init__(
        self,
        rules: tuple[str, ...],
        paths: tuple[str, ...],
        reason: str,
        selectors: tuple[str, ...] = ...,
    ) -> None: ...

class SelectStarAllow(NativeRecord):
    paths: tuple[str, ...]
    reason: str
    def __init__(
        self,
        paths: tuple[str, ...],
        reason: str,
    ) -> None: ...

class ThresholdOverride(NativeRecord):
    paths: tuple[str, ...]
    thresholds: dict[str, int]
    reason: str
    def __init__(
        self,
        paths: tuple[str, ...],
        thresholds: dict[str, int],
        reason: str,
    ) -> None: ...

class RulesCacheConfig(NativeRecord):
    enabled: bool
    def __init__(
        self,
        enabled: bool = ...,
    ) -> None: ...

class SqlTestRulesConfig(NativeRecord):
    pipeline_directory: str
    def __init__(
        self,
        pipeline_directory: str = ...,
    ) -> None: ...

class SqlRulesEvaluation(NativeRecord):
    """SQL rule findings of one run, their cache accounting and elapsed time."""

    findings: tuple[Finding, ...]
    cache_hits: int
    cache_misses: int
    sql_ms: int
    def __init__(
        self,
        findings: tuple[Finding, ...],
        cache_hits: int,
        cache_misses: int,
        sql_ms: int = ...,
    ) -> None: ...

class RuleRunPlan(NativeRecord):
    """The catalogue, selected rules, project slice and settings one rules run evaluates."""

    effective_config: RulesConfig
    project_dir: Path
    catalogue: tuple[Rule, ...]
    custom_payloads: list[dict[str, object]]
    selected: tuple[Rule, ...]
    skipped_type_proof_rules: tuple[str, ...]
    model_paths: frozenset[str] | None
    project: CompiledProjectView
    config: RulesConfig
    def __init__(
        self,
        effective_config: RulesConfig,
        project_dir: Path,
        catalogue: tuple[Rule, ...],
        custom_payloads: list[dict[str, object]],
        selected: tuple[Rule, ...],
        skipped_type_proof_rules: tuple[str, ...],
        model_paths: frozenset[str] | None,
        project: CompiledProjectView,
        config: RulesConfig,
    ) -> None: ...

class RulesRunResult(NativeRecord):
    """Unified findings and phase measurements for configured rules."""

    findings: tuple[Finding, ...]
    evaluated_models: int
    built_in_ms: int
    custom_ms: int
    custom_cpu_ms: int
    unevaluated_resources: int
    cache_hits: int
    cache_misses: int
    skipped_type_proof_rules: tuple[str, ...]
    custom_rule_timings: tuple[tuple[str, int], ...]
    def __init__(
        self,
        findings: tuple[Finding, ...],
        evaluated_models: int,
        built_in_ms: int,
        custom_ms: int,
        custom_cpu_ms: int = ...,
        unevaluated_resources: int = ...,
        cache_hits: int = ...,
        cache_misses: int = ...,
        skipped_type_proof_rules: tuple[str, ...] = ...,
        custom_rule_timings: tuple[tuple[str, int], ...] = ...,
    ) -> None: ...

class CustomRulesOutcome(NativeRecord):
    """Custom-rule findings in canonical order and their cache accounting."""

    findings: tuple[Finding, ...]
    cache_hits: int
    cache_misses: int
    custom_ms: int
    custom_cpu_ms: int
    custom_rule_timings: tuple[tuple[str, int], ...]
    def __init__(
        self,
        findings: tuple[Finding, ...],
        cache_hits: int,
        cache_misses: int,
        custom_ms: int,
        custom_cpu_ms: int = ...,
        custom_rule_timings: tuple[tuple[str, int], ...] = ...,
    ) -> None: ...

class ResourceKind(StrEnum):
    """Kinds of authored resources represented in a scope index."""

    MODEL = "model"
    TEST = "test"
    SCENARIO = "scenario"
    FUNCTION = "function"
    SOURCE = "source"
    SEED = "seed"

class DeclarationKind(StrEnum):
    """Independent public declaration namespaces."""

    MACRO = "macro"
    ENUM = "enum"
    CONSTANT = "constant"
    AUDIT = "audit"
    SINGULAR_AUDIT = "singular_audit"
    SCHEMA = "schema"
    SQL_HOOK = "sql_hook"
    PYTHON_HOOK = "python_hook"

class ScopeKind(StrEnum):
    """Compiler-enforced declaration visibility tiers."""

    GLOBAL = "global"
    INHERITED = "inherited"
    LOCAL = "local"
    PRIVATE = "private"

class OwnershipRootKind(StrEnum):
    """How an ownership root participates in authored resource scope."""

    RESOURCE = "resource"
    GLOBAL = "global"

class UsageKind(StrEnum):
    """Why one indexed identity consumes a declaration."""

    RUNTIME = "runtime"
    GENERATED = "generated"
    DECLARATION_DEPENDENCY = "declaration_dependency"

class GrantKind(StrEnum):
    """Compiler relationships that grant additional public vocabulary."""

    EXPECTED_MODEL = "expected_model"
    TESTED_MACRO = "tested_macro"

class VisibilityReason(StrEnum):
    """Positive explanations emitted by the canonical resolver."""

    GLOBAL = "global"
    INHERITED_ANCESTOR = "inherited_ancestor"
    LOCAL_OWNER = "local_owner"
    PRIVATE_OWNER = "private_owner"
    EXPECTED_MODEL = "expected_model"
    TESTED_MACRO = "tested_macro"

class InaccessibleReason(StrEnum):
    """Stable explanations for declarations outside effective scope."""

    LOCAL_BOUNDARY = "local_boundary"
    SIBLING_SCOPE = "sibling_scope"
    DESCENDANT_SCOPE = "descendant_scope"
    UNRELATED_SCOPE = "unrelated_scope"
    PRIVATE_OWNER = "private_owner"
    UNSUPPORTED_RESOURCE_KIND = "unsupported_resource_kind"

class ScopeDiagnosticSeverity(StrEnum):
    """Severity of a stable scope diagnostic."""

    ERROR = "error"
    WARNING = "warning"

class ScopeDiagnosticCode(StrEnum):
    """Stable diagnostic identities shared by future scope consumers."""

    UNKNOWN_TARGET = "S001"
    UNQUALIFIED_TARGET = "S002"
    DUPLICATE_DECLARATION = "S003"
    INVALID_DECLARATION_NAME = "S004"
    RESERVED_DECLARATION_NAME = "S005"
    INACCESSIBLE_DECLARATION = "S006"
    LOCAL_NEEDED_BY_DESCENDANT = "S007"
    OVER_BROAD_INHERITED = "S008"
    REQUIRES_GLOBAL_PLACEMENT = "S009"
    UNUSED_DECLARATION = "S010"
    INVALID_PROSPECTIVE_PATH = "S011"
    NESTED_DECLARATION_ROOT = "S012"
    INCOMPLETE_USAGE = "S013"
    INVALID_CURSOR = "S014"
    MACRO_MODULE_IMPORT = "S015"
    INVALID_MACRO_DEPENDENCY = "S016"
    MACRO_DEPENDENCY_CYCLE = "S017"
    DUPLICATE_RESOURCE = "S018"
    RESOURCE_PARSE_ERROR = "S019"
    DECLARATION_PARSE_ERROR = "S020"
    MACRO_PARSE_ERROR = "S021"
    RELATIONSHIP_PARSE_ERROR = "S022"
    CONFIG_PARSE_ERROR = "S023"
    OVER_BROAD_GLOBAL = "S024"

class ResourceIdentity(NativeRecord):
    """Stable kind-qualified identity of one authored resource."""

    kind: ResourceKind
    name: str
    def __init__(
        self,
        kind: ResourceKind,
        name: str,
    ) -> None: ...

class DeclarationIdentity(NativeRecord):
    """Stable declaration identity, including a private declaration owner."""

    kind: DeclarationKind
    name: str
    owner: ResourceIdentity | None
    def __init__(
        self,
        kind: DeclarationKind,
        name: str,
        owner: ResourceIdentity | None = ...,
    ) -> None: ...

class OwnershipRoot(NativeRecord):
    """Normalized project-relative directory that owns resource scope."""

    path: str
    kind: OwnershipRootKind
    resource_kind: ResourceKind | None
    def __init__(
        self,
        path: str,
        kind: OwnershipRootKind = ...,
        resource_kind: ResourceKind | None = ...,
    ) -> None: ...

class ResourceRecord(NativeRecord):
    """One resource participating in lexical declaration scope."""

    identity: ResourceIdentity
    path: str
    ownership_root: OwnershipRoot
    def __init__(
        self,
        identity: ResourceIdentity,
        path: str,
        ownership_root: OwnershipRoot,
    ) -> None: ...

class MacroMetadata(NativeRecord):
    """Safe macro metadata required by scope and dependency inspection."""

    parameters: tuple[str, ...]
    dependencies: tuple[DeclarationIdentity, ...]
    source_digest: str
    def __init__(
        self,
        parameters: tuple[str, ...] = ...,
        dependencies: tuple[DeclarationIdentity, ...] = ...,
        source_digest: str = ...,
    ) -> None: ...

class EnumMemberMetadata(NativeRecord):
    """One value-free authored enum member identity."""

    name: str
    def __init__(
        self,
        name: str,
    ) -> None: ...

class EnumMetadata(NativeRecord):
    """Safe enum metadata containing member names and their shared scalar type."""

    members: tuple[EnumMemberMetadata, ...]
    scalar_type: str
    def __init__(
        self,
        members: tuple[EnumMemberMetadata, ...],
        scalar_type: str,
    ) -> None: ...

class ConstantMetadata(NativeRecord):
    """Value-free normalized metadata for one typed constant."""

    logical_type: str
    collection_kind: str | None
    item_count: int | None
    nullable: bool
    render_as: str | None
    def __init__(
        self,
        logical_type: str,
        collection_kind: str | None = ...,
        item_count: int | None = ...,
        nullable: bool = ...,
        render_as: str | None = ...,
    ) -> None: ...

class DeclarationRecord(NativeRecord):
    """One declaration definition and its compiler-owned scope metadata."""

    identity: DeclarationIdentity
    path: str
    line: int
    column: int
    scope: ScopeKind
    ownership_root: OwnershipRoot
    owning_path: str | None
    macro: MacroMetadata | None
    enum: EnumMetadata | None
    constant: ConstantMetadata | None
    def __init__(
        self,
        identity: DeclarationIdentity,
        path: str,
        line: int,
        column: int,
        scope: ScopeKind,
        ownership_root: OwnershipRoot,
        owning_path: str | None = ...,
        macro: MacroMetadata | None = ...,
        enum: EnumMetadata | None = ...,
        constant: ConstantMetadata | None = ...,
    ) -> None: ...

class UsageRecord(NativeRecord):
    """A runtime or declaration dependency edge."""

    consumer: ResourceIdentity | DeclarationIdentity
    declaration: DeclarationIdentity
    kind: UsageKind
    through: ResourceIdentity | DeclarationIdentity | None
    enum_member: str | None
    def __init__(
        self,
        consumer: ResourceIdentity | DeclarationIdentity,
        declaration: DeclarationIdentity,
        kind: UsageKind = ...,
        through: ResourceIdentity | DeclarationIdentity | None = ...,
        enum_member: str | None = ...,
    ) -> None: ...

class GrantRecord(NativeRecord):
    """A compiler relationship granting a public declaration to a resource."""

    resource: ResourceIdentity
    declaration: DeclarationIdentity
    through: ResourceIdentity | DeclarationIdentity
    kind: GrantKind
    def __init__(
        self,
        resource: ResourceIdentity,
        declaration: DeclarationIdentity,
        through: ResourceIdentity | DeclarationIdentity,
        kind: GrantKind = ...,
    ) -> None: ...

class VisibilityRecord(NativeRecord):
    """A resolved positive visibility fact with stable provenance."""

    resource: ResourceIdentity
    declaration: DeclarationIdentity
    reason: VisibilityReason
    through: ResourceIdentity | DeclarationIdentity | None
    def __init__(
        self,
        resource: ResourceIdentity,
        declaration: DeclarationIdentity,
        reason: VisibilityReason,
        through: ResourceIdentity | DeclarationIdentity | None = ...,
    ) -> None: ...

class InaccessibleRecord(NativeRecord):
    """A resolved negative visibility fact with stable provenance."""

    resource: ResourceIdentity
    declaration: DeclarationIdentity
    reason: InaccessibleReason
    def __init__(
        self,
        resource: ResourceIdentity,
        declaration: DeclarationIdentity,
        reason: InaccessibleReason,
    ) -> None: ...

class ScopeDiagnostic(NativeRecord):
    """Stable diagnostic retained even when an index is partial."""

    code: ScopeDiagnosticCode
    message: str
    severity: ScopeDiagnosticSeverity
    path: str | None
    line: int | None
    column: int | None
    declaration: DeclarationIdentity | None
    resource: ResourceIdentity | None
    def __init__(
        self,
        code: ScopeDiagnosticCode,
        message: str,
        severity: ScopeDiagnosticSeverity = ...,
        path: str | None = ...,
        line: int | None = ...,
        column: int | None = ...,
        declaration: DeclarationIdentity | None = ...,
        resource: ResourceIdentity | None = ...,
    ) -> None: ...

class ScopeCompleteness(NativeRecord):
    """Independent completeness facts for partially invalid projects."""

    discovery: bool
    static_visibility: bool
    runtime_usage: bool
    relationships: bool
    placement: bool
    promotion_impact: bool
    def __init__(
        self,
        discovery: bool = ...,
        static_visibility: bool = ...,
        runtime_usage: bool = ...,
        relationships: bool = ...,
        placement: bool = ...,
        promotion_impact: bool = ...,
    ) -> None: ...
    @property
    def complete(self) -> bool: ...

class ScopeIndex(NativeRecord):
    """Canonical deterministic facts used by every future scope consumer."""

    ownership_roots: tuple[OwnershipRoot, ...]
    resources: tuple[ResourceRecord, ...]
    declarations: tuple[DeclarationRecord, ...]
    usages: tuple[UsageRecord, ...]
    grants: tuple[GrantRecord, ...]
    visibility: tuple[VisibilityRecord, ...]
    inaccessible: tuple[InaccessibleRecord, ...]
    diagnostics: tuple[ScopeDiagnostic, ...]
    completeness: ScopeCompleteness
    def __init__(
        self,
        ownership_roots: tuple[OwnershipRoot, ...] = ...,
        resources: tuple[ResourceRecord, ...] = ...,
        declarations: tuple[DeclarationRecord, ...] = ...,
        usages: tuple[UsageRecord, ...] = ...,
        grants: tuple[GrantRecord, ...] = ...,
        visibility: tuple[VisibilityRecord, ...] = ...,
        inaccessible: tuple[InaccessibleRecord, ...] = ...,
        diagnostics: tuple[ScopeDiagnostic, ...] = ...,
        completeness: ScopeCompleteness = ...,
    ) -> None: ...

class DeclarationVisibilityIndex(NativeRecord):
    """Declaration positions grouped by the scope facts that make them visible."""

    identities: tuple[DeclarationIdentity, ...]
    global_positions: tuple[int, ...]
    private_positions: Mapping[ResourceIdentity, tuple[int, ...]]
    local_positions: Mapping[str, tuple[int, ...]]
    inherited_positions: Mapping[str, tuple[int, ...]]
    def __init__(
        self,
        identities: tuple[DeclarationIdentity, ...],
        global_positions: tuple[int, ...],
        private_positions: Mapping[ResourceIdentity, tuple[int, ...]],
        local_positions: Mapping[str, tuple[int, ...]],
        inherited_positions: Mapping[str, tuple[int, ...]],
    ) -> None: ...

class ScopeLookup(NativeRecord):
    """Immutable indexes over one canonical ``ScopeIndex``."""

    index: ScopeIndex
    resources: Mapping[ResourceIdentity, tuple[ResourceRecord, ...]]
    resources_by_path: Mapping[str, tuple[ResourceRecord, ...]]
    declarations: Mapping[DeclarationIdentity, tuple[DeclarationRecord, ...]]
    usages_by_consumer: Mapping[ResourceIdentity | DeclarationIdentity, tuple[UsageRecord, ...]]
    usages_by_declaration: Mapping[DeclarationIdentity, tuple[UsageRecord, ...]]
    grants_by_resource: Mapping[ResourceIdentity, tuple[GrantRecord, ...]]
    visibility_index: DeclarationVisibilityIndex
    native: object | None
    def __init__(
        self,
        index: ScopeIndex,
        resources: Mapping[ResourceIdentity, tuple[ResourceRecord, ...]],
        resources_by_path: Mapping[str, tuple[ResourceRecord, ...]],
        declarations: Mapping[DeclarationIdentity, tuple[DeclarationRecord, ...]],
        usages_by_consumer: Mapping[
            ResourceIdentity | DeclarationIdentity, tuple[UsageRecord, ...]
        ],
        usages_by_declaration: Mapping[DeclarationIdentity, tuple[UsageRecord, ...]],
        grants_by_resource: Mapping[ResourceIdentity, tuple[GrantRecord, ...]],
        visibility_index: DeclarationVisibilityIndex,
        native: object | None = ...,
    ) -> None: ...

class ScopeTargetQuery(NativeRecord):
    """Non-throwing qualified-identity or path lookup result."""

    value: str | ResourceIdentity | DeclarationIdentity
    matches: tuple[ResourceRecord, ...]
    declaration_matches: tuple[DeclarationRecord, ...]
    def __init__(
        self,
        value: str | ResourceIdentity | DeclarationIdentity,
        matches: tuple[ResourceRecord, ...] = ...,
        declaration_matches: tuple[DeclarationRecord, ...] = ...,
    ) -> None: ...
    @property
    def unknown(self) -> bool: ...

class VisibilityResolution(NativeRecord):
    """Complete static visibility classification for one queried target."""

    target: ScopeTargetQuery
    visible: tuple[VisibilityRecord, ...]
    inaccessible: tuple[InaccessibleRecord, ...]
    def __init__(
        self,
        target: ScopeTargetQuery,
        visible: tuple[VisibilityRecord, ...] = ...,
        inaccessible: tuple[InaccessibleRecord, ...] = ...,
    ) -> None: ...

class DeclarationVisibility(NativeRecord):
    """Visible facts and inaccessible declaration identities for one queried target."""

    target: ScopeTargetQuery
    visible: tuple[VisibilityRecord, ...]
    inaccessible: tuple[DeclarationIdentity, ...]
    def __init__(
        self,
        target: ScopeTargetQuery,
        visible: tuple[VisibilityRecord, ...] = ...,
        inaccessible: tuple[DeclarationIdentity, ...] = ...,
    ) -> None: ...

class ScopeReportFilters(NativeRecord):
    """Orthogonal declaration filters applied in their documented fixed order."""

    sections: tuple[str, ...]
    include_nearby: bool
    defined_under: str | None
    kinds: tuple[DeclarationKind, ...]
    glob: str | None
    used_only: bool
    dependency_depth: int
    sort: str
    cursor: str | None
    page_size: int
    nearby_depth: int
    globals: str
    def __init__(
        self,
        sections: tuple[str, ...] = ...,
        include_nearby: bool = ...,
        defined_under: str | None = ...,
        kinds: tuple[DeclarationKind, ...] = ...,
        glob: str | None = ...,
        used_only: bool = ...,
        dependency_depth: int = ...,
        sort: str = ...,
        cursor: str | None = ...,
        page_size: int = ...,
        nearby_depth: int = ...,
        globals: str = ...,
    ) -> None: ...

class VisibilityProvenance(NativeRecord):
    """Why a declaration is visible or inaccessible."""

    reason: str
    through: str | None
    def __init__(
        self,
        reason: str,
        through: str | None = ...,
    ) -> None: ...

class DeclarationReport(NativeRecord):
    """Value-free declaration projection used by every report section."""

    identity: str
    kind: str
    name: str
    owner: str | None
    definition: SourceLocation
    scope: str
    owning_path: str | None
    visibility: VisibilityProvenance | None
    inaccessible_reason: str | None
    metadata: tuple[tuple[str, object], ...]
    consumers: tuple[str, ...]
    dependencies: tuple[str, ...]
    grants: tuple[str, ...]
    required_scope: str | None
    required_path: str | None
    promotion_impact: tuple[str, ...]
    def __init__(
        self,
        identity: str,
        kind: str,
        name: str,
        owner: str | None,
        definition: SourceLocation,
        scope: str,
        owning_path: str | None,
        visibility: VisibilityProvenance | None = ...,
        inaccessible_reason: str | None = ...,
        metadata: tuple[tuple[str, object], ...] = ...,
        consumers: tuple[str, ...] = ...,
        dependencies: tuple[str, ...] = ...,
        grants: tuple[str, ...] = ...,
        required_scope: str | None = ...,
        required_path: str | None = ...,
        promotion_impact: tuple[str, ...] = ...,
    ) -> None: ...

class ScopeChainEntry(NativeRecord):
    """One exact or inherited lexical scope in deterministic path order."""

    kind: str
    path: str
    declaration_count: int
    def __init__(
        self,
        kind: str,
        path: str,
        declaration_count: int,
    ) -> None: ...

class ScopeSection(NativeRecord):
    """Pagination and completeness metadata for one report section."""

    name: str
    total: int
    returned: int
    collapsed: bool
    collapsed_count: int
    truncated: bool
    complete: bool
    next_cursor: str | None
    cursor: str | None
    page_size: int
    def __init__(
        self,
        name: str,
        total: int,
        returned: int,
        collapsed: bool = ...,
        collapsed_count: int = ...,
        truncated: bool = ...,
        complete: bool = ...,
        next_cursor: str | None = ...,
        cursor: str | None = ...,
        page_size: int = ...,
    ) -> None: ...

class ScopeResourceReport(NativeRecord):
    """Resolved existing resource or prospective authored path."""

    target: str
    identity: str | None
    path: str | None
    prospective: bool
    directory: bool
    duplicate_count: int
    def __init__(
        self,
        target: str,
        identity: str | None,
        path: str | None,
        prospective: bool = ...,
        directory: bool = ...,
        duplicate_count: int = ...,
    ) -> None: ...

class DeclarationExplanation(NativeRecord):
    """Detailed explanation for one qualified declaration."""

    declaration: DeclarationReport | None
    complete: bool
    def __init__(
        self,
        declaration: DeclarationReport | None,
        complete: bool,
    ) -> None: ...

class MovePreview(NativeRecord):
    """Pure visibility delta for moving one existing resource."""

    resource: str
    destination: str
    new_ownership_root: str | None
    retained: tuple[DeclarationReport, ...]
    gained: tuple[DeclarationReport, ...]
    lost: tuple[DeclarationReport, ...]
    invalidated_usages: tuple[str, ...]
    private_retained: tuple[DeclarationReport, ...]
    relationship_retained: tuple[DeclarationReport, ...]
    complete: bool
    def __init__(
        self,
        resource: str,
        destination: str,
        new_ownership_root: str | None,
        retained: tuple[DeclarationReport, ...] = ...,
        gained: tuple[DeclarationReport, ...] = ...,
        lost: tuple[DeclarationReport, ...] = ...,
        invalidated_usages: tuple[str, ...] = ...,
        private_retained: tuple[DeclarationReport, ...] = ...,
        relationship_retained: tuple[DeclarationReport, ...] = ...,
        complete: bool = ...,
    ) -> None: ...

class ScopeFolder(NativeRecord):
    """One declaration-definition folder with exact recursive counts."""

    path: str
    name: str
    descendant_count: int
    child_count: int
    used_count: int
    kind_counts: tuple[tuple[str, int], ...]
    def __init__(
        self,
        path: str,
        name: str,
        descendant_count: int,
        child_count: int,
        used_count: int,
        kind_counts: tuple[tuple[str, int], ...],
    ) -> None: ...

class ScopeBrowseResult(NativeRecord):
    """Folder-first browse result."""

    folder: str
    folders: tuple[ScopeFolder, ...]
    diagnostics: tuple[ScopeDiagnostic, ...]
    complete: bool
    def __init__(
        self,
        folder: str,
        folders: tuple[ScopeFolder, ...],
        diagnostics: tuple[ScopeDiagnostic, ...] = ...,
        complete: bool = ...,
    ) -> None: ...

class ScopeListResult(NativeRecord):
    """Recursive declaration list beneath one definition folder."""

    folder: str
    declarations: tuple[DeclarationReport, ...]
    section: ScopeSection
    diagnostics: tuple[ScopeDiagnostic, ...]
    def __init__(
        self,
        folder: str,
        declarations: tuple[DeclarationReport, ...],
        section: ScopeSection,
        diagnostics: tuple[ScopeDiagnostic, ...] = ...,
    ) -> None: ...

class ScopeReport(NativeRecord):
    """Canonical payload shared by all non-rendering scope consumers."""

    resource: ScopeResourceReport
    scope_chain: tuple[ScopeChainEntry, ...]
    available: tuple[DeclarationReport, ...]
    used: tuple[DeclarationReport, ...]
    relationship_scope: tuple[DeclarationReport, ...]
    nearby_unavailable: tuple[DeclarationReport, ...]
    filters: ScopeReportFilters
    sections: tuple[ScopeSection, ...]
    explanation: DeclarationExplanation | None
    move_preview: MovePreview | None
    diagnostics: tuple[ScopeDiagnostic, ...]
    complete: bool
    def __init__(
        self,
        resource: ScopeResourceReport,
        scope_chain: tuple[ScopeChainEntry, ...] = ...,
        available: tuple[DeclarationReport, ...] = ...,
        used: tuple[DeclarationReport, ...] = ...,
        relationship_scope: tuple[DeclarationReport, ...] = ...,
        nearby_unavailable: tuple[DeclarationReport, ...] = ...,
        filters: ScopeReportFilters = ...,
        sections: tuple[ScopeSection, ...] = ...,
        explanation: DeclarationExplanation | None = ...,
        move_preview: MovePreview | None = ...,
        diagnostics: tuple[ScopeDiagnostic, ...] = ...,
        complete: bool = ...,
    ) -> None: ...

class EventExportSeverity(StrEnum):
    """Severity used to filter exported lifecycle events."""

    DEBUG = "debug"
    INFO = "info"
    WARNING = "warning"
    ERROR = "error"

class WarehouseGroup(StrEnum):
    """Command group that selects a target's default warehouse."""

    BUILD = "build"
    QUERY = "query"

class SourceWriteStrategy(StrEnum):
    APPEND = "append"
    DELETE_INSERT = "delete_insert"
    MERGE = "merge"
    TABLE = "table"

class FutureCursorAction(StrEnum):
    """Response when an effective cursor lies beyond the future horizon."""

    CAP = "cap"
    ERROR = "error"

class MicrobatchLimitAction(StrEnum):
    """Response when one model plans more than the configured batch limit."""

    CAP_FROM_END = "cap_from_end"
    CAP_FROM_START = "cap_from_start"
    ERROR = "error"
    WARN = "warn"

class ColumnContractMode(StrEnum):
    """Default contract behavior for model column declarations."""

    IMPLICIT = "implicit"
    EXPLICIT = "explicit"

class SourceFreshnessStrategy(StrEnum):
    ADAPTER = "adapter"
    COLUMN = "column"
    SQL = "sql"

class SourceFreshnessValueKind(StrEnum):
    TIMESTAMP = "timestamp"
    INTEGER = "integer"
    STRING = "string"

class TimeTravelRetentionSource(StrEnum):
    """Authored layer that supplied the effective retention policy."""

    TARGET = "target"
    MATERIALIZATION = "materialization"
    MODEL = "model"

class TableType(StrEnum):
    """Snowflake managed table type."""

    PERMANENT = "permanent"
    TRANSIENT = "transient"

class TableTypeSource(StrEnum):
    """Authored layer that supplied the effective table type."""

    DEFAULT = "default"
    TARGET = "target"
    MATERIALIZATION = "materialization"
    MODEL = "model"

class TableTypeDowngradePolicy(StrEnum):
    """Permanent-to-transient conversion policy."""

    DENY = "deny"
    REQUIRE_CONFIRMATION = "require_confirmation"
    ALLOW = "allow"

class MissingMigrationOriginPolicy(StrEnum):
    """Policy for a declared migrate_from whose origin does not exist in a target."""

    DENY = "deny"
    REQUIRE_CONFIRMATION = "require_confirmation"
    ALLOW = "allow"

class RetentionDecreasePolicy(StrEnum):
    """Policy for lowering live time-travel retention."""

    DENY = "deny"
    REQUIRE_CONFIRMATION = "require_confirmation"
    ALLOW = "allow"

class ClonePolicy(NativeRecord):
    """Environment clone policy."""

    allow_as_clone_origin: bool
    allow_as_clone_destination: bool
    def __init__(
        self,
        allow_as_clone_origin: bool = ...,
        allow_as_clone_destination: bool = ...,
    ) -> None: ...

class LocalClonePolicy(NativeRecord):
    """Local environment clone policy overrides."""

    allow_as_clone_origin: bool | None
    allow_as_clone_destination: bool | None
    def __init__(
        self,
        allow_as_clone_origin: bool | None = ...,
        allow_as_clone_destination: bool | None = ...,
    ) -> None: ...

class AuthoredTimeTravelRetention(NativeRecord):
    """One authored time-travel retention policy."""

    desired_days: int | None
    unmanaged: bool
    def __init__(
        self,
        desired_days: int | None = ...,
        unmanaged: bool = ...,
    ) -> None: ...

class MaterializationRetentionDefaults(NativeRecord):
    """Retention defaults for one built-in materialization."""

    time_travel_retention: AuthoredTimeTravelRetention | None
    table_type: TableType | None
    def __init__(
        self,
        time_travel_retention: AuthoredTimeTravelRetention | None = ...,
        table_type: TableType | None = ...,
    ) -> None: ...

class MaterializationDefaultsConfig(NativeRecord):
    """Project retention defaults by materialization type."""

    table: MaterializationRetentionDefaults
    incremental: MaterializationRetentionDefaults
    snapshot: MaterializationRetentionDefaults
    def __init__(
        self,
        table: MaterializationRetentionDefaults = ...,
        incremental: MaterializationRetentionDefaults = ...,
        snapshot: MaterializationRetentionDefaults = ...,
    ) -> None: ...

class ResolvedTimeTravelRetention(NativeRecord):
    """Effective retention policy attached to a compiled model."""

    desired_days: int | None
    unmanaged: bool
    source: TimeTravelRetentionSource | None
    def __init__(
        self,
        desired_days: int | None = ...,
        unmanaged: bool = ...,
        source: TimeTravelRetentionSource | None = ...,
    ) -> None: ...

class ResolvedTableType(NativeRecord):
    """Effective Snowflake table type attached to a compiled model."""

    value: TableType
    source: TableTypeSource
    declared: bool
    def __init__(
        self,
        value: TableType = ...,
        source: TableTypeSource = ...,
        declared: bool = ...,
    ) -> None: ...

class ExecutionLimitsConfig(NativeRecord):
    """Optional target-scoped build execution limits."""

    max_models: int | None
    max_duration: str | None
    max_duration_seconds: int | None
    remediation: str | None
    test_timeout: str | None
    test_timeout_seconds: int | None
    scenario_timeout: str | None
    scenario_timeout_seconds: int | None
    def __init__(
        self,
        max_models: int | None = ...,
        max_duration: str | None = ...,
        max_duration_seconds: int | None = ...,
        remediation: str | None = ...,
        test_timeout: str | None = ...,
        test_timeout_seconds: int | None = ...,
        scenario_timeout: str | None = ...,
        scenario_timeout_seconds: int | None = ...,
    ) -> None: ...

class TargetDiffConfig(NativeRecord):
    """Optional target-scoped data-diff limits."""

    max_full_rows: int | Literal["unlimited"] | None
    def __init__(
        self,
        max_full_rows: int | Literal["unlimited"] | None = ...,
    ) -> None: ...

class TargetWarehousesConfig(NativeRecord):
    """Optional per-command-group default warehouses for one target."""

    build: str | None
    query: str | None
    def __init__(
        self,
        build: str | None = ...,
        query: str | None = ...,
    ) -> None: ...

class TargetConfig(NativeRecord):
    """One named target configuration."""

    connection: dict[str, object]
    connection_name: str | None
    vars: dict[str, str]
    database: str | None
    schema: str | None
    loader_schema: str | None
    defer_sources_to: str | None
    defer_clone_from: str | None
    clone: ClonePolicy
    compile_cache: bool | None
    time_travel_retention: AuthoredTimeTravelRetention | None
    time_travel_retention_by_materialization: dict[str, AuthoredTimeTravelRetention]
    owns_time_travel_retention_namespace: bool
    default_table_type: TableType | None
    table_type_downgrade: TableTypeDowngradePolicy
    time_travel_retention_decrease: RetentionDecreasePolicy
    missing_migration_origin: MissingMigrationOriginPolicy
    execution_limits: ExecutionLimitsConfig
    diff: TargetDiffConfig
    warehouses: TargetWarehousesConfig
    concurrency: int | None
    def __init__(
        self,
        connection: dict[str, object] = ...,
        connection_name: str | None = ...,
        vars: dict[str, str] = ...,
        database: str | None = ...,
        schema: str | None = ...,
        loader_schema: str | None = ...,
        defer_sources_to: str | None = ...,
        defer_clone_from: str | None = ...,
        clone: ClonePolicy = ...,
        compile_cache: bool | None = ...,
        time_travel_retention: AuthoredTimeTravelRetention | None = ...,
        time_travel_retention_by_materialization: dict[str, AuthoredTimeTravelRetention] = ...,
        owns_time_travel_retention_namespace: bool = ...,
        default_table_type: TableType | None = ...,
        table_type_downgrade: TableTypeDowngradePolicy = ...,
        time_travel_retention_decrease: RetentionDecreasePolicy = ...,
        missing_migration_origin: MissingMigrationOriginPolicy = ...,
        execution_limits: ExecutionLimitsConfig = ...,
        diff: TargetDiffConfig = ...,
        warehouses: TargetWarehousesConfig = ...,
        concurrency: int | None = ...,
    ) -> None: ...

class LocalTargetConfig(NativeRecord):
    """Local developer overrides for one named target."""

    connection: dict[str, object]
    connection_name: str | None
    vars: dict[str, str]
    database: str | None
    schema: str | None
    loader_schema: str | None
    defer_sources_to: str | None
    defer_clone_from: str | None
    clone: LocalClonePolicy
    compile_cache: bool | None
    time_travel_retention: AuthoredTimeTravelRetention | None
    time_travel_retention_by_materialization: dict[str, AuthoredTimeTravelRetention] | None
    owns_time_travel_retention_namespace: bool | None
    default_table_type: TableType | None
    table_type_downgrade: TableTypeDowngradePolicy | None
    time_travel_retention_decrease: RetentionDecreasePolicy | None
    missing_migration_origin: MissingMigrationOriginPolicy | None
    execution_limits: ExecutionLimitsConfig
    diff: TargetDiffConfig
    warehouses: TargetWarehousesConfig
    concurrency: int | None
    def __init__(
        self,
        connection: dict[str, object] = ...,
        connection_name: str | None = ...,
        vars: dict[str, str] = ...,
        database: str | None = ...,
        schema: str | None = ...,
        loader_schema: str | None = ...,
        defer_sources_to: str | None = ...,
        defer_clone_from: str | None = ...,
        clone: LocalClonePolicy = ...,
        compile_cache: bool | None = ...,
        time_travel_retention: AuthoredTimeTravelRetention | None = ...,
        time_travel_retention_by_materialization: dict[str, AuthoredTimeTravelRetention]
        | None = ...,
        owns_time_travel_retention_namespace: bool | None = ...,
        default_table_type: TableType | None = ...,
        table_type_downgrade: TableTypeDowngradePolicy | None = ...,
        time_travel_retention_decrease: RetentionDecreasePolicy | None = ...,
        missing_migration_origin: MissingMigrationOriginPolicy | None = ...,
        execution_limits: ExecutionLimitsConfig = ...,
        diff: TargetDiffConfig = ...,
        warehouses: TargetWarehousesConfig = ...,
        concurrency: int | None = ...,
    ) -> None: ...

class SettingsConfig(NativeRecord):
    """Global feature toggles."""

    sql_analysis: bool
    require_sql_analysis: bool
    query_change_tracking: bool
    column_contract_mode: ColumnContractMode
    concurrency: int
    auto_load_sources: bool
    microbatch_concurrency: bool
    microbatch_unaccounted_partition_policy: str
    table_promotion_mode: str | None
    default_audit_severity: AuditSeverity | None
    default_audit_run_scope: str | None
    unit_test_max_real_models: int
    test_timeout: str
    test_timeout_seconds: int | None
    scenario_timeout: str | None
    scenario_timeout_seconds: int | None
    delta_compile: bool
    def __init__(
        self,
        sql_analysis: bool = ...,
        require_sql_analysis: bool = ...,
        query_change_tracking: bool = ...,
        column_contract_mode: ColumnContractMode = ...,
        concurrency: int = ...,
        auto_load_sources: bool = ...,
        microbatch_concurrency: bool = ...,
        microbatch_unaccounted_partition_policy: str = ...,
        table_promotion_mode: str | None = ...,
        default_audit_severity: AuditSeverity | None = ...,
        default_audit_run_scope: str | None = ...,
        unit_test_max_real_models: int = ...,
        test_timeout: str = ...,
        test_timeout_seconds: int | None = ...,
        scenario_timeout: str | None = ...,
        scenario_timeout_seconds: int | None = ...,
        delta_compile: bool = ...,
    ) -> None: ...
    @property
    def sql_validation(self) -> bool: ...

class ScopesConfig(NativeRecord):
    """Declaration scope policy configuration."""

    enforce_placement: bool
    def __init__(
        self,
        enforce_placement: bool = ...,
    ) -> None: ...

class ReferencesConfig(NativeRecord):
    """Explicit-reference enforcement policy configuration."""

    enforce_explicit: bool
    def __init__(
        self,
        enforce_explicit: bool = ...,
    ) -> None: ...

class CostConfig(NativeRecord):
    """Snowflake compute estimate configuration."""

    usd_per_credit: Decimal
    usd_per_credit_is_default: bool
    def __init__(
        self,
        usd_per_credit: Decimal = ...,
        usd_per_credit_is_default: bool = ...,
    ) -> None: ...

class ConstantsConfig(NativeRecord):
    """Project-wide constant rendering configuration."""

    collection_rendering: CollectionRendering
    def __init__(
        self,
        collection_rendering: CollectionRendering = ...,
    ) -> None: ...

class FutureCursorsConfig(NativeRecord):
    """Safety policy for automatically discovered future cursor watermarks."""

    max_distance: str | None
    action: FutureCursorAction
    def __init__(
        self,
        max_distance: str | None = ...,
        action: FutureCursorAction = ...,
    ) -> None: ...

class StartCursorsConfig(NativeRecord):
    """Safety policy for automatically discovered incremental starts."""

    max_ahead: str | None
    action: FutureCursorAction
    def __init__(
        self,
        max_ahead: str | None = ...,
        action: FutureCursorAction = ...,
    ) -> None: ...

class CursorsConfig(NativeRecord):
    """Project-wide cursor safety configuration."""

    future: FutureCursorsConfig
    start: StartCursorsConfig
    def __init__(
        self,
        future: FutureCursorsConfig = ...,
        start: StartCursorsConfig = ...,
    ) -> None: ...

class MicrobatchLimitsConfig(NativeRecord):
    """Per-model microbatch planning safety limit."""

    max_batches: int | None
    action: MicrobatchLimitAction
    def __init__(
        self,
        max_batches: int | None = ...,
        action: MicrobatchLimitAction = ...,
    ) -> None: ...

class MicrobatchesConfig(NativeRecord):
    """Project-wide microbatch configuration."""

    limits: MicrobatchLimitsConfig
    def __init__(
        self,
        limits: MicrobatchLimitsConfig = ...,
    ) -> None: ...

class DefaultsConfig(NativeRecord):
    """Project-wide resource defaults."""

    materialized: str | None
    database: str | None
    schema: str | None
    seed_database: str | None
    seed_schema: str | None
    function_database: str | None
    function_schema: str | None
    incremental_strategy: str | None
    incremental_mode: str | None
    microbatch_strategy: str | None
    cursor_watermark_mode: str | None
    merge_exclude_columns: tuple[str, ...]
    full_refresh: bool | None
    append_cursor_inclusive: object | None
    cursor_start: object | None
    cursor_end: object | None
    cursor_start_max_ahead: str | None
    cursor_start_max_action: str | None
    cursor_future_max_distance: str | None
    cursor_future_action: str | None
    lookback: str | None
    batch_size: str | int | None
    batch_concurrency: int | None
    max_microbatches: int | None
    max_duration: str | None
    unaccounted_partition_policy: str | None
    replay_on_change: str | None
    row_diff_exclude_columns: tuple[str, ...]
    row_diff_tolerances: dict[str, object]
    row_diff_sample_rows: int | None
    row_diff_sample_seed: int | None
    tags: tuple[str, ...]
    contract: str | None
    pre_hooks: object | None
    post_hooks: object | None
    def __init__(
        self,
        materialized: str | None = ...,
        database: str | None = ...,
        schema: str | None = ...,
        seed_database: str | None = ...,
        seed_schema: str | None = ...,
        function_database: str | None = ...,
        function_schema: str | None = ...,
        incremental_strategy: str | None = ...,
        incremental_mode: str | None = ...,
        microbatch_strategy: str | None = ...,
        cursor_watermark_mode: str | None = ...,
        merge_exclude_columns: tuple[str, ...] = ...,
        full_refresh: bool | None = ...,
        append_cursor_inclusive: object | None = ...,
        cursor_start: object | None = ...,
        cursor_end: object | None = ...,
        cursor_start_max_ahead: str | None = ...,
        cursor_start_max_action: str | None = ...,
        cursor_future_max_distance: str | None = ...,
        cursor_future_action: str | None = ...,
        lookback: str | None = ...,
        batch_size: str | int | None = ...,
        batch_concurrency: int | None = ...,
        max_microbatches: int | None = ...,
        max_duration: str | None = ...,
        unaccounted_partition_policy: str | None = ...,
        replay_on_change: str | None = ...,
        row_diff_exclude_columns: tuple[str, ...] = ...,
        row_diff_tolerances: dict[str, object] = ...,
        row_diff_sample_rows: int | None = ...,
        row_diff_sample_seed: int | None = ...,
        tags: tuple[str, ...] = ...,
        contract: str | None = ...,
        pre_hooks: object | None = ...,
        post_hooks: object | None = ...,
    ) -> None: ...

class DiffConfig(NativeRecord):
    """Project-wide data-diff configuration."""

    query_artifact_ttl: str
    def __init__(
        self,
        query_artifact_ttl: str = ...,
    ) -> None: ...

class JanitorConfig(NativeRecord):
    """Janitor command defaults."""

    enabled: bool
    retention_days: int | None
    archive_retention_days: int
    direct_state_history_versions: int
    delete_tracked_only: bool
    exclude_patterns: tuple[str, ...]
    in_use_window: str
    keep_in_use: bool
    def __init__(
        self,
        enabled: bool = ...,
        retention_days: int | None = ...,
        archive_retention_days: int = ...,
        direct_state_history_versions: int = ...,
        delete_tracked_only: bool = ...,
        exclude_patterns: tuple[str, ...] = ...,
        in_use_window: str = ...,
        keep_in_use: bool = ...,
    ) -> None: ...

class MigrationsConfig(NativeRecord):
    """Model migration defaults."""

    old_name_views: str | None
    def __init__(
        self,
        old_name_views: str | None = ...,
    ) -> None: ...

class SnapshotsConfig(NativeRecord):
    """Snapshot materialization safety defaults."""

    current_state_full_refresh: str
    historical_full_refresh: str
    schema_change: str
    wildcard_check_schema_change: str
    def __init__(
        self,
        current_state_full_refresh: str = ...,
        historical_full_refresh: str = ...,
        schema_change: str = ...,
        wildcard_check_schema_change: str = ...,
    ) -> None: ...

class ScenarioSnapshotLimitsConfig(NativeRecord):
    """Scenario snapshot capture safety limits."""

    max_rows_per_relation: int | None
    max_total_rows: int | None
    max_bytes_per_relation: int | None
    max_total_bytes: int | None
    def __init__(
        self,
        max_rows_per_relation: int | None = ...,
        max_total_rows: int | None = ...,
        max_bytes_per_relation: int | None = ...,
        max_total_bytes: int | None = ...,
    ) -> None: ...

class ScenarioConfig(NativeRecord):
    """Scenario command configuration."""

    run_namespace: str | None
    local_type_overrides: dict[str, dict[str, str]]
    snapshot_limits: ScenarioSnapshotLimitsConfig
    def __init__(
        self,
        run_namespace: str | None = ...,
        local_type_overrides: dict[str, dict[str, str]] = ...,
        snapshot_limits: ScenarioSnapshotLimitsConfig = ...,
    ) -> None: ...

class DbtConfig(NativeRecord):
    """dbt interop configuration."""

    project_dir: str | None
    profiles_dir: str | None
    target: str | None
    target_path: str | None
    vars: dict[str, object]
    def __init__(
        self,
        project_dir: str | None = ...,
        profiles_dir: str | None = ...,
        target: str | None = ...,
        target_path: str | None = ...,
        vars: dict[str, object] = ...,
    ) -> None: ...

class LocalDbtConfig(NativeRecord):
    """Local dbt interop configuration overrides."""

    target: str | None
    vars: dict[str, object]
    def __init__(
        self,
        target: str | None = ...,
        vars: dict[str, object] = ...,
    ) -> None: ...

class LifecycleEventSinkFilterConfig(NativeRecord):
    """Destination-neutral runtime narrowing for lifecycle-event sinks."""

    event_kinds: frozenset[str] | None
    min_severity: EventExportSeverity | None
    def __init__(
        self,
        event_kinds: frozenset[str] | None = ...,
        min_severity: EventExportSeverity | None = ...,
    ) -> None: ...

class LifecycleEventSinksConfig(NativeRecord):
    """Global and named lifecycle-event sink runtime filters and delivery settings."""

    defaults: LifecycleEventSinkFilterConfig
    named: dict[str, LifecycleEventSinkFilterConfig]
    shutdown_timeout: str | None
    shutdown_timeout_seconds: int | None
    def __init__(
        self,
        defaults: LifecycleEventSinkFilterConfig = ...,
        named: dict[str, LifecycleEventSinkFilterConfig] = ...,
        shutdown_timeout: str | None = ...,
        shutdown_timeout_seconds: int | None = ...,
    ) -> None: ...

class SinksConfig(NativeRecord):
    """Typed destination sink configuration."""

    lifecycle: LifecycleEventSinksConfig
    def __init__(
        self,
        lifecycle: LifecycleEventSinksConfig = ...,
    ) -> None: ...

class ProjectConfig(NativeRecord):
    """Shared project configuration loaded from sqlbuild_project.toml."""

    name: str
    adapter: str
    default_target: str | None
    connection: dict[str, object]
    connections: dict[str, dict[str, object]]
    settings: SettingsConfig
    scopes: ScopesConfig
    references: ReferencesConfig
    cost: CostConfig
    constants: ConstantsConfig
    cursors: CursorsConfig
    microbatches: MicrobatchesConfig
    defaults: DefaultsConfig
    diff: DiffConfig
    materialization_defaults: MaterializationDefaultsConfig
    path_defaults: dict[str, dict[str, object]]
    vars: dict[str, str]
    targets: dict[str, TargetConfig]
    janitor: JanitorConfig
    migrations: MigrationsConfig
    snapshots: SnapshotsConfig
    scenario: ScenarioConfig
    dbt: DbtConfig
    sinks: SinksConfig
    def __init__(
        self,
        name: str,
        adapter: str,
        default_target: str | None = ...,
        connection: dict[str, object] = ...,
        connections: dict[str, dict[str, object]] = ...,
        settings: SettingsConfig = ...,
        scopes: ScopesConfig = ...,
        references: ReferencesConfig = ...,
        cost: CostConfig = ...,
        constants: ConstantsConfig = ...,
        cursors: CursorsConfig = ...,
        microbatches: MicrobatchesConfig = ...,
        defaults: DefaultsConfig = ...,
        diff: DiffConfig = ...,
        materialization_defaults: MaterializationDefaultsConfig = ...,
        path_defaults: dict[str, dict[str, object]] = ...,
        vars: dict[str, str] = ...,
        targets: dict[str, TargetConfig] = ...,
        janitor: JanitorConfig = ...,
        migrations: MigrationsConfig = ...,
        snapshots: SnapshotsConfig = ...,
        scenario: ScenarioConfig = ...,
        dbt: DbtConfig = ...,
        sinks: SinksConfig = ...,
    ) -> None: ...

class LocalConfig(NativeRecord):
    """Local developer overrides from sqlbuild_local.toml."""

    target: str | None
    adapter: str | None
    connection: dict[str, object]
    connections: dict[str, dict[str, object]]
    targets: dict[str, LocalTargetConfig]
    settings: SettingsConfig
    setting_overrides: frozenset[str]
    vars: dict[str, str]
    dbt: LocalDbtConfig
    scenario: ScenarioConfig
    def __init__(
        self,
        target: str | None = ...,
        adapter: str | None = ...,
        connection: dict[str, object] = ...,
        connections: dict[str, dict[str, object]] = ...,
        targets: dict[str, LocalTargetConfig] = ...,
        settings: SettingsConfig = ...,
        setting_overrides: frozenset[str] = ...,
        vars: dict[str, str] = ...,
        dbt: LocalDbtConfig = ...,
        scenario: ScenarioConfig = ...,
    ) -> None: ...

class SourceLocation(NativeRecord):
    """An authored source location for compiler diagnostics."""

    path: Path
    line: int
    column: int
    end_line: int | None
    end_column: int | None
    def __init__(
        self,
        path: Path,
        line: int,
        column: int,
        end_line: int | None = ...,
        end_column: int | None = ...,
    ) -> None: ...

class SeedCsvSettings(NativeRecord):
    """CSV reader settings for a seed file."""

    delimiter: str | None
    quotechar: str | None
    doublequote: bool | None
    escapechar: str | None
    skipinitialspace: bool | None
    lineterminator: str | None
    encoding: str | None
    na_values: tuple[object, ...] | dict[str, tuple[object, ...]] | None
    keep_default_na: bool | None
    def __init__(
        self,
        delimiter: str | None = ...,
        quotechar: str | None = ...,
        doublequote: bool | None = ...,
        escapechar: str | None = ...,
        skipinitialspace: bool | None = ...,
        lineterminator: str | None = ...,
        encoding: str | None = ...,
        na_values: tuple[object, ...] | dict[str, tuple[object, ...]] | None = ...,
        keep_default_na: bool | None = ...,
    ) -> None: ...

class SchemaAuditInstance(NativeRecord):
    """One audit instance attached to model, column, or seed metadata."""

    definition_name: str
    arguments: dict[str, object]
    name: str | None
    description: str | None
    severity: AuditSeverity | None
    run_scope: str | None
    always_run: bool
    thresholds: MeasurementThresholds | None
    minimum_samples: int | None
    evidence_limit: int | None
    meta: dict[str, object] | None
    location: SourceLocation | None
    def __init__(
        self,
        definition_name: str,
        arguments: dict[str, object] = ...,
        name: str | None = ...,
        description: str | None = ...,
        severity: AuditSeverity | None = ...,
        run_scope: str | None = ...,
        always_run: bool = ...,
        thresholds: MeasurementThresholds | None = ...,
        minimum_samples: int | None = ...,
        evidence_limit: int | None = ...,
        meta: dict[str, object] | None = ...,
        location: SourceLocation | None = ...,
    ) -> None: ...

class SchemaColumn(NativeRecord):
    """One declared model, seed, or source column entry."""

    name: str
    type: str | None
    nullable: bool | None
    description: str | None
    meta: dict[str, object]
    audits: tuple[SchemaAuditInstance, ...]
    location: SourceLocation | None
    migrate_from: str | None
    def __init__(
        self,
        name: str,
        type: str | None = ...,
        nullable: bool | None = ...,
        description: str | None = ...,
        meta: dict[str, object] = ...,
        audits: tuple[SchemaAuditInstance, ...] = ...,
        location: SourceLocation | None = ...,
        migrate_from: str | None = ...,
    ) -> None: ...

class SchemaDynamicColumnFamily(NativeRecord):
    """One declared family of columns generated by a runtime-dynamic pivot."""

    name: str
    pivot_column: str
    value_column: str
    aggregate: str
    type: str
    name_pattern: str | None
    def __init__(
        self,
        name: str,
        pivot_column: str,
        value_column: str,
        aggregate: str,
        type: str,
        name_pattern: str | None = ...,
    ) -> None: ...

class SchemaModelEntry(NativeRecord):
    """One model metadata entry normalized from MODEL(...)."""

    name: str
    model_schema: str | None
    description: str | None
    type_enforcement: bool | None
    meta: dict[str, object]
    columns: tuple[SchemaColumn, ...]
    dynamic_columns: tuple[SchemaDynamicColumnFamily, ...]
    audits: tuple[SchemaAuditInstance, ...]
    tags: tuple[str, ...]
    def __init__(
        self,
        name: str,
        model_schema: str | None = ...,
        description: str | None = ...,
        type_enforcement: bool | None = ...,
        meta: dict[str, object] = ...,
        columns: tuple[SchemaColumn, ...] = ...,
        dynamic_columns: tuple[SchemaDynamicColumnFamily, ...] = ...,
        audits: tuple[SchemaAuditInstance, ...] = ...,
        tags: tuple[str, ...] = ...,
    ) -> None: ...

class SchemaSeedEntry(NativeRecord):
    """One seed metadata entry from seed YAML."""

    name: str
    description: str | None
    database: str | None
    schema: str | None
    meta: dict[str, object]
    csv_settings: SeedCsvSettings
    columns: tuple[SchemaColumn, ...]
    tags: tuple[str, ...]
    audits: tuple[SchemaAuditInstance, ...]
    def __init__(
        self,
        name: str,
        description: str | None = ...,
        database: str | None = ...,
        schema: str | None = ...,
        meta: dict[str, object] = ...,
        csv_settings: SeedCsvSettings = ...,
        columns: tuple[SchemaColumn, ...] = ...,
        tags: tuple[str, ...] = ...,
        audits: tuple[SchemaAuditInstance, ...] = ...,
    ) -> None: ...

class IntegrationLoaderConfig(NativeRecord):
    """Typed configuration for one declarative integration loader."""

    kind: str
    config: object
    def __init__(
        self,
        kind: str,
        config: object,
    ) -> None: ...

class SourceFreshnessAgePolicy(NativeRecord):
    """Age-based source freshness warning/error thresholds."""

    warn_after: str | None
    error_after: str | None
    def __init__(
        self,
        warn_after: str | None = ...,
        error_after: str | None = ...,
    ) -> None: ...

class SourceFreshnessConfig(NativeRecord):
    """Configured source freshness observation for planning."""

    strategy: SourceFreshnessStrategy
    value_kind: SourceFreshnessValueKind | None
    column: str | None
    query: str | None
    filter: str | None
    lag_tolerance: str | None
    age_policy: SourceFreshnessAgePolicy | None
    def __init__(
        self,
        strategy: SourceFreshnessStrategy,
        value_kind: SourceFreshnessValueKind | None = ...,
        column: str | None = ...,
        query: str | None = ...,
        filter: str | None = ...,
        lag_tolerance: str | None = ...,
        age_policy: SourceFreshnessAgePolicy | None = ...,
    ) -> None: ...

class SourceColumnEntry(NativeRecord):
    """One source column entry from sources/*.yml."""

    name: str
    type: str | None
    nullable: bool | None
    description: str | None
    meta: dict[str, object]
    audits: tuple[SchemaAuditInstance, ...]
    def __init__(
        self,
        name: str,
        type: str | None = ...,
        nullable: bool | None = ...,
        description: str | None = ...,
        meta: dict[str, object] = ...,
        audits: tuple[SchemaAuditInstance, ...] = ...,
    ) -> None: ...

class SourceEntry(NativeRecord):
    """One source declaration from sources/*.yml."""

    name: str
    database: str | None
    schema: str | None
    table: str | None
    final: bool
    loader: str | None
    managed: bool
    integration_loader: IntegrationLoaderConfig | None
    freshness: SourceFreshnessConfig | None
    write_strategy: SourceWriteStrategy | None
    load_batch_size: int | None
    cursor_column: str | None
    unique_key: tuple[str, ...]
    expression: str | None
    description: str | None
    type_enforcement: bool | None
    contract: str | None
    meta: dict[str, object]
    columns: tuple[SourceColumnEntry, ...]
    audits: tuple[SchemaAuditInstance, ...]
    def __init__(
        self,
        name: str,
        database: str | None = ...,
        schema: str | None = ...,
        table: str | None = ...,
        final: bool = ...,
        loader: str | None = ...,
        managed: bool = ...,
        integration_loader: IntegrationLoaderConfig | None = ...,
        freshness: SourceFreshnessConfig | None = ...,
        write_strategy: SourceWriteStrategy | None = ...,
        load_batch_size: int | None = ...,
        cursor_column: str | None = ...,
        unique_key: tuple[str, ...] = ...,
        expression: str | None = ...,
        description: str | None = ...,
        type_enforcement: bool | None = ...,
        contract: str | None = ...,
        meta: dict[str, object] = ...,
        columns: tuple[SourceColumnEntry, ...] = ...,
        audits: tuple[SchemaAuditInstance, ...] = ...,
    ) -> None: ...

class LoaderDestinationParts(NativeRecord):
    """Parsed warehouse relation parts for a loader destination."""

    database: str | None
    schema: str | None
    table: str
    def __init__(
        self,
        database: str | None,
        schema: str | None,
        table: str,
    ) -> None: ...

class MeasurementThresholdBound(NativeRecord):
    """One directional threshold bound for a measurement audit."""

    operator: ThresholdOperator
    limit: float | None
    lower: float | None
    upper: float | None
    def __init__(
        self,
        operator: ThresholdOperator,
        limit: float | None = ...,
        lower: float | None = ...,
        upper: float | None = ...,
    ) -> None: ...

class MeasurementThresholds(NativeRecord):
    """Warning and error policy for a measurement audit."""

    warn: MeasurementThresholdBound | None
    error: MeasurementThresholdBound | None
    def __init__(
        self,
        warn: MeasurementThresholdBound | None = ...,
        error: MeasurementThresholdBound | None = ...,
    ) -> None: ...

class MeasurementContract(NativeRecord):
    """Columns and sample unit returned by a measurement query."""

    value_column: str
    sample_count_column: str | None
    sample_unit: str | None
    def __init__(
        self,
        value_column: str,
        sample_count_column: str | None = ...,
        sample_unit: str | None = ...,
    ) -> None: ...

class AuditIdentity(NativeRecord):
    """Stable identity values for one planned audit binding."""

    binding_key: str
    audit_name: str
    definition_fingerprint: str
    execution_fingerprint: str
    severity: AuditSeverity
    run_scope_phase: str
    attachment_kind: str
    attached_target_name: str | None
    attached_column_name: str | None
    always_run: bool
    def __init__(
        self,
        binding_key: str,
        audit_name: str,
        definition_fingerprint: str,
        execution_fingerprint: str,
        severity: AuditSeverity,
        run_scope_phase: str,
        attachment_kind: str,
        attached_target_name: str | None = ...,
        attached_column_name: str | None = ...,
        always_run: bool = ...,
    ) -> None: ...

class AuditGateIdentity(NativeRecord):
    """Aggregate identity for a model's runtime audit gate."""

    binding_set_hash: str
    blocking_set_hash: str
    audits: tuple[AuditIdentity, ...]
    def __init__(
        self,
        binding_set_hash: str,
        blocking_set_hash: str,
        audits: tuple[AuditIdentity, ...],
    ) -> None: ...

class SqlValueKind(StrEnum):
    """Closed logical kinds accepted by the typed SQL value boundary."""

    STRING = "string"
    INTEGER = "integer"
    BOOLEAN = "boolean"
    FLOAT = "float"
    DECIMAL = "decimal"
    NULL = "null"
    LIST = "list"
    SET = "set"
    OBJECT = "object"

class StateSqlValueType(StrEnum):
    """Closed physical value kinds used by warehouse state columns."""

    STRING = "string"
    INTEGER = "integer"
    BOOLEAN = "boolean"
    TIMESTAMP = "timestamp"
    TEXT_TIMESTAMP = "text_timestamp"
    DATE = "date"
    JSON = "json"

class SqlLogicalType(NativeRecord):
    """Logical type inferred for a normalized SQL value."""

    kind: SqlValueKind
    element_type: SqlLogicalType | None
    def __init__(
        self,
        kind: SqlValueKind,
        element_type: SqlLogicalType | None = ...,
    ) -> None: ...
    @property
    def display_name(self) -> str: ...

class SqlValue(NativeRecord):
    """One validated, canonical SQL value."""

    logical_type: SqlLogicalType
    value: SqlValuePayload
    def __init__(
        self,
        logical_type: SqlLogicalType,
        value: SqlValuePayload,
    ) -> None: ...
    @property
    def kind(self) -> SqlValueKind: ...

class Fingerprint(NativeRecord):
    """One applied fingerprint record from a successful materialization."""

    node_type: str
    node_name: str
    target_database: str | None
    target_schema: str | None
    target_name: str | None
    run_id: str
    definition_hash: str
    schema_fingerprint: str
    definition: str
    ts: datetime
    metadata_json: str
    version_hash: str
    def __init__(
        self,
        node_type: str,
        node_name: str,
        target_database: str | None,
        target_schema: str | None,
        target_name: str | None,
        run_id: str,
        definition_hash: str,
        schema_fingerprint: str,
        definition: str,
        ts: datetime,
        metadata_json: str = ...,
        version_hash: str = ...,
    ) -> None: ...

class FingerprintSet(NativeRecord):
    """Latest fingerprints per node name for one target schema."""

    schema: str
    fingerprints: dict[str, Fingerprint]
    fingerprints_by_identity: dict[tuple[str, str], Fingerprint] | None
    def __init__(
        self,
        schema: str,
        fingerprints: dict[str, Fingerprint],
        fingerprints_by_identity: dict[tuple[str, str], Fingerprint] | None = ...,
    ) -> None: ...

class SourceFreshnessRecord(NativeRecord):
    """One append-only direct observation whose observed time is always aware UTC."""

    source_name: str
    target_database: str | None
    target_schema: str | None
    target_name: str | None
    run_id: str
    strategy: str
    value_kind: str
    data_version: str | None
    data_version_hash: str
    observed_at: datetime
    def __init__(
        self,
        source_name: str,
        target_database: str | None,
        target_schema: str | None,
        target_name: str | None,
        run_id: str,
        strategy: str,
        value_kind: str,
        data_version: str | None,
        data_version_hash: str,
        observed_at: datetime,
    ) -> None: ...
    @property
    def identity(self) -> SourceFreshnessIdentity: ...

class SelectedDbtProfileOutput(NativeRecord):
    """Selected dbt profile target output before rendering."""

    profile_name: str
    target_name: str
    output: dict[str, object]
    def __init__(
        self,
        profile_name: str,
        target_name: str,
        output: dict[str, object],
    ) -> None: ...

class ResolvedDbtProfileOutput(NativeRecord):
    """Rendered dbt profile target output."""

    project_dir: Path
    profiles_dir: Path
    profile_name: str
    target_name: str
    output: dict[str, object]
    def __init__(
        self,
        project_dir: Path,
        profiles_dir: Path,
        profile_name: str,
        target_name: str,
        output: dict[str, object],
    ) -> None: ...

class DiscoveredPythonNode(NativeRecord):
    """Shared internal view of a discovered Python DAG node."""

    kind: PythonNodeKind
    file_path: Path
    relative_path: Path
    name: str
    function: Callable[..., object]
    depends_on: tuple[Callable[..., object] | SqlResourceRef, ...]
    sql_deps: tuple[SqlResourceRef, ...]
    tags: tuple[str, ...]
    group: str | None
    description: str | None
    meta: dict[str, object] | None
    provider_usages: tuple[DiscoveredProviderUsage, ...]
    loader: DiscoveredPythonLoaderMetadata | None
    task: DiscoveredPythonTaskMetadata | None
    asset: DiscoveredPythonAssetMetadata | None
    check: DiscoveredPythonCheckMetadata | None
    identity: PythonNodeIdentity | None
    def __init__(
        self,
        kind: PythonNodeKind,
        file_path: Path,
        relative_path: Path,
        name: str,
        function: Callable[..., object],
        depends_on: tuple[Callable[..., object] | SqlResourceRef, ...] = ...,
        sql_deps: tuple[SqlResourceRef, ...] = ...,
        tags: tuple[str, ...] = ...,
        group: str | None = ...,
        description: str | None = ...,
        meta: dict[str, object] | None = ...,
        provider_usages: tuple[DiscoveredProviderUsage, ...] = ...,
        loader: DiscoveredPythonLoaderMetadata | None = ...,
        task: DiscoveredPythonTaskMetadata | None = ...,
        asset: DiscoveredPythonAssetMetadata | None = ...,
        check: DiscoveredPythonCheckMetadata | None = ...,
        identity: PythonNodeIdentity | None = ...,
    ) -> None: ...

class PythonNodeGraph(NativeRecord):
    """Internal graph inventory for discovered executable Python nodes."""

    nodes: tuple[DiscoveredPythonNode, ...]
    dependency_edges: tuple[PythonNodeDependencyEdge, ...]
    upstream_deps: dict[str, tuple[str, ...]]
    downstream_deps: dict[str, tuple[str, ...]]
    tag_index: dict[str, frozenset[str]]
    path_index: dict[str, str]
    nodes_by_name: dict[str, DiscoveredPythonNode]
    nodes_by_typed_selector: dict[str, DiscoveredPythonNode]
    def __init__(
        self,
        nodes: tuple[DiscoveredPythonNode, ...],
        dependency_edges: tuple[PythonNodeDependencyEdge, ...],
        upstream_deps: dict[str, tuple[str, ...]],
        downstream_deps: dict[str, tuple[str, ...]],
        tag_index: dict[str, frozenset[str]],
        path_index: dict[str, str],
        nodes_by_name: dict[str, DiscoveredPythonNode],
        nodes_by_typed_selector: dict[str, DiscoveredPythonNode],
    ) -> None: ...
    def selected_sql_refs(self, *, selected_names: frozenset[str]) -> frozenset[SqlResourceRef]: ...

class PythonSqlRunLifecyclePlan(NativeRecord):
    """Lifecycle-aware run classification for SQL resources and Python nodes."""

    ingress_python_node_names: frozenset[str]
    ingress_loader_names: frozenset[str]
    read_side_python_node_names: frozenset[str]
    def __init__(
        self,
        ingress_python_node_names: frozenset[str],
        ingress_loader_names: frozenset[str],
        read_side_python_node_names: frozenset[str],
    ) -> None: ...

class PythonIdentityDependency(NativeRecord):
    """One first-party Python source object included in a Python node identity."""

    kind: str
    module: str
    qualname: str
    source_path: str
    source_hash: str
    source_text: str
    def __init__(
        self,
        kind: str,
        module: str,
        qualname: str,
        source_path: str,
        source_hash: str,
        source_text: str,
    ) -> None: ...

class PythonNodeIdentity(NativeRecord):
    """Read-only identity metadata for a SQLBuild Python node."""

    node_type: str
    node_name: str
    source_path: str
    source_hash: str
    definition_hash: str
    version_hash: str
    dependencies: tuple[PythonIdentityDependency, ...]
    definition_json: str
    metadata_json: str
    def __init__(
        self,
        node_type: str,
        node_name: str,
        source_path: str,
        source_hash: str,
        definition_hash: str,
        version_hash: str,
        dependencies: tuple[PythonIdentityDependency, ...] = ...,
        definition_json: str = ...,
        metadata_json: str = ...,
    ) -> None: ...

class SqlLexicalSyntax(NativeRecord):
    """Dialect rules for where SQL quoted text and comments end."""

    backslash_escape_quotes: frozenset[str]
    escape_string_prefix: bool
    raw_string_prefix: bool
    triple_quoted_strings: bool
    nested_block_comments: bool
    line_comment_prefixes: frozenset[str]
    def __init__(
        self,
        backslash_escape_quotes: frozenset[str] = ...,
        escape_string_prefix: bool = ...,
        raw_string_prefix: bool = ...,
        triple_quoted_strings: bool = ...,
        nested_block_comments: bool = ...,
        line_comment_prefixes: frozenset[str] = ...,
    ) -> None: ...
    @property
    def cache_key(self) -> str: ...
    @property
    def native_mapping(self) -> dict[str, object]: ...
    def reads_differently_from_generic(self, sql: str) -> bool: ...

class TimestampValue(NativeRecord):
    """Timestamp cursor value; aware datetimes are normalized to UTC at ingestion."""

    value: datetime
    source_text: str | None
    def __init__(
        self,
        value: datetime,
        source_text: str | None = ...,
    ) -> None: ...

class DateValue(NativeRecord):
    """Date cursor value which must remain date-formatted when rendered."""

    value: date
    def __init__(
        self,
        value: date,
    ) -> None: ...

class IntegerValue(NativeRecord):
    """Integral cursor value."""

    value: int
    def __init__(
        self,
        value: int,
    ) -> None: ...

class AlignedInterval(NativeRecord):
    """Validated half-open cursor interval."""

    start: TimestampValue | DateValue | IntegerValue
    end: TimestampValue | DateValue | IntegerValue
    grain: CursorGrain | None
    def __init__(
        self,
        start: TimestampValue | DateValue | IntegerValue,
        end: TimestampValue | DateValue | IntegerValue,
        grain: CursorGrain | None,
    ) -> None: ...

class Duration(NativeRecord):
    """A calendar-aware duration (years/months plus fixed time units)."""

    years: int
    months: int
    days: int
    hours: int
    minutes: int
    seconds: int
    def __init__(
        self,
        years: int = ...,
        months: int = ...,
        days: int = ...,
        hours: int = ...,
        minutes: int = ...,
        seconds: int = ...,
    ) -> None: ...
    @classmethod
    def parse(cls, value: str) -> Duration | None: ...
    @property
    def is_zero(self) -> bool: ...
    @property
    def units(self) -> frozenset[str]: ...
    def is_single_unit_in(self, allowed_units: frozenset[str]) -> bool: ...
    @property
    def has_calendar_component(self) -> bool: ...
    @property
    def fixed_seconds(self) -> int: ...
    @property
    def total_months(self) -> int: ...
    def add_to(self, moment: datetime) -> datetime: ...
    def subtract_from(self, moment: datetime) -> datetime: ...

class MigrationDecision(StrEnum):
    """Per-run decision for one declared or discovered model migration."""

    DONE = "done"
    MIGRATE = "migrate"
    REDO = "redo"
    SUPERSEDED_REPLACE = "superseded_replace"
    FORCED_REPLACE = "forced_replace"
    CONFLICT = "conflict"
    ORIGIN_MISSING = "origin_missing"
    RENAMED = "renamed"
    @property
    def moves_data(self) -> bool: ...
    @property
    def blocks_build(self) -> bool: ...
    @property
    def checks_compatibility(self) -> bool: ...
    @property
    def label(self) -> str: ...

class MigrationPromotion(StrEnum):
    """How a completed migration stage replaces or creates the destination."""

    SWAP = "swap"
    RENAME = "rename"
    TRANSACTIONAL_RENAME = "transactional_rename"
    @property
    def label(self) -> str: ...

class ColumnMigrationDecision(StrEnum):
    """Per-run decision for one declared or detected column rename."""

    RENAME = "rename"
    RECORD = "record"
    DONE = "done"
    CONFLICT = "conflict"
    SOURCE_MISSING = "source_missing"
    STILL_PRODUCED = "still_produced"
    UNSUPPORTED = "unsupported"
    @property
    def renames(self) -> bool: ...
    @property
    def records_event(self) -> bool: ...
    @property
    def blocks_build(self) -> bool: ...
    @property
    def label(self) -> str: ...

class OldNameViewEventType(StrEnum):
    """Immutable facts about what happened at a migrated relation's old name."""

    REQUIRED = "required"
    ORIGIN_ARCHIVED = "origin_archived"
    VIEW_CREATED = "view_created"
    VIEW_DROPPED = "view_dropped"

class OldNameViewDropReason(StrEnum):
    """Why a compatibility view at an old name was dropped."""

    EXPIRED = "expired"
    EARLY = "early"
    MISSING = "missing"

class OldNameViewStatus(StrEnum):
    """Projected state of the old-name steps of one recorded move."""

    PENDING_ARCHIVE = "pending_archive"
    PENDING_VIEW = "pending_view"
    LIVE = "live"
    EXPIRED = "expired"
    DROPPED = "dropped"

class MigrationRelation(NativeRecord):
    """One physical relation named by a migration event."""

    database: str | None
    schema: str | None
    name: str
    def __init__(
        self,
        database: str | None,
        schema: str | None,
        name: str,
    ) -> None: ...
    @property
    def identity(self) -> tuple[str | None, str, str]: ...
    def matches(self, other: MigrationRelation) -> bool: ...

class MigrationEvent(NativeRecord):
    """One immutable fact that a origin relation was cloned over a destination."""

    event_id: str
    target_name: str | None
    origin_model: str | None
    origin: MigrationRelation
    destination_model: str
    destination: MigrationRelation
    origin_version_hash: str
    discovery: MigrationDiscovery
    decision: MigrationDecision
    run_id: str
    created_at: datetime
    def __init__(
        self,
        event_id: str,
        target_name: str | None,
        origin_model: str | None,
        origin: MigrationRelation,
        destination_model: str,
        destination: MigrationRelation,
        origin_version_hash: str,
        discovery: MigrationDiscovery,
        decision: MigrationDecision,
        run_id: str,
        created_at: datetime,
    ) -> None: ...
    def mentions(self, relation: MigrationRelation) -> bool: ...

class ColumnMigrationEvent(NativeRecord):
    """One immutable fact that a column of a model relation was renamed in place."""

    event_id: str
    target_name: str | None
    model_name: str
    relation: MigrationRelation
    origin_column: str
    destination_column: str
    discovery: MigrationDiscovery
    decision: ColumnMigrationDecision
    run_id: str
    created_at: datetime
    def __init__(
        self,
        event_id: str,
        target_name: str | None,
        model_name: str,
        relation: MigrationRelation,
        origin_column: str,
        destination_column: str,
        discovery: MigrationDiscovery,
        decision: ColumnMigrationDecision,
        run_id: str,
        created_at: datetime,
    ) -> None: ...
    def mentions(self, *, relation: MigrationRelation, columns: frozenset[str]) -> bool: ...
    def renamed(self, *, origin_column: str, destination_column: str) -> bool: ...

class OldNameViewEvent(NativeRecord):
    """One immutable fact about the old name of one recorded move."""

    event_id: str
    target_name: str | None
    event_type: OldNameViewEventType
    migration_event_id: str
    destination_model: str
    old: MigrationRelation
    new: MigrationRelation
    run_id: str
    created_at: datetime
    view_retention: str | None
    archive_name: str | None
    column_aliases: tuple[tuple[str, str], ...]
    expires_at: datetime | None
    drop_reason: OldNameViewDropReason | None
    grants_copied: tuple[str, ...] | None
    view_sql: str | None
    def __init__(
        self,
        event_id: str,
        target_name: str | None,
        event_type: OldNameViewEventType,
        migration_event_id: str,
        destination_model: str,
        old: MigrationRelation,
        new: MigrationRelation,
        run_id: str,
        created_at: datetime,
        view_retention: str | None = ...,
        archive_name: str | None = ...,
        column_aliases: tuple[tuple[str, str], ...] = ...,
        expires_at: datetime | None = ...,
        drop_reason: OldNameViewDropReason | None = ...,
        grants_copied: tuple[str, ...] | None = ...,
        view_sql: str | None = ...,
    ) -> None: ...

class OldNameViewHistory(NativeRecord):
    """The recorded move and every old-name fact that references it."""

    move: MigrationEvent
    required: OldNameViewEvent
    archived: OldNameViewEvent | None
    created: OldNameViewEvent | None
    dropped: OldNameViewEvent | None
    def __init__(
        self,
        move: MigrationEvent,
        required: OldNameViewEvent,
        archived: OldNameViewEvent | None = ...,
        created: OldNameViewEvent | None = ...,
        dropped: OldNameViewEvent | None = ...,
    ) -> None: ...
    def status(self, *, now: datetime) -> OldNameViewStatus: ...
    @property
    def old(self) -> MigrationRelation: ...
    @property
    def new(self) -> MigrationRelation: ...

class BackfillAction(StrEnum):
    FULL = "full"
    BOUNDED = "bounded"
    FORWARD_ONLY = "forward"

class OnSchemaChange(StrEnum):
    IGNORE = "ignore"
    FAIL = "fail"
    APPEND_NEW_COLUMNS = "append_new_columns"
    SYNC_ALL_COLUMNS = "sync_all_columns"

class ContractPolicy(StrEnum):
    NONE = "none"
    ENFORCED = "enforced"

class RetentionDirection(StrEnum):
    INCREASE = "increase"
    DECREASE = "decrease"
    MIXED = "mixed"
    MATCH = "match"
    APPLY_AFTER_CREATE = "apply_after_create"

class RetentionPlanPhase(StrEnum):
    PRE = "pre"
    POST = "post"
    AFTER_CREATE = "after_create"
    NONE = "none"

class PlanAction(StrEnum):
    CREATE_VIEW = "create_view"
    CREATE_TABLE = "create_table"
    INCREMENTAL_APPEND = "incremental_append"
    INCREMENTAL_DELETE_INSERT = "incremental_delete_insert"
    INCREMENTAL_MERGE = "incremental_merge"
    SNAPSHOT = "snapshot"
    LOAD_SEED = "load_seed"
    SKIP = "skip"
    CUSTOM = "custom"

class PlanReason(StrEnum):
    FIRST_RUN = "first_run"
    RENAMED = "renamed"
    FULL_REFRESH = "full_refresh"
    QUERY_CHANGED = "query_changed"
    FUNCTION_CHANGED = "function_changed"
    CONFIG_CHANGED = "config_changed"
    SCHEMA_CHANGED = "schema_changed"
    NORMAL_INCREMENTAL = "normal_incremental"
    NO_CHANGE = "no_change"
    DISABLED = "disabled"
    SOURCE_FRESHNESS_ERROR = "source_freshness_error"
    EXTERNAL_UPSTREAM_FAILED = "external_upstream_failed"

class IncrementalStrategy(StrEnum):
    APPEND = "append"
    DELETE_INSERT = "delete_insert"
    MERGE = "merge"

class MicrobatchStrategy(StrEnum):
    """How an ordinary microbatch range is selected."""

    ROLLING_WINDOW = "rolling_window"
    WATERMARK = "watermark"

class IncrementalMode(StrEnum):
    FULL = "full"
    MICROBATCH = "microbatch"

class SnapshotStrategy(StrEnum):
    TIMESTAMP = "timestamp"
    CHECK = "check"

class HistoricalInput(StrEnum):
    SNAPSHOT = "snapshot"
    CHANGES = "changes"

class InitialValidFrom(StrEnum):
    UPDATED_AT = "updated_at"
    OBSERVED_AT = "observed_at"
    EXECUTION_TIME = "execution_time"

class SnapshotSchemaChangePolicy(StrEnum):
    DENY = "deny"
    REQUIRE_CONFIRMATION = "require_confirmation"
    APPEND_NEW_COLUMNS = "append_new_columns"

class CursorType(StrEnum):
    TIMESTAMP = "timestamp"
    INTEGER = "integer"

class CursorGrain(StrEnum):
    SECOND = "second"
    MINUTE = "minute"
    HOUR = "hour"
    DAY = "day"
    MONTH = "month"
    YEAR = "year"

class MaterializationType(StrEnum):
    VIEW = "view"
    TABLE = "table"
    INCREMENTAL = "incremental"
    SNAPSHOT = "snapshot"
    SEED = "seed"
    CUSTOM = "custom"
    @classmethod
    def is_table_backed(cls, *, materialized: object | None) -> bool: ...

class ScenarioArtifactKind(StrEnum):
    SOURCE = "source"
    REF = "ref"
    SEED = "seed"
    DBT_REF = "dbt_ref"
    MODEL = "model"

class PlannerInputError(ValueError):
    """Raised when planner inputs cannot be resolved safely."""

    code: str
    message: str
    help: str | None
    def __init__(
        self, message: str, *, code: str | None = None, help: str | None = None
    ) -> None: ...

class FutureCursorSafetyError(PlannerInputError):
    """Raised when an effective cursor exceeds the configured future limit."""

class MaximumAutomaticStartError(PlannerInputError):
    """Raised when automatic cursor recovery would exceed its configured horizon."""

    evidence: Any

class CursorOverrides(NativeRecord):
    """Cursor override values from CLI flags; untyped `start`/`end` await planner typing."""

    start_ts: CursorScalar | None
    end_ts: CursorScalar | None
    start_int: CursorScalar | None
    end_int: CursorScalar | None
    start: str | None
    end: str | None
    def __init__(
        self,
        *,
        start_ts: str | CursorScalar | None = None,
        end_ts: str | CursorScalar | None = None,
        start_int: str | CursorScalar | None = None,
        end_int: str | CursorScalar | None = None,
        start: str | None = None,
        end: str | None = None,
    ) -> None: ...
    @property
    def has_untyped(self) -> bool: ...
    def typed_as(self, *, cursor_type: CursorType) -> CursorOverrides: ...

class EmptyCursorInputDecision(NativeRecord):
    """Cursor inputs whose lack of rows leaves no window, and whether other inputs have rows."""

    input_names: tuple[str, ...]
    waiting_on_empty_inputs: bool
    def __init__(
        self,
        input_names: tuple[str, ...] = ...,
        waiting_on_empty_inputs: bool = ...,
    ) -> None: ...

class ModelCursorSnapshot(NativeRecord):
    """Cursor MIN/MAX values gathered from warehouse for one incremental model."""

    target_max: CursorScalar | None
    upstream_mins: tuple[CursorScalar, ...]
    upstream_maxes: tuple[CursorScalar, ...]
    physical_target_max: CursorScalar | None
    target_eligible_max: CursorScalar | None
    target_relation: str | None
    destination_cursor_column: str | None
    input_evidence: tuple[CursorInputEvidence, ...]
    expected_watermark_count: int
    unavailable_watermark_tags: tuple[str, ...]
    cursor_watermark_mode: CursorWatermarkMode
    upstream_terminal_starts: tuple[CursorScalar, ...]
    upstream_terminal_ends: tuple[CursorScalar, ...]
    upstream_end_inputs: tuple[tuple[CursorScalar | None, CursorScalar | None], ...]
    upstream_availability_ends: tuple[CursorScalar, ...]
    empty_input_names: tuple[str, ...]
    unreadable_input_names: tuple[str, ...]
    def __init__(self, **values: Any) -> None: ...
    @property
    def watermarks_available(self) -> bool: ...

class CursorBounds(NativeRecord):
    """Effective cursor start and end values for one incremental model."""

    start: Bound
    end: Bound
    future_safety: FutureCursorSafetyEvidence | None
    maximum_start_safety: MaximumStartSafetyEvidence | None
    def __init__(
        self,
        *,
        start: Bound | str,
        end: Bound | str,
        future_safety: FutureCursorSafetyEvidence | None = None,
        maximum_start_safety: MaximumStartSafetyEvidence | None = None,
    ) -> None: ...

class CursorInputEvidence(NativeRecord):
    """Observed bounds for one physical cursor input."""

    relation: str
    cursor_column: str
    minimum: CursorScalar | None
    maximum: CursorScalar
    def __init__(
        self,
        *,
        relation: str,
        cursor_column: str,
        minimum: CursorScalar | str | None,
        maximum: CursorScalar | str,
    ) -> None: ...

class FutureCursorSafetyEvidence(NativeRecord):
    """Structured evidence for one applied future-cursor cap."""

    action: FutureCursorAction
    max_distance: str
    invocation_time: CursorScalar
    discovered_start: CursorScalar
    discovered_end: CursorScalar
    applied_start: CursorScalar
    applied_end: CursorScalar
    maximum_allowed_start: CursorScalar
    maximum_allowed_end: CursorScalar
    future_start_detected: bool
    future_end_detected: bool
    determining_relation: str | None
    determining_cursor_column: str | None
    inputs: tuple[CursorInputEvidence, ...]
    def __init__(self, **values: Any) -> None: ...

class MaximumStartSafetyEvidence(NativeRecord):
    """Structured evidence for an automatic-start eligibility decision."""

    action: FutureCursorAction
    max_ahead: str
    invocation_time: CursorScalar
    physical_target_max: CursorScalar
    highest_eligible_target_max: CursorScalar | None
    effective_start: CursorScalar
    maximum_allowed_start: CursorScalar
    target_relation: str
    cursor_column: str
    def __init__(
        self,
        *,
        action: FutureCursorAction,
        max_ahead: str,
        invocation_time: CursorScalar | str,
        physical_target_max: CursorScalar | str,
        highest_eligible_target_max: CursorScalar | str | None,
        effective_start: CursorScalar | str,
        maximum_allowed_start: CursorScalar | str,
        target_relation: str,
        cursor_column: str,
    ) -> None: ...

class CursorInputRelation(NativeRecord):
    """One cursor-bearing input relation for runtime range discovery."""

    relation: str
    cursor_column: str
    cursor_grain: CursorGrain | None
    is_model_backed: bool
    is_runtime_produced: bool
    terminal_cursor_start: CursorScalar | None
    terminal_cursor_end: CursorScalar | None
    input_name: str | None
    input_kind: GraphResourceKind | None
    def __init__(
        self,
        *,
        relation: str,
        cursor_column: str,
        cursor_grain: CursorGrain | str | None = None,
        is_model_backed: bool = False,
        is_runtime_produced: bool = False,
        terminal_cursor_start: CursorScalar | str | None = None,
        terminal_cursor_end: CursorScalar | str | None = None,
        input_name: str | None = None,
        input_kind: GraphResourceKind | None = None,
    ) -> None: ...
    @property
    def is_runtime_owned(self) -> bool: ...

class MaximumStartPolicyInputs(NativeRecord):
    """Effective policy and materialization safety inputs for automatic starts."""

    config: StartCursorsConfig | None
    invocation_time: datetime | None
    incremental_strategy: str | None
    incremental_mode: str | None
    def __init__(
        self,
        config: StartCursorsConfig | None = ...,
        invocation_time: datetime | None = ...,
        incremental_strategy: str | None = ...,
        incremental_mode: str | None = ...,
    ) -> None: ...

class SchemaFinding(NativeRecord):
    """One detected schema difference between expected and warehouse columns."""

    kind: SchemaChangeKind
    column_name: str
    source: SchemaColumnSource
    expected_type: str | None
    actual_type: str | None
    def __init__(
        self,
        kind: SchemaChangeKind,
        column_name: str,
        source: SchemaColumnSource,
        expected_type: str | None = ...,
        actual_type: str | None = ...,
    ) -> None: ...

class RetentionPlanEntry(NativeRecord):
    """Warehouse retention work independent of model identity actions."""

    request: RetentionRequest
    model_names: tuple[str, ...]
    actual_days: int | None
    effective_days: int | None
    source: str
    direction: RetentionDirection
    phase: RetentionPlanPhase
    statements: tuple[str, ...]
    irreversible_warning: str | None
    decrease_policy: str
    def __init__(
        self,
        request: RetentionRequest,
        model_names: tuple[str, ...],
        actual_days: int | None,
        effective_days: int | None,
        source: str,
        direction: RetentionDirection,
        phase: RetentionPlanPhase,
        statements: tuple[str, ...] = ...,
        irreversible_warning: str | None = ...,
        decrease_policy: str = ...,
    ) -> None: ...
    @property
    def decreases(self) -> bool: ...

class TableTypePlanEntry(NativeRecord):
    """Snowflake table-type work independent of model identity actions."""

    model_name: str
    destination: CompiledRelationLocation
    copy_name: str
    desired_type: str
    actual_type: str | None
    source: str
    downgrade: bool
    downgrade_policy: str
    irreversible_warning: str | None
    def __init__(
        self,
        model_name: str,
        destination: CompiledRelationLocation,
        copy_name: str,
        desired_type: str,
        actual_type: str | None,
        source: str,
        downgrade: bool,
        downgrade_policy: str,
        irreversible_warning: str | None = ...,
    ) -> None: ...

class ModelMigrationPlanEntry(NativeRecord):
    """One declared or discovered model migration and its per-run decision."""

    model_name: str
    discovery: MigrationDiscovery
    decision: MigrationDecision
    compatibility: MigrationCompatibility
    origin_model: str | None
    origin: CompiledRelationLocation
    destination: CompiledRelationLocation
    target_name: str | None
    origin_version_hash: str
    origin_is_transient: bool
    stage_is_transient: bool | None
    transfer: MigrationTransfer | None
    transfer_fallback: MigrationTransfer | None
    promotion: MigrationPromotion | None
    completed_at: datetime | None
    compatibility_findings: tuple[str, ...]
    message: str | None
    origin_tracked: bool
    missing_origin_policy: MissingMigrationOriginPolicy
    def __init__(
        self,
        model_name: str,
        discovery: MigrationDiscovery,
        decision: MigrationDecision,
        compatibility: MigrationCompatibility,
        origin_model: str | None,
        origin: CompiledRelationLocation,
        destination: CompiledRelationLocation,
        target_name: str | None,
        origin_version_hash: str = ...,
        origin_is_transient: bool = ...,
        stage_is_transient: bool | None = ...,
        transfer: MigrationTransfer | None = ...,
        transfer_fallback: MigrationTransfer | None = ...,
        promotion: MigrationPromotion | None = ...,
        completed_at: datetime | None = ...,
        compatibility_findings: tuple[str, ...] = ...,
        message: str | None = ...,
        origin_tracked: bool = ...,
        missing_origin_policy: MissingMigrationOriginPolicy = ...,
    ) -> None: ...
    @property
    def origin_missing(self) -> bool: ...
    @property
    def origin_hidden(self) -> bool: ...
    @property
    def storage_transition(self) -> str | None: ...
    @property
    def target_label(self) -> str: ...
    @property
    def blocks_build(self) -> bool: ...

class OldNameViewPlanEntry(NativeRecord):
    """What one build does at the old name of one migrated model."""

    model_name: str
    origin: CompiledRelationLocation
    destination: CompiledRelationLocation
    action: OldNameViewAction
    target_name: str | None
    retention: str | None
    expires_at: datetime | None
    column_aliases: tuple[tuple[str, str], ...]
    stores_history: bool
    migration_event_id: str | None
    records_requirement: bool
    reason: str | None
    grants_copied: int | None
    grants_supported: bool
    archived: bool
    def __init__(
        self,
        model_name: str,
        origin: CompiledRelationLocation,
        destination: CompiledRelationLocation,
        action: OldNameViewAction,
        target_name: str | None,
        retention: str | None = ...,
        expires_at: datetime | None = ...,
        column_aliases: tuple[tuple[str, str], ...] = ...,
        stores_history: bool = ...,
        migration_event_id: str | None = ...,
        records_requirement: bool = ...,
        reason: str | None = ...,
        grants_copied: int | None = ...,
        grants_supported: bool = ...,
        archived: bool = ...,
    ) -> None: ...
    @property
    def runs_steps(self) -> bool: ...

class OldNameView(NativeRecord):
    """One compatibility view recorded at an old name and not recorded as dropped."""

    destination_model: str
    old: CompiledRelationLocation
    new: CompiledRelationLocation
    expires_at: datetime | None
    column_aliases: tuple[tuple[str, str], ...]
    migration_event_id: str
    target_name: str | None
    name_reused_by: str | None
    def __init__(
        self,
        destination_model: str,
        old: CompiledRelationLocation,
        new: CompiledRelationLocation,
        expires_at: datetime | None,
        column_aliases: tuple[tuple[str, str], ...],
        migration_event_id: str,
        target_name: str | None,
        name_reused_by: str | None = ...,
    ) -> None: ...

class ColumnMigrationPlanEntry(NativeRecord):
    """One declared or detected in-place column rename and its per-run decision."""

    model_name: str
    destination: CompiledRelationLocation
    origin_column: str
    destination_column: str
    discovery: MigrationDiscovery
    decision: ColumnMigrationDecision
    target_name: str | None
    completed_at: datetime | None
    message: str | None
    missing_origin_policy: MissingMigrationOriginPolicy
    def __init__(
        self,
        model_name: str,
        destination: CompiledRelationLocation,
        origin_column: str,
        destination_column: str,
        discovery: MigrationDiscovery,
        decision: ColumnMigrationDecision,
        target_name: str | None,
        completed_at: datetime | None = ...,
        message: str | None = ...,
        missing_origin_policy: MissingMigrationOriginPolicy = ...,
    ) -> None: ...
    @property
    def origin_missing(self) -> bool: ...
    @property
    def blocks_build(self) -> bool: ...

class ModelPlanEntry(NativeRecord):
    """Per-model execution plan entry with action, reason, and resolved artifacts."""

    key: CompiledObjectKey
    name: str
    relative_path: Path
    materialization_type: MaterializationType
    action: PlanAction
    reason: PlanReason
    destination: CompiledRelationLocation
    fingerprint_query_sql: str
    resolved_sql: str
    logical_ddl: str
    migration_fingerprint: str | None
    incremental_strategy: str | None
    incremental_mode: str | None
    microbatch_strategy: str | None
    cursor_watermark_mode: str | None
    cursor_column: str | None
    cursor_type: str | None
    cursor_grain: str | None
    cursor_start: str | None
    cursor_end: str | None
    lookback: str | None
    cursor_bounds: CursorBounds | None
    empty_cursor_inputs: tuple[str, ...]
    waiting_on_empty_inputs: bool
    cursor_input_relations: tuple[CursorInputRelation, ...]
    batch_size: str | None
    batch_concurrency: int
    unaccounted_partition_policy: str | None
    microbatch_range: CursorBounds | None
    microbatch_limit: int | None
    declared_microbatch_limit_action: MicrobatchLimitAction | None
    microbatch_safety_limit: int | None
    microbatch_safety_limit_action: MicrobatchLimitAction | None
    microbatch_limit_count: int | None
    microbatch_limit_action: MicrobatchLimitAction | None
    microbatch_limit_warning: str | None
    start_cursor_override: str | None
    end_cursor_override: str | None
    future_cursor_config: FutureCursorsConfig | None
    start_cursor_config: StartCursorsConfig | None
    invocation_time: datetime | None
    unique_key: tuple[str, ...]
    merge_exclude_columns: tuple[str, ...]
    snapshot_strategy: str | None
    updated_at_column: str | None
    check_columns: tuple[str, ...]
    observed_at_column: str | None
    historical_input: str | None
    valid_from_column: str | None
    valid_to_column: str | None
    initial_valid_from: str | None
    invalidate_hard_deletes: bool
    snapshot_full_refresh: str | None
    snapshot_schema_change: str | None
    table_type: str
    physical_storage: dict[str, object]
    expected_output_columns: tuple[str, ...] | None
    on_schema_change: OnSchemaChange | None
    type_enforcement: bool
    declared_columns: tuple[ColumnInfo, ...]
    inferred_columns: tuple[ColumnInfo, ...]
    contract_enforced: bool
    contract_columns: tuple[ColumnInfo, ...]
    contract_dynamic_columns: tuple[SchemaDynamicColumnFamily, ...]
    pre_hooks: object
    post_hooks: object
    previous_query_sql: str | None
    previous_fingerprint: Fingerprint | None
    fingerprint_metadata_json: str | None
    previous_metadata_json: str | None
    fingerprint_version_hash: str | None
    previous_version_hash: str | None
    query_changed: bool
    config_changed: bool
    schema_actions: tuple[SchemaAction, ...]
    schema_findings: tuple[SchemaFinding, ...]
    backfill: BackfillResult
    changed_functions: tuple[str, ...]
    custom_materialization_name: str | None
    custom_config: dict[str, object]
    custom_placeholders: dict[str, str]
    column_rename_hints: tuple[ColumnRenameHint, ...]
    max_duration: str | None
    max_duration_seconds: int | None
    max_duration_source: str | None
    def __init__(
        self,
        key: CompiledObjectKey,
        name: str,
        relative_path: Path,
        materialization_type: MaterializationType,
        action: PlanAction,
        reason: PlanReason,
        destination: CompiledRelationLocation,
        fingerprint_query_sql: str,
        resolved_sql: str,
        logical_ddl: str,
        migration_fingerprint: str | None = ...,
        incremental_strategy: str | None = ...,
        incremental_mode: str | None = ...,
        microbatch_strategy: str | None = ...,
        cursor_watermark_mode: str | None = ...,
        cursor_column: str | None = ...,
        cursor_type: str | None = ...,
        cursor_grain: str | None = ...,
        cursor_start: str | None = ...,
        cursor_end: str | None = ...,
        lookback: str | None = ...,
        cursor_bounds: CursorBounds | None = ...,
        empty_cursor_inputs: tuple[str, ...] = ...,
        waiting_on_empty_inputs: bool = ...,
        cursor_input_relations: tuple[CursorInputRelation, ...] = ...,
        batch_size: str | None = ...,
        batch_concurrency: int = ...,
        unaccounted_partition_policy: str | None = ...,
        microbatch_range: CursorBounds | None = ...,
        microbatch_limit: int | None = ...,
        declared_microbatch_limit_action: MicrobatchLimitAction | None = ...,
        microbatch_safety_limit: int | None = ...,
        microbatch_safety_limit_action: MicrobatchLimitAction | None = ...,
        microbatch_limit_count: int | None = ...,
        microbatch_limit_action: MicrobatchLimitAction | None = ...,
        microbatch_limit_warning: str | None = ...,
        start_cursor_override: str | None = ...,
        end_cursor_override: str | None = ...,
        future_cursor_config: FutureCursorsConfig | None = ...,
        start_cursor_config: StartCursorsConfig | None = ...,
        invocation_time: datetime | None = ...,
        unique_key: tuple[str, ...] = ...,
        merge_exclude_columns: tuple[str, ...] = ...,
        snapshot_strategy: str | None = ...,
        updated_at_column: str | None = ...,
        check_columns: tuple[str, ...] = ...,
        observed_at_column: str | None = ...,
        historical_input: str | None = ...,
        valid_from_column: str | None = ...,
        valid_to_column: str | None = ...,
        initial_valid_from: str | None = ...,
        invalidate_hard_deletes: bool = ...,
        snapshot_full_refresh: str | None = ...,
        snapshot_schema_change: str | None = ...,
        table_type: str = ...,
        physical_storage: dict[str, object] = ...,
        expected_output_columns: tuple[str, ...] | None = ...,
        on_schema_change: OnSchemaChange | None = ...,
        type_enforcement: bool = ...,
        declared_columns: tuple[ColumnInfo, ...] = ...,
        inferred_columns: tuple[ColumnInfo, ...] = ...,
        contract_enforced: bool = ...,
        contract_columns: tuple[ColumnInfo, ...] = ...,
        contract_dynamic_columns: tuple[SchemaDynamicColumnFamily, ...] = ...,
        pre_hooks: object = ...,
        post_hooks: object = ...,
        previous_query_sql: str | None = ...,
        previous_fingerprint: Fingerprint | None = ...,
        fingerprint_metadata_json: str | None = ...,
        previous_metadata_json: str | None = ...,
        fingerprint_version_hash: str | None = ...,
        previous_version_hash: str | None = ...,
        query_changed: bool = ...,
        config_changed: bool = ...,
        schema_actions: tuple[SchemaAction, ...] = ...,
        schema_findings: tuple[SchemaFinding, ...] = ...,
        backfill: BackfillResult = ...,
        changed_functions: tuple[str, ...] = ...,
        custom_materialization_name: str | None = ...,
        custom_config: dict[str, object] = ...,
        custom_placeholders: dict[str, str] = ...,
        column_rename_hints: tuple[ColumnRenameHint, ...] = ...,
        max_duration: str | None = ...,
        max_duration_seconds: int | None = ...,
        max_duration_source: str | None = ...,
    ) -> None: ...

class CloneSourcePlanEntry(NativeRecord):
    """One managed source relation selected for target cloning."""

    key: CompiledObjectKey
    name: str
    destination: CompiledRelationLocation
    def __init__(
        self,
        key: CompiledObjectKey,
        name: str,
        destination: CompiledRelationLocation,
    ) -> None: ...

class SeedPlanEntry(NativeRecord):
    """Per-seed execution plan entry."""

    key: CompiledObjectKey
    name: str
    destination: CompiledRelationLocation
    file_path: Path
    columns: tuple[ColumnInfo, ...]
    csv_settings: SeedCsvSettings
    fingerprint_definition: str
    fingerprint_version_hash: str
    fingerprint_metadata_json: str
    action: PlanAction
    reason: PlanReason
    def __init__(
        self,
        key: CompiledObjectKey,
        name: str,
        destination: CompiledRelationLocation,
        file_path: Path,
        columns: tuple[ColumnInfo, ...],
        csv_settings: SeedCsvSettings,
        fingerprint_definition: str = ...,
        fingerprint_version_hash: str = ...,
        fingerprint_metadata_json: str = ...,
        action: PlanAction = ...,
        reason: PlanReason = ...,
    ) -> None: ...

class SourceLoadPlanEntry(NativeRecord):
    """Per-managed-source loader execution plan entry."""

    key: CompiledObjectKey
    name: str
    loader: str
    destination: str
    resource_kind: ExecutionResourceKind
    write_strategy: SourceWriteStrategy | None
    cursor_column: str | None
    unique_key: tuple[str, ...]
    is_reload: bool
    integration_kind: str | None
    def __init__(
        self,
        key: CompiledObjectKey,
        name: str,
        loader: str,
        destination: str,
        resource_kind: ExecutionResourceKind = ...,
        write_strategy: SourceWriteStrategy | None = ...,
        cursor_column: str | None = ...,
        unique_key: tuple[str, ...] = ...,
        is_reload: bool = ...,
        integration_kind: str | None = ...,
    ) -> None: ...

class FunctionPlanEntry(NativeRecord):
    """Per-SQL-function execution plan entry."""

    key: CompiledObjectKey
    name: str
    relative_path: Path
    destination: CompiledRelationLocation
    arguments: tuple[object, ...]
    returns: str
    body_sql: str
    fingerprint_query_sql: str
    fingerprint_destination: CompiledRelationLocation
    return_columns: tuple[FunctionReturnColumn, ...]
    language: FunctionLanguage
    source_file_path: Path | None
    runtime_version: str | None
    entry_point: str | None
    packages: tuple[str, ...]
    previous_query_sql: str | None
    reason: PlanReason
    def __init__(
        self,
        key: CompiledObjectKey,
        name: str,
        relative_path: Path,
        destination: CompiledRelationLocation,
        arguments: tuple[object, ...],
        returns: str,
        body_sql: str,
        fingerprint_query_sql: str,
        fingerprint_destination: CompiledRelationLocation,
        return_columns: tuple[FunctionReturnColumn, ...] = ...,
        language: FunctionLanguage = ...,
        source_file_path: Path | None = ...,
        runtime_version: str | None = ...,
        entry_point: str | None = ...,
        packages: tuple[str, ...] = ...,
        previous_query_sql: str | None = ...,
        reason: PlanReason = ...,
    ) -> None: ...

class AuditPlanEntry(NativeRecord):
    """Per-audit execution plan entry with resolved SQL and scheduling metadata."""

    key: CompiledObjectKey
    name: str
    definition_name: str
    resolved_sql: str
    unresolved_sql: str
    attachment_kind: AuditAttachmentKind
    severity: AuditSeverity
    requested_run_scope: AuditRunScope
    effective_run_scope: AuditRunScope
    evaluation_mode: AuditEvaluationMode
    value_column: str | None
    sample_count_column: str | None
    sample_unit: str | None
    thresholds: MeasurementThresholds | None
    minimum_samples: int | None
    evidence_resolved_sql: str | None
    evidence_unresolved_sql: str | None
    evidence_limit: int | None
    scope_deps: tuple[CompiledObjectKey, ...]
    attached_target_kind: AttachedAuditTargetKind | None
    attached_target_name: str | None
    attached_column_name: str | None
    always_run: bool
    description: str | None
    reads_outside_target_lineage: bool
    meta: dict[str, object] | None
    def __init__(
        self,
        key: CompiledObjectKey,
        name: str,
        definition_name: str,
        resolved_sql: str,
        unresolved_sql: str,
        attachment_kind: AuditAttachmentKind,
        severity: AuditSeverity,
        requested_run_scope: AuditRunScope,
        effective_run_scope: AuditRunScope,
        evaluation_mode: AuditEvaluationMode = ...,
        value_column: str | None = ...,
        sample_count_column: str | None = ...,
        sample_unit: str | None = ...,
        thresholds: MeasurementThresholds | None = ...,
        minimum_samples: int | None = ...,
        evidence_resolved_sql: str | None = ...,
        evidence_unresolved_sql: str | None = ...,
        evidence_limit: int | None = ...,
        scope_deps: tuple[CompiledObjectKey, ...] = ...,
        attached_target_kind: AttachedAuditTargetKind | None = ...,
        attached_target_name: str | None = ...,
        attached_column_name: str | None = ...,
        always_run: bool = ...,
        description: str | None = ...,
        reads_outside_target_lineage: bool = ...,
        meta: dict[str, object] | None = ...,
    ) -> None: ...

class ChainStep(NativeRecord):
    """One step in a chained test execution sequence."""

    model_name: str
    resolved_sql: str
    expected_cte_sql: str | None
    lifted_ctes: tuple[tuple[str, str], ...]
    comparison_body_sql: str | None
    expected_columns: tuple[str, ...] | None
    expected_lifted_ctes: tuple[tuple[str, str], ...]
    def __init__(
        self,
        model_name: str,
        resolved_sql: str,
        expected_cte_sql: str | None = ...,
        lifted_ctes: tuple[tuple[str, str], ...] = ...,
        comparison_body_sql: str | None = ...,
        expected_columns: tuple[str, ...] | None = ...,
        expected_lifted_ctes: tuple[tuple[str, str], ...] = ...,
    ) -> None: ...

class SqlTestPlanEntry(NativeRecord):
    """Per-test execution plan entry with chained resolution."""

    key: CompiledObjectKey
    name: str
    source_path: Path | None
    block_index: int | None
    parent_name: str | None
    case_name: str | None
    case_index: int | None
    case_fingerprint: str | None
    parameter_schema: tuple[SqlTestParameterDeclaration, ...]
    parameter_values: tuple[tuple[str, SqlValue], ...]
    mock_ref_names: tuple[str, ...]
    mock_source_names: tuple[str, ...]
    mock_seed_names: tuple[str, ...]
    mock_dbt_ref_names: tuple[str, ...]
    mock_table_function_names: tuple[str, ...]
    chain: tuple[ChainStep, ...]
    assertions: tuple[SqlTestAssertionStep, ...]
    scope_deps: tuple[CompiledObjectKey, ...]
    function_deps: tuple[CompiledObjectKey, ...]
    sql_analysis_enabled: bool
    real_model_count: int | None
    timeout: str | None
    timeout_seconds: int | None
    def __init__(
        self,
        key: CompiledObjectKey,
        name: str,
        source_path: Path | None = ...,
        block_index: int | None = ...,
        parent_name: str | None = ...,
        case_name: str | None = ...,
        case_index: int | None = ...,
        case_fingerprint: str | None = ...,
        parameter_schema: tuple[SqlTestParameterDeclaration, ...] = ...,
        parameter_values: tuple[tuple[str, SqlValue], ...] = ...,
        mock_ref_names: tuple[str, ...] = ...,
        mock_source_names: tuple[str, ...] = ...,
        mock_seed_names: tuple[str, ...] = ...,
        mock_dbt_ref_names: tuple[str, ...] = ...,
        mock_table_function_names: tuple[str, ...] = ...,
        chain: tuple[ChainStep, ...] = ...,
        assertions: tuple[SqlTestAssertionStep, ...] = ...,
        scope_deps: tuple[CompiledObjectKey, ...] = ...,
        function_deps: tuple[CompiledObjectKey, ...] = ...,
        sql_analysis_enabled: bool = ...,
        real_model_count: int | None = ...,
        timeout: str | None = ...,
        timeout_seconds: int | None = ...,
    ) -> None: ...

class ScenarioRelationMap(NativeRecord):
    """Resolved scenario hash prefix and scenario-owned relation names."""

    scenario_name: str
    hash_prefix: str
    artifacts: tuple[ScenarioArtifactName, ...]
    def __init__(
        self,
        scenario_name: str,
        hash_prefix: str,
        artifacts: tuple[ScenarioArtifactName, ...] = ...,
    ) -> None: ...

class ScenarioRelationPlan(NativeRecord):
    """Resolved scenario relation locations for model/source/seed ref resolution."""

    scenario_name: str
    relation_map: ScenarioRelationMap
    model_locations: dict[str, CompiledRelationLocation]
    seed_locations: dict[str, CompiledRelationLocation]
    project_source_map: dict[str, SourceEntry]
    source_map: dict[str, SourceEntry]
    source_fixture_locations: dict[str, CompiledRelationLocation]
    ref_fixture_locations: dict[str, CompiledRelationLocation]
    seed_fixture_locations: dict[str, CompiledRelationLocation]
    dbt_ref_fixture_locations: dict[str, CompiledRelationLocation]
    def __init__(
        self,
        scenario_name: str,
        relation_map: ScenarioRelationMap,
        model_locations: dict[str, CompiledRelationLocation] = ...,
        seed_locations: dict[str, CompiledRelationLocation] = ...,
        project_source_map: dict[str, SourceEntry] = ...,
        source_map: dict[str, SourceEntry] = ...,
        source_fixture_locations: dict[str, CompiledRelationLocation] = ...,
        ref_fixture_locations: dict[str, CompiledRelationLocation] = ...,
        seed_fixture_locations: dict[str, CompiledRelationLocation] = ...,
        dbt_ref_fixture_locations: dict[str, CompiledRelationLocation] = ...,
    ) -> None: ...

class ScenarioFixturePlan(NativeRecord):
    """Self-contained fixture SQL planned for scenario materialization."""

    kind: ScenarioArtifactKind
    logical_name: str
    destination: CompiledRelationLocation
    sql: str
    fingerprint_sql: str
    def __init__(
        self,
        kind: ScenarioArtifactKind,
        logical_name: str,
        destination: CompiledRelationLocation,
        sql: str,
        fingerprint_sql: str = ...,
    ) -> None: ...

class ScenarioExpectedExpectationPlan(NativeRecord):
    """Expected-output comparison inputs for one scenario target model."""

    model_name: str
    actual_destination: CompiledRelationLocation
    expected_sql: str
    compared_columns: tuple[str, ...] | None
    def __init__(
        self,
        model_name: str,
        actual_destination: CompiledRelationLocation,
        expected_sql: str,
        compared_columns: tuple[str, ...] | None = ...,
    ) -> None: ...

class ScenarioAssertionExpectationPlan(NativeRecord):
    """Zero-row assertion SQL for one scenario assertion CTE."""

    name: str
    sql: str
    def __init__(
        self,
        name: str,
        sql: str,
    ) -> None: ...

class ScenarioExecutionPlan(NativeRecord):
    """Dry-run execution plan for a SQL scenario graph slice."""

    key: CompiledObjectKey
    name: str
    graph_plan: ScenarioGraphPlan
    relation_plan: ScenarioRelationPlan
    fixture_plans: tuple[ScenarioFixturePlan, ...]
    seed_entries: tuple[SeedPlanEntry, ...]
    function_entries: tuple[FunctionPlanEntry, ...]
    model_entries: tuple[ModelPlanEntry, ...]
    hook_functions: tuple[DiscoveredHookFunction, ...]
    expected_expectations: tuple[ScenarioExpectedExpectationPlan, ...]
    assertion_expectations: tuple[ScenarioAssertionExpectationPlan, ...]
    def __init__(
        self,
        key: CompiledObjectKey,
        name: str,
        graph_plan: ScenarioGraphPlan,
        relation_plan: ScenarioRelationPlan,
        fixture_plans: tuple[ScenarioFixturePlan, ...] = ...,
        seed_entries: tuple[SeedPlanEntry, ...] = ...,
        function_entries: tuple[FunctionPlanEntry, ...] = ...,
        model_entries: tuple[ModelPlanEntry, ...] = ...,
        hook_functions: tuple[DiscoveredHookFunction, ...] = ...,
        expected_expectations: tuple[ScenarioExpectedExpectationPlan, ...] = ...,
        assertion_expectations: tuple[ScenarioAssertionExpectationPlan, ...] = ...,
    ) -> None: ...

class PlanOutput(NativeRecord):
    """Complete execution plan produced by the planner."""

    execution_order: tuple[CompiledObjectKey, ...]
    model_entries: tuple[ModelPlanEntry, ...]
    seed_entries: tuple[SeedPlanEntry, ...]
    source_load_entries: tuple[SourceLoadPlanEntry, ...]
    function_entries: tuple[FunctionPlanEntry, ...]
    audit_entries: tuple[AuditPlanEntry, ...]
    test_entries: tuple[SqlTestPlanEntry, ...]
    available_test_case_names: tuple[str, ...]
    selected_keys: frozenset[CompiledObjectKey]
    warnings: tuple[PlanWarning, ...]
    retention_entries: tuple[RetentionPlanEntry, ...]
    table_type_entries: tuple[TableTypePlanEntry, ...]
    migration_entries: tuple[ModelMigrationPlanEntry, ...]
    column_migration_entries: tuple[ColumnMigrationPlanEntry, ...]
    old_name_view_entries: tuple[OldNameViewPlanEntry, ...]
    old_name_views: tuple[OldNameView, ...]
    upstream_deps: dict[CompiledObjectKey, tuple[CompiledObjectKey, ...]]
    downstream_deps: dict[CompiledObjectKey, tuple[CompiledObjectKey, ...]]
    model_locations: dict[str, CompiledRelationLocation]
    seed_locations: dict[str, CompiledRelationLocation]
    function_locations: dict[str, CompiledRelationLocation]
    source_map: dict[str, SourceEntry]
    source_read_map: dict[str, SourceEntry]
    python_source_read_map: dict[str, SourceEntry]
    hook_functions: tuple[DiscoveredHookFunction, ...]
    enforce_explicit_references: bool
    provider_usages: tuple[PlanProviderUsage, ...]
    source_freshness: DirectSourceFreshnessPlanningResult | None
    python_identity_fingerprints: dict[tuple[str, str], Fingerprint]
    metadata: dict[str, object]
    cursor_overrides: CursorOverrides | None
    existing_model_names: frozenset[str]
    end_audit_source_keys: frozenset[CompiledObjectKey]
    def __init__(
        self,
        execution_order: tuple[CompiledObjectKey, ...] = ...,
        model_entries: tuple[ModelPlanEntry, ...] = ...,
        seed_entries: tuple[SeedPlanEntry, ...] = ...,
        source_load_entries: tuple[SourceLoadPlanEntry, ...] = ...,
        function_entries: tuple[FunctionPlanEntry, ...] = ...,
        audit_entries: tuple[AuditPlanEntry, ...] = ...,
        test_entries: tuple[SqlTestPlanEntry, ...] = ...,
        available_test_case_names: tuple[str, ...] = ...,
        selected_keys: frozenset[CompiledObjectKey] = ...,
        warnings: tuple[PlanWarning, ...] = ...,
        retention_entries: tuple[RetentionPlanEntry, ...] = ...,
        table_type_entries: tuple[TableTypePlanEntry, ...] = ...,
        migration_entries: tuple[ModelMigrationPlanEntry, ...] = ...,
        column_migration_entries: tuple[ColumnMigrationPlanEntry, ...] = ...,
        old_name_view_entries: tuple[OldNameViewPlanEntry, ...] = ...,
        old_name_views: tuple[OldNameView, ...] = ...,
        upstream_deps: dict[CompiledObjectKey, tuple[CompiledObjectKey, ...]] = ...,
        downstream_deps: dict[CompiledObjectKey, tuple[CompiledObjectKey, ...]] = ...,
        model_locations: dict[str, CompiledRelationLocation] = ...,
        seed_locations: dict[str, CompiledRelationLocation] = ...,
        function_locations: dict[str, CompiledRelationLocation] = ...,
        source_map: dict[str, SourceEntry] = ...,
        source_read_map: dict[str, SourceEntry] = ...,
        python_source_read_map: dict[str, SourceEntry] = ...,
        hook_functions: tuple[DiscoveredHookFunction, ...] = ...,
        enforce_explicit_references: bool = ...,
        provider_usages: tuple[PlanProviderUsage, ...] = ...,
        source_freshness: DirectSourceFreshnessPlanningResult | None = ...,
        python_identity_fingerprints: dict[tuple[str, str], Fingerprint] = ...,
        metadata: dict[str, object] = ...,
        cursor_overrides: CursorOverrides | None = ...,
        existing_model_names: frozenset[str] = ...,
        end_audit_source_keys: frozenset[CompiledObjectKey] = ...,
    ) -> None: ...
    @property
    def python_source_entries(self) -> dict[str, SourceEntry]: ...

class CompileInputError(ValueError):
    """Raised when discovered inputs cannot be attached into a compile view."""

    code: str
    message: str
    help: str | None
    def __init__(
        self, message: str, *, code: str | None = None, help: str | None = None
    ) -> None: ...

class SqlTestReferenceError(CompileInputError):
    """Raised when a SQL-test CTE calls a reference the test query cannot resolve."""

    location: SourceLocation
    def __init__(self, message: str, *, location: SourceLocation, help: str) -> None: ...

class RulesError(RuntimeError):
    """Raised for invalid compiler rules or execution."""

class RulesConfigError(RulesError):
    """Raised when rules configuration breaks a project policy; help shows the setting to change."""

    message: str
    help: str
    def __init__(self, message: str, *, help: str) -> None: ...

class RuleUsageError(RulesError):
    """Raised when a rule or rule test uses the public API incorrectly."""

class NonHermeticRuleError(RulesError):
    """Raised when a custom rule observes state the Rules cache cannot track."""

class RuleAssertionError(RulesError):
    """Raised when a custom-rule harness case produces an unexpected result."""

class OpaqueModuleStateError(RulesError):
    """Raised when custom-rule module state holds a value whose mutations cannot be observed."""

class HostCancelledError(RulesError):
    """Raised in a custom-rule host that stops because another host already failed."""

class SqlReferenceKind(StrEnum):
    REF = "ref"
    SEED = "seed"
    SOURCE = "source"
    DBT_REF = "dbt_ref"
    UDF = "udf"
    TABLE_FUNCTION = "table_fn"
    @property
    def function_name(self) -> str: ...
    @property
    def fixture_cte_prefix(self) -> str: ...
    def example_call(self, *args: str, quote: Literal["'", '"'] = "'") -> str: ...
    def placeholder_call(self, placeholder: str = "") -> str: ...

class HardCodedRelationOwnerKind(StrEnum):
    """The kind of Python code that names a project relation in SQL."""

    NODE = "node"
    HOOK = "hook"
    LOADER = "loader"

class LoaderConnectionMode(StrEnum):
    """How a source loader owns the destination warehouse connection."""

    SQLBUILD = "sqlbuild"
    EXTERNAL = "external"

class SqlHookEntry(NativeRecord):
    """A model lifecycle hook that executes SQL."""

    statement: str
    name: str | None
    relative_path: Path | None
    definition_sql: str | None
    kwargs: dict[str, object] | None
    description: str | None
    reads: tuple[SqlResourceRef, ...]
    def __init__(
        self,
        statement: str,
        name: str | None = ...,
        relative_path: Path | None = ...,
        definition_sql: str | None = ...,
        kwargs: dict[str, object] | None = ...,
        description: str | None = ...,
        reads: tuple[SqlResourceRef, ...] = ...,
    ) -> None: ...

class PythonHookEntry(NativeRecord):
    """A model lifecycle hook that invokes a discovered Python hook."""

    name: str
    kwargs: dict[str, object]
    def __init__(
        self,
        name: str,
        kwargs: dict[str, object],
    ) -> None: ...

class EnumDeclaration(NativeRecord):
    """One validated public or model-local enum declaration."""

    name: str
    members: tuple[EnumMember, ...]
    scalar_type: str
    relative_path: Path
    model_name: str | None
    def __init__(
        self,
        name: str,
        members: tuple[EnumMember, ...],
        scalar_type: str,
        relative_path: Path,
        model_name: str | None = ...,
    ) -> None: ...

class ConstantDeclaration(NativeRecord):
    """One validated public or model-local constant declaration."""

    name: str
    value: SqlValue
    relative_path: Path
    model_name: str | None
    render_as: CollectionRendering | None
    def __init__(
        self,
        name: str,
        value: SqlValue,
        relative_path: Path,
        model_name: str | None = ...,
        render_as: CollectionRendering | None = ...,
    ) -> None: ...
    @property
    def logical_type(self) -> SqlLogicalType: ...
    @property
    def scalar_type(self) -> str: ...

class DiscoveredSourceFile(NativeRecord):
    """A discovered source declaration file and its raw contents."""

    file_path: Path
    relative_path: Path
    contents: str
    source_entries: tuple[SourceEntry, ...]
    def __init__(
        self,
        file_path: Path,
        relative_path: Path,
        contents: str,
        source_entries: tuple[SourceEntry, ...],
    ) -> None: ...

class DiscoveredSqlTestFile(NativeRecord):
    """A discovered SQL-native test file and its raw contents."""

    file_path: Path
    relative_path: Path
    contents: str
    blocks: tuple[DiscoveredSqlTestBlock, ...]
    ownership_root: Path
    def __init__(
        self,
        file_path: Path,
        relative_path: Path,
        contents: str,
        blocks: tuple[DiscoveredSqlTestBlock, ...],
        ownership_root: Path = ...,
    ) -> None: ...

class SqlTestParameterDeclaration(NativeRecord):
    """One typed parameter declared by a SQL-native test template."""

    name: str
    value_type: SqlValueKind
    nullable: bool
    def __init__(
        self,
        name: str,
        value_type: SqlValueKind,
        nullable: bool = ...,
    ) -> None: ...

class DiscoveredSqlScenarioFile(NativeRecord):
    """A discovered SQL-native scenario file and its raw contents."""

    file_path: Path
    relative_path: Path
    contents: str
    header_values: dict[str, object]
    sql_body: str
    name: str
    ownership_root: Path
    sql_body_span: tuple[int, int] | None
    def __init__(
        self,
        file_path: Path,
        relative_path: Path,
        contents: str,
        header_values: dict[str, object],
        sql_body: str,
        name: str,
        ownership_root: Path = ...,
        sql_body_span: tuple[int, int] | None = ...,
    ) -> None: ...

class DiscoveredMacroFile(NativeRecord):
    """A discovered project macro file and its raw contents."""

    file_path: Path
    relative_path: Path
    contents: str
    scope_kind: ScopeKind
    ownership_root: Path | None
    owning_path: Path | None
    declaration_root: Path | None
    def __init__(
        self,
        file_path: Path,
        relative_path: Path,
        contents: str,
        scope_kind: ScopeKind = ...,
        ownership_root: Path | None = ...,
        owning_path: Path | None = ...,
        declaration_root: Path | None = ...,
    ) -> None: ...

class DiscoveredMaterializationFile(NativeRecord):
    """A discovered custom materialization Python file."""

    file_path: Path
    relative_path: Path
    name: str
    provider_usages: tuple[DiscoveredProviderUsage, ...]
    def __init__(
        self,
        file_path: Path,
        relative_path: Path,
        name: str,
        provider_usages: tuple[DiscoveredProviderUsage, ...] = ...,
    ) -> None: ...

class DiscoveredProviderUsage(NativeRecord):
    """Framework-owned metadata for provider usage by a Python callable surface."""

    provider_name: str
    parameter_name: str
    annotation_class_name: str | None
    annotation_module: str | None
    def __init__(
        self,
        provider_name: str,
        parameter_name: str,
        annotation_class_name: str | None = ...,
        annotation_module: str | None = ...,
    ) -> None: ...

class DiscoveredLoaderFunction(NativeRecord):
    """A discovered project source loader function."""

    file_path: Path
    relative_path: Path
    name: str
    function: Callable[..., object]
    depends_on: tuple[Callable[..., object] | SqlResourceRef, ...]
    destination: str | None
    write_strategy: SourceWriteStrategy | None
    cursor_column: str | None
    unique_key: tuple[str, ...]
    columns: tuple[SourceColumnEntry, ...]
    contract: str | None
    description: str | None
    connection_mode: LoaderConnectionMode
    provider_usages: tuple[DiscoveredProviderUsage, ...]
    identity: PythonNodeIdentity | None
    def __init__(
        self,
        file_path: Path,
        relative_path: Path,
        name: str,
        function: Callable[..., object],
        depends_on: tuple[Callable[..., object] | SqlResourceRef, ...] = ...,
        destination: str | None = ...,
        write_strategy: SourceWriteStrategy | None = ...,
        cursor_column: str | None = ...,
        unique_key: tuple[str, ...] = ...,
        columns: tuple[SourceColumnEntry, ...] = ...,
        contract: str | None = ...,
        description: str | None = ...,
        connection_mode: LoaderConnectionMode = ...,
        provider_usages: tuple[DiscoveredProviderUsage, ...] = ...,
        identity: PythonNodeIdentity | None = ...,
    ) -> None: ...

class DiscoveredTaskFunction(NativeRecord):
    """A discovered project task function."""

    file_path: Path
    relative_path: Path
    name: str
    function: Callable[..., object]
    depends_on: tuple[Callable[..., object] | SqlResourceRef, ...]
    tags: tuple[str, ...]
    group: str | None
    description: str | None
    meta: dict[str, object] | None
    retry: RetryPolicy | None
    provider_usages: tuple[DiscoveredProviderUsage, ...]
    identity: PythonNodeIdentity | None
    def __init__(
        self,
        file_path: Path,
        relative_path: Path,
        name: str,
        function: Callable[..., object],
        depends_on: tuple[Callable[..., object] | SqlResourceRef, ...] = ...,
        tags: tuple[str, ...] = ...,
        group: str | None = ...,
        description: str | None = ...,
        meta: dict[str, object] | None = ...,
        retry: RetryPolicy | None = ...,
        provider_usages: tuple[DiscoveredProviderUsage, ...] = ...,
        identity: PythonNodeIdentity | None = ...,
    ) -> None: ...

class DiscoveredAssetFunction(NativeRecord):
    """A discovered project asset function."""

    file_path: Path
    relative_path: Path
    name: str
    function: Callable[..., object]
    depends_on: tuple[Callable[..., object] | SqlResourceRef, ...]
    tags: tuple[str, ...]
    group: str | None
    description: str | None
    meta: dict[str, object] | None
    columns: tuple[SourceColumnEntry, ...]
    column_lineage: dict[str, tuple[ColumnLineageRef, ...]] | None
    retry: RetryPolicy | None
    provider_usages: tuple[DiscoveredProviderUsage, ...]
    identity: PythonNodeIdentity | None
    def __init__(
        self,
        file_path: Path,
        relative_path: Path,
        name: str,
        function: Callable[..., object],
        depends_on: tuple[Callable[..., object] | SqlResourceRef, ...] = ...,
        tags: tuple[str, ...] = ...,
        group: str | None = ...,
        description: str | None = ...,
        meta: dict[str, object] | None = ...,
        columns: tuple[SourceColumnEntry, ...] = ...,
        column_lineage: dict[str, tuple[ColumnLineageRef, ...]] | None = ...,
        retry: RetryPolicy | None = ...,
        provider_usages: tuple[DiscoveredProviderUsage, ...] = ...,
        identity: PythonNodeIdentity | None = ...,
    ) -> None: ...

class DiscoveredCheckFunction(NativeRecord):
    """A discovered project check function."""

    file_path: Path
    relative_path: Path
    name: str
    function: Callable[..., object]
    depends_on: tuple[Callable[..., object] | SqlResourceRef, ...]
    severity: PythonCheckSeverity
    tags: tuple[str, ...]
    group: str | None
    description: str | None
    meta: dict[str, object] | None
    provider_usages: tuple[DiscoveredProviderUsage, ...]
    identity: PythonNodeIdentity | None
    def __init__(
        self,
        file_path: Path,
        relative_path: Path,
        name: str,
        function: Callable[..., object],
        depends_on: tuple[Callable[..., object] | SqlResourceRef, ...],
        severity: PythonCheckSeverity = ...,
        tags: tuple[str, ...] = ...,
        group: str | None = ...,
        description: str | None = ...,
        meta: dict[str, object] | None = ...,
        provider_usages: tuple[DiscoveredProviderUsage, ...] = ...,
        identity: PythonNodeIdentity | None = ...,
    ) -> None: ...

class DiscoveredHookFunction(NativeRecord):
    """A discovered project model lifecycle hook function."""

    file_path: Path
    relative_path: Path
    name: str
    function: Callable[..., object]
    description: str | None
    reads: tuple[SqlResourceRef, ...]
    provider_usages: tuple[DiscoveredProviderUsage, ...]
    scope_kind: ScopeKind
    ownership_root: Path | None
    owning_path: Path | None
    declaration_root: Path | None
    def __init__(
        self,
        file_path: Path,
        relative_path: Path,
        name: str,
        function: Callable[..., object],
        description: str | None = ...,
        reads: tuple[SqlResourceRef, ...] = ...,
        provider_usages: tuple[DiscoveredProviderUsage, ...] = ...,
        scope_kind: ScopeKind = ...,
        ownership_root: Path | None = ...,
        owning_path: Path | None = ...,
        declaration_root: Path | None = ...,
    ) -> None: ...

class DiscoveredAuditFactory(NativeRecord):
    """A discovered audit factory and its normalized cases."""

    name: str
    function: Callable[..., object]
    file_path: Path
    relative_path: Path
    line: int
    cases: tuple[AuditCase, ...]
    def __init__(
        self,
        name: str,
        function: Callable[..., object],
        file_path: Path,
        relative_path: Path,
        line: int,
        cases: tuple[AuditCase, ...] = ...,
    ) -> None: ...

class DiscoveredEventExporterDeclaration(NativeRecord):
    """A discovered exporter declaration before provider binding."""

    file_path: Path
    relative_path: Path
    name: str
    function: Callable[..., object]
    event_kinds: frozenset[str]
    min_severity: EventExportSeverity
    def __init__(
        self,
        file_path: Path,
        relative_path: Path,
        name: str,
        function: Callable[..., object],
        event_kinds: frozenset[str],
        min_severity: EventExportSeverity,
    ) -> None: ...

class DiscoveredEventExporter(NativeRecord):
    """A discovered canonical lifecycle event exporter function."""

    file_path: Path
    relative_path: Path
    name: str
    function: Callable[..., object]
    event_kinds: frozenset[str]
    min_severity: EventExportSeverity
    provider_usages: tuple[DiscoveredProviderUsage, ...]
    def __init__(
        self,
        file_path: Path,
        relative_path: Path,
        name: str,
        function: Callable[..., object],
        event_kinds: frozenset[str] = ...,
        min_severity: EventExportSeverity = ...,
        provider_usages: tuple[DiscoveredProviderUsage, ...] = ...,
    ) -> None: ...

class DiscoveredCommandOutputSinkDeclaration(NativeRecord):
    """A command-output sink declaration before provider binding."""

    file_path: Path
    relative_path: Path
    name: str
    function: Callable[..., object]
    streams: frozenset[CommandOutputStream]
    def __init__(
        self,
        file_path: Path,
        relative_path: Path,
        name: str,
        function: Callable[..., object],
        streams: frozenset[CommandOutputStream],
    ) -> None: ...

class DiscoveredCommandOutputSink(NativeRecord):
    """A discovered command-output sink function."""

    file_path: Path
    relative_path: Path
    name: str
    function: Callable[..., object]
    streams: frozenset[CommandOutputStream]
    provider_usages: tuple[DiscoveredProviderUsage, ...]
    def __init__(
        self,
        file_path: Path,
        relative_path: Path,
        name: str,
        function: Callable[..., object],
        streams: frozenset[CommandOutputStream],
        provider_usages: tuple[DiscoveredProviderUsage, ...] = ...,
    ) -> None: ...

class DiscoveredProvider(NativeRecord):
    """A discovered project provider class and validated settings object."""

    file_path: Path
    relative_path: Path
    name: str
    provider_class: type[ProjectProvider]
    settings: ProjectProvider
    def __init__(
        self,
        file_path: Path,
        relative_path: Path,
        name: str,
        provider_class: type[ProjectProvider],
        settings: ProjectProvider,
    ) -> None: ...

class DiscoveredPythonNodeFunctions(NativeRecord):
    """Discovered project Python DAG node functions grouped by kind."""

    loaders: tuple[DiscoveredLoaderFunction, ...]
    tasks: tuple[DiscoveredTaskFunction, ...]
    assets: tuple[DiscoveredAssetFunction, ...]
    checks: tuple[DiscoveredCheckFunction, ...]
    audit_factories: tuple[DiscoveredAuditFactory, ...]
    def __init__(
        self,
        loaders: tuple[DiscoveredLoaderFunction, ...] = ...,
        tasks: tuple[DiscoveredTaskFunction, ...] = ...,
        assets: tuple[DiscoveredAssetFunction, ...] = ...,
        checks: tuple[DiscoveredCheckFunction, ...] = ...,
        audit_factories: tuple[DiscoveredAuditFactory, ...] = ...,
    ) -> None: ...

class DiscoveredProjectInputs(NativeRecord):
    """All raw project inputs discovered from disk before semantic resolution."""

    project_config: ProjectConfig
    local_config: LocalConfig
    project_dir: Path | None
    model_files: tuple[DiscoveredSqlModelFile, ...]
    enum_files: tuple[DiscoveredEnumFile, ...]
    constant_files: tuple[DiscoveredConstantFile, ...]
    model_schema_files: tuple[DiscoveredModelSchemaFile, ...]
    sql_function_files: tuple[DiscoveredSqlFunctionFile, ...]
    sql_hook_files: tuple[DiscoveredSqlHookFile, ...]
    python_function_files: tuple[DiscoveredPythonFunctionFile, ...]
    schema_files: tuple[DiscoveredSchemaFile, ...]
    source_files: tuple[DiscoveredSourceFile, ...]
    seed_files: tuple[DiscoveredSeedFile, ...]
    test_files: tuple[DiscoveredSqlTestFile, ...]
    scenario_files: tuple[DiscoveredSqlScenarioFile, ...]
    audit_files: tuple[DiscoveredAuditFile, ...]
    macro_files: tuple[DiscoveredMacroFile, ...]
    materialization_files: tuple[DiscoveredMaterializationFile, ...]
    loader_functions: tuple[DiscoveredLoaderFunction, ...]
    task_functions: tuple[DiscoveredTaskFunction, ...]
    asset_functions: tuple[DiscoveredAssetFunction, ...]
    check_functions: tuple[DiscoveredCheckFunction, ...]
    audit_factories: tuple[DiscoveredAuditFactory, ...]
    hook_functions: tuple[DiscoveredHookFunction, ...]
    event_exporters: tuple[DiscoveredEventExporter, ...]
    command_output_sinks: tuple[DiscoveredCommandOutputSink, ...]
    providers: tuple[DiscoveredProvider, ...]
    adapter_file: DiscoveredAdapterFile | None
    native_session: NativeDiscoverySession | None
    def __init__(
        self,
        project_config: ProjectConfig,
        local_config: LocalConfig,
        project_dir: Path | None = ...,
        model_files: tuple[DiscoveredSqlModelFile, ...] = ...,
        enum_files: tuple[DiscoveredEnumFile, ...] = ...,
        constant_files: tuple[DiscoveredConstantFile, ...] = ...,
        model_schema_files: tuple[DiscoveredModelSchemaFile, ...] = ...,
        sql_function_files: tuple[DiscoveredSqlFunctionFile, ...] = ...,
        sql_hook_files: tuple[DiscoveredSqlHookFile, ...] = ...,
        python_function_files: tuple[DiscoveredPythonFunctionFile, ...] = ...,
        schema_files: tuple[DiscoveredSchemaFile, ...] = ...,
        source_files: tuple[DiscoveredSourceFile, ...] = ...,
        seed_files: tuple[DiscoveredSeedFile, ...] = ...,
        test_files: tuple[DiscoveredSqlTestFile, ...] = ...,
        scenario_files: tuple[DiscoveredSqlScenarioFile, ...] = ...,
        audit_files: tuple[DiscoveredAuditFile, ...] = ...,
        macro_files: tuple[DiscoveredMacroFile, ...] = ...,
        materialization_files: tuple[DiscoveredMaterializationFile, ...] = ...,
        loader_functions: tuple[DiscoveredLoaderFunction, ...] = ...,
        task_functions: tuple[DiscoveredTaskFunction, ...] = ...,
        asset_functions: tuple[DiscoveredAssetFunction, ...] = ...,
        check_functions: tuple[DiscoveredCheckFunction, ...] = ...,
        audit_factories: tuple[DiscoveredAuditFactory, ...] = ...,
        hook_functions: tuple[DiscoveredHookFunction, ...] = ...,
        event_exporters: tuple[DiscoveredEventExporter, ...] = ...,
        command_output_sinks: tuple[DiscoveredCommandOutputSink, ...] = ...,
        providers: tuple[DiscoveredProvider, ...] = ...,
        adapter_file: DiscoveredAdapterFile | None = ...,
        native_session: NativeDiscoverySession | None = ...,
    ) -> None: ...

class DiscoveredRuntimeExtensions(NativeRecord):
    """Command-startup providers, typed sinks, and lifecycle delivery settings."""

    providers: tuple[DiscoveredProvider, ...]
    event_exporters: tuple[DiscoveredEventExporter, ...]
    command_output_sinks: tuple[DiscoveredCommandOutputSink, ...]
    lifecycle_shutdown_timeout_seconds: int | None
    recall_lifecycle_sink: tuple[DiscoveredEventExporter, str] | None
    recall_output_sink: tuple[DiscoveredCommandOutputSink, str] | None
    def __init__(
        self,
        providers: tuple[DiscoveredProvider, ...] = ...,
        event_exporters: tuple[DiscoveredEventExporter, ...] = ...,
        command_output_sinks: tuple[DiscoveredCommandOutputSink, ...] = ...,
        lifecycle_shutdown_timeout_seconds: int | None = ...,
        recall_lifecycle_sink: tuple[DiscoveredEventExporter, str] | None = ...,
        recall_output_sink: tuple[DiscoveredCommandOutputSink, str] | None = ...,
    ) -> None: ...

class ProjectGraph:
    """Static compiled project graph held natively; dict views are built only when read."""

    def __init__(
        self,
        *,
        project: CompiledProjectView,
        native: NativeProjectGraph,
    ) -> None: ...
    @property
    def project(self) -> CompiledProjectView: ...
    @property
    def native(self) -> NativeProjectGraph: ...
    @property
    def lineage_views(self) -> LineageGraphViews: ...
    @property
    def upstream_deps(self) -> dict[CompiledObjectKey, tuple[CompiledObjectKey, ...]]: ...
    @property
    def downstream_deps(self) -> dict[CompiledObjectKey, tuple[CompiledObjectKey, ...]]: ...
    @property
    def tag_index(self) -> dict[str, frozenset[CompiledObjectKey]]: ...
    @property
    def path_index(self) -> dict[CompiledObjectKey, str]: ...
    @property
    def all_keys(self) -> dict[str, CompiledObjectKey]: ...

class CompilePipelineResult(NativeRecord):
    """Complete output from the compile-and-plan pipeline."""

    project: CompiledProjectView
    plan_output: PlanOutput
    custom_materializations: dict[str, Callable[..., Any]]
    python_node_names: frozenset[str]
    python_plan_entries: tuple[PythonPlanEntry, ...]
    compile_seconds: float | None
    planning_seconds: float | None
    def __init__(
        self,
        project: CompiledProjectView,
        plan_output: PlanOutput,
        custom_materializations: dict[str, Callable[..., Any]] = ...,
        python_node_names: frozenset[str] = ...,
        python_plan_entries: tuple[PythonPlanEntry, ...] = ...,
        compile_seconds: float | None = ...,
        planning_seconds: float | None = ...,
    ) -> None: ...

class CompilerDiagnostic(NativeRecord):
    """One project diagnostic produced by compile-time checks."""

    phase: DiagnosticPhase | str
    severity: DiagnosticSeverity | str
    code: str
    message: str
    resource_type: CompiledResourceType | str | None
    resource_name: str | None
    column_name: str | None
    path: Path | None
    line: int | None
    column: int | None
    location: SourceLocation | None
    related_locations: tuple[RelatedLocation, ...]
    help: str | None
    notes: tuple[str, ...]
    affected_rules: tuple[str, ...]
    def __init__(
        self,
        phase: DiagnosticPhase | str,
        severity: DiagnosticSeverity | str,
        code: str,
        message: str,
        resource_type: CompiledResourceType | str | None = ...,
        resource_name: str | None = ...,
        column_name: str | None = ...,
        path: Path | None = ...,
        line: int | None = ...,
        column: int | None = ...,
        location: SourceLocation | None = ...,
        related_locations: tuple[RelatedLocation, ...] = ...,
        help: str | None = ...,
        notes: tuple[str, ...] = ...,
        affected_rules: tuple[str, ...] = ...,
    ) -> None: ...
    @property
    def is_error(self) -> bool: ...

class InferredColumn(NativeRecord):
    """One output column inferred from query SQL via sql_analysis parsing."""

    name: str
    type: str | None
    nullability: InferredNullability
    def __init__(
        self,
        name: str,
        type: str | None = ...,
        nullability: InferredNullability = ...,
    ) -> None: ...

class LoadedMacro(NativeRecord):
    """One loaded project macro available for compile-time SQL expansion."""

    name: str
    file_path: Path
    relative_path: Path
    raw_source: str
    function: Callable[..., object]
    dependencies: tuple[DeclarationIdentity, ...]
    language: str
    def __init__(
        self,
        name: str,
        file_path: Path,
        relative_path: Path,
        raw_source: str,
        function: Callable[..., object],
        dependencies: tuple[DeclarationIdentity, ...] = ...,
        language: str = ...,
    ) -> None: ...
    @property
    def injects_context(self) -> bool: ...

class StaticMacroExport(NativeRecord):
    """Value-free AST inventory for one project-owned macro export."""

    name: str
    relative_path: Path
    parameters: tuple[str, ...]
    line: int
    source_digest: str
    dependencies: tuple[DeclarationIdentity, ...]
    def __init__(
        self,
        name: str,
        relative_path: Path,
        parameters: tuple[str, ...],
        line: int,
        source_digest: str,
        dependencies: tuple[DeclarationIdentity, ...] = ...,
    ) -> None: ...

class StaticMacroFault(NativeRecord):
    """One value-free static macro inventory fault."""

    relative_path: Path
    message: str
    def __init__(
        self,
        relative_path: Path,
        message: str,
    ) -> None: ...

class StaticMacroInventory(NativeRecord):
    """AST macro exports and faults collected without module execution."""

    exports: tuple[StaticMacroExport, ...]
    faults: tuple[StaticMacroFault, ...]
    def __init__(
        self,
        exports: tuple[StaticMacroExport, ...] = ...,
        faults: tuple[StaticMacroFault, ...] = ...,
    ) -> None: ...

class MacroContext(NativeRecord):
    """Expose target settings and caller-visible declarations to a Python SQL macro."""

    adapter_name: str
    sql_analysis_enabled: bool
    target_name: str | None
    vars: dict[str, object]
    constants: Mapping[str, object]
    enums: Mapping[str, Mapping[str, str | int]]
    _value_renderer: TypedSqlValueRenderer | None
    _collection_rendering: CollectionRendering
    _constant_declarations: Mapping[str, ConstantDeclaration]
    _enforce_explicit_references: bool
    _lexical_syntax: SqlLexicalSyntax | None
    def __init__(
        self,
        adapter_name: str,
        sql_analysis_enabled: bool,
        target_name: str | None,
        vars: dict[str, object] = ...,
        constants: Mapping[str, object] = ...,
        enums: Mapping[str, Mapping[str, str | int]] = ...,
        _value_renderer: TypedSqlValueRenderer | None = ...,
        _collection_rendering: CollectionRendering = ...,
        _constant_declarations: Mapping[str, ConstantDeclaration] = ...,
        _enforce_explicit_references: bool = ...,
        _lexical_syntax: SqlLexicalSyntax | None = ...,
    ) -> None: ...
    def render_constant(self, name: str) -> str: ...
    def render_enum_member(self, *, enum_name: str, member_name: str) -> str: ...

class DeclarationResolutionContext(NativeRecord):
    """Declarations visible during one authored SQL expansion."""

    enums: dict[str, EnumDeclaration]
    constants: dict[str, ConstantDeclaration]
    inaccessible_enums: dict[str, DeclarationRecord]
    inaccessible_constants: dict[str, DeclarationRecord]
    enum_visibility: dict[str, tuple[VisibilityRecord, ...]]
    constant_visibility: dict[str, tuple[VisibilityRecord, ...]]
    macros: dict[str, LoadedMacro]
    macro_records: dict[str, DeclarationRecord]
    macro_visibility: dict[str, tuple[VisibilityRecord, ...]]
    inaccessible_macros: dict[str, DeclarationRecord]
    consumer: ResourceIdentity | DeclarationIdentity | None
    def __init__(
        self,
        enums: dict[str, EnumDeclaration] = ...,
        constants: dict[str, ConstantDeclaration] = ...,
        inaccessible_enums: dict[str, DeclarationRecord] = ...,
        inaccessible_constants: dict[str, DeclarationRecord] = ...,
        enum_visibility: dict[str, tuple[VisibilityRecord, ...]] = ...,
        constant_visibility: dict[str, tuple[VisibilityRecord, ...]] = ...,
        macros: dict[str, LoadedMacro] = ...,
        macro_records: dict[str, DeclarationRecord] = ...,
        macro_visibility: dict[str, tuple[VisibilityRecord, ...]] = ...,
        inaccessible_macros: dict[str, DeclarationRecord] = ...,
        consumer: ResourceIdentity | DeclarationIdentity | None = ...,
    ) -> None: ...

class CompileSqlReference(NativeRecord):
    """One logical SQL reference discovered from compiled SQL text."""

    ref_kind: SqlReferenceKind | str
    ref_name: str
    ref_package: str | None
    call_argument_count: int | None
    def __init__(
        self,
        ref_kind: SqlReferenceKind | str,
        ref_name: str,
        ref_package: str | None = ...,
        call_argument_count: int | None = ...,
    ) -> None: ...

class CompiledLineageSourceFact(NativeRecord):
    """Compact upstream column fact extracted during SQL analysis."""

    resource_type: CompiledResourceType | str
    resource_name: str
    column_name: str
    def __init__(
        self,
        resource_type: CompiledResourceType | str,
        resource_name: str,
        column_name: str,
    ) -> None: ...

class CompactLineageFacts(NativeRecord, Sequence[CompiledLineageColumnFact]):
    """Indexed native lineage rows with lazy object projection."""

    string_pool: tuple[str, ...]
    rows: tuple[tuple[int, int, int, tuple[tuple[int, int, int], ...]], ...]
    resource_name_indexes: dict[int, int]
    resource_names: dict[int, str]
    def __init__(
        self,
        string_pool: tuple[str, ...],
        rows: tuple[tuple[int, int, int, tuple[tuple[int, int, int], ...]], ...],
        resource_name_indexes: dict[int, int] = ...,
        resource_names: dict[int, str] = ...,
    ) -> None: ...
    @property
    def facts(self) -> tuple[CompiledLineageColumnFact, ...]: ...
    def __len__(self) -> int: ...
    @overload
    def __getitem__(self, index: int) -> CompiledLineageColumnFact: ...
    @overload
    def __getitem__(self, index: slice) -> Sequence[CompiledLineageColumnFact]: ...
    def __iter__(self) -> Iterator[CompiledLineageColumnFact]: ...
    def __eq__(self, other: object) -> bool: ...
    def resource_name(self, index: int) -> str: ...

class CompiledObjectKey(NativeRecord):
    """Stable logical identity for one compiled resource or external dependency."""

    resource_type: CompiledResourceType | str
    name: str
    def __init__(
        self,
        resource_type: CompiledResourceType | str,
        name: str,
    ) -> None: ...

class CompileModelInput(NativeRecord):
    """One discovered model file with its attached schema metadata, if any."""

    model_file: DiscoveredSqlModelFile
    config: CompileModelConfig
    query_sql: str
    macro_source_sql: str
    references: tuple[CompileSqlReference, ...]
    schema_entry: SchemaModelEntry | None
    schema_file: DiscoveredSchemaFile | None
    sql_validation_enabled: bool
    rejected_sql_analysis_opt_out: SourceLocation | None
    enum_declarations: tuple[EnumDeclaration, ...]
    constant_declarations: tuple[ConstantDeclaration, ...]
    enum_columns: dict[str, EnumDeclaration]
    macro_deps: tuple[str, ...]
    macro_usages: tuple[UsageRecord, ...]
    declaration_usages: tuple[UsageRecord, ...]
    sql_expansion: CompiledSqlExpansion | None
    lint_expansion: CompiledSqlExpansion | None
    def __init__(
        self,
        model_file: DiscoveredSqlModelFile,
        config: CompileModelConfig = ...,
        query_sql: str = ...,
        macro_source_sql: str = ...,
        references: tuple[CompileSqlReference, ...] = ...,
        schema_entry: SchemaModelEntry | None = ...,
        schema_file: DiscoveredSchemaFile | None = ...,
        sql_validation_enabled: bool = ...,
        rejected_sql_analysis_opt_out: SourceLocation | None = ...,
        enum_declarations: tuple[EnumDeclaration, ...] = ...,
        constant_declarations: tuple[ConstantDeclaration, ...] = ...,
        enum_columns: dict[str, EnumDeclaration] = ...,
        macro_deps: tuple[str, ...] = ...,
        macro_usages: tuple[UsageRecord, ...] = ...,
        declaration_usages: tuple[UsageRecord, ...] = ...,
        sql_expansion: CompiledSqlExpansion | None = ...,
        lint_expansion: CompiledSqlExpansion | None = ...,
    ) -> None: ...
    @property
    def reference_rows(self) -> list[tuple[str, str, str | None, int | None]]: ...
    @property
    def relation_key_row(self) -> RelationKeyRow: ...
    @property
    def attached_schema_name(self) -> str | None: ...

class CompiledRelationLocation(NativeRecord):
    """Logical and physical relation location resolved during compile."""

    database: str | None
    schema: str | None
    name: str
    qualified_name: str | None
    logical_schema: str | None
    logical_database: str | None
    def __init__(
        self,
        database: str | None,
        schema: str | None,
        name: str,
        qualified_name: str | None,
        logical_schema: str | None = ...,
        logical_database: str | None = ...,
    ) -> None: ...

class CompiledModel(NativeRecord):
    """Planner-ready compiled model metadata."""

    key: CompiledObjectKey
    deps: tuple[CompiledObjectKey, ...]
    name: str
    relative_path: Path
    query_sql: str
    config: CompileModelConfig
    destination: CompiledRelationLocation
    references: tuple[CompileSqlReference, ...]
    schema_entry: SchemaModelEntry | None
    inferred_columns: tuple[InferredColumn, ...] | None
    fast_lineage_columns: Sequence[CompiledLineageColumnFact] | None
    fast_lineage_has_star: bool
    fast_lineage_star_resolved: bool
    authored_sql: str
    authored_query_sql: str
    output_column_locations: dict[str, SourceLocation]
    extract_implicit_alias_columns: bool
    macro_deps: tuple[str, ...]
    enum_declarations: tuple[EnumDeclaration, ...]
    constant_declarations: tuple[ConstantDeclaration, ...]
    enum_columns: dict[str, EnumDeclaration]
    binding_diagnostics: tuple[CompilerDiagnostic, ...]
    binding_validated: bool
    dynamic_column_contract: DynamicColumnContractProof | None
    unchecked_output_columns: frozenset[str]
    rejected_sql_analysis_opt_out: SourceLocation | None
    def __init__(
        self,
        key: CompiledObjectKey,
        deps: tuple[CompiledObjectKey, ...],
        name: str,
        relative_path: Path,
        query_sql: str,
        config: CompileModelConfig,
        destination: CompiledRelationLocation,
        references: tuple[CompileSqlReference, ...] = ...,
        schema_entry: SchemaModelEntry | None = ...,
        inferred_columns: tuple[InferredColumn, ...] | None = ...,
        fast_lineage_columns: Sequence[CompiledLineageColumnFact] | None = ...,
        fast_lineage_has_star: bool = ...,
        fast_lineage_star_resolved: bool = ...,
        authored_sql: str = ...,
        authored_query_sql: str = ...,
        output_column_locations: dict[str, SourceLocation] = ...,
        extract_implicit_alias_columns: bool = ...,
        macro_deps: tuple[str, ...] = ...,
        enum_declarations: tuple[EnumDeclaration, ...] = ...,
        constant_declarations: tuple[ConstantDeclaration, ...] = ...,
        enum_columns: dict[str, EnumDeclaration] = ...,
        binding_diagnostics: tuple[CompilerDiagnostic, ...] = ...,
        binding_validated: bool = ...,
        dynamic_column_contract: DynamicColumnContractProof | None = ...,
        unchecked_output_columns: frozenset[str] = ...,
        rejected_sql_analysis_opt_out: SourceLocation | None = ...,
    ) -> None: ...
    @property
    def reference_rows(self) -> list[tuple[str, str, str | None, int | None]]: ...
    @property
    def relation_key_row(self) -> RelationKeyRow: ...
    @property
    def key_row(self) -> tuple[str, str]: ...
    @property
    def dep_rows(self) -> list[tuple[str, str]]: ...
    @property
    def destination_row(self) -> tuple[str | None, str | None, str]: ...
    @property
    def inferred_rows(self) -> list[tuple[str, str | None, str]] | None: ...
    @property
    def schema_column_rows(self) -> list[tuple[str, str | None]] | None: ...
    @property
    def has_dynamic_columns(self) -> bool: ...
    @property
    def output_column_count(self) -> int: ...

class CompiledAudit(NativeRecord):
    """Compiled audit metadata selected by scope dependencies."""

    key: CompiledObjectKey
    scope_deps: tuple[CompiledObjectKey, ...]
    name: str
    definition_name: str
    audit_file: DiscoveredAuditFile
    audit_block: DiscoveredAuditBlock
    sql_body: str
    evaluation_mode: AuditEvaluationMode
    measurement_contract: MeasurementContract | None
    thresholds: MeasurementThresholds | None
    minimum_samples: int | None
    measure_sql: str | None
    evidence_sql: str | None
    evidence_limit: int | None
    references: tuple[CompileSqlReference, ...]
    attached_target_kind: AttachedAuditTargetKind | None
    attached_target_name: str | None
    attached_column_name: str | None
    severity: AuditSeverity | None
    run_scope: str | None
    always_run: bool
    description: str | None
    meta: dict[str, object] | None
    def __init__(
        self,
        key: CompiledObjectKey,
        scope_deps: tuple[CompiledObjectKey, ...],
        name: str,
        definition_name: str,
        audit_file: DiscoveredAuditFile,
        audit_block: DiscoveredAuditBlock,
        sql_body: str,
        evaluation_mode: AuditEvaluationMode = ...,
        measurement_contract: MeasurementContract | None = ...,
        thresholds: MeasurementThresholds | None = ...,
        minimum_samples: int | None = ...,
        measure_sql: str | None = ...,
        evidence_sql: str | None = ...,
        evidence_limit: int | None = ...,
        references: tuple[CompileSqlReference, ...] = ...,
        attached_target_kind: AttachedAuditTargetKind | None = ...,
        attached_target_name: str | None = ...,
        attached_column_name: str | None = ...,
        severity: AuditSeverity | None = ...,
        run_scope: str | None = ...,
        always_run: bool = ...,
        description: str | None = ...,
        meta: dict[str, object] | None = ...,
    ) -> None: ...

class CompiledSqlScenario(NativeRecord):
    """Compiled SQL-native scenario metadata selected by inferred graph targets."""

    key: CompiledObjectKey
    name: str
    scenario_file: DiscoveredSqlScenarioFile
    sql_body: str
    authored_ctes: tuple[CompileSqlScenarioCte, ...]
    expected_ctes: tuple[CompileSqlScenarioCte, ...]
    assertion_ctes: tuple[CompileSqlScenarioCte, ...]
    source_fixture_names: tuple[str, ...]
    ref_fixture_names: tuple[str, ...]
    seed_fixture_names: tuple[str, ...]
    dbt_ref_fixture_names: tuple[str, ...]
    expected_model_names: tuple[str, ...]
    assertion_names: tuple[str, ...]
    assertion_target_model_names: tuple[str, ...]
    target_model_names: tuple[str, ...]
    source_path: Path | None
    ownership_root: Path | None
    def __init__(
        self,
        key: CompiledObjectKey,
        name: str,
        scenario_file: DiscoveredSqlScenarioFile,
        sql_body: str,
        authored_ctes: tuple[CompileSqlScenarioCte, ...] = ...,
        expected_ctes: tuple[CompileSqlScenarioCte, ...] = ...,
        assertion_ctes: tuple[CompileSqlScenarioCte, ...] = ...,
        source_fixture_names: tuple[str, ...] = ...,
        ref_fixture_names: tuple[str, ...] = ...,
        seed_fixture_names: tuple[str, ...] = ...,
        dbt_ref_fixture_names: tuple[str, ...] = ...,
        expected_model_names: tuple[str, ...] = ...,
        assertion_names: tuple[str, ...] = ...,
        assertion_target_model_names: tuple[str, ...] = ...,
        target_model_names: tuple[str, ...] = ...,
        source_path: Path | None = ...,
        ownership_root: Path | None = ...,
    ) -> None: ...

class CompiledProject(NativeRecord):
    """Planner-ready whole-project compile output."""

    run_id: str
    effective_target_name: str | None
    effective_connection: dict[str, object]
    effective_vars: dict[str, object]
    binding_catalog: Any | None
    effective_target_database: str | None
    effective_target_schema: str | None
    sql_analysis_dialect: str | None
    sql_lexical_syntax: SqlLexicalSyntax
    compile_cache_dir: Path | None
    settings: SettingsConfig
    scenario: ScenarioConfig
    models: tuple[CompiledModelView, ...]
    sources: tuple[CompiledSourceView, ...]
    seeds: tuple[CompiledSeedView, ...]
    functions: tuple[CompiledFunctionView, ...]
    audits: tuple[CompiledAuditView, ...]
    sql_tests: tuple[CompiledSqlTest, ...]
    sql_scenarios: tuple[CompiledSqlScenario, ...]
    loader_functions: tuple[DiscoveredLoaderFunction, ...]
    hook_functions: tuple[DiscoveredHookFunction, ...]
    enforce_explicit_references: bool
    sql_hook_files: tuple[DiscoveredSqlHookFile, ...]
    materialization_files: tuple[DiscoveredMaterializationFile, ...]
    public_enums: dict[str, EnumDeclaration]
    public_constants: dict[str, ConstantDeclaration]
    loaded_macros: dict[str, LoadedMacro]
    diagnostics: tuple[CompilerDiagnostic, ...]
    unmatched_literal_sql_relations: tuple[LiteralSqlRelation, ...]
    external_sql_reference_resolver: ExternalSqlReferenceResolver | None
    scope_index: ScopeIndex
    sql_expansions: dict[Path, CompiledSqlExpansion]
    native_session: Any | None
    native_project: NativeCompiledProject | None
    def __init__(
        self,
        run_id: str,
        effective_target_name: str | None,
        effective_connection: dict[str, object],
        effective_vars: dict[str, object],
        binding_catalog: Any | None = ...,
        effective_target_database: str | None = ...,
        effective_target_schema: str | None = ...,
        sql_analysis_dialect: str | None = ...,
        sql_lexical_syntax: SqlLexicalSyntax = ...,
        compile_cache_dir: Path | None = ...,
        settings: SettingsConfig = ...,
        scenario: ScenarioConfig = ...,
        models: tuple[CompiledModelView, ...] = ...,
        sources: tuple[CompiledSourceView, ...] = ...,
        seeds: tuple[CompiledSeedView, ...] = ...,
        functions: tuple[CompiledFunctionView, ...] = ...,
        audits: tuple[CompiledAuditView, ...] = ...,
        sql_tests: tuple[CompiledSqlTest, ...] = ...,
        sql_scenarios: tuple[CompiledSqlScenario, ...] = ...,
        loader_functions: tuple[DiscoveredLoaderFunction, ...] = ...,
        hook_functions: tuple[DiscoveredHookFunction, ...] = ...,
        enforce_explicit_references: bool = ...,
        sql_hook_files: tuple[DiscoveredSqlHookFile, ...] = ...,
        materialization_files: tuple[DiscoveredMaterializationFile, ...] = ...,
        public_enums: dict[str, EnumDeclaration] = ...,
        public_constants: dict[str, ConstantDeclaration] = ...,
        loaded_macros: dict[str, LoadedMacro] = ...,
        diagnostics: tuple[CompilerDiagnostic, ...] = ...,
        unmatched_literal_sql_relations: tuple[LiteralSqlRelation, ...] = ...,
        external_sql_reference_resolver: ExternalSqlReferenceResolver | None = ...,
        scope_index: ScopeIndex = ...,
        sql_expansions: dict[Path, CompiledSqlExpansion] = ...,
        native_session: Any | None = ...,
        native_project: NativeCompiledProject | None = ...,
    ) -> None: ...
    @property
    def lineage_graph(self) -> NativeProjectGraph: ...

class CompiledSqlTest(NativeRecord):
    """Compiled SQL-native unit test metadata selected by expected targets."""

    key: CompiledObjectKey
    scope_deps: tuple[CompiledObjectKey, ...]
    name: str
    test_file: DiscoveredSqlTestFile
    test_block: DiscoveredSqlTestBlock
    sql_body: str
    mode: SqlTestMode
    payload: CompiledModelSqlTestPayload | CompiledDirectLogicSqlTestPayload
    source_path: Path | None
    ownership_root: Path | None
    block_index: int | None
    explicit_name: str | None
    parent_name: str | None
    case_name: str | None
    case_index: int | None
    case_fingerprint: str | None
    parameter_schema: tuple[SqlTestParameterDeclaration, ...]
    parameter_values: tuple[tuple[str, SqlValue], ...]
    expected_model_names: tuple[str, ...]
    assertion_names: tuple[str, ...]
    assertion_target_model_names: tuple[str, ...]
    read_helper_names: tuple[str, ...]
    reference_target_model_names: tuple[str, ...]
    target_model_names: tuple[str, ...]
    tested_resources: tuple[CompiledSqlTestResource, ...]
    def __init__(
        self,
        key: CompiledObjectKey,
        scope_deps: tuple[CompiledObjectKey, ...],
        name: str,
        test_file: DiscoveredSqlTestFile,
        test_block: DiscoveredSqlTestBlock,
        sql_body: str,
        mode: SqlTestMode = ...,
        payload: CompiledModelSqlTestPayload | CompiledDirectLogicSqlTestPayload = ...,
        source_path: Path | None = ...,
        ownership_root: Path | None = ...,
        block_index: int | None = ...,
        explicit_name: str | None = ...,
        parent_name: str | None = ...,
        case_name: str | None = ...,
        case_index: int | None = ...,
        case_fingerprint: str | None = ...,
        parameter_schema: tuple[SqlTestParameterDeclaration, ...] = ...,
        parameter_values: tuple[tuple[str, SqlValue], ...] = ...,
        expected_model_names: tuple[str, ...] = ...,
        assertion_names: tuple[str, ...] = ...,
        assertion_target_model_names: tuple[str, ...] = ...,
        read_helper_names: tuple[str, ...] = ...,
        reference_target_model_names: tuple[str, ...] = ...,
        target_model_names: tuple[str, ...] = ...,
        tested_resources: tuple[CompiledSqlTestResource, ...] = ...,
    ) -> None: ...

class RuleSubject(StrEnum):
    """Compiler-owned unit used to invoke and cache one custom rule."""

    MODEL = "model"
    PROJECT = "project"

class Model(NativeRecord):
    """Stable identity and common metadata for one compiled SQL model."""

    name: str
    path: Path
    materialization: str | None
    def __init__(
        self,
        name: str,
        path: Path,
        materialization: str | None,
    ) -> None: ...

class Project(NativeRecord):
    """Stable summary for one compiled project invocation."""

    model_count: int
    target_name: str | None
    def __init__(
        self,
        model_count: int,
        target_name: str | None,
    ) -> None: ...

class ProjectPath(NativeRecord):
    """One normalized project-relative file or directory."""

    value: str
    is_file: bool
    is_dir: bool
    def __init__(
        self,
        value: str,
        is_file: bool,
        is_dir: bool,
    ) -> None: ...
    @property
    def parts(self) -> tuple[str, ...]: ...
    @property
    def name(self) -> str: ...
    @property
    def depth(self) -> int: ...

class SqlNode(NativeRecord):
    """Stable source location for one common SQL structure fact."""

    kind: str
    line: int
    column: int
    def __init__(
        self,
        kind: str,
        line: int,
        column: int,
    ) -> None: ...

class ModelSql(NativeRecord):
    """Authored and compiler-expanded SQL documents for one model."""

    authored: Any
    expanded: Any
    def __init__(
        self,
        authored: Any,
        expanded: Any,
    ) -> None: ...

class RuleFactViews(NativeRecord):
    """Views shared by every invocation in one custom-rule host evaluation."""

    project: Any
    sql: Any
    graph: Any
    columns: Any
    contracts: Any
    tests: Any
    audits: Any
    declarations: Any
    def __init__(
        self,
        project: Any,
        sql: Any,
        graph: Any,
        columns: Any,
        contracts: Any,
        tests: Any,
        audits: Any,
        declarations: Any,
    ) -> None: ...

class RuleOption[T](NativeRecord):
    """One typed configuration option declared by a rule."""

    @property
    def name(self) -> str: ...
    @property
    def value_type(self) -> type[T]: ...
    @property
    def default(self) -> T: ...
    @property
    def description(self) -> str: ...
    @property
    def choices(self) -> tuple[T, ...]: ...
    @property
    def minimum(self) -> int | None: ...
    @property
    def maximum(self) -> int | None: ...
    @property
    def minimum_items(self) -> int | None: ...
    @property
    def item_type(self) -> type[object] | None: ...
    def __init__(
        self,
        name: str,
        value_type: type[T],
        default: T,
        description: str,
        choices: tuple[T, ...] = ...,
        minimum: int | None = ...,
        maximum: int | None = ...,
        minimum_items: int | None = ...,
        item_type: type[object] | None = ...,
    ) -> None: ...
    @classmethod
    def boolean(cls, *, name: str, default: bool, description: str) -> RuleOption[bool]: ...
    @classmethod
    def integer(
        cls,
        *,
        name: str,
        default: int,
        description: str,
        minimum: int | None = None,
        maximum: int | None = None,
    ) -> RuleOption[int]: ...
    @classmethod
    def string(
        cls, *, name: str, default: str, description: str, choices: tuple[str, ...] = ()
    ) -> RuleOption[str]: ...
    @classmethod
    def string_list(
        cls,
        *,
        name: str,
        default: tuple[str, ...],
        description: str,
        minimum_items: int | None = None,
    ) -> RuleOption[tuple[str, ...]]: ...
    @classmethod
    def integer_list(
        cls,
        *,
        name: str,
        default: tuple[int, ...],
        description: str,
        minimum_items: int | None = None,
    ) -> RuleOption[tuple[int, ...]]: ...

class Rule(NativeRecord):
    """Immutable metadata and implementation for one rule."""

    code: str
    family: str
    slug: str
    message: str
    remediation: str
    check: RuleCheck
    options: tuple[RuleOption[object], ...]
    enabled_by_default: bool
    custom: bool
    source: str | None
    subject: RuleSubject | None
    subject_parameter: str | None
    context_parameter: str | None
    guidance: RuleGuidance | None
    def __init__(
        self,
        code: str,
        family: str,
        slug: str,
        message: str,
        remediation: str,
        check: RuleCheck,
        options: tuple[RuleOption[object], ...] = ...,
        enabled_by_default: bool = ...,
        custom: bool = ...,
        source: str | None = ...,
        subject: RuleSubject | None = ...,
        subject_parameter: str | None = ...,
        context_parameter: str | None = ...,
        guidance: RuleGuidance | None = ...,
    ) -> None: ...
    @property
    def project_wide(self) -> bool: ...

class Finding(NativeRecord):
    """One deterministic compiler rules violation."""

    code: str
    path: Path
    line: int
    column: int
    message: str
    remediation: str
    unevaluated: bool
    affected_rules: tuple[str, ...]
    fixable: bool
    def __init__(
        self,
        code: str,
        path: Path,
        line: int,
        column: int,
        message: str,
        remediation: str,
        unevaluated: bool = ...,
        affected_rules: tuple[str, ...] = ...,
        fixable: bool = ...,
    ) -> None: ...

class LayoutConfig(NativeRecord):
    levels: tuple[str, ...]
    domain_roots: tuple[str, ...]
    def __init__(
        self,
        levels: tuple[str, ...] = ...,
        domain_roots: tuple[str, ...] = ...,
    ) -> None: ...

class RulesConfig(NativeRecord):
    select: tuple[str, ...]
    ignore: tuple[str, ...]
    thresholds: dict[str, int]
    threshold_overrides: tuple[ThresholdOverride, ...]
    rule_options: dict[str, dict[str, RuleOptionValue]]
    rule_exceptions: tuple[RuleExemption, ...]
    graph_edge_exceptions: tuple[GraphEdgeExclusion, ...]
    rule_ignores: tuple[RuleIgnore, ...]
    select_star_allow: tuple[SelectStarAllow, ...]
    domains: tuple[str, ...]
    approved_source_tokens: tuple[str, ...]
    retired_source_tokens: dict[str, str]
    sql_tests: SqlTestRulesConfig
    layout: LayoutConfig
    cache: RulesCacheConfig
    def __init__(
        self,
        select: tuple[str, ...] = ...,
        ignore: tuple[str, ...] = ...,
        thresholds: dict[str, int] = ...,
        threshold_overrides: tuple[ThresholdOverride, ...] = ...,
        rule_options: dict[str, dict[str, RuleOptionValue]] = ...,
        rule_exceptions: tuple[RuleExemption, ...] = ...,
        graph_edge_exceptions: tuple[GraphEdgeExclusion, ...] = ...,
        rule_ignores: tuple[RuleIgnore, ...] = ...,
        select_star_allow: tuple[SelectStarAllow, ...] = ...,
        domains: tuple[str, ...] = ...,
        approved_source_tokens: tuple[str, ...] = ...,
        retired_source_tokens: dict[str, str] = ...,
        sql_tests: SqlTestRulesConfig = ...,
        layout: LayoutConfig = ...,
        cache: RulesCacheConfig = ...,
    ) -> None: ...

class RulesResult(NativeRecord):
    findings: tuple[Finding, ...]
    evaluated_models: int
    unevaluated_resources: int
    cache_hits: int
    cache_misses: int
    built_in_ms: int
    custom_ms: int
    custom_cpu_ms: int
    custom_rule_timings: tuple[tuple[str, int], ...]
    def __init__(
        self,
        findings: tuple[Finding, ...],
        evaluated_models: int,
        unevaluated_resources: int = ...,
        cache_hits: int = ...,
        cache_misses: int = ...,
        built_in_ms: int = ...,
        custom_ms: int = ...,
        custom_cpu_ms: int = ...,
        custom_rule_timings: tuple[tuple[str, int], ...] = ...,
    ) -> None: ...

class RuleFile(NativeRecord):
    path: str
    source: str
    def __init__(
        self,
        path: str,
        source: str,
    ) -> None: ...

class RuleCase(NativeRecord):
    description: str
    source: str
    expected_finding_count: int
    path: str
    files: tuple[RuleFile, ...]
    config: dict[str, object]
    def __init__(
        self,
        description: str,
        source: str,
        expected_finding_count: int,
        path: str = ...,
        files: tuple[RuleFile, ...] = ...,
        config: dict[str, object] = ...,
    ) -> None: ...

class RuleResult(NativeRecord):
    findings: tuple[Finding, ...]
    def __init__(
        self,
        findings: tuple[Finding, ...],
    ) -> None: ...
    @property
    def finding_count(self) -> int: ...

class CustomRuleEvaluation(NativeRecord):
    """Findings and observed fact reads of one rule invocation for one subject."""

    code: str
    subject: str | None
    findings: tuple[Finding, ...]
    reads: frozenset[tuple[object, ...]] | None
    def __init__(
        self,
        code: str,
        subject: str | None,
        findings: tuple[Finding, ...],
        reads: frozenset[tuple[object, ...]] | None,
    ) -> None: ...

class ModuleStateTrace(NativeRecord):
    """What exact module-state walks touched: object and type identities and their decisions."""

    touched: dict[int, int]
    descended_types: frozenset[int]
    checked_instances: frozenset[int]
    def __init__(
        self,
        touched: dict[int, int],
        descended_types: frozenset[int],
        checked_instances: frozenset[int],
    ) -> None: ...

class ModuleStateFingerprint(NativeRecord):
    """One exact custom-rule module-state fingerprint and the trace of the walk behind it."""

    token: str
    trace: ModuleStateTrace
    def __init__(
        self,
        token: str,
        trace: ModuleStateTrace,
    ) -> None: ...

class CustomRuleRun(NativeRecord):
    """Every invocation of one host run plus rules whose reads could not be attributed."""

    evaluations: tuple[CustomRuleEvaluation, ...]
    untracked_codes: frozenset[str]
    uncacheable_codes: frozenset[str]
    observed: tuple[tuple[tuple[str, ...], str | None], ...]
    stateful_codes: frozenset[str]
    rule_timings: dict[str, tuple[int, int]]
    monitor_ns: int
    def __init__(
        self,
        evaluations: tuple[CustomRuleEvaluation, ...],
        untracked_codes: frozenset[str],
        uncacheable_codes: frozenset[str] = ...,
        observed: tuple[tuple[tuple[str, ...], str | None], ...] = ...,
        stateful_codes: frozenset[str] = ...,
        rule_timings: dict[str, tuple[int, int]] = ...,
        monitor_ns: int = ...,
    ) -> None: ...

class CustomHostPartition(NativeRecord):
    """How one host of a split run detects module state and learns the run was cancelled."""

    cancelled: Callable[[], bool]
    def __init__(
        self,
        cancelled: Callable[[], bool],
    ) -> None: ...
