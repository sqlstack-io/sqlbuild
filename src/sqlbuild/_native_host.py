"""The only module native code imports by path: staying-Python entry points by name."""

from __future__ import annotations

import importlib

_TARGETS: dict[str, tuple[str, str]] = {
    "BaseAdapter": ("sqlbuild.adapter.contract.classes.base_adapter", "BaseAdapter"),
    "AdapterUserError": ("sqlbuild.adapter.contract.exceptions", "AdapterUserError"),
    "ColumnInfo": ("sqlbuild.adapter.contract.models", "ColumnInfo"),
    "ExpressionInferenceProfile": (
        "sqlbuild.adapter.contract.models",
        "ExpressionInferenceProfile",
    ),
    "RelationInfo": ("sqlbuild.adapter.contract.models", "RelationInfo"),
    "RelationLookup": ("sqlbuild.adapter.contract.models", "RelationLookup"),
    "RetentionRequest": ("sqlbuild.adapter.contract.models", "RetentionRequest"),
    "RowDiffSampling": ("sqlbuild.adapter.contract.models", "RowDiffSampling"),
    "RowDiffTolerance": ("sqlbuild.adapter.contract.models", "RowDiffTolerance"),
    "RowDiffTolerances": ("sqlbuild.adapter.contract.models", "RowDiffTolerances"),
    "TableFreshnessMetadata": ("sqlbuild.adapter.contract.models", "TableFreshnessMetadata"),
    "TableFreshnessRequest": ("sqlbuild.adapter.contract.models", "TableFreshnessRequest"),
    "BuiltinAdapter": ("sqlbuild.adapter.contract.types", "BuiltinAdapter"),
    "FrameworkType": ("sqlbuild.adapter.contract.types", "FrameworkType"),
    "LifeCycleEventKind": ("sqlbuild.adapter.contract.types", "LifeCycleEventKind"),
    "MigrationTransfer": ("sqlbuild.adapter.contract.types", "MigrationTransfer"),
    "RetentionScope": ("sqlbuild.adapter.contract.types", "RetentionScope"),
    "TableFreshnessStatus": ("sqlbuild.adapter.contract.types", "TableFreshnessStatus"),
    "adapter_supports_session_warehouse": (
        "sqlbuild.adapter.discovery.main._session_warehouse_support",
        "adapter_supports_session_warehouse",
    ),
    "builtin_adapter_classes": (
        "sqlbuild.adapter.discovery.main.builtins",
        "builtin_adapter_classes",
    ),
    "discover_project_adapters": (
        "sqlbuild.adapter.discovery.main.project_adapters",
        "discover_project_adapters",
    ),
    "resolve_adapter": ("sqlbuild.adapter.discovery.main.resolve_adapter", "resolve_adapter"),
    "get_columns_for_inspection": (
        "sqlbuild.adapter.relations.main._get_columns_for_inspection",
        "get_columns_for_inspection",
    ),
    "list_relations_for_inspection": (
        "sqlbuild.adapter.relations.main._list_relations_for_inspection",
        "list_relations_for_inspection",
    ),
    "active_inspection_catalog": (
        "sqlbuild.adapter.relations.main.active_inspection_catalog",
        "active_inspection_catalog",
    ),
    "inspection_query_sink": (
        "sqlbuild.adapter.relations.main.inspection_query_sink",
        "inspection_query_sink",
    ),
    "open_inspection_catalog": (
        "sqlbuild.adapter.relations.main.open_inspection_catalog",
        "open_inspection_catalog",
    ),
    "record_inspection_query": (
        "sqlbuild.adapter.relations.main.record_inspection_query",
        "record_inspection_query",
    ),
    "build_relation_lookup": (
        "sqlbuild.adapter.relations.main.relation_lookup",
        "build_relation_lookup",
    ),
    "resolve_relation_location_qualified_name": (
        "sqlbuild.adapter.relations.main.resolve_relation_location_qualified_name",
        "resolve_relation_location_qualified_name",
    ),
    "run_bounded_inspections": (
        "sqlbuild.adapter.relations.main.run_bounded_inspections",
        "run_bounded_inspections",
    ),
    "run_recorded_inspection_query": (
        "sqlbuild.adapter.relations.main.run_recorded_inspection_query",
        "run_recorded_inspection_query",
    ),
    "InspectionQueryRecord": ("sqlbuild.adapter.relations.models", "InspectionQueryRecord"),
    "StatementMetadataEffect": ("sqlbuild.adapter.relations.models", "StatementMetadataEffect"),
    "conditional_result_nullability": (
        "sqlbuild.adapter.type_system.main.conditional_result_nullability",
        "conditional_result_nullability",
    ),
    "first_arg_nullability": (
        "sqlbuild.adapter.type_system.main.first_arg_nullability",
        "first_arg_nullability",
    ),
    "normalize_relation_type": (
        "sqlbuild.adapter.type_system.main.normalize_relation_type",
        "normalize_relation_type",
    ),
    "normalize_type": ("sqlbuild.adapter.type_system.main.normalize_type", "normalize_type"),
    "safe_cast_nullability": (
        "sqlbuild.adapter.type_system.main.safe_cast_nullability",
        "safe_cast_nullability",
    ),
    "connect_recall_store": (
        "sqlbuild.adapters.clickhouse._helpers.recall_store",
        "connect_recall_store",
    ),
    "run_recall_query": ("sqlbuild.adapters.clickhouse._helpers.recall_store", "run_recall_query"),
    "run_recall_command": (
        "sqlbuild.adapters.clickhouse._helpers.recall_store",
        "run_recall_command",
    ),
    "insert_recall_rows": (
        "sqlbuild.adapters.clickhouse._helpers.recall_store",
        "insert_recall_rows",
    ),
    "close_recall_store": (
        "sqlbuild.adapters.clickhouse._helpers.recall_store",
        "close_recall_store",
    ),
    "relation_metadata_cache_scope": (
        "sqlbuild.adapter.relations.main.relation_metadata_cache_scope",
        "relation_metadata_cache_scope",
    ),
    "python_resource_rows": (
        "sqlbuild.compiler.compile._helpers.diagnostics.python_resources",
        "python_resource_rows",
    ),
    "target_context_with_macro_call_store": (
        "sqlbuild.compiler.compile._helpers.macro_bridge.call_store",
        "target_context_with_macro_call_store",
    ),
    "inventory_project_macros": (
        "sqlbuild.compiler.compile._helpers.render.user_macros",
        "inventory_project_macros",
    ),
    "load_project_macros": (
        "sqlbuild.compiler.compile._helpers.render.user_macros",
        "load_project_macros",
    ),
    "macro_invocation_context": (
        "sqlbuild.compiler.compile._helpers.render.user_macros",
        "macro_invocation_context",
    ),
    "discover_command_output_sink_functions": (
        "sqlbuild.compiler.discovery._helpers.filesystem.command_output_sinks",
        "discover_command_output_sink_functions",
    ),
    "import_python_root_module": (
        "sqlbuild.compiler.discovery._helpers.filesystem.project_modules",
        "import_python_root_module",
    ),
    "install_python_root_package": (
        "sqlbuild.compiler.discovery._helpers.filesystem.project_modules",
        "install_python_root_package",
    ),
    "load_materialize_function": (
        "sqlbuild.compiler.discovery._helpers.filesystem.project_modules",
        "load_materialize_function",
    ),
    "load_provider_module": (
        "sqlbuild.compiler.discovery._helpers.filesystem.project_modules",
        "load_provider_module",
    ),
    "load_python_node_module": (
        "sqlbuild.compiler.discovery._helpers.filesystem.project_modules",
        "load_python_node_module",
    ),
    "load_sink_module": (
        "sqlbuild.compiler.discovery._helpers.filesystem.project_modules",
        "load_sink_module",
    ),
    "load_orchestration_module": (
        "sqlbuild.compiler.discovery._helpers.filesystem.project_modules",
        "load_orchestration_module",
    ),
    "module_functions": (
        "sqlbuild.compiler.discovery._helpers.filesystem.project_modules",
        "module_functions",
    ),
    "module_provider_classes": (
        "sqlbuild.compiler.discovery._helpers.filesystem.project_modules",
        "module_provider_classes",
    ),
    "parameter_annotations": (
        "sqlbuild.compiler.discovery._helpers.filesystem.project_modules",
        "parameter_annotations",
    ),
    "provider_usages": (
        "sqlbuild.compiler.discovery._helpers.filesystem.project_modules",
        "provider_usages",
    ),
    "requires_arguments": (
        "sqlbuild.compiler.discovery._helpers.filesystem.project_modules",
        "requires_arguments",
    ),
    "restore_python_root_modules": (
        "sqlbuild.compiler.discovery._helpers.filesystem.project_modules",
        "restore_python_root_modules",
    ),
    "source_line": (
        "sqlbuild.compiler.discovery._helpers.filesystem.project_modules",
        "source_line",
    ),
    "validate_event_exporter_signature": (
        "sqlbuild.compiler.discovery._helpers.filesystem.project_modules",
        "validate_event_exporter_signature",
    ),
    "validate_discovered_inputs": (
        "sqlbuild.compiler.discovery._helpers.validation.discovery",
        "validate_discovered_inputs",
    ),
    "discover_runtime_extensions": (
        "sqlbuild.compiler.discovery.main.runtime_extensions",
        "discover_runtime_extensions",
    ),
    "run_with_macro_bridge": (
        "sqlbuild.compiler.macro_bridge.classes.macro_bridge",
        "run_with_macro_bridge",
    ),
    "macro_store_module_paths": (
        "sqlbuild.compiler.macro_bridge.main._macro_store_module_paths",
        "macro_store_module_paths",
    ),
    "active_macro_bridge": (
        "sqlbuild.compiler.macro_bridge.main.active_macro_bridge",
        "active_macro_bridge",
    ),
    "load_custom_materializations": (
        "sqlbuild.compiler.pipeline._helpers.materializations",
        "load_custom_materializations",
    ),
    "python_node_dependency_key": (
        "sqlbuild.compiler.python_nodes._helpers.user_inventory",
        "python_node_dependency_key",
    ),
    "python_node_identity": (
        "sqlbuild.compiler.python_nodes._helpers.user_inventory",
        "python_node_identity",
    ),
    "build_hook_identities": (
        "sqlbuild.compiler.python_nodes.main._hook_identities",
        "build_hook_identities",
    ),
    "PythonNodeKind": ("sqlbuild.compiler.python_nodes.types", "PythonNodeKind"),
    "PythonNodeStatus": ("sqlbuild.compiler.python_nodes.types", "PythonNodeStatus"),
    "ProviderInputError": ("sqlbuild.provider.exceptions", "ProviderInputError"),
    "build_provider_session": ("sqlbuild.provider.main.session", "build_provider_session"),
    "read_asset_definition": (
        "sqlbuild.python_nodes.main.read_asset_definition",
        "read_asset_definition",
    ),
    "read_audit_factory_definition": (
        "sqlbuild.python_nodes.main.read_audit_factory_definition",
        "read_audit_factory_definition",
    ),
    "read_check_definition": (
        "sqlbuild.python_nodes.main.read_check_definition",
        "read_check_definition",
    ),
    "read_factory_definition": (
        "sqlbuild.python_nodes.main.read_factory_definition",
        "read_factory_definition",
    ),
    "read_hook_definition": (
        "sqlbuild.python_nodes.main.read_hook_definition",
        "read_hook_definition",
    ),
    "read_loader_definition": (
        "sqlbuild.python_nodes.main.read_loader_definition",
        "read_loader_definition",
    ),
    "read_task_definition": (
        "sqlbuild.python_nodes.main.read_task_definition",
        "read_task_definition",
    ),
    "AuditCase": ("sqlbuild.python_nodes.models", "AuditCase"),
    "RELATION_RENDERER": ("sqlbuild.python_nodes.models", "RELATION_RENDERER"),
    "SqlResourceRef": ("sqlbuild.python_nodes.models", "SqlResourceRef"),
    "PythonCheckSeverity": ("sqlbuild.python_nodes.types", "PythonCheckSeverity"),
    "SqlResourceRefKind": ("sqlbuild.python_nodes.types", "SqlResourceRefKind"),
    "load_custom_rules": (
        "sqlbuild.rule_engine._helpers.engine._custom_catalogue",
        "load_custom_rules",
    ),
    "custom_rule_payloads": (
        "sqlbuild.rule_engine._helpers.engine.custom_payloads",
        "custom_rule_payloads",
    ),
    "EventExporterInputError": (
        "sqlbuild.runtime.event_exporting.exceptions",
        "EventExporterInputError",
    ),
    "EventExporterStateError": (
        "sqlbuild.runtime.event_exporting.exceptions",
        "EventExporterStateError",
    ),
    "invoke_lifecycle_event_sink": (
        "sqlbuild.runtime.event_exporting.main._invoke_lifecycle_event_sink",
        "invoke_lifecycle_event_sink",
    ),
    "invoke_recall_sink": (
        "sqlbuild.runtime.event_exporting.main._invoke_recall_sink",
        "invoke_recall_sink",
    ),
    "run_command_output_sinks": (
        "sqlbuild.runtime.event_exporting.main._run_command_output_sinks",
        "run_command_output_sinks",
    ),
    "get_lifecycle_event_sink_definition": (
        "sqlbuild.runtime.event_exporting.main.get_lifecycle_event_sink_definition",
        "get_lifecycle_event_sink_definition",
    ),
    "ProjectRelation": ("sqlbuild.compiler.references.models", "ProjectRelation"),
    "ProjectRelationIndex": ("sqlbuild.compiler.references.models", "ProjectRelationIndex"),
    "RelationName": ("sqlbuild.compiler.references.models", "RelationName"),
    "CostResourceContext": ("sqlbuild.cost.models", "CostResourceContext"),
    "RunCostSummary": ("sqlbuild.cost.models", "RunCostSummary"),
    "CostCapability": ("sqlbuild.cost.types", "CostCapability"),
    "CostStatus": ("sqlbuild.cost.types", "CostStatus"),
    "BoundSentinel": ("sqlbuild.cursor_algebra.types", "BoundSentinel"),
    "read_process_resources": (
        "sqlbuild.diagnostics._helpers.process_resources",
        "read_process_resources",
    ),
    "SQL_PRIVATE_RECORD_MARKER": ("sqlbuild.diagnostics.constants", "SQL_PRIVATE_RECORD_MARKER"),
    "diagnostics_context": ("sqlbuild.diagnostics.main.diagnostics_context", "diagnostics_context"),
    "log_debug_event": ("sqlbuild.diagnostics.main.log_debug_event", "log_debug_event"),
    "log_sql": ("sqlbuild.diagnostics.main.log_sql", "log_sql"),
    "process_resource_reporting": (
        "sqlbuild.diagnostics.main.process_resource_reporting",
        "process_resource_reporting",
    ),
    "ProcessResourceUsage": ("sqlbuild.diagnostics.models", "ProcessResourceUsage"),
    "ExecutorInputError": ("sqlbuild.errors.contracts.exceptions", "ExecutorInputError"),
    "current_audit_result_projection": (
        "sqlbuild.executor.auditing.main._current_audit_result_projection",
        "current_audit_result_projection",
    ),
    "AuditExecutionResult": ("sqlbuild.executor.auditing.models", "AuditExecutionResult"),
    "BuildCallbacks": ("sqlbuild.executor.build.models", "BuildCallbacks"),
    "BuildCustomizations": ("sqlbuild.executor.build.models", "BuildCustomizations"),
    "BuildExecutionResult": ("sqlbuild.executor.build.models", "BuildExecutionResult"),
    "BuildInitialState": ("sqlbuild.executor.build.models", "BuildInitialState"),
    "BuildRuntimeParams": ("sqlbuild.executor.build.models", "BuildRuntimeParams"),
    "FunctionExecutionResult": ("sqlbuild.executor.build.models", "FunctionExecutionResult"),
    "SeedExecutionResult": ("sqlbuild.executor.build.models", "SeedExecutionResult"),
    "BuildStatus": ("sqlbuild.executor.build.types", "BuildStatus"),
    "build_destination_transient_models": (
        "sqlbuild.executor.clone.main._build_destination_transient_models",
        "build_destination_transient_models",
    ),
    "build_destination_retention_requests": (
        "sqlbuild.executor.clone.main._build_retention_requests",
        "build_destination_retention_requests",
    ),
    "copy_clone_fingerprints": (
        "sqlbuild.executor.clone.main._fingerprinting",
        "copy_clone_fingerprints",
    ),
    "run_prephase_clone_stream": (
        "sqlbuild.executor.clone.main._run_prephase_clone_stream",
        "run_prephase_clone_stream",
    ),
    "execute_clone": ("sqlbuild.executor.clone.main.execute", "execute_clone"),
    "CloneExecutionInput": ("sqlbuild.executor.clone.models", "CloneExecutionInput"),
    "CloneItemResult": ("sqlbuild.executor.clone.models", "CloneItemResult"),
    "CloneSourceEntries": ("sqlbuild.executor.clone.models", "CloneSourceEntries"),
    "CloneStatus": ("sqlbuild.executor.clone.types", "CloneStatus"),
    "QueryDiffArtifactLifecycle": (
        "sqlbuild.executor.diff.classes.query_artifact_lifecycle",
        "QueryDiffArtifactLifecycle",
    ),
    "create_review_schema": (
        "sqlbuild.executor.review.main.create_review_schema",
        "create_review_schema",
    ),
    "drop_review_schema": (
        "sqlbuild.executor.review.main.drop_review_schema",
        "drop_review_schema",
    ),
    "leftover_review_schemas": (
        "sqlbuild.executor.review.main.leftover_review_schemas",
        "leftover_review_schemas",
    ),
    "FullDiffSizeGuardError": ("sqlbuild.executor.diff.exceptions", "FullDiffSizeGuardError"),
    "execute_query_diff": ("sqlbuild.executor.diff.main._execute_query", "execute_query_diff"),
    "execute_diff": ("sqlbuild.executor.diff.main.execute", "execute_diff"),
    "DiffExecutionOptions": ("sqlbuild.executor.diff.models", "DiffExecutionOptions"),
    "DiffExecutionResult": ("sqlbuild.executor.diff.models", "DiffExecutionResult"),
    "FullDiffSizeLimits": ("sqlbuild.executor.diff.models", "FullDiffSizeLimits"),
    "RowDiffSamplingOverride": ("sqlbuild.executor.diff.models", "RowDiffSamplingOverride"),
    "execute_janitor_plan": ("sqlbuild.executor.janitor.main.execute", "execute_janitor_plan"),
    "JanitorArchiveCandidate": ("sqlbuild.executor.janitor.models", "JanitorArchiveCandidate"),
    "JanitorArchivedRelation": ("sqlbuild.executor.janitor.models", "JanitorArchivedRelation"),
    "JanitorBlockedSchema": ("sqlbuild.executor.janitor.models", "JanitorBlockedSchema"),
    "JanitorDeleteCandidate": ("sqlbuild.executor.janitor.models", "JanitorDeleteCandidate"),
    "JanitorDirectStatePruneCandidate": (
        "sqlbuild.executor.janitor.models",
        "JanitorDirectStatePruneCandidate",
    ),
    "JanitorOldNameView": ("sqlbuild.executor.janitor.models", "JanitorOldNameView"),
    "JanitorOldNameViewPlanning": (
        "sqlbuild.executor.janitor.models",
        "JanitorOldNameViewPlanning",
    ),
    "JanitorPlan": ("sqlbuild.executor.janitor.models", "JanitorPlan"),
    "JanitorQueryDiffArtifactCandidate": (
        "sqlbuild.executor.janitor.models",
        "JanitorQueryDiffArtifactCandidate",
    ),
    "JanitorRelationKey": ("sqlbuild.executor.janitor.models", "JanitorRelationKey"),
    "JanitorSkippedRelation": ("sqlbuild.executor.janitor.models", "JanitorSkippedRelation"),
    "JanitorSkippedSchema": ("sqlbuild.executor.janitor.models", "JanitorSkippedSchema"),
    "JanitorUsageCheck": ("sqlbuild.executor.janitor.models", "JanitorUsageCheck"),
    "run_load_pipeline": ("sqlbuild.executor.load.main.run", "run_load_pipeline"),
    "LoadCallbacks": ("sqlbuild.executor.load.models", "LoadCallbacks"),
    "LoadExecutionResult": ("sqlbuild.executor.load.models", "LoadExecutionResult"),
    "LoadRuntimeParams": ("sqlbuild.executor.load.models", "LoadRuntimeParams"),
    "migration_relation_for_location": (
        "sqlbuild.executor.migrations._helpers.relation_for_location",
        "migration_relation_for_location",
    ),
    "is_compatibility_view": (
        "sqlbuild.executor.old_name_views._helpers.janitor",
        "is_compatibility_view",
    ),
    "AuditExecutionError": ("sqlbuild.executor.pipeline.exceptions", "AuditExecutionError"),
    "WarehouseConnectionError": (
        "sqlbuild.executor.pipeline.exceptions",
        "WarehouseConnectionError",
    ),
    "TemporaryTestFunctionError": (
        "sqlbuild.executor.pipeline.exceptions",
        "TemporaryTestFunctionError",
    ),
    "raise_warehouse_connection_failure": (
        "sqlbuild.executor.pipeline.main.raise_warehouse_connection_failure",
        "raise_warehouse_connection_failure",
    ),
    "AuditPipelineCallbacks": ("sqlbuild.executor.pipeline.main.run", "AuditPipelineCallbacks"),
    "run_audit_pipeline_with_callbacks": (
        "sqlbuild.executor.pipeline.main.run",
        "run_audit_pipeline_with_callbacks",
    ),
    "run_build_pipeline": ("sqlbuild.executor.pipeline.main.run", "run_build_pipeline"),
    "run_scenario_capture_pipeline": (
        "sqlbuild.executor.pipeline.main.run",
        "run_scenario_capture_pipeline",
    ),
    "run_scenario_local_test_pipeline": (
        "sqlbuild.executor.pipeline.main.run",
        "run_scenario_local_test_pipeline",
    ),
    "run_scenario_test_pipeline": (
        "sqlbuild.executor.pipeline.main.run",
        "run_scenario_test_pipeline",
    ),
    "run_seed_pipeline": ("sqlbuild.executor.pipeline.main.run", "run_seed_pipeline"),
    "run_test_pipeline": ("sqlbuild.executor.pipeline.main.run", "run_test_pipeline"),
    "select_scenario_snapshot_capture_candidates": (
        "sqlbuild.executor.pipeline.main.run",
        "select_scenario_snapshot_capture_candidates",
    ),
    "TestPipelineCallbacks": ("sqlbuild.executor.pipeline.models", "TestPipelineCallbacks"),
    "RunItemTimeouts": ("sqlbuild.runtime.execution_limits.models", "RunItemTimeouts"),
    "ScenarioTestSettings": ("sqlbuild.executor.scenario.models", "ScenarioTestSettings"),
    "execute_python_checks": (
        "sqlbuild.executor.python_nodes.main._checks",
        "execute_python_checks",
    ),
    "execute_ingress_python_loader_nodes": (
        "sqlbuild.executor.python_nodes.main._ingress",
        "execute_ingress_python_loader_nodes",
    ),
    "run_ingress_python_loader_nodes": (
        "sqlbuild.executor.python_nodes.main._ingress",
        "run_ingress_python_loader_nodes",
    ),
    "create_read_side_python_execution_tracker": (
        "sqlbuild.executor.python_nodes.main.read_side",
        "create_read_side_python_execution_tracker",
    ),
    "IngressCallbacks": ("sqlbuild.executor.python_nodes.models", "IngressCallbacks"),
    "PythonCheckCallbacks": ("sqlbuild.executor.python_nodes.models", "PythonCheckCallbacks"),
    "PythonCheckExecutionResult": (
        "sqlbuild.executor.python_nodes.models",
        "PythonCheckExecutionResult",
    ),
    "PythonNodeExecutionResult": (
        "sqlbuild.executor.python_nodes.models",
        "PythonNodeExecutionResult",
    ),
    "PythonNodeRunState": ("sqlbuild.executor.python_nodes.models", "PythonNodeRunState"),
    "PythonNodeRuntime": ("sqlbuild.executor.python_nodes.models", "PythonNodeRuntime"),
    "ModelExecutionResult": ("sqlbuild.executor.run.models", "ModelExecutionResult"),
    "ScenarioCaptureSettings": ("sqlbuild.executor.scenario.models", "ScenarioCaptureSettings"),
    "ScenarioLocalReplaySource": ("sqlbuild.executor.scenario.models", "ScenarioLocalReplaySource"),
    "ScenarioSnapshotCaptureLimits": (
        "sqlbuild.executor.scenario.models",
        "ScenarioSnapshotCaptureLimits",
    ),
    "ExecutionStatus": ("sqlbuild.executor.scheduling.types", "ExecutionStatus"),
    "SqlTestExecutionResult": ("sqlbuild.executor.testing.models", "SqlTestExecutionResult"),
    "MicrobatchEventCodec": ("sqlbuild.microbatches.classes.event_codec", "MicrobatchEventCodec"),
    "MICROBATCH_COLUMNS": ("sqlbuild.microbatches.constants", "MICROBATCH_COLUMNS"),
    "MICROBATCH_COLUMN_TYPES": ("sqlbuild.microbatches.constants", "MICROBATCH_COLUMN_TYPES"),
    "MICROBATCH_DIRECT_INDEXES": ("sqlbuild.microbatches.constants", "MICROBATCH_DIRECT_INDEXES"),
    "MICROBATCH_GENERATION_WILDCARD": (
        "sqlbuild.microbatches.constants",
        "MICROBATCH_GENERATION_WILDCARD",
    ),
    "MICROBATCH_TABLE_NAME": ("sqlbuild.microbatches.constants", "MICROBATCH_TABLE_NAME"),
    "MicrobatchStateError": ("sqlbuild.microbatches.exceptions", "MicrobatchStateError"),
    "MicrobatchCoverageProjection": (
        "sqlbuild.microbatches.models",
        "MicrobatchCoverageProjection",
    ),
    "MicrobatchInterval": ("sqlbuild.microbatches.models", "MicrobatchInterval"),
    "ProjectedMicrobatchInterval": ("sqlbuild.microbatches.models", "ProjectedMicrobatchInterval"),
    "ReplayRequirementProjection": ("sqlbuild.microbatches.models", "ReplayRequirementProjection"),
    "ReplayRequirementState": ("sqlbuild.microbatches.types", "ReplayRequirementState"),
    "TextComputeLogTee": ("sqlbuild.runtime.compute_logs.classes.text_tee", "TextComputeLogTee"),
    "CaptureAlreadyExistsError": (
        "sqlbuild.runtime.compute_logs.exceptions",
        "CaptureAlreadyExistsError",
    ),
    "CaptureNotFoundError": ("sqlbuild.runtime.compute_logs.exceptions", "CaptureNotFoundError"),
    "CaptureStateError": ("sqlbuild.runtime.compute_logs.exceptions", "CaptureStateError"),
    "ComputeLogMetadataError": (
        "sqlbuild.runtime.compute_logs.exceptions",
        "ComputeLogMetadataError",
    ),
    "ComputeLogPathError": ("sqlbuild.runtime.compute_logs.exceptions", "ComputeLogPathError"),
    "InvalidCaptureIdError": ("sqlbuild.runtime.compute_logs.exceptions", "InvalidCaptureIdError"),
    "InvalidComputeLogCursorError": (
        "sqlbuild.runtime.compute_logs.exceptions",
        "InvalidComputeLogCursorError",
    ),
    "InvalidComputeLogLimitError": (
        "sqlbuild.runtime.compute_logs.exceptions",
        "InvalidComputeLogLimitError",
    ),
    "CaptureByteCounts": ("sqlbuild.runtime.compute_logs.models", "CaptureByteCounts"),
    "CaptureInventory": ("sqlbuild.runtime.compute_logs.models", "CaptureInventory"),
    "CaptureInventoryItem": ("sqlbuild.runtime.compute_logs.models", "CaptureInventoryItem"),
    "CaptureMetadata": ("sqlbuild.runtime.compute_logs.models", "CaptureMetadata"),
    "ComputeLogReadChunk": ("sqlbuild.runtime.compute_logs.models", "ComputeLogReadChunk"),
    "FinalCaptureMetadata": ("sqlbuild.runtime.compute_logs.models", "FinalCaptureMetadata"),
    "PruneResult": ("sqlbuild.runtime.compute_logs.models", "PruneResult"),
    "ComputeLogStream": ("sqlbuild.runtime.compute_logs.types", "ComputeLogStream"),
    "ConnectionHooks": ("sqlbuild.runtime.contracts.models", "ConnectionHooks"),
    "ExecutionResourceKind": ("sqlbuild.runtime.contracts.types", "ExecutionResourceKind"),
    "ExecutionHistoryStorageError": (
        "sqlbuild.runtime.execution_history.exceptions",
        "ExecutionHistoryStorageError",
    ),
    "IntegrityConflictError": (
        "sqlbuild.runtime.execution_history.exceptions",
        "IntegrityConflictError",
    ),
    "InvalidCursorError": ("sqlbuild.runtime.execution_history.exceptions", "InvalidCursorError"),
    "InvalidEventError": ("sqlbuild.runtime.execution_history.exceptions", "InvalidEventError"),
    "InvalidFilterError": ("sqlbuild.runtime.execution_history.exceptions", "InvalidFilterError"),
    "InvalidLimitError": ("sqlbuild.runtime.execution_history.exceptions", "InvalidLimitError"),
    "ProjectionConsistencyError": (
        "sqlbuild.runtime.execution_history.exceptions",
        "ProjectionConsistencyError",
    ),
    "UnsupportedSchemaVersionError": (
        "sqlbuild.runtime.execution_history.exceptions",
        "UnsupportedSchemaVersionError",
    ),
    "EventPage": ("sqlbuild.runtime.execution_history.models", "EventPage"),
    "RunPage": ("sqlbuild.runtime.execution_history.models", "RunPage"),
    "RunRecord": ("sqlbuild.runtime.execution_history.models", "RunRecord"),
    "StoredEvent": ("sqlbuild.runtime.execution_history.models", "StoredEvent"),
    "RunStatus": ("sqlbuild.runtime.execution_history.types", "RunStatus"),
    "ExecutionDurationLimitError": (
        "sqlbuild.runtime.execution_limits.exceptions",
        "ExecutionDurationLimitError",
    ),
    "execution_deadline_scope": (
        "sqlbuild.runtime.execution_limits.main.execution_deadline_scope",
        "execution_deadline_scope",
    ),
    "audit_evidence_scope": (
        "sqlbuild.executor.auditing.main._audit_evidence_scope",
        "audit_evidence_scope",
    ),
    "cancel_running_statements": (
        "sqlbuild.runtime.execution_limits.main._cancel_running_statements",
        "cancel_running_statements",
    ),
    "EventDispatcher": (
        "sqlbuild.runtime.observability.classes.event_dispatcher",
        "EventDispatcher",
    ),
    "OperationLifecycle": (
        "sqlbuild.runtime.observability.classes.operation_lifecycle",
        "OperationLifecycle",
    ),
    "StatementLifecycle": (
        "sqlbuild.runtime.observability.classes.statement_lifecycle",
        "StatementLifecycle",
    ),
    "ObservabilityValidationError": (
        "sqlbuild.runtime.observability.exceptions",
        "ObservabilityValidationError",
    ),
    "create_lifecycle_event": (
        "sqlbuild.runtime.observability.main.create_lifecycle_event",
        "create_lifecycle_event",
    ),
    "current_execution_identity": (
        "sqlbuild.runtime.observability.main.current_execution_identity",
        "current_execution_identity",
    ),
    "diagnostic_log_to_json": (
        "sqlbuild.runtime.observability.main.diagnostic_log_to_json",
        "diagnostic_log_to_json",
    ),
    "dispatcher_scope": (
        "sqlbuild.runtime.observability.main.dispatcher_scope",
        "dispatcher_scope",
    ),
    "invocation_external_context_scope": (
        "sqlbuild.runtime.observability.main.invocation_external_context_scope",
        "invocation_external_context_scope",
    ),
    "invocation_scope": (
        "sqlbuild.runtime.observability.main.invocation_scope",
        "invocation_scope",
    ),
    "lifecycle_event_from_json": (
        "sqlbuild.runtime.observability.main.lifecycle_event_from_json",
        "lifecycle_event_from_json",
    ),
    "lifecycle_event_to_json": (
        "sqlbuild.runtime.observability.main.lifecycle_event_to_json",
        "lifecycle_event_to_json",
    ),
    "run_scope": ("sqlbuild.runtime.observability.main.run_scope", "run_scope"),
    "DiagnosticLog": ("sqlbuild.runtime.observability.models", "DiagnosticLog"),
    "LifecycleEvent": ("sqlbuild.runtime.observability.models", "LifecycleEvent"),
    "OperationAttributes": ("sqlbuild.runtime.observability.models", "OperationAttributes"),
    "CommandOutputValidationError": (
        "sqlbuild.runtime.output_capture.exceptions",
        "CommandOutputValidationError",
    ),
    "OutputCaptureInputError": (
        "sqlbuild.runtime.output_capture.exceptions",
        "OutputCaptureInputError",
    ),
    "configured_output_capture_scope": (
        "sqlbuild.runtime.output_capture.main.configured_output_capture_scope",
        "configured_output_capture_scope",
    ),
    "BoundCommandOutputSink": ("sqlbuild.runtime.output_capture.models", "BoundCommandOutputSink"),
    "CommandOutputCaptureSummary": (
        "sqlbuild.runtime.output_capture.models",
        "CommandOutputCaptureSummary",
    ),
    "CommandOutputRecord": ("sqlbuild.runtime.output_capture.models", "CommandOutputRecord"),
    "OutputCaptureContext": ("sqlbuild.runtime.output_capture.models", "OutputCaptureContext"),
    "CommandOutputStream": ("sqlbuild.runtime.output_capture.types", "CommandOutputStream"),
    "OutputRecordPriority": ("sqlbuild.runtime.output_capture.types", "OutputRecordPriority"),
    "SqlValueRenderingError": ("sqlbuild.sql_values.exceptions", "SqlValueRenderingError"),
    "ASSET_SELECTION_COMMANDS": (
        "sqlbuild.integrations.dagster.constants",
        "ASSET_SELECTION_COMMANDS",
    ),
    "CHECK_METADATA_EXCLUDED_KEYS": (
        "sqlbuild.integrations.dagster.constants",
        "CHECK_METADATA_EXCLUDED_KEYS",
    ),
    "CLONE_COMMAND": ("sqlbuild.integrations.dagster.constants", "CLONE_COMMAND"),
    "COMPLETED_EXECUTION_STATUSES": (
        "sqlbuild.integrations.dagster.constants",
        "COMPLETED_EXECUTION_STATUSES",
    ),
    "DAGSTER_ASSET_ENABLED_FIELD": (
        "sqlbuild.integrations.dagster.constants",
        "DAGSTER_ASSET_ENABLED_FIELD",
    ),
    "DAGSTER_ASSET_NODE_KINDS": (
        "sqlbuild.integrations.dagster.constants",
        "DAGSTER_ASSET_NODE_KINDS",
    ),
    "DAGSTER_CHECK_ENABLED_FIELD": (
        "sqlbuild.integrations.dagster.constants",
        "DAGSTER_CHECK_ENABLED_FIELD",
    ),
    "DAGSTER_CHECK_NAME_FIELD": (
        "sqlbuild.integrations.dagster.constants",
        "DAGSTER_CHECK_NAME_FIELD",
    ),
    "DAGSTER_DIRECT_KIND_NODE_KINDS": (
        "sqlbuild.integrations.dagster.constants",
        "DAGSTER_DIRECT_KIND_NODE_KINDS",
    ),
    "DEFAULT_SELECTABLE_NODE_KINDS": (
        "sqlbuild.integrations.dagster.constants",
        "DEFAULT_SELECTABLE_NODE_KINDS",
    ),
    "EXPLICIT_SELECTION_FLAGS": (
        "sqlbuild.integrations.dagster.constants",
        "EXPLICIT_SELECTION_FLAGS",
    ),
    "INSUFFICIENT_CHECK_STATUS": (
        "sqlbuild.integrations.dagster.constants",
        "INSUFFICIENT_CHECK_STATUS",
    ),
    "LIVE_EVENT_COMMANDS": ("sqlbuild.integrations.dagster.constants", "LIVE_EVENT_COMMANDS"),
    "LOADER_NODE_KIND": ("sqlbuild.integrations.dagster.constants", "LOADER_NODE_KIND"),
    "LOAD_SELECTABLE_NODE_KINDS": (
        "sqlbuild.integrations.dagster.constants",
        "LOAD_SELECTABLE_NODE_KINDS",
    ),
    "MATERIALIZABLE_NODE_KINDS": (
        "sqlbuild.integrations.dagster.constants",
        "MATERIALIZABLE_NODE_KINDS",
    ),
    "MODEL_NODE_KIND": ("sqlbuild.integrations.dagster.constants", "MODEL_NODE_KIND"),
    "SCENARIO_CHECK_KIND": ("sqlbuild.integrations.dagster.constants", "SCENARIO_CHECK_KIND"),
    "SOURCE_NODE_KIND": ("sqlbuild.integrations.dagster.constants", "SOURCE_NODE_KIND"),
    "STANDALONE_CHECK_COMMANDS": (
        "sqlbuild.integrations.dagster.constants",
        "STANDALONE_CHECK_COMMANDS",
    ),
    "SUCCESS_EXECUTION_STATUS": (
        "sqlbuild.integrations.dagster.constants",
        "SUCCESS_EXECUTION_STATUS",
    ),
    "VIEW_MATERIALIZATION_TYPE": (
        "sqlbuild.integrations.dagster.constants",
        "VIEW_MATERIALIZATION_TYPE",
    ),
    "WARNING_CHECK_SEVERITY": ("sqlbuild.integrations.dagster.constants", "WARNING_CHECK_SEVERITY"),
    "DagsterDagInputError": ("sqlbuild.integrations.dagster.exceptions", "DagsterDagInputError"),
    "load_dbt_yaml": ("sqlbuild.integrations.dbt._helpers.profile.load", "load_dbt_yaml"),
    "render_selected_dbt_profile_output": (
        "sqlbuild.integrations.dbt._helpers.profile.render",
        "render_selected_dbt_profile_output",
    ),
    "DbtProfileError": ("sqlbuild.integrations.dbt.exceptions", "DbtProfileError"),
    "DLT_INTEGRATION_KIND": ("sqlbuild.integrations.dlt.constants", "DLT_INTEGRATION_KIND"),
    "DltIntegrationError": ("sqlbuild.integrations.dlt.exceptions", "DltIntegrationError"),
    "dlt_loader_function": ("sqlbuild.integrations.dlt.main._loaders", "dlt_loader_function"),
    "DltResourceConfig": ("sqlbuild.integrations.dlt.models", "DltResourceConfig"),
    "DltSourceConfig": ("sqlbuild.integrations.dlt.models", "DltSourceConfig"),
    "INGESTR_INTEGRATION_KIND": (
        "sqlbuild.integrations.ingestr.constants",
        "INGESTR_INTEGRATION_KIND",
    ),
    "IngestrIntegrationError": (
        "sqlbuild.integrations.ingestr.exceptions",
        "IngestrIntegrationError",
    ),
    "ingestr_loader_function": (
        "sqlbuild.integrations.ingestr.main._loaders",
        "ingestr_loader_function",
    ),
    "IngestrSourceConfig": ("sqlbuild.integrations.ingestr.models", "IngestrSourceConfig"),
    "RIVERS_ASSET_NODE_KINDS": (
        "sqlbuild.integrations.rivers.constants",
        "RIVERS_ASSET_NODE_KINDS",
    ),
    "package_file": ("sqlbuild", "__file__"),
    "dispatch_with_compute_logs": (
        "sqlbuild.cli.commands.main.entrypoint._dispatch_with_compute_logs",
        "dispatch_with_compute_logs",
    ),
    "provider_settings_inputs": (
        "sqlbuild.cli.compile_reuse._helpers.provider_settings",
        "provider_settings_inputs",
    ),
    "startup_inputs": ("sqlbuild.cli.compile_reuse._helpers.startup_inputs", "startup_inputs"),
    "CompileReuseRequest": ("sqlbuild.cli.compile_reuse.models", "CompileReuseRequest"),
    "AuditEvaluationMode": ("sqlbuild.compiler.auditing.types", "AuditEvaluationMode"),
    "AuditOutcome": ("sqlbuild.compiler.auditing.types", "AuditOutcome"),
    "AuditSeverity": ("sqlbuild.compiler.auditing.types", "AuditSeverity"),
    "ThresholdOperator": ("sqlbuild.compiler.auditing.types", "ThresholdOperator"),
    "AttachedAuditTargetKind": ("sqlbuild.compiler.compile.types", "AttachedAuditTargetKind"),
    "FunctionLanguage": ("sqlbuild.compiler.compile.types", "FunctionLanguage"),
    "SqlTestMode": ("sqlbuild.compiler.compile.types", "SqlTestMode"),
    "CursorAlgebraError": ("sqlbuild.cursor_algebra.exceptions", "CursorAlgebraError"),
    "SharedInputError": ("sqlbuild.errors.contracts.exceptions", "SharedInputError"),
}

__all__ = ("host_targets", *_TARGETS)


def host_targets() -> dict[str, tuple[str, str]]:
    """Each host name with the module and attribute it resolves to."""
    return dict(_TARGETS)


def __getattr__(name: str) -> object:
    """Resolve `name` from its target module on every read."""
    target: tuple[str, str] | None = _TARGETS.get(name)
    if target is None:
        raise AttributeError(f"module {__name__!r} has no attribute {name!r}")
    module, attribute = target
    return getattr(importlib.import_module(module), attribute)
