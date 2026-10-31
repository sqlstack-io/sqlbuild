//! Every executor entry of `sqlbuild._native_host`.

use crate::entries::models::{HostEdge, HostEntry, HostKind};

/// Every executor entry, in `ExecutorHost` order.
pub(crate) const ENTRIES: &[HostEntry] = &[
    HostEntry {
        name: "ProjectRelation",
        kind: HostKind::Class,
        edge: HostEdge::Executor,
    },
    HostEntry {
        name: "ProjectRelationIndex",
        kind: HostKind::Class,
        edge: HostEdge::Executor,
    },
    HostEntry {
        name: "RelationName",
        kind: HostKind::Class,
        edge: HostEdge::Executor,
    },
    HostEntry {
        name: "CostResourceContext",
        kind: HostKind::Class,
        edge: HostEdge::Executor,
    },
    HostEntry {
        name: "RunCostSummary",
        kind: HostKind::Class,
        edge: HostEdge::Executor,
    },
    HostEntry {
        name: "CostCapability",
        kind: HostKind::Class,
        edge: HostEdge::Executor,
    },
    HostEntry {
        name: "CostStatus",
        kind: HostKind::Class,
        edge: HostEdge::Executor,
    },
    HostEntry {
        name: "BoundSentinel",
        kind: HostKind::Class,
        edge: HostEdge::Executor,
    },
    HostEntry {
        name: "read_process_resources",
        kind: HostKind::Function,
        edge: HostEdge::Executor,
    },
    HostEntry {
        name: "SQL_PRIVATE_RECORD_MARKER",
        kind: HostKind::Value,
        edge: HostEdge::Executor,
    },
    HostEntry {
        name: "diagnostics_context",
        kind: HostKind::Function,
        edge: HostEdge::Executor,
    },
    HostEntry {
        name: "log_debug_event",
        kind: HostKind::Function,
        edge: HostEdge::Executor,
    },
    HostEntry {
        name: "log_sql",
        kind: HostKind::Function,
        edge: HostEdge::Executor,
    },
    HostEntry {
        name: "process_resource_reporting",
        kind: HostKind::Function,
        edge: HostEdge::Executor,
    },
    HostEntry {
        name: "ProcessResourceUsage",
        kind: HostKind::Class,
        edge: HostEdge::Executor,
    },
    HostEntry {
        name: "ExecutorInputError",
        kind: HostKind::Exception,
        edge: HostEdge::Executor,
    },
    HostEntry {
        name: "current_audit_result_projection",
        kind: HostKind::Function,
        edge: HostEdge::Executor,
    },
    HostEntry {
        name: "AuditExecutionResult",
        kind: HostKind::Class,
        edge: HostEdge::Executor,
    },
    HostEntry {
        name: "BuildCallbacks",
        kind: HostKind::Class,
        edge: HostEdge::Executor,
    },
    HostEntry {
        name: "BuildCustomizations",
        kind: HostKind::Class,
        edge: HostEdge::Executor,
    },
    HostEntry {
        name: "BuildExecutionResult",
        kind: HostKind::Class,
        edge: HostEdge::Executor,
    },
    HostEntry {
        name: "BuildInitialState",
        kind: HostKind::Class,
        edge: HostEdge::Executor,
    },
    HostEntry {
        name: "BuildRuntimeParams",
        kind: HostKind::Class,
        edge: HostEdge::Executor,
    },
    HostEntry {
        name: "FunctionExecutionResult",
        kind: HostKind::Class,
        edge: HostEdge::Executor,
    },
    HostEntry {
        name: "SeedExecutionResult",
        kind: HostKind::Class,
        edge: HostEdge::Executor,
    },
    HostEntry {
        name: "BuildStatus",
        kind: HostKind::Class,
        edge: HostEdge::Executor,
    },
    HostEntry {
        name: "build_destination_transient_models",
        kind: HostKind::Function,
        edge: HostEdge::Executor,
    },
    HostEntry {
        name: "build_destination_retention_requests",
        kind: HostKind::Function,
        edge: HostEdge::Executor,
    },
    HostEntry {
        name: "copy_clone_fingerprints",
        kind: HostKind::Function,
        edge: HostEdge::Executor,
    },
    HostEntry {
        name: "run_prephase_clone_stream",
        kind: HostKind::Function,
        edge: HostEdge::Executor,
    },
    HostEntry {
        name: "execute_clone",
        kind: HostKind::Function,
        edge: HostEdge::Executor,
    },
    HostEntry {
        name: "CloneExecutionInput",
        kind: HostKind::Class,
        edge: HostEdge::Executor,
    },
    HostEntry {
        name: "CloneItemResult",
        kind: HostKind::Class,
        edge: HostEdge::Executor,
    },
    HostEntry {
        name: "CloneSourceEntries",
        kind: HostKind::Class,
        edge: HostEdge::Executor,
    },
    HostEntry {
        name: "CloneStatus",
        kind: HostKind::Class,
        edge: HostEdge::Executor,
    },
    HostEntry {
        name: "QueryDiffArtifactLifecycle",
        kind: HostKind::Class,
        edge: HostEdge::Executor,
    },
    HostEntry {
        name: "FullDiffSizeGuardError",
        kind: HostKind::Exception,
        edge: HostEdge::Executor,
    },
    HostEntry {
        name: "execute_query_diff",
        kind: HostKind::Function,
        edge: HostEdge::Executor,
    },
    HostEntry {
        name: "execute_diff",
        kind: HostKind::Function,
        edge: HostEdge::Executor,
    },
    HostEntry {
        name: "DiffExecutionOptions",
        kind: HostKind::Class,
        edge: HostEdge::Executor,
    },
    HostEntry {
        name: "DiffExecutionResult",
        kind: HostKind::Class,
        edge: HostEdge::Executor,
    },
    HostEntry {
        name: "FullDiffSizeLimits",
        kind: HostKind::Class,
        edge: HostEdge::Executor,
    },
    HostEntry {
        name: "RowDiffSamplingOverride",
        kind: HostKind::Class,
        edge: HostEdge::Executor,
    },
    HostEntry {
        name: "execute_janitor_plan",
        kind: HostKind::Function,
        edge: HostEdge::Executor,
    },
    HostEntry {
        name: "JanitorArchiveCandidate",
        kind: HostKind::Class,
        edge: HostEdge::Executor,
    },
    HostEntry {
        name: "JanitorArchivedRelation",
        kind: HostKind::Class,
        edge: HostEdge::Executor,
    },
    HostEntry {
        name: "JanitorBlockedSchema",
        kind: HostKind::Class,
        edge: HostEdge::Executor,
    },
    HostEntry {
        name: "JanitorDeleteCandidate",
        kind: HostKind::Class,
        edge: HostEdge::Executor,
    },
    HostEntry {
        name: "JanitorDirectStatePruneCandidate",
        kind: HostKind::Class,
        edge: HostEdge::Executor,
    },
    HostEntry {
        name: "JanitorOldNameView",
        kind: HostKind::Class,
        edge: HostEdge::Executor,
    },
    HostEntry {
        name: "JanitorOldNameViewPlanning",
        kind: HostKind::Class,
        edge: HostEdge::Executor,
    },
    HostEntry {
        name: "JanitorPlan",
        kind: HostKind::Class,
        edge: HostEdge::Executor,
    },
    HostEntry {
        name: "JanitorQueryDiffArtifactCandidate",
        kind: HostKind::Class,
        edge: HostEdge::Executor,
    },
    HostEntry {
        name: "JanitorRelationKey",
        kind: HostKind::Class,
        edge: HostEdge::Executor,
    },
    HostEntry {
        name: "JanitorSkippedRelation",
        kind: HostKind::Class,
        edge: HostEdge::Executor,
    },
    HostEntry {
        name: "JanitorSkippedSchema",
        kind: HostKind::Class,
        edge: HostEdge::Executor,
    },
    HostEntry {
        name: "JanitorUsageCheck",
        kind: HostKind::Class,
        edge: HostEdge::Executor,
    },
    HostEntry {
        name: "run_load_pipeline",
        kind: HostKind::Function,
        edge: HostEdge::Executor,
    },
    HostEntry {
        name: "LoadCallbacks",
        kind: HostKind::Class,
        edge: HostEdge::Executor,
    },
    HostEntry {
        name: "LoadExecutionResult",
        kind: HostKind::Class,
        edge: HostEdge::Executor,
    },
    HostEntry {
        name: "LoadRuntimeParams",
        kind: HostKind::Class,
        edge: HostEdge::Executor,
    },
    HostEntry {
        name: "migration_relation_for_location",
        kind: HostKind::Function,
        edge: HostEdge::Executor,
    },
    HostEntry {
        name: "is_compatibility_view",
        kind: HostKind::Function,
        edge: HostEdge::Executor,
    },
    HostEntry {
        name: "AuditExecutionError",
        kind: HostKind::Exception,
        edge: HostEdge::Executor,
    },
    HostEntry {
        name: "WarehouseConnectionError",
        kind: HostKind::Exception,
        edge: HostEdge::Executor,
    },
    HostEntry {
        name: "TemporaryTestFunctionError",
        kind: HostKind::Exception,
        edge: HostEdge::Executor,
    },
    HostEntry {
        name: "raise_warehouse_connection_failure",
        kind: HostKind::Function,
        edge: HostEdge::Executor,
    },
    HostEntry {
        name: "AuditPipelineCallbacks",
        kind: HostKind::Class,
        edge: HostEdge::Executor,
    },
    HostEntry {
        name: "run_audit_pipeline_with_callbacks",
        kind: HostKind::Function,
        edge: HostEdge::Executor,
    },
    HostEntry {
        name: "run_build_pipeline",
        kind: HostKind::Function,
        edge: HostEdge::Executor,
    },
    HostEntry {
        name: "run_scenario_capture_pipeline",
        kind: HostKind::Function,
        edge: HostEdge::Executor,
    },
    HostEntry {
        name: "run_scenario_local_test_pipeline",
        kind: HostKind::Function,
        edge: HostEdge::Executor,
    },
    HostEntry {
        name: "run_scenario_test_pipeline",
        kind: HostKind::Function,
        edge: HostEdge::Executor,
    },
    HostEntry {
        name: "run_seed_pipeline",
        kind: HostKind::Function,
        edge: HostEdge::Executor,
    },
    HostEntry {
        name: "run_test_pipeline",
        kind: HostKind::Function,
        edge: HostEdge::Executor,
    },
    HostEntry {
        name: "select_scenario_snapshot_capture_candidates",
        kind: HostKind::Function,
        edge: HostEdge::Executor,
    },
    HostEntry {
        name: "TestPipelineCallbacks",
        kind: HostKind::Class,
        edge: HostEdge::Executor,
    },
    HostEntry {
        name: "RunItemTimeouts",
        kind: HostKind::Class,
        edge: HostEdge::Executor,
    },
    HostEntry {
        name: "LocalTestOptions",
        kind: HostKind::Class,
        edge: HostEdge::Executor,
    },
    HostEntry {
        name: "ScenarioTestSettings",
        kind: HostKind::Class,
        edge: HostEdge::Executor,
    },
    HostEntry {
        name: "execute_python_checks",
        kind: HostKind::Function,
        edge: HostEdge::Executor,
    },
    HostEntry {
        name: "execute_ingress_python_loader_nodes",
        kind: HostKind::Function,
        edge: HostEdge::Executor,
    },
    HostEntry {
        name: "run_ingress_python_loader_nodes",
        kind: HostKind::Function,
        edge: HostEdge::Executor,
    },
    HostEntry {
        name: "create_read_side_python_execution_tracker",
        kind: HostKind::Function,
        edge: HostEdge::Executor,
    },
    HostEntry {
        name: "IngressCallbacks",
        kind: HostKind::Class,
        edge: HostEdge::Executor,
    },
    HostEntry {
        name: "PythonCheckCallbacks",
        kind: HostKind::Class,
        edge: HostEdge::Executor,
    },
    HostEntry {
        name: "PythonCheckExecutionResult",
        kind: HostKind::Class,
        edge: HostEdge::Executor,
    },
    HostEntry {
        name: "PythonNodeExecutionResult",
        kind: HostKind::Class,
        edge: HostEdge::Executor,
    },
    HostEntry {
        name: "PythonNodeRunState",
        kind: HostKind::Class,
        edge: HostEdge::Executor,
    },
    HostEntry {
        name: "PythonNodeRuntime",
        kind: HostKind::Class,
        edge: HostEdge::Executor,
    },
    HostEntry {
        name: "ModelExecutionResult",
        kind: HostKind::Class,
        edge: HostEdge::Executor,
    },
    HostEntry {
        name: "ScenarioCaptureSettings",
        kind: HostKind::Class,
        edge: HostEdge::Executor,
    },
    HostEntry {
        name: "ScenarioLocalReplaySource",
        kind: HostKind::Class,
        edge: HostEdge::Executor,
    },
    HostEntry {
        name: "ScenarioSnapshotCaptureLimits",
        kind: HostKind::Class,
        edge: HostEdge::Executor,
    },
    HostEntry {
        name: "ExecutionStatus",
        kind: HostKind::Class,
        edge: HostEdge::Executor,
    },
    HostEntry {
        name: "SqlTestExecutionResult",
        kind: HostKind::Class,
        edge: HostEdge::Executor,
    },
    HostEntry {
        name: "MicrobatchEventCodec",
        kind: HostKind::Class,
        edge: HostEdge::Executor,
    },
    HostEntry {
        name: "MICROBATCH_COLUMNS",
        kind: HostKind::Value,
        edge: HostEdge::Executor,
    },
    HostEntry {
        name: "MICROBATCH_COLUMN_TYPES",
        kind: HostKind::Value,
        edge: HostEdge::Executor,
    },
    HostEntry {
        name: "MICROBATCH_DIRECT_INDEXES",
        kind: HostKind::Value,
        edge: HostEdge::Executor,
    },
    HostEntry {
        name: "MICROBATCH_GENERATION_WILDCARD",
        kind: HostKind::Value,
        edge: HostEdge::Executor,
    },
    HostEntry {
        name: "MICROBATCH_TABLE_NAME",
        kind: HostKind::Value,
        edge: HostEdge::Executor,
    },
    HostEntry {
        name: "MicrobatchStateError",
        kind: HostKind::Exception,
        edge: HostEdge::Executor,
    },
    HostEntry {
        name: "MicrobatchCoverageProjection",
        kind: HostKind::Class,
        edge: HostEdge::Executor,
    },
    HostEntry {
        name: "MicrobatchInterval",
        kind: HostKind::Class,
        edge: HostEdge::Executor,
    },
    HostEntry {
        name: "ProjectedMicrobatchInterval",
        kind: HostKind::Class,
        edge: HostEdge::Executor,
    },
    HostEntry {
        name: "ReplayRequirementProjection",
        kind: HostKind::Class,
        edge: HostEdge::Executor,
    },
    HostEntry {
        name: "ReplayRequirementState",
        kind: HostKind::Class,
        edge: HostEdge::Executor,
    },
    HostEntry {
        name: "TextComputeLogTee",
        kind: HostKind::Class,
        edge: HostEdge::Executor,
    },
    HostEntry {
        name: "CaptureAlreadyExistsError",
        kind: HostKind::Exception,
        edge: HostEdge::Executor,
    },
    HostEntry {
        name: "CaptureNotFoundError",
        kind: HostKind::Exception,
        edge: HostEdge::Executor,
    },
    HostEntry {
        name: "CaptureStateError",
        kind: HostKind::Exception,
        edge: HostEdge::Executor,
    },
    HostEntry {
        name: "ComputeLogMetadataError",
        kind: HostKind::Exception,
        edge: HostEdge::Executor,
    },
    HostEntry {
        name: "ComputeLogPathError",
        kind: HostKind::Exception,
        edge: HostEdge::Executor,
    },
    HostEntry {
        name: "InvalidCaptureIdError",
        kind: HostKind::Exception,
        edge: HostEdge::Executor,
    },
    HostEntry {
        name: "InvalidComputeLogCursorError",
        kind: HostKind::Exception,
        edge: HostEdge::Executor,
    },
    HostEntry {
        name: "InvalidComputeLogLimitError",
        kind: HostKind::Exception,
        edge: HostEdge::Executor,
    },
    HostEntry {
        name: "CaptureByteCounts",
        kind: HostKind::Class,
        edge: HostEdge::Executor,
    },
    HostEntry {
        name: "CaptureInventory",
        kind: HostKind::Class,
        edge: HostEdge::Executor,
    },
    HostEntry {
        name: "CaptureInventoryItem",
        kind: HostKind::Class,
        edge: HostEdge::Executor,
    },
    HostEntry {
        name: "CaptureMetadata",
        kind: HostKind::Class,
        edge: HostEdge::Executor,
    },
    HostEntry {
        name: "ComputeLogReadChunk",
        kind: HostKind::Class,
        edge: HostEdge::Executor,
    },
    HostEntry {
        name: "FinalCaptureMetadata",
        kind: HostKind::Class,
        edge: HostEdge::Executor,
    },
    HostEntry {
        name: "PruneResult",
        kind: HostKind::Class,
        edge: HostEdge::Executor,
    },
    HostEntry {
        name: "ComputeLogStream",
        kind: HostKind::Class,
        edge: HostEdge::Executor,
    },
    HostEntry {
        name: "ConnectionHooks",
        kind: HostKind::Class,
        edge: HostEdge::Executor,
    },
    HostEntry {
        name: "ExecutionResourceKind",
        kind: HostKind::Class,
        edge: HostEdge::Executor,
    },
    HostEntry {
        name: "ExecutionHistoryStorageError",
        kind: HostKind::Exception,
        edge: HostEdge::Executor,
    },
    HostEntry {
        name: "IntegrityConflictError",
        kind: HostKind::Exception,
        edge: HostEdge::Executor,
    },
    HostEntry {
        name: "InvalidCursorError",
        kind: HostKind::Exception,
        edge: HostEdge::Executor,
    },
    HostEntry {
        name: "InvalidEventError",
        kind: HostKind::Exception,
        edge: HostEdge::Executor,
    },
    HostEntry {
        name: "InvalidFilterError",
        kind: HostKind::Exception,
        edge: HostEdge::Executor,
    },
    HostEntry {
        name: "InvalidLimitError",
        kind: HostKind::Exception,
        edge: HostEdge::Executor,
    },
    HostEntry {
        name: "ProjectionConsistencyError",
        kind: HostKind::Exception,
        edge: HostEdge::Executor,
    },
    HostEntry {
        name: "UnsupportedSchemaVersionError",
        kind: HostKind::Exception,
        edge: HostEdge::Executor,
    },
    HostEntry {
        name: "EventPage",
        kind: HostKind::Class,
        edge: HostEdge::Executor,
    },
    HostEntry {
        name: "RunPage",
        kind: HostKind::Class,
        edge: HostEdge::Executor,
    },
    HostEntry {
        name: "RunRecord",
        kind: HostKind::Class,
        edge: HostEdge::Executor,
    },
    HostEntry {
        name: "StoredEvent",
        kind: HostKind::Class,
        edge: HostEdge::Executor,
    },
    HostEntry {
        name: "RunStatus",
        kind: HostKind::Class,
        edge: HostEdge::Executor,
    },
    HostEntry {
        name: "ExecutionDurationLimitError",
        kind: HostKind::Exception,
        edge: HostEdge::Executor,
    },
    HostEntry {
        name: "execution_deadline_scope",
        kind: HostKind::Function,
        edge: HostEdge::Executor,
    },
    HostEntry {
        name: "audit_evidence_scope",
        kind: HostKind::Function,
        edge: HostEdge::Executor,
    },
    HostEntry {
        name: "cancel_running_statements",
        kind: HostKind::Function,
        edge: HostEdge::Executor,
    },
    HostEntry {
        name: "EventDispatcher",
        kind: HostKind::Class,
        edge: HostEdge::Executor,
    },
    HostEntry {
        name: "OperationLifecycle",
        kind: HostKind::Class,
        edge: HostEdge::Executor,
    },
    HostEntry {
        name: "StatementLifecycle",
        kind: HostKind::Class,
        edge: HostEdge::Executor,
    },
    HostEntry {
        name: "ObservabilityValidationError",
        kind: HostKind::Exception,
        edge: HostEdge::Executor,
    },
    HostEntry {
        name: "create_lifecycle_event",
        kind: HostKind::Function,
        edge: HostEdge::Executor,
    },
    HostEntry {
        name: "current_execution_identity",
        kind: HostKind::Function,
        edge: HostEdge::Executor,
    },
    HostEntry {
        name: "diagnostic_log_to_json",
        kind: HostKind::Function,
        edge: HostEdge::Executor,
    },
    HostEntry {
        name: "dispatcher_scope",
        kind: HostKind::Function,
        edge: HostEdge::Executor,
    },
    HostEntry {
        name: "invocation_external_context_scope",
        kind: HostKind::Function,
        edge: HostEdge::Executor,
    },
    HostEntry {
        name: "invocation_scope",
        kind: HostKind::Function,
        edge: HostEdge::Executor,
    },
    HostEntry {
        name: "lifecycle_event_from_json",
        kind: HostKind::Function,
        edge: HostEdge::Executor,
    },
    HostEntry {
        name: "lifecycle_event_to_json",
        kind: HostKind::Function,
        edge: HostEdge::Executor,
    },
    HostEntry {
        name: "run_scope",
        kind: HostKind::Function,
        edge: HostEdge::Executor,
    },
    HostEntry {
        name: "DiagnosticLog",
        kind: HostKind::Class,
        edge: HostEdge::Executor,
    },
    HostEntry {
        name: "LifecycleEvent",
        kind: HostKind::Class,
        edge: HostEdge::Executor,
    },
    HostEntry {
        name: "OperationAttributes",
        kind: HostKind::Class,
        edge: HostEdge::Executor,
    },
    HostEntry {
        name: "CommandOutputValidationError",
        kind: HostKind::Exception,
        edge: HostEdge::Executor,
    },
    HostEntry {
        name: "OutputCaptureInputError",
        kind: HostKind::Exception,
        edge: HostEdge::Executor,
    },
    HostEntry {
        name: "configured_output_capture_scope",
        kind: HostKind::Function,
        edge: HostEdge::Executor,
    },
    HostEntry {
        name: "BoundCommandOutputSink",
        kind: HostKind::Class,
        edge: HostEdge::Executor,
    },
    HostEntry {
        name: "CommandOutputCaptureSummary",
        kind: HostKind::Class,
        edge: HostEdge::Executor,
    },
    HostEntry {
        name: "CommandOutputRecord",
        kind: HostKind::Class,
        edge: HostEdge::Executor,
    },
    HostEntry {
        name: "OutputCaptureContext",
        kind: HostKind::Class,
        edge: HostEdge::Executor,
    },
    HostEntry {
        name: "CommandOutputStream",
        kind: HostKind::Class,
        edge: HostEdge::Executor,
    },
    HostEntry {
        name: "OutputRecordPriority",
        kind: HostKind::Class,
        edge: HostEdge::Executor,
    },
    HostEntry {
        name: "SqlValueRenderingError",
        kind: HostKind::Exception,
        edge: HostEdge::Executor,
    },
    HostEntry {
        name: "create_review_schema",
        kind: HostKind::Function,
        edge: HostEdge::Executor,
    },
    HostEntry {
        name: "drop_review_schema",
        kind: HostKind::Function,
        edge: HostEdge::Executor,
    },
    HostEntry {
        name: "leftover_review_schemas",
        kind: HostKind::Function,
        edge: HostEdge::Executor,
    },
];
