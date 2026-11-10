from dataclasses import dataclass, field

from sqlbuild._native import (
    BackfillAction,
    BackfillResult,
    ChangeKind,
    InferredColumn,
    PlanAction,
    PlanReason,
    ReferenceRename,
    SchemaDynamicColumnFamily,
    SchemaFinding,
)
from sqlbuild.adapter.contract.models import ColumnInfo


@dataclass(frozen=True)
class DetectModelChangesTestCase:
    description: str
    model_name: str
    query_sql: str
    config_values: dict[str, object]
    schema_columns: tuple[tuple[str, str | None], ...]
    relation_exists: bool
    fingerprint_query_hash: str | None
    warehouse_column_names: tuple[tuple[str, str], ...]
    sql_analysis_enabled: bool
    query_change_tracking: bool
    full_refresh: bool
    expected_change_kind: ChangeKind
    expected_backfill_action: BackfillAction
    fingerprint_config_values: dict[str, object] | None = None
    inferred_columns: tuple[InferredColumn, ...] | None = None
    fast_lineage_has_star: bool = False
    expected_recorded_build_relation_missing: bool = False
    renamed_by_migration: bool = False
    schema_type_enforcement: bool | None = None
    fingerprint_extra_metadata: dict[str, object] = field(default_factory=dict)
    fingerprint_definition: str = "SELECT 1"
    expected_definition_hash: str | None = None


@dataclass(frozen=True)
class DroppedRelationPlanActionTestCase:
    description: str
    config_values: dict[str, object]
    expected_action: PlanAction
    expected_reason: PlanReason


@dataclass(frozen=True)
class DetectModelMetadataTestCase:
    description: str
    config_values: dict[str, object]
    schema_columns: tuple[tuple[str, str | None, bool | None], ...]
    deps: tuple[str, ...]
    function_local_hashes: dict[str, str]
    previous_metadata_json: str
    expected_change_kind: ChangeKind
    expected_metadata_fragments: tuple[str, ...] = ()


@dataclass(frozen=True)
class DetectFunctionCallerChangeTestCase:
    description: str
    metadata_case: DetectModelMetadataTestCase
    query_change_tracking: bool
    expected_changed_functions: tuple[str, ...]
    expected_config_changed: bool
    expected_backfill: BackfillResult


@dataclass(frozen=True)
class DetectSchemaChangesTestCase:
    description: str
    yml_columns: tuple[ColumnInfo, ...]
    inferred_columns: tuple[InferredColumn, ...] | None
    warehouse_columns: tuple[ColumnInfo, ...]
    type_enforcement: bool
    expected_findings: tuple[SchemaFinding, ...]
    inferred_schema_complete: bool = True
    dynamic_columns: tuple[SchemaDynamicColumnFamily, ...] = ()
    dialect: str | None = None


@dataclass(frozen=True)
class DetectRenamedModelTestCase:
    description: str
    previous_definition: str
    config_values: dict[str, object]
    expected_query_changed: bool
    expected_backfill: BackfillResult


@dataclass(frozen=True)
class OriginReferenceNamesTestCase:
    description: str
    query_sql: str
    recorded_query_sql: str
    renames: tuple[ReferenceRename, ...]
    expected_mapping: dict[str, str] | None


@dataclass(frozen=True)
class DeclaredColumnsMetadataTestCase:
    description: str
    metadata_json: str
    expected_recorded_hash: str | None
