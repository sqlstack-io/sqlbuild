//! Every adapter entry of `sqlbuild._native_host`.

use crate::entries::models::{HostEdge, HostEntry, HostKind};

/// Every adapter entry, in `AdapterHost` order.
pub(crate) const ENTRIES: &[HostEntry] = &[
    HostEntry {
        name: "BaseAdapter",
        kind: HostKind::Class,
        edge: HostEdge::Adapter,
    },
    HostEntry {
        name: "AdapterUserError",
        kind: HostKind::Exception,
        edge: HostEdge::Adapter,
    },
    HostEntry {
        name: "ColumnInfo",
        kind: HostKind::Class,
        edge: HostEdge::Adapter,
    },
    HostEntry {
        name: "ExpressionInferenceProfile",
        kind: HostKind::Class,
        edge: HostEdge::Adapter,
    },
    HostEntry {
        name: "RelationInfo",
        kind: HostKind::Class,
        edge: HostEdge::Adapter,
    },
    HostEntry {
        name: "RelationLookup",
        kind: HostKind::Class,
        edge: HostEdge::Adapter,
    },
    HostEntry {
        name: "RetentionRequest",
        kind: HostKind::Class,
        edge: HostEdge::Adapter,
    },
    HostEntry {
        name: "RowDiffSampling",
        kind: HostKind::Class,
        edge: HostEdge::Adapter,
    },
    HostEntry {
        name: "RowDiffTolerance",
        kind: HostKind::Class,
        edge: HostEdge::Adapter,
    },
    HostEntry {
        name: "RowDiffTolerances",
        kind: HostKind::Class,
        edge: HostEdge::Adapter,
    },
    HostEntry {
        name: "TableFreshnessMetadata",
        kind: HostKind::Class,
        edge: HostEdge::Adapter,
    },
    HostEntry {
        name: "TableFreshnessRequest",
        kind: HostKind::Class,
        edge: HostEdge::Adapter,
    },
    HostEntry {
        name: "BuiltinAdapter",
        kind: HostKind::Class,
        edge: HostEdge::Adapter,
    },
    HostEntry {
        name: "FrameworkType",
        kind: HostKind::Class,
        edge: HostEdge::Adapter,
    },
    HostEntry {
        name: "LifeCycleEventKind",
        kind: HostKind::Class,
        edge: HostEdge::Adapter,
    },
    HostEntry {
        name: "MigrationTransfer",
        kind: HostKind::Class,
        edge: HostEdge::Adapter,
    },
    HostEntry {
        name: "RetentionScope",
        kind: HostKind::Class,
        edge: HostEdge::Adapter,
    },
    HostEntry {
        name: "TableFreshnessStatus",
        kind: HostKind::Class,
        edge: HostEdge::Adapter,
    },
    HostEntry {
        name: "adapter_supports_session_warehouse",
        kind: HostKind::Function,
        edge: HostEdge::Adapter,
    },
    HostEntry {
        name: "builtin_adapter_classes",
        kind: HostKind::Function,
        edge: HostEdge::Adapter,
    },
    HostEntry {
        name: "discover_project_adapters",
        kind: HostKind::Function,
        edge: HostEdge::Adapter,
    },
    HostEntry {
        name: "resolve_adapter",
        kind: HostKind::Function,
        edge: HostEdge::Adapter,
    },
    HostEntry {
        name: "get_columns_for_inspection",
        kind: HostKind::Function,
        edge: HostEdge::Adapter,
    },
    HostEntry {
        name: "list_relations_for_inspection",
        kind: HostKind::Function,
        edge: HostEdge::Adapter,
    },
    HostEntry {
        name: "active_inspection_catalog",
        kind: HostKind::Function,
        edge: HostEdge::Adapter,
    },
    HostEntry {
        name: "inspection_query_sink",
        kind: HostKind::Function,
        edge: HostEdge::Adapter,
    },
    HostEntry {
        name: "open_inspection_catalog",
        kind: HostKind::Function,
        edge: HostEdge::Adapter,
    },
    HostEntry {
        name: "record_inspection_query",
        kind: HostKind::Function,
        edge: HostEdge::Adapter,
    },
    HostEntry {
        name: "build_relation_lookup",
        kind: HostKind::Function,
        edge: HostEdge::Adapter,
    },
    HostEntry {
        name: "resolve_relation_location_qualified_name",
        kind: HostKind::Function,
        edge: HostEdge::Adapter,
    },
    HostEntry {
        name: "run_bounded_inspections",
        kind: HostKind::Function,
        edge: HostEdge::Adapter,
    },
    HostEntry {
        name: "run_recorded_inspection_query",
        kind: HostKind::Function,
        edge: HostEdge::Adapter,
    },
    HostEntry {
        name: "InspectionQueryRecord",
        kind: HostKind::Class,
        edge: HostEdge::Adapter,
    },
    HostEntry {
        name: "StatementMetadataEffect",
        kind: HostKind::Class,
        edge: HostEdge::Adapter,
    },
    HostEntry {
        name: "conditional_result_nullability",
        kind: HostKind::Function,
        edge: HostEdge::Adapter,
    },
    HostEntry {
        name: "first_arg_nullability",
        kind: HostKind::Function,
        edge: HostEdge::Adapter,
    },
    HostEntry {
        name: "normalize_relation_type",
        kind: HostKind::Function,
        edge: HostEdge::Adapter,
    },
    HostEntry {
        name: "normalize_type",
        kind: HostKind::Function,
        edge: HostEdge::Adapter,
    },
    HostEntry {
        name: "safe_cast_nullability",
        kind: HostKind::Function,
        edge: HostEdge::Adapter,
    },
    HostEntry {
        name: "connect_recall_store",
        kind: HostKind::Function,
        edge: HostEdge::Adapter,
    },
    HostEntry {
        name: "run_recall_query",
        kind: HostKind::Function,
        edge: HostEdge::Adapter,
    },
    HostEntry {
        name: "run_recall_command",
        kind: HostKind::Function,
        edge: HostEdge::Adapter,
    },
    HostEntry {
        name: "insert_recall_rows",
        kind: HostKind::Function,
        edge: HostEdge::Adapter,
    },
    HostEntry {
        name: "close_recall_store",
        kind: HostKind::Function,
        edge: HostEdge::Adapter,
    },
    HostEntry {
        name: "relation_metadata_cache_scope",
        kind: HostKind::Function,
        edge: HostEdge::Adapter,
    },
];
