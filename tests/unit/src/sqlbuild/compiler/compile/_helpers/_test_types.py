from __future__ import annotations

from collections.abc import Callable
from dataclasses import dataclass, field
from pathlib import Path
from typing import TYPE_CHECKING

from sqlbuild._native import (
    CompiledLineageColumnFact,
    CompiledObjectKey,
    CompileSqlReference,
    ExpansionSpan,
    InferredColumn,
    InferredNullability,
    ResolvedTimeTravelRetention,
    SourceLocation,
    SqlLexicalSyntax,
    SqlReferenceKind,
    SqlValue,
    UsageRecord,
)
from sqlbuild.adapter.contract.models import ExpressionInferenceProfile
from sqlbuild.compiler.compile.types import AttachedAuditTargetKind

if TYPE_CHECKING:
    from sqlbuild._native import CompileInputsView


@dataclass(frozen=True)
class SqlReferenceExtractionTestCase:
    description: str
    sql: str
    expected_references: tuple[tuple[SqlReferenceKind, str, str | None], ...]


@dataclass(frozen=True)
class DialectSqlScanTestCase:
    description: str
    syntax: SqlLexicalSyntax
    sql: str
    expected_names: tuple[str, ...]


@dataclass(frozen=True)
class SqlReferenceExtractionErrorTestCase:
    description: str
    sql: str
    expected_error: str
    expected_code: str = "P001"
    expected_help: str | None = None


@dataclass(frozen=True)
class AuditFactoryAttachmentTestCase:
    description: str
    model_header: str
    factory_cases: str
    expected_error_fragment: str | None = None
    expected_audit_names: tuple[str, ...] = ()
    expected_warning_code: str | None = None


@dataclass(frozen=True)
class WatermarkLimitValidationTestCase:
    description: str
    incremental_strategy: str
    batch_size: str
    lookback: str | None
    max_batches: int
    action: str
    expected_error_fragment: str | None


@dataclass(frozen=True)
class RetentionValidationErrorTestCase:
    description: str
    config_values: dict[str, object]
    retention: ResolvedTimeTravelRetention
    expected_error_fragment: str


@dataclass(frozen=True)
class RetentionValidationValidTestCase:
    description: str
    config_values: dict[str, object]
    retention: ResolvedTimeTravelRetention
    expected_unmanaged: bool


@dataclass(frozen=True)
class LoadProjectMacrosTestCase:
    description: str
    macro_files: dict[str, str]
    expected_macro_names: tuple[str, ...]


@dataclass(frozen=True)
class LoadProjectMacrosErrorTestCase:
    description: str
    macro_files: dict[str, str]
    expected_error_fragment: str
    expected_marker_exists: bool = False


@dataclass(frozen=True)
class SqlMacroInventoryTestCase:
    description: str
    macro_files: dict[str, str]
    expected_exports: tuple[str, ...]
    expected_fault_paths: tuple[str, ...]


@dataclass(frozen=True)
class MacroDependencyTestCase:
    description: str
    macro_files: dict[str, str]
    macro_name: str
    expected_result: str
    expected_dependencies: tuple[str, ...]


@dataclass(frozen=True)
class ScopedMacroImportErrorTestCase:
    description: str
    expected_error_fragment: str


@dataclass(frozen=True)
class ExpandSqlMacrosTestCase:
    description: str
    macro_file_contents: str
    sql: str
    expected_sql: str
    macro_overrides: dict[str, str] = field(default_factory=dict)


@dataclass(frozen=True)
class ExpandSqlMacrosErrorTestCase:
    description: str
    macro_file_contents: str
    sql: str
    expected_error_fragment: str
    macro_overrides: dict[str, str] = field(default_factory=dict)


@dataclass(frozen=True)
class FindMacroCallNamesTestCase:
    description: str
    sql: str
    expected_names: tuple[str, ...]


@dataclass(frozen=True)
class CompileDeclarationsErrorTestCase:
    description: str
    repo_files: dict[str, str]
    expected_error_fragment: str


@dataclass(frozen=True)
class CompileDeclarationsTestCase:
    description: str
    expected_query_sql: str


@dataclass(frozen=True)
class CompileTypedConstantsTestCase:
    description: str
    expected_model_sql: str
    expected_hook_sql: str
    expected_named_hook_sql: str
    expected_function_sql: str
    expected_source_sql: str
    expected_test_fragment: str
    expected_scenario_fragment: str
    expected_audit_sql: str
    expected_attached_audit_sql: str


@dataclass(frozen=True)
class DeclarationFingerprintTestCase:
    description: str
    declaration_path: str
    initial_declaration: str
    changed_declaration: str
    model_sql: str
    expected_query_hash_changed: bool
    expected_metadata_changed: bool
    additional_files: dict[str, str] = field(default_factory=dict)


@dataclass(frozen=True)
class ScopedDeclarationCompileTestCase:
    description: str
    files: dict[str, str]
    model_path: str
    model_sql: str
    expected_sql: str


@dataclass(frozen=True)
class ScopedDeclarationErrorTestCase:
    description: str
    files: dict[str, str]
    expected_error_fragments: tuple[str, ...]


@dataclass(frozen=True)
class ScopedDeclarationSurfaceTestCase:
    description: str
    files: dict[str, str]
    result: Callable[[CompileInputsView], str | None]
    expected_sql: str


@dataclass(frozen=True)
class ExpectedModelDeclarationGrantTestCase:
    description: str
    files: dict[str, str]
    expected_sql_fragments: tuple[str, ...]
    expected_grants: tuple[tuple[str, str, str], ...]
    input_collection: str = "test_inputs"


@dataclass(frozen=True)
class RelationshipUsageTestCase:
    description: str
    files: dict[str, str]
    expected_usage: UsageRecord


@dataclass(frozen=True)
class ParameterizedSqlTestCompilationTestCase:
    description: str
    repo_files: dict[str, str]
    expected_names: tuple[str, ...]
    expected_case_names: tuple[str, ...]
    expected_sql_fragments: tuple[tuple[str, ...], ...]


@dataclass(frozen=True)
class ParameterizedSqlTestCompilationErrorTestCase:
    description: str
    test_sql: str
    expected_error_fragment: str


@dataclass(frozen=True)
class ParameterizedSqlTestRenderingErrorTestCase:
    description: str
    rendered_value: str
    expected_error_fragment: str


@dataclass(frozen=True)
class ParameterizedSqlTestAdapterRenderingTestCase:
    description: str
    adapter_name: str
    value: SqlValue
    expected_sql: str


@dataclass(frozen=True)
class ScopePlacementCompileTestCase:
    description: str
    files: dict[str, str]
    expected_fragment: str | None = None
    expected_complete: bool = True
    expected_model_names: tuple[str, ...] = ()
    expected_diagnostics: tuple[tuple[str, str], ...] = ()


@dataclass(frozen=True)
class ModelSchemaErrorTestCase:
    description: str
    repo_files: dict[str, str]
    expected_error_fragment: str


@dataclass(frozen=True)
class ModelSchemaCompilationTestCase:
    description: str
    expected_schema_paths: tuple[str, ...]
    expected_exact_column_names: tuple[str, ...]
    expected_exact_column_types: tuple[str | None, ...]
    expected_superset_column_names: tuple[str, ...]
    expected_superset_inferred_column_names: tuple[str, ...]
    expected_exact_order_audit_names: tuple[str, ...]
    expected_exact_order_audit_paths: tuple[str, ...]
    expected_superset_order_audit_names: tuple[str, ...]


@dataclass(frozen=True)
class ModelSchemaContractDiagnosticTestCase:
    description: str
    contract: str
    projection: str
    expected_codes: tuple[str, ...]
    expected_help_fragment: str


@dataclass(frozen=True)
class ModelSchemaIdentityTestCase:
    description: str
    initial_type: str
    changed_type: str
    expected_identity_changed: bool


@dataclass(frozen=True)
class ModelSchemaCursorTestCase:
    description: str
    expected_column_names: tuple[str, ...]


@dataclass(frozen=True)
class DynamicModelSchemaTestCase:
    description: str
    expected_codes: tuple[str, ...] = ()
    expected_fixed_columns: tuple[str, ...] = ()


@dataclass(frozen=True)
class AssembleCompiledProjectTestCase:
    description: str
    repo_files: dict[str, str]
    expected_model_names: tuple[str, ...]
    expected_model_deps: tuple[tuple[CompiledObjectKey, ...], ...]
    expected_model_target_names: tuple[str, ...]
    expected_model_target_schemas: tuple[str | None, ...]
    expected_source_names: tuple[str, ...]
    expected_seed_names: tuple[str, ...]
    expected_audit_names: tuple[str, ...]
    expected_audit_scope_deps: tuple[tuple[CompiledObjectKey, ...], ...]
    expected_test_names: tuple[str, ...]
    expected_test_scope_deps: tuple[tuple[CompiledObjectKey, ...], ...]
    expected_test_expected_model_names: tuple[tuple[str, ...], ...]
    expected_model_macro_deps: tuple[tuple[str, ...], ...] = field(default_factory=tuple)
    expected_test_modes: tuple[str, ...] = field(default_factory=tuple)
    expected_tested_macro_names: tuple[tuple[str, ...], ...] = field(default_factory=tuple)
    expected_seed_target_schemas: tuple[str | None, ...] = field(default_factory=tuple)
    expected_seed_target_databases: tuple[str | None, ...] = field(default_factory=tuple)
    expected_seed_target_qualified_names: tuple[str | None, ...] = field(default_factory=tuple)
    expected_seed_logical_schemas: tuple[str | None, ...] = field(default_factory=tuple)
    expected_seed_logical_databases: tuple[str | None, ...] = field(default_factory=tuple)
    expected_source_databases: tuple[str | None, ...] = field(default_factory=tuple)
    expected_source_schemas: tuple[str | None, ...] = field(default_factory=tuple)
    expected_audit_attached_target_kinds: tuple[AttachedAuditTargetKind | None, ...] = ()
    expected_audit_always_runs: tuple[bool, ...] = ()


@dataclass(frozen=True)
class AttachedAuditNameTestCase:
    description: str
    expected_names: tuple[str, ...]


@dataclass(frozen=True)
class AssembleCompiledProjectEffectiveTargetTestCase:
    description: str
    project_toml: str
    expected_effective_target_database: str | None
    expected_effective_target_schema: str | None


@dataclass(frozen=True)
class PreservedSeedTargetTestCase:
    description: str
    expected_declared_location: tuple[str | None, str | None]
    expected_defaulted_location: tuple[str | None, str | None]
    expected_literal_target_location: tuple[str | None, str | None]


@dataclass(frozen=True)
class AssembleSqlHookValidationTestCase:
    description: str
    model_sql: str
    named_hook_sql: str
    dialect: str
    expected_hook_statements: tuple[str, str]


@dataclass(frozen=True)
class InferColumnsTestCase:
    description: str
    query_sql: str
    expected_columns: tuple[InferredColumn, ...] | None
    column_nullability_by_table: dict[str, dict[str, InferredNullability]] = field(
        default_factory=dict
    )
    inference_profile: ExpressionInferenceProfile | None = None


@dataclass(frozen=True)
class PolyglotAnalysisTestCase:
    description: str
    query_sql: str
    references: tuple[CompileSqlReference, ...]
    expected_columns: tuple[InferredColumn, ...] | None
    expected_lineage_columns: tuple[CompiledLineageColumnFact, ...]
    expected_has_star: bool
    column_nullability_by_table: dict[str, dict[str, InferredNullability]] = field(
        default_factory=dict
    )
    column_types_by_table: dict[str, dict[str, str]] = field(default_factory=dict)
    inference_profile: ExpressionInferenceProfile | None = None


@dataclass(frozen=True)
class RequiredExternalColumnsTestCase:
    description: str
    query_sql: str
    references: tuple[CompileSqlReference, ...]
    expected_columns: tuple[tuple[str, str, str], ...]


@dataclass(frozen=True)
class LongUnionRequiredColumnsTestCase:
    description: str
    branch_count: int
    expected_max_seconds: float


@dataclass(frozen=True)
class ValidateSqlSyntaxTestCase:
    description: str
    query_sql: str
    expected_valid: bool
    expected_error_fragment: str | None = None
    dialect: str | None = None


@dataclass(frozen=True)
class ValidateHookSqlSyntaxTestCase:
    description: str
    hook_sql: str
    dialect: str
    expected_valid: bool
    expected_error_fragment: str | None = None


@dataclass(frozen=True)
class IncrementalConfigValidTestCase:
    description: str
    config_values: dict[str, object]
    ref_count: int
    expected_valid: bool = True


@dataclass(frozen=True)
class IncrementalConfigErrorTestCase:
    description: str
    config_values: dict[str, object]
    ref_count: int
    expected_error_fragment: str


@dataclass(frozen=True)
class ChangePolicyHelpTestCase:
    description: str
    key: str
    value: str
    expected_help: str


@dataclass(frozen=True)
class ContractConfigValidTestCase:
    description: str
    config_values: dict[str, object]
    expected_valid: bool = True


@dataclass(frozen=True)
class ContractConfigErrorTestCase:
    description: str
    config_values: dict[str, object]
    expected_error_fragment: str


@dataclass(frozen=True)
class SubstituteSqlVarsTestCase:
    description: str
    sql: str
    effective_vars: dict[str, object]
    expected_sql: str
    environment_variables: dict[str, str] = field(default_factory=dict)
    context_values: dict[str, str | None] | None = None


@dataclass(frozen=True)
class SubstituteSqlVarsErrorTestCase:
    description: str
    sql: str
    effective_vars: dict[str, object]
    expected_error_fragment: str
    context_values: dict[str, str | None] | None = None


@dataclass(frozen=True)
class ExpandTemplateDataTestCase:
    description: str
    value: object
    variables: dict[str, object]
    context_values: dict[str, str | None]
    context_label: str
    allow_context: bool
    preserve_context_tokens: bool
    preserve_unknown_context: bool
    expected_value: object


@dataclass(frozen=True)
class ExpandTemplateDataErrorTestCase:
    description: str
    value: object
    variables: dict[str, object]
    context_values: dict[str, str | None]
    context_label: str
    allow_context: bool
    preserve_context_tokens: bool
    preserve_unknown_context: bool
    expected_error_fragment: str


@dataclass(frozen=True)
class NonIncrementalConfigValidTestCase:
    description: str
    config_values: dict[str, object]
    expected_valid: bool = True


@dataclass(frozen=True)
class NonIncrementalConfigErrorTestCase:
    description: str
    config_values: dict[str, object]
    expected_error_fragment: str


@dataclass(frozen=True)
class SnapshotConfigValidTestCase:
    description: str
    config_values: dict[str, object]
    expected_valid: bool = True


@dataclass(frozen=True)
class SnapshotConfigErrorTestCase:
    description: str
    config_values: dict[str, object]
    expected_error_fragment: str
    expected_error_code: str = "P001"


@dataclass(frozen=True)
class CustomMaterializationConfigErrorTestCase:
    description: str
    config_values: dict[str, object]
    custom_materialization_names: frozenset[str]
    expected_error_fragment: str


@dataclass(frozen=True)
class CustomMaterializationConfigValidTestCase:
    description: str
    config_values: dict[str, object]
    custom_materialization_names: frozenset[str]
    expected_valid: bool = True


@dataclass(frozen=True)
class PlaceholderConfigErrorTestCase:
    description: str
    config_values: dict[str, object]
    query_sql: str
    custom_materialization_names: frozenset[str]
    expected_error_fragment: str


@dataclass(frozen=True)
class PlaceholderConfigValidTestCase:
    description: str
    config_values: dict[str, object]
    query_sql: str
    custom_materialization_names: frozenset[str]
    expected_valid: bool = True


@dataclass(frozen=True)
class SubstitutePlaceholderDefaultsTestCase:
    description: str
    query_sql: str
    placeholders: dict[str, str]
    expected_sql: str


@dataclass(frozen=True)
class MapOutputOffsetTestCase:
    description: str
    spans: tuple[ExpansionSpan, ...]
    offset: int
    expected_offset: int
    expected_generated: bool


@dataclass(frozen=True)
class MapThroughPassesTestCase:
    description: str
    passes: tuple[tuple[ExpansionSpan, ...], ...]
    offset: int
    expected_offset: int
    expected_generated: bool


@dataclass(frozen=True)
class ExpandWithSpansTestCase:
    description: str
    macro_file_contents: str
    sql: str
    effective_vars: dict[str, object]
    expected_expanded_sql: str
    probe_expanded_offset: int
    expected_authored_offset: int
    expected_generated: bool


@dataclass(frozen=True)
class MicrobatchRedesignBehaviorTestCase:
    description: str
    expected_outcome: object


@dataclass(frozen=True)
class MicrobatchCursorTypeTestCase:
    description: str
    declared_type: str
    cursor_type: str
    expected_compatible: bool


@dataclass(frozen=True)
class MicrobatchGrainOwnershipTestCase:
    description: str
    consumer_grain: str
    producer_grain: str
    microbatch_strategy: str | None
    expected_grain: str


@dataclass(frozen=True)
class ExpectedMessageTestCase:
    description: str
    value: str
    expected_message: str


@dataclass(frozen=True)
class ExpectedBooleanTestCase:
    description: str
    expected_result: bool


@dataclass(frozen=True)
class ExpectedCountTestCase:
    description: str
    expected_count: int


@dataclass(frozen=True)
class NamedDeclarationAcceptedTestCase:
    """A valid audit, schema, or hook layout."""

    description: str
    files: dict[str, str]
    expected_diagnostic_codes: tuple[str, ...] = ()


@dataclass(frozen=True)
class NamedDeclarationErrorTestCase:
    """An invalid audit, schema, or hook layout and the diagnostic fragments it must produce."""

    description: str
    files: dict[str, str]
    expected_error_fragments: tuple[str, ...]


@dataclass(frozen=True)
class NamedDeclarationWarningTestCase:
    """A misplaced declaration compiled with advisory placement."""

    description: str
    files: dict[str, str]
    expected_code: str


@dataclass(frozen=True)
class AttachedAuditGateTestCase:
    """A project whose attached audit reads another resource, and the ordering edges it adds."""

    description: str
    files: dict[str, str]
    expected_edges: tuple[tuple[str, str], ...]


@dataclass(frozen=True)
class AttachedAuditGateCycleTestCase:
    """A project whose attached audit reads a resource built from its own target."""

    description: str
    files: dict[str, str]
    expected_error_fragments: tuple[str, ...]


@dataclass(frozen=True)
class TypedMacroReferenceTestCase:
    """A macro call passing typed references, and the SQL it expands to."""

    description: str
    macro_file_contents: str
    sql: str
    expected_sql: str


@dataclass(frozen=True)
class MacroGeneratedReferenceErrorTestCase:
    """A macro call whose expanded output contains a reference it was not given."""

    description: str
    macro_file_contents: str
    sql: str
    expected_error_fragments: tuple[str, ...]


@dataclass(frozen=True)
class MacroGeneratedReferenceSwitchTestCase:
    """A macro emitting a reference while explicit-reference enforcement is disabled."""

    description: str
    macro_file_contents: str
    sql: str
    expected_sql: str


@dataclass(frozen=True)
class MalformedTypedMacroReferenceTestCase:
    """A macro argument that looks like a reference call but is not a valid typed reference."""

    description: str
    sql: str
    expected_error_fragment: str


@dataclass(frozen=True)
class TypedReferenceAdapterRenderingTestCase:
    """An adapter resolving a typed macro reference with its own relation quoting."""

    description: str
    adapter_name: str
    expected_resolved_sql: str


@dataclass(frozen=True)
class HookReadEdgeTestCase:
    """A Python hook declaring reads, and the ordering edges its model gains."""

    description: str
    files: dict[str, str]
    expected_edges: tuple[tuple[str, str], ...]


@dataclass(frozen=True)
class HookReadErrorTestCase:
    """A Python hook declaring reads that cannot be satisfied."""

    description: str
    files: dict[str, str]
    expected_error_fragments: tuple[str, ...]


@dataclass(frozen=True)
class PythonSqlReferenceErrorTestCase:
    """A Python node or hook whose literal SQL hard-codes a project relation name."""

    description: str
    files: dict[str, str]
    expected_error_fragments: tuple[str, ...]
    unexpected_error_fragments: tuple[str, ...] = ()


@dataclass(frozen=True)
class PythonSqlReferenceAllowedTestCase:
    """Python SQL that compiles because it names no undeclared project relation literally."""

    description: str
    files: dict[str, str]
    expected_model_names: tuple[str, ...]


@dataclass(frozen=True)
class MacroGeneratedReferenceCollectionTestCase:
    """Model SQL whose macros emit several references, all reported by one expansion."""

    description: str
    macro_file_contents: str
    sql: str
    expected_messages: tuple[str, ...]


@dataclass(frozen=True)
class TypedMacroArgumentDependencyTestCase:
    """A model passing typed references to a macro that may not render all of them."""

    description: str
    macro_file_contents: str
    sql: str
    expected_dependency_names: frozenset[str]


@dataclass(frozen=True)
class RebindDeclarationScopeTestCase:
    description: str
    macro_name: str
    expected_rebound: bool
    private_macro_source: str | None = None


@dataclass(frozen=True)
class DescriptionInputClassificationCase:
    """Discovered input fields split between required and exempt description kinds."""

    description: str
    required: frozenset[str]
    exempt: frozenset[str]
    expected_fields: frozenset[str]


@dataclass(frozen=True)
class CollectedReferenceCallSyntaxTestCase:
    description: str
    sql: str
    expected_diagnostics: tuple[tuple[str, str, Path | None, SourceLocation | None], ...]


@dataclass(frozen=True)
class UndecodableSecretTestCase:
    """An env or var value holding a secret-like prefix and invalid text after it."""

    description: str
    value: object
    secret: str
    expected_message: str


@dataclass(frozen=True)
class LoadAssemblyTestCase:
    """A project with a load: the load node, the sources it fills and selection through it."""

    description: str
    repo_files: dict[str, str]
    selector: str
    expected_loads: tuple[tuple[str, tuple[str, ...]], ...]
    expected_source_deps: tuple[tuple[str, tuple[CompiledObjectKey, ...]], ...]
    expected_selected: tuple[tuple[str, str], ...]
