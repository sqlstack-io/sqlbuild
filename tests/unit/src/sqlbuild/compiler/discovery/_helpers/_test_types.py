from dataclasses import dataclass, field
from decimal import Decimal
from pathlib import Path

from sqlbuild._native import (
    AuthoredTimeTravelRetention,
    CollectionRendering,
    MissingMigrationOriginPolicy,
    RetentionDecreasePolicy,
    TargetWarehousesConfig,
)


@dataclass(frozen=True)
class AuditFactoryDiscoveryTestCase:
    description: str
    files: dict[str, str]
    expected_factory_names: tuple[str, ...] = ()
    expected_task_names: tuple[str, ...] = ()
    expected_error_fragment: str | None = None


@dataclass(frozen=True)
class CanonicalResourceIdentityTestCase:
    description: str
    name: str
    private_identity: bool
    expected_name: str


@dataclass(frozen=True)
class InvalidResourceIdentityTestCase:
    description: str
    name: str
    expected_corrected_name: str
    private_identity: bool = False


@dataclass(frozen=True)
class DiscoveredResourceIdentityErrorTestCase:
    description: str
    project_files: dict[str, str]
    expected_error_fragment: str


@dataclass(frozen=True)
class DeclarationResourceIdentityErrorTestCase:
    description: str
    contents: str
    expected_error_fragment: str


@dataclass(frozen=True)
class EventExporterDiscoveryTestCase:
    description: str
    expected_name: str


@dataclass(frozen=True)
class DiffConfigTestCase:
    """One project-level diff configuration case."""

    description: str
    expected_query_artifact_ttl: str
    expected_error_fragment: str | None = None


@dataclass(frozen=True)
class TargetExecutionLimitsConfigTestCase:
    description: str
    contents: str
    expected_max_models: int
    expected_max_duration: str
    expected_max_duration_seconds: int
    expected_remediation: str


@dataclass(frozen=True)
class CommandOutputSinkDiscoveryTestCase:
    description: str
    expected_name: str | None = None


@dataclass(frozen=True)
class EventExporterSignatureErrorTestCase:
    description: str
    body: str
    expected_error: str


@dataclass(frozen=True)
class EventExporterTypedSignatureTestCase:
    description: str
    declaration: str
    expected_error: str


@dataclass(frozen=True)
class FutureCursorConfigTestCase:
    description: str
    future_toml: str
    expected_max_distance: str
    expected_action: str


@dataclass(frozen=True)
class StartCursorConfigTestCase:
    description: str
    start_toml: str
    expected_max_ahead: str
    expected_action: str


@dataclass(frozen=True)
class FutureCursorConfigErrorTestCase:
    description: str
    future_toml: str
    expected_error_fragment: str


@dataclass(frozen=True)
class MicrobatchLimitConfigTestCase:
    description: str
    limits_toml: str
    expected_max_batches: int | None
    expected_action: str


@dataclass(frozen=True)
class MicrobatchLimitConfigErrorTestCase:
    description: str
    limits_toml: str
    expected_error_fragment: str


@dataclass(frozen=True)
class EffectiveBatchDefaultTestCase:
    description: str
    project_file_contents: str
    expected_batch_size: str


@dataclass(frozen=True)
class CheckDeclaredColumnsConfigTestCase:
    description: str
    settings_toml: str
    expected_checked: bool


@dataclass(frozen=True)
class CheckDeclaredColumnsConfigErrorTestCase:
    description: str
    settings_toml: str
    expected_error_fragment: str


@dataclass(frozen=True)
class DiscoverMacroFilesTestCase:
    description: str
    files: dict[str, str]
    expected_relative_paths: tuple[str, ...]


@dataclass(frozen=True)
class DiscoverScopedDeclarationTestCase:
    description: str
    authored_root: str
    expected_ownership_root: str


@dataclass(frozen=True)
class DiscoverGroupedDeclarationTestCase:
    """Discovery expectation for one declaration inside a grouped root."""

    description: str
    declaration_kind: str
    directory_name: str
    expected_scope_kind: str
    expected_owning_path: str


@dataclass(frozen=True)
class DiscoverGlobalDeclarationTestCase:
    description: str
    declaration_kind: str
    directory_name: str
    expected_scope_kind: str


@dataclass(frozen=True)
class InvalidScopedDeclarationRootTestCase:
    description: str
    relative_path: str
    expected_error_fragment: str


@dataclass(frozen=True)
class OrdinaryDiscoveryExclusionTestCase:
    description: str
    relative_path: str
    contents: str
    discoverer_name: str
    expected_relative_paths: tuple[str, ...]


@dataclass(frozen=True)
class OrdinaryFolderTestCase:
    description: str
    root_relative_path: str
    model_relative_path: str
    expected_model_relative_paths: tuple[str, ...]


@dataclass(frozen=True)
class DiscoveryPathInventoryTestCase:
    description: str
    files: tuple[str, ...]
    expected_relative_paths: tuple[str, ...]


@dataclass(frozen=True)
class StrictScopedDiscoveryTestCase:
    description: str
    relative_path: str
    contents: str
    expected_error_fragment: str


@dataclass(frozen=True)
class DeferredModelOutputLocationTestCase:
    description: str
    expected_extract_implicit_alias_columns: bool


@dataclass(frozen=True)
class LoadProjectConfigTestCase:
    description: str
    project_file_contents: str
    expected_name: str
    expected_adapter: str
    expected_default_target: str
    expected_connection: dict[str, str]
    expected_sql_analysis: bool
    expected_max_concurrency: int
    expected_materialized: str | None
    expected_row_diff_exclude_columns: tuple[str, ...]
    expected_row_diff_tolerances: dict[str, object]
    expected_contract: str | None
    expected_path_defaults: dict[str, dict[str, object]]
    expected_vars: dict[str, str]
    expected_targets: dict[str, dict[str, object]]
    expected_janitor_enabled: bool
    expected_retention_days: int | None
    expected_janitor_delete_tracked_only: bool
    expected_janitor_exclude_patterns: tuple[str, ...]
    expected_seed_database: str | None = None
    expected_seed_schema: str | None = None
    expected_function_database: str | None = None
    expected_function_schema: str | None = None
    expected_janitor_direct_state_history_versions: int = 20
    expected_janitor_archive_retention_days: int = 14
    expected_current_state_full_refresh: str = "deny"
    expected_historical_full_refresh: str = "require_confirmation"
    expected_snapshot_schema_change: str = "append_new_columns"
    expected_wildcard_check_schema_change: str = "require_confirmation"
    expected_scenario_local_type_overrides: dict[str, dict[str, str]] = field(default_factory=dict)
    expected_snapshot_limits: dict[str, int | None] = field(
        default_factory=lambda: {
            "max_rows_per_relation": None,
            "max_total_rows": None,
            "max_bytes_per_relation": None,
            "max_total_bytes": None,
        }
    )
    expected_dbt_project_dir: str | None = None
    expected_dbt_profiles_dir: str | None = None
    expected_dbt_target: str | None = None
    expected_dbt_target_path: str | None = None
    expected_dbt_vars: dict[str, object] = field(default_factory=dict)
    expected_auto_load_sources: bool = True
    expected_enforce_placement: bool = True
    expected_enforce_explicit_references: bool = True


@dataclass(frozen=True)
class LoadLocalConfigTestCase:
    description: str
    repo_files: dict[str, str]
    expected_target: str | None
    expected_adapter: str | None
    expected_connection: dict[str, object]
    expected_sql_analysis: bool
    expected_sql_validation: bool
    expected_max_concurrency: int
    expected_setting_overrides: frozenset[str]
    expected_vars: dict[str, str]
    expected_dbt_target: str | None = None
    expected_dbt_vars: dict[str, object] = field(default_factory=dict)
    expected_auto_load_sources: bool = True
    expected_targets: dict[str, dict[str, object]] = field(default_factory=dict)
    expected_missing_attributes: tuple[str, ...] = ()
    expected_scenario_local_type_overrides: dict[str, dict[str, str]] = field(default_factory=dict)
    expected_snapshot_limits: dict[str, int | None] = field(
        default_factory=lambda: {
            "max_rows_per_relation": None,
            "max_total_rows": None,
            "max_bytes_per_relation": None,
            "max_total_bytes": None,
        }
    )


@dataclass(frozen=True)
class LoadProjectConfigErrorTestCase:
    description: str
    project_file_contents: str
    expected_error_fragment: str


@dataclass(frozen=True)
class LoadProjectConfigHelpTestCase:
    description: str
    project_file_contents: str
    expected_error_fragment: str
    expected_help: str


@dataclass(frozen=True)
class LoadProjectCostConfigTestCase:
    description: str
    project_file_contents: str
    expected_usd_per_credit: Decimal
    expected_is_default: bool


@dataclass(frozen=True)
class LoadProjectCostConfigErrorTestCase:
    description: str
    value: str
    expected_error_fragment: str


@dataclass(frozen=True)
class LoadProjectConstantsConfigTestCase:
    description: str
    constants_toml: str
    expected_collection_rendering: CollectionRendering


@dataclass(frozen=True)
class LoadProjectConstantsConfigErrorTestCase:
    description: str
    constants_toml: str
    expected_error_fragment: str


@dataclass(frozen=True)
class LoadRetentionConfigTestCase:
    description: str
    project_file_contents: str
    expected_table_days: int
    expected_incremental_unmanaged: bool
    expected_target_days: int


@dataclass(frozen=True)
class LoadRetentionConfigErrorTestCase:
    description: str
    project_file_contents: str
    expected_error_fragment: str


@dataclass(frozen=True)
class LoadLocalConfigErrorTestCase:
    description: str
    local_file_contents: str
    expected_error_fragment: str


@dataclass(frozen=True)
class LoadNamedConnectionsTestCase:
    description: str
    filename: str
    contents: str
    expected_connections: dict[str, dict[str, object]]
    expected_connection_name: str | None


@dataclass(frozen=True)
class DiscoverHookFunctionsTestCase:
    description: str
    repo_files: dict[str, str]
    expected_hook_names: tuple[str, ...]
    expected_hook_paths: tuple[str, ...]
    expected_hook_descriptions: tuple[str | None, ...]
    expected_function_names: tuple[str, ...]
    expected_marker_file_exists: bool = False


@dataclass(frozen=True)
class DiscoverHookFunctionsErrorTestCase:
    description: str
    repo_files: dict[str, str]
    expected_error_fragment: str


@dataclass(frozen=True)
class ParseSqlHookTestCase:
    description: str
    contents: str
    expected_name: str
    expected_description: str | None
    expected_sql_body: str


@dataclass(frozen=True)
class ParseSqlHookErrorTestCase:
    description: str
    contents: str
    expected_error_fragment: str


@dataclass(frozen=True)
class DiscoverSqlHooksTestCase:
    description: str
    repo_files: dict[str, str]
    expected_names: tuple[str, ...]
    expected_paths: tuple[str, ...]


@dataclass(frozen=True)
class DiscoverProviderClassesTestCase:
    description: str
    repo_files: dict[str, str]
    expected_provider_names: tuple[str, ...]
    expected_provider_paths: tuple[str, ...]
    expected_provider_class_names: tuple[str, ...]
    expected_marker_file_exists: bool = False


@dataclass(frozen=True)
class DiscoverProviderCacheIsolationTestCase:
    description: str
    first_repo_files: dict[str, str]
    second_repo_files: dict[str, str]
    expected_first_provider_names: tuple[str, ...]
    expected_second_provider_names: tuple[str, ...]


@dataclass(frozen=True)
class DiscoverProviderClassesErrorTestCase:
    description: str
    repo_files: dict[str, str]
    expected_error_fragment: str
    expected_error_type: type[Exception]


@dataclass(frozen=True)
class DiscoverProviderEnvSettingsTestCase:
    description: str
    repo_files: dict[str, str]
    env_name: str
    env_value: str
    expected_provider_name: str
    expected_field_name: str
    expected_field_value: object


@dataclass(frozen=True)
class DiscoverProviderSecretErrorTestCase:
    description: str
    repo_files: dict[str, str]
    env_name: str
    env_value: str
    expected_error_fragment: str
    unexpected_error_fragment: str


@dataclass(frozen=True)
class ParseModelSqlHeaderTestCase:
    description: str
    contents: str
    expected_header_values: dict[str, object]
    expected_query: str


@dataclass(frozen=True)
class ParseModelSqlErrorTestCase:
    description: str
    contents: str
    expected_error_fragment: str


@dataclass(frozen=True)
class ParseDeclarationFileTestCase:
    description: str
    contents: str
    expected_names: tuple[str, ...]
    expected_scalar_types: tuple[str, ...]
    expected_values: tuple[tuple[str | int, ...], ...]


@dataclass(frozen=True)
class ParseDeclarationFileErrorTestCase:
    description: str
    contents: str
    expected_error_fragment: str


@dataclass(frozen=True)
class ParseTypedConstantsTestCase:
    description: str
    contents: str
    expected_kinds: tuple[str, ...]
    expected_decimal: Decimal
    expected_rendering: str
    expected_object_keys: tuple[str, ...]


@dataclass(frozen=True)
class ParseLocalTypedConstantsTestCase:
    description: str
    raw_value: dict[str, object]
    expected_kinds: tuple[str, ...]
    expected_decimal: Decimal


@dataclass(frozen=True)
class ParseModelSchemaDeclarationTestCase:
    description: str
    contents: str
    expected_names: tuple[str, ...]
    expected_description: str
    expected_parent: str
    expected_base_column_names: tuple[str, ...]
    expected_base_column_line: int
    expected_child_column_line: int
    expected_audit_names: tuple[str, ...]


@dataclass(frozen=True)
class ModelHeaderColumnLocationTestCase:
    description: str
    contents: str
    expected_locations: dict[str, tuple[Path, int, int, int | None, int | None]]


@dataclass(frozen=True)
class ModelOutputColumnLocationTestCase:
    description: str
    contents: str
    expected_locations: dict[str, tuple[Path, int, int, int | None, int | None]]
    extract_implicit_alias_columns: bool = True


@dataclass(frozen=True)
class ParseSqlTestFileTestCase:
    description: str
    contents: str
    expected_names: tuple[str | None, ...]
    expected_sql_bodies: tuple[str, ...]
    expected_test_indexes: tuple[int, ...]
    expected_header_values: tuple[dict[str, object], ...] = ()
    expected_parameter_types: tuple[str, ...] = ()
    expected_parameter_nullability: tuple[bool, ...] = ()
    expected_case_names: tuple[str, ...] = ()
    expected_case_values: tuple[tuple[object, ...], ...] = ()


@dataclass(frozen=True)
class ParseSqlTestFileErrorTestCase:
    description: str
    contents: str
    expected_error_fragment: str


@dataclass(frozen=True)
class ParseSqlTestModeTestCase:
    description: str
    contents: str
    expected_modes: tuple[str, ...]
    expected_header_values: tuple[dict[str, object], ...]


@dataclass(frozen=True)
class OmittedSelectPerformanceTestCase:
    description: str
    trailing_sql: str
    fixture_rows: int
    expected_offset_found: bool
    expected_max_seconds: float


@dataclass(frozen=True)
class ParseSqlTestCursorWindowTestCase:
    description: str
    contents: str
    expected_cursor_start: str | None
    expected_cursor_end: str | None


@dataclass(frozen=True)
class ParseSqlScenarioFileTestCase:
    description: str
    contents: str
    expected_name: str
    expected_header_values: dict[str, object]
    expected_sql_body: str


@dataclass(frozen=True)
class ParseSqlScenarioFileErrorTestCase:
    description: str
    contents: str
    expected_error_fragment: str


@dataclass(frozen=True)
class ParseSourcesYamlDltTestCase:
    description: str
    contents: str
    expected_source_names: tuple[str, ...]
    expected_loaders: tuple[str, ...]
    expected_kind: str
    expected_dlt_names: tuple[str, ...]
    expected_schemas: tuple[str | None, ...]
    expected_destination_config: dict[str, object] = field(default_factory=dict)
    expected_descriptions: tuple[str | None, ...] = ()


@dataclass(frozen=True)
class ParseSqlAuditFileTestCase:
    description: str
    contents: str
    expected_names: tuple[str | None, ...]
    expected_sql_bodies: tuple[str, ...]
    expected_audit_indexes: tuple[int, ...]
    expected_header_values: tuple[dict[str, object], ...] = ()


@dataclass(frozen=True)
class ParseSqlAuditFileErrorTestCase:
    description: str
    contents: str
    expected_error_fragment: str


@dataclass(frozen=True)
class ParseMeasurementAuditTestCase:
    description: str
    contents: str
    expected_value_column: str
    expected_sample_count_column: str
    expected_sample_unit: str
    expected_measure_fragment: str
    expected_evidence_fragment: str


@dataclass(frozen=True)
class ParseSchemaYamlTestCase:
    description: str
    contents: str
    expected_model_names: tuple[str, ...]
    expected_seed_names: tuple[str, ...]
    expected_model_column_names: tuple[tuple[str, ...], ...]
    expected_seed_column_names: tuple[tuple[str, ...], ...]
    expected_model_audit_names: tuple[tuple[str, ...], ...]
    expected_column_audit_names: tuple[tuple[tuple[str, ...], ...], ...]
    expected_model_column_nullables: tuple[tuple[bool | None, ...], ...] | None = None
    expected_seed_column_nullables: tuple[tuple[bool | None, ...], ...] | None = None
    expected_seed_databases: tuple[str | None, ...] = ()
    expected_seed_schemas: tuple[str | None, ...] = ()
    expected_seed_tags: tuple[tuple[str, ...], ...] | None = None


@dataclass(frozen=True)
class ParseSchemaYamlErrorTestCase:
    description: str
    contents: str
    expected_error_fragment: str


@dataclass(frozen=True)
class ParseSeedCsvSettingsYamlTestCase:
    description: str
    contents: str
    expected_delimiter: str | None
    expected_quotechar: str | None
    expected_doublequote: bool | None
    expected_escapechar: str | None
    expected_skipinitialspace: bool | None
    expected_lineterminator: str | None
    expected_encoding: str | None
    expected_na_values: tuple[object, ...] | dict[str, tuple[object, ...]] | None
    expected_keep_default_na: bool | None


@dataclass(frozen=True)
class DiscoverProjectInputsErrorTestCase:
    description: str
    repo_files: dict[str, str]
    expected_error_fragment: str


@dataclass(frozen=True)
class ParseSourcesYamlTestCase:
    description: str
    contents: str
    expected_source_names: tuple[str, ...]
    expected_column_names: tuple[tuple[str, ...], ...]
    expected_type_enforcement_values: tuple[bool | None, ...]
    expected_contract_values: tuple[str | None, ...]
    expected_expressions: tuple[str | None, ...]
    expected_loaders: tuple[str | None, ...] | None = None
    expected_write_strategies: tuple[str | None, ...] | None = None
    expected_load_batch_sizes: tuple[int | None, ...] | None = None
    expected_cursor_columns: tuple[str | None, ...] | None = None
    expected_unique_keys: tuple[tuple[str, ...], ...] | None = None
    expected_freshness_strategies: tuple[str | None, ...] | None = None
    expected_freshness_value_kinds: tuple[str | None, ...] | None = None
    expected_freshness_columns: tuple[str | None, ...] | None = None
    expected_freshness_queries: tuple[str | None, ...] | None = None
    expected_freshness_filters: tuple[str | None, ...] | None = None
    expected_freshness_lag_tolerances: tuple[str | None, ...] | None = None
    expected_freshness_age_policy_warn_afters: tuple[str | None, ...] | None = None
    expected_freshness_age_policy_error_afters: tuple[str | None, ...] | None = None
    expected_source_audit_names: tuple[tuple[str, ...], ...] = ()
    expected_column_audit_names: tuple[tuple[tuple[str, ...], ...], ...] = ()
    expected_column_nullables: tuple[tuple[bool | None, ...], ...] | None = None


@dataclass(frozen=True)
class ParseSourcesYamlErrorTestCase:
    description: str
    contents: str
    expected_error_fragment: str


@dataclass(frozen=True)
class ParseSourcesYamlHelpTestCase:
    description: str
    contents: str
    expected_error_fragment: str
    expected_help: str


@dataclass(frozen=True)
class ParseSourcesYamlIngestrTestCase:
    description: str
    contents: str
    expected_loader: str
    expected_kind: str
    expected_primary_key: tuple[str, ...]
    expected_extra_args: tuple[str, ...]


@dataclass(frozen=True)
class DiscoverMaterializationFilesTestCase:
    description: str
    files: dict[str, str]
    expected_names: tuple[str, ...]


@dataclass(frozen=True)
class DiscoverLoaderFunctionsTestCase:
    description: str
    files: dict[str, str]
    expected_names: tuple[str, ...]
    expected_targets: tuple[str | None, ...] = ()
    expected_dependency_counts: tuple[int, ...] = ()
    expected_descriptions: tuple[str | None, ...] = ()
    expected_write_strategies: tuple[str | None, ...] = ()
    expected_cursor_columns: tuple[str | None, ...] = ()
    expected_unique_keys: tuple[tuple[str, ...], ...] = ()
    expected_column_names: tuple[tuple[str, ...], ...] = ()
    expected_contracts: tuple[str | None, ...] = ()
    expected_error_fragment: str = ""


@dataclass(frozen=True)
class DiscoverPythonNodeFactoriesTestCase:
    description: str
    files: dict[str, str]
    expected_loader_names: tuple[str, ...]
    expected_task_names: tuple[str, ...]
    expected_asset_names: tuple[str, ...]
    expected_check_names: tuple[str, ...]
    expected_loader_dependency_counts: tuple[int, ...] = ()
    expected_task_dependency_counts: tuple[int, ...] = ()
    expected_asset_dependency_counts: tuple[int, ...] = ()
    expected_check_dependency_counts: tuple[int, ...] = ()
    expected_error_fragment: str = ""


@dataclass(frozen=True)
class DiscoverTaskAssetFunctionsTestCase:
    description: str
    files: dict[str, str]
    expected_task_names: tuple[str, ...]
    expected_task_dependency_counts: tuple[int, ...]
    expected_task_tags: tuple[tuple[str, ...], ...]
    expected_asset_names: tuple[str, ...]
    expected_asset_dependency_counts: tuple[int, ...]
    expected_asset_column_names: tuple[tuple[str, ...], ...]
    expected_asset_lineage_columns: tuple[tuple[str, ...], ...]
    expected_error_fragment: str = ""


@dataclass(frozen=True)
class DiscoverCheckFunctionsTestCase:
    description: str
    files: dict[str, str]
    expected_check_names: tuple[str, ...]
    expected_check_dependency_counts: tuple[int, ...]
    expected_check_severities: tuple[str, ...]
    expected_check_tags: tuple[tuple[str, ...], ...]


@dataclass(frozen=True)
class DiscoverPythonRootNodesTestCase:
    description: str
    files: dict[str, str]
    expected_nodes: tuple[tuple[str, str, str], ...]


@dataclass(frozen=True)
class DiscoverPythonRootImportTestCase:
    description: str
    files: dict[str, str]
    expected_task_names: tuple[str, ...]
    expected_init_count: int = 0


@dataclass(frozen=True)
class PythonRootProjectIsolationTestCase:
    description: str
    first_files: dict[str, str]
    second_files: dict[str, str]
    expected_first_result: object
    expected_second_result: object


@dataclass(frozen=True)
class PythonRootHelperIdentityTestCase:
    description: str
    files: dict[str, str]
    edited_path: str
    original_text: str
    edited_text: str
    expected_dependency_path: str


@dataclass(frozen=True)
class UnrelatedPythonPackageTestCase:
    description: str
    unrelated_files: dict[str, str]
    project_files: dict[str, str]
    expected_task_names: tuple[str, ...]
    expected_task_result: object


@dataclass(frozen=True)
class ExpectedBooleanTestCase:
    description: str
    expected_result: bool


@dataclass(frozen=True)
class ExpectedCountTestCase:
    description: str
    expected_count: int


@dataclass(frozen=True)
class LoadTargetRetentionPoliciesTestCase:
    description: str
    target_lines: tuple[str, ...]
    expected_default: AuthoredTimeTravelRetention | None
    expected_by_materialization: dict[str, AuthoredTimeTravelRetention]
    expected_decrease_policy: RetentionDecreasePolicy


@dataclass(frozen=True)
class LoadTargetRetentionPoliciesErrorTestCase:
    description: str
    target_lines: tuple[str, ...]
    expected_error_fragment: str


@dataclass(frozen=True)
class DiscoverFileFaultsTestCase:
    description: str
    discovery_name: str
    unreadable_files: tuple[str, ...]
    expected_fault_paths: tuple[Path, ...]
    expected_error_type: type[Exception] = UnicodeDecodeError


@dataclass(frozen=True)
class DiscoverModuleImportFailureTestCase:
    description: str
    discovery_name: str
    broken_file: str
    expected_error_type: type[Exception]
    expected_error_fragment: str


@dataclass(frozen=True)
class ParseSeedAuditsYamlTestCase:
    """Seed YAML with table and column audits."""

    description: str
    contents: str
    expected_table_audits: tuple[str, ...]
    expected_column_audits: tuple[tuple[str, ...], ...]


@dataclass(frozen=True)
class MissingMigrationOriginConfigTestCase:
    description: str
    target_lines: tuple[str, ...]
    expected_policy: MissingMigrationOriginPolicy


@dataclass(frozen=True)
class FunctionHeaderKeysTestCase:
    description: str
    file_name: str
    contents: str
    expected_error: str = ""
    expected_keys: tuple[str, ...] = ()
    expected_description: str | None = None


@dataclass(frozen=True)
class StatementHeaderKeysTestCase:
    description: str
    file_name: str
    contents: str
    expected_error: str
    expected_help: str


@dataclass(frozen=True)
class SnapshotGlobTestCase:
    description: str
    seed: int
    entry_count: int
    expected_mismatches: tuple[str, ...] = ()


@dataclass(frozen=True)
class SharedSnapshotDiscoveryTestCase:
    description: str
    expected_model_paths: tuple[str, ...]
    expected_mismatches: tuple[str, ...] = ()


@dataclass(frozen=True)
class DiscoveryPassRefreshTestCase:
    description: str
    added_files: dict[str, str]
    removed_path: str
    expected_first_paths: tuple[str, ...]
    expected_second_paths: tuple[str, ...]


@dataclass(frozen=True)
class SnapshotScopeReuseTestCase:
    description: str
    expected_nested_reuse: bool
    expected_later_reuse: bool


@dataclass(frozen=True)
class TargetWarehousesConfigTestCase:
    description: str
    contents: str
    expected_warehouses: TargetWarehousesConfig


@dataclass(frozen=True)
class TargetWarehousesConfigErrorTestCase:
    description: str
    project_contents: str
    expected_error_fragments: tuple[str, ...]
    local_contents: str = ""
    expected_help_fragments: tuple[str, ...] = ()
    adapter_file_contents: str = ""


@dataclass(frozen=True)
class TargetWarehousesDiscoveryTestCase:
    description: str
    project_contents: str
    expected_build: str | None
    local_contents: str = ""
    adapter_file_contents: str = ""


@dataclass(frozen=True)
class TargetDiffConfigTestCase:
    description: str
    project_contents: str
    expected_max_full_rows: int | str | None
    local_contents: str = ""


@dataclass(frozen=True)
class TargetDiffConfigErrorTestCase:
    description: str
    project_contents: str
    expected_error_fragment: str
    local_contents: str = ""


@dataclass(frozen=True)
class DeepProjectConnectionTestCase:
    """Test case for project TOML nested deeper than the native parser's own limit."""

    description: str
    depth: int
    expected_depth: int


@dataclass(frozen=True)
class TooDeepProjectConnectionTestCase:
    """Test case for project TOML nested past the parser's limit, and the error it raises."""

    description: str
    depth: int
    expected_error_fragment: str


@dataclass(frozen=True)
class OversizedDurationTestCase:
    """Test case for a duration with more digits than Python's int() converts."""

    description: str
    section: str
    expected_error_fragment: str


@dataclass(frozen=True)
class TargetConcurrencyConfigTestCase:
    description: str
    target_lines: str
    expected_concurrency: int | None = None
    expected_error_fragment: str = ""
    expected_error_code: str = "D001"
