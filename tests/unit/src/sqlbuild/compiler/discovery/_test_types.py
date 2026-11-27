from dataclasses import dataclass

from sqlbuild._native import SourceWriteStrategy


@dataclass(frozen=True)
class InvalidEventExporterConfigTestCase:
    description: str
    config: str
    expected_error: str


@dataclass(frozen=True)
class EventExporterConfigTestCase:
    description: str
    config: str
    expected_kinds: frozenset[str]
    expected_min_severity: str


@dataclass(frozen=True)
class LifecycleShutdownTimeoutTestCase:
    description: str
    config: str
    expected_seconds: int | None


@dataclass(frozen=True)
class DiscoverProjectInputsTestCase:
    description: str
    repo_files: dict[str, str]
    expected_model_paths: tuple[str, ...]
    expected_model_header_values: tuple[dict[str, object], ...]
    expected_model_query_sql: tuple[str, ...]
    expected_schema_paths: tuple[str, ...]
    expected_schema_model_names: tuple[tuple[str, ...], ...]
    expected_schema_seed_names: tuple[tuple[str, ...], ...]
    expected_source_paths: tuple[str, ...]
    expected_source_entry_names: tuple[tuple[str, ...], ...]
    expected_seed_paths: tuple[str, ...]
    expected_test_paths: tuple[str, ...]
    expected_test_block_indexes: tuple[int, ...]
    expected_test_block_names: tuple[str | None, ...]
    expected_test_block_sql_bodies: tuple[str, ...]
    expected_scenario_paths: tuple[str, ...]
    expected_scenario_names: tuple[str, ...]
    expected_scenario_header_values: tuple[dict[str, object], ...]
    expected_scenario_sql_bodies: tuple[str, ...]
    expected_audit_paths: tuple[str, ...]
    expected_audit_block_indexes: tuple[int, ...]
    expected_audit_block_names: tuple[str | None, ...]
    expected_audit_block_sql_bodies: tuple[str, ...]
    expected_macro_paths: tuple[str, ...]
    expected_loader_names: tuple[str, ...]
    expected_adapter_path: str | None
    expected_asset_names: tuple[str, ...] = ()
    expected_check_names: tuple[str, ...] = ()
    expected_hook_names: tuple[str, ...] = ()
    expected_provider_names: tuple[str, ...] = ()
    expected_provider_paths: tuple[str, ...] = ()
    expected_provider_class_names: tuple[str, ...] = ()


@dataclass(frozen=True)
class DiscoverProjectInputsErrorTestCase:
    description: str
    repo_files: dict[str, str]
    expected_error_fragment: str


@dataclass(frozen=True)
class SourceConnectionErrorTestCase:
    description: str
    repo_files: dict[str, str]
    expected_error_fragment: str
    expected_file: str


@dataclass(frozen=True)
class ConnectionRolesTestCase:
    description: str
    repo_files: dict[str, str]
    expected_target_connection: str


@dataclass(frozen=True)
class DiscoverFactoryValidationTestCase:
    description: str
    repo_files: dict[str, str]
    expected_loader_names: tuple[str, ...] = ()
    expected_asset_names: tuple[str, ...] = ()
    expected_check_names: tuple[str, ...] = ()
    expected_error_fragment: str = ""


@dataclass(frozen=True)
class ProjectPythonPathAcceptanceTestCase:
    description: str
    repo_files: dict[str, str]
    expected_project_name: str


@dataclass(frozen=True)
class DiscoveryLifecycleTestCase:
    description: str
    expected_event_types: tuple[str, ...]
    expected_operation_names: tuple[str, ...]


@dataclass(frozen=True)
class DiscoveryRelevantCountTestCase:
    description: str
    expected_item_count: int
    unexpected_root_pattern: str


@dataclass(frozen=True)
class DiscoverUniqueTestNameTestCase:
    description: str
    repo_files: dict[str, str]
    expected_test_block_names: tuple[str | None, ...]
    expected_case_names: tuple[str, ...]


@dataclass(frozen=True)
class RecallSinkDiscoveryTestCase:
    description: str
    sink_files: dict[str, str]
    expected_replacements: tuple[tuple[tuple[str, str], ...], ...]
    expected_ordinary: tuple[tuple[str, ...], tuple[str, ...]]


@dataclass(frozen=True)
class InvalidRecallSinkTestCase:
    description: str
    project_files: dict[str, str]
    expected_error: type[Exception]
    expected_code: str
    expected_message: str


@dataclass(frozen=True)
class LoadDiscoveryTestCase:
    description: str
    repo_files: dict[str, str]
    expected_loads: tuple[tuple[str, str | None], ...]


@dataclass(frozen=True)
class LoadErrorTestCase:
    description: str
    repo_files: dict[str, str]
    expected_error_class: type[Exception]
    expected_error_fragment: str


@dataclass(frozen=True)
class LoaderSettingsTestCase:
    """A loader's SQLLoad table options, and the loading facts of the source it fills."""

    description: str
    decorator: str
    expected_entry: tuple[SourceWriteStrategy | None, str | None, tuple[str, ...]]
