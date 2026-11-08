from dataclasses import dataclass
from pathlib import Path

from sqlbuild._native import ScenarioArtifactKind, ScenarioExecutionPlan
from sqlbuild.executor.scenario.models import (
    ScenarioSnapshotCapturePlan,
    ScenarioSnapshotFileStats,
    ScenarioSnapshotInputSpec,
    ScenarioSnapshotManifest,
)
from sqlbuild.executor.scenario.types import ScenarioSnapshotState
from sqlbuild.executor.scheduling.types import ExecutionStatus


@dataclass(frozen=True)
class ScenarioSnapshotPathTestCase:
    description: str
    project_dir: Path
    scenario_name: str
    kind: ScenarioArtifactKind
    logical_name: str
    expected_root: Path
    expected_manifest_path: Path
    expected_relation_path: Path


@dataclass(frozen=True)
class ScenarioSnapshotFingerprintTestCase:
    description: str
    scenario_name: str
    input_specs: tuple[ScenarioSnapshotInputSpec, ...]
    equivalent_input_specs: tuple[ScenarioSnapshotInputSpec, ...]
    changed_input_specs: tuple[ScenarioSnapshotInputSpec, ...]
    expected_matches_equivalent: bool
    expected_differs_from_changed: bool


@dataclass(frozen=True)
class ScenarioSnapshotDialectFingerprintTestCase:
    description: str
    scenario_name: str
    input_specs: tuple[ScenarioSnapshotInputSpec, ...]
    capture_adapter: str
    capture_dialect: str
    changed_capture_adapter: str
    changed_capture_dialect: str
    expected_differs_from_changed: bool


@dataclass(frozen=True)
class ScenarioSnapshotInputSpecsFromPlanTestCase:
    description: str
    scenario_plan: ScenarioExecutionPlan
    expected_input_specs: tuple[ScenarioSnapshotInputSpec, ...]
    changed_check_plan: ScenarioExecutionPlan
    changed_fixture_plan: ScenarioExecutionPlan
    expected_check_fingerprint_matches: bool
    expected_fixture_fingerprint_differs: bool


@dataclass(frozen=True)
class ScenarioSnapshotFreshnessTestCase:
    description: str
    manifest_fingerprint: str
    current_fingerprint: str
    expected_is_fresh: bool


@dataclass(frozen=True)
class ScenarioSnapshotManifestIoTestCase:
    description: str
    manifest: ScenarioSnapshotManifest
    expected_json_fragments: tuple[str, ...]


@dataclass(frozen=True)
class ScenarioSnapshotStateTestCase:
    description: str
    manifest: ScenarioSnapshotManifest | None
    manifest_contents: str | None
    expected_state: ScenarioSnapshotState
    expected_has_manifest: bool
    expected_error_code: str | None = None
    expected_error_fragment: str | None = None


@dataclass(frozen=True)
class ScenarioSnapshotJsonlRoundTripTestCase:
    description: str
    relative_file_path: Path
    rows: tuple[dict[str, object], ...]
    expected_stats: ScenarioSnapshotFileStats
    expected_file_contents: str
    expected_rows: tuple[dict[str, object], ...]


@dataclass(frozen=True)
class ScenarioSnapshotJsonlErrorTestCase:
    description: str
    file_contents: str
    expected_error_fragment: str


@dataclass(frozen=True)
class ScenarioLocalSnapshotLoadTestCase:
    description: str
    columns: tuple[tuple[str, str, str], ...]
    rows: tuple[dict[str, object], ...]
    expected_table_name: str
    expected_summary_row: tuple[object, ...]


@dataclass(frozen=True)
class ScenarioLocalSnapshotLoadErrorTestCase:
    description: str
    columns: tuple[tuple[str, str, str], ...]
    file_contents: str
    expected_error_code: str
    expected_error_fragments: tuple[str, ...]


@dataclass(frozen=True)
class ScenarioSnapshotCapturePlanTestCase:
    description: str
    project_dir: Path
    scenario_plan: ScenarioExecutionPlan
    expected_capture_plan: ScenarioSnapshotCapturePlan
    expected_manifest: ScenarioSnapshotManifest


@dataclass(frozen=True)
class ScenarioSnapshotRelationPathErrorTestCase:
    description: str
    kind: ScenarioArtifactKind
    logical_name: str
    expected_error_type: type[Exception]


@dataclass(frozen=True)
class ScenarioSnapshotLocalTypeMappingTestCase:
    description: str
    sql_analysis_dialect: str | None
    warehouse_type: str
    expected_local_type: str
    local_type_overrides: dict[str, str] | None = None


@dataclass(frozen=True)
class ScenarioSnapshotLocalTypeRefusalTestCase:
    description: str
    sql_analysis_dialect: str | None
    warehouse_type: str
    expected_message: str


@dataclass(frozen=True)
class ExecuteScenarioSnapshotCaptureStepsTestCase:
    description: str
    retain: bool
    fail_on_create_target: str | None
    fail_on_seed: bool
    fail_on_query_target: str | None
    expected_status: ExecutionStatus
    expected_retained: bool
    expected_fixture_result_count: int
    expected_seed_result_count: int
    expected_has_capture_result: bool
    expected_has_cleanup_result: bool
    expected_error_fragment: str | None = None


@dataclass(frozen=True)
class ScenarioCaptureRunIdentityTestCase:
    description: str
    expected_run_id: str
    expected_seed_resource_ids: tuple[str, ...]
    expected_operation_name: str


@dataclass(frozen=True)
class ScenarioCaptureMissingFixtureMessageTestCase:
    description: str
    fixture_error_message: str | None
    expected_error_message: str


@dataclass(frozen=True)
class FirstFailureDetailsTestCase:
    description: str
    step_outcomes: tuple[tuple[ExecutionStatus, str | None, str | None, str | None], ...]
    fallback_code: str | None
    expected_error_code: str | None
    expected_error_help: str | None
    expected_error_message: str | None


@dataclass(frozen=True)
class LocalScenarioSqlTestCase:
    description: str
    source_dialect: str | None
    sql: str
    expected_sql: str


@dataclass(frozen=True)
class ScenarioLocalVariantLoadTestCase:
    """A Snowflake-captured VARIANT JSONL value and the variant type it must load as."""

    description: str
    value: object
    expected_variant_type: str
    expected_matches_parsed_json: bool


@dataclass(frozen=True)
class CanonicalJsonTestCase:
    """JSON text and the canonical text equal values of it share."""

    description: str
    text: str | None
    expected_canonical: str | None
