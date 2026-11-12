from __future__ import annotations

from dataclasses import dataclass, field

from sqlbuild._native import CompileLineageMode


@dataclass(frozen=True)
class TargetWriterTestCase:
    description: str
    expected_files: dict[str, str] = field(default_factory=dict)
    initial_files: dict[str, str] = field(default_factory=dict)
    model_count: int = 0
    expected_max_component_bytes: int = 0


@dataclass(frozen=True)
class TargetWriterCacheTestCase:
    description: str
    expected_builder_calls: int


@dataclass(frozen=True)
class TargetWriterPlanningErrorTestCase:
    description: str
    expected_builder_calls: int
    expected_message: str


@dataclass(frozen=True)
class CompileCommandTestCase:
    description: str
    expected_exit_code: int
    expected_stdout_fragments: tuple[str, ...]
    model_sql: str | None = None


@dataclass(frozen=True)
class CompileColumnContractModeTestCase:
    description: str
    settings_toml: str
    expected_audit_name: str
    expected_column_name: str
    expected_stderr_fragment: str
    expected_stderr_count: int


@dataclass(frozen=True)
class CompileDagArtifactTestCase:
    description: str
    dag_path: str
    expected_project_name: str
    expected_node_ids: tuple[str, ...]


@dataclass(frozen=True)
class CompilePythonDagArtifactTestCase:
    description: str
    dag_path: str
    expected_project_name: str
    expected_node_ids: set[str]
    expected_edges: set[tuple[str, str]]
    expected_check_ids: set[str]


@dataclass(frozen=True)
class CompileTextOutputTestCase:
    description: str
    model_count: int
    expected_fragments: tuple[str, ...]
    unexpected_fragments: tuple[str, ...] = ()


@dataclass(frozen=True)
class CompileRichLineageOutputTestCase:
    description: str
    expected_fragment: str


@dataclass(frozen=True)
class CompileTextColorTestCase:
    description: str
    expected_fragments: tuple[str, ...]


@dataclass(frozen=True)
class CompileSelectedOutputTestCase:
    description: str
    selected_model_names: frozenset[str]
    expected_fragments: tuple[str, ...]
    unexpected_fragments: tuple[str, ...]


@dataclass(frozen=True)
class CompileSelectedJsonOutputTestCase:
    description: str
    selected_model_names: frozenset[str]
    expected_selected_models: int
    expected_selected_seeds: int
    expected_selected_functions: int


@dataclass(frozen=True)
class CompileJsonExecutionLayersTestCase:
    description: str
    model_count: int
    expected_execution_layers: int


@dataclass(frozen=True)
class CompileJsonLargeIntegerTestCase:
    description: str
    expected_value: int


@dataclass(frozen=True)
class CompileJsonDiagnosticsTestCase:
    description: str
    model_sql: str
    expected_exit_code: int
    expected_code: str
    expected_severity: str
    expected_message: str
    expected_line: int
    expected_column: int


@dataclass(frozen=True)
class CompileLineageModeTestCase:
    description: str
    lineage_mode: CompileLineageMode
    expected_lineage_mode_values: tuple[str, ...]


@dataclass(frozen=True)
class ResolveAdapterTestCase:
    description: str
    adapter_name: str
    expected_adapter_class_name: str


@dataclass(frozen=True)
class ResolveAdapterErrorTestCase:
    description: str
    adapter_name: str
    expected_error_fragment: str


@dataclass(frozen=True)
class ResolveEffectiveAdapterNameTestCase:
    description: str
    project_adapter: str
    local_adapter: str | None
    expected_adapter_name: str


@dataclass(frozen=True)
class ExpectedMessageTestCase:
    description: str
    expected_message: str


@dataclass(frozen=True)
class RuleGatedTestPlanningTestCase:
    description: str
    min_prepared_models: int
    test_header: str
    expected_codes: tuple[str, ...]
    expected_message_fragment: str
    expected_inline_planning_calls: int


@dataclass(frozen=True)
class StagedPublishTestCase:
    description: str
    existing_files: dict[str, str]
    expected_files: dict[str, str]
    expected_removed: tuple[str, ...]


@dataclass(frozen=True)
class TamperedStagedPublishTestCase:
    description: str
    existing_files: dict[str, str]
    removed_staged_files: tuple[str, ...]
    expected_kept_files: dict[str, str]


@dataclass(frozen=True)
class StagingInterleavingTestCase:
    description: str
    existing_files: dict[str, str]
    expected_files: dict[str, str]


@dataclass(frozen=True)
class CompileCyclicCollectionTestCase:
    description: str
    collection_enabled_before: bool
    expected_enabled_during: bool
    expected_enabled_after: bool


@dataclass(frozen=True)
class CompileImportFootprintTestCase:
    description: str
    checked_modules: tuple[str, ...]
    expected_exit_code: int
    expected_loaded_modules: tuple[str, ...]
