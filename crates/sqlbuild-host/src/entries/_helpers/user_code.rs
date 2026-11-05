//! Every user code entry of `sqlbuild._native_host`.

use crate::entries::models::{HostEdge, HostEntry, HostKind};

/// Every user-code entry, in `UserCodeHost` order.
pub(crate) const ENTRIES: &[HostEntry] = &[
    HostEntry {
        name: "python_resource_rows",
        kind: HostKind::Function,
        edge: HostEdge::UserCode,
    },
    HostEntry {
        name: "target_context_with_macro_call_store",
        kind: HostKind::Function,
        edge: HostEdge::UserCode,
    },
    HostEntry {
        name: "inventory_project_macros",
        kind: HostKind::Function,
        edge: HostEdge::UserCode,
    },
    HostEntry {
        name: "load_project_macros",
        kind: HostKind::Function,
        edge: HostEdge::UserCode,
    },
    HostEntry {
        name: "macro_invocation_context",
        kind: HostKind::Function,
        edge: HostEdge::UserCode,
    },
    HostEntry {
        name: "discover_command_output_sink_functions",
        kind: HostKind::Function,
        edge: HostEdge::UserCode,
    },
    HostEntry {
        name: "import_python_root_module",
        kind: HostKind::Function,
        edge: HostEdge::UserCode,
    },
    HostEntry {
        name: "install_python_root_package",
        kind: HostKind::Function,
        edge: HostEdge::UserCode,
    },
    HostEntry {
        name: "load_materialize_function",
        kind: HostKind::Function,
        edge: HostEdge::UserCode,
    },
    HostEntry {
        name: "load_provider_module",
        kind: HostKind::Function,
        edge: HostEdge::UserCode,
    },
    HostEntry {
        name: "load_python_node_module",
        kind: HostKind::Function,
        edge: HostEdge::UserCode,
    },
    HostEntry {
        name: "load_sink_module",
        kind: HostKind::Function,
        edge: HostEdge::UserCode,
    },
    HostEntry {
        name: "load_orchestration_module",
        kind: HostKind::Function,
        edge: HostEdge::UserCode,
    },
    HostEntry {
        name: "module_functions",
        kind: HostKind::Function,
        edge: HostEdge::UserCode,
    },
    HostEntry {
        name: "module_provider_classes",
        kind: HostKind::Function,
        edge: HostEdge::UserCode,
    },
    HostEntry {
        name: "parameter_annotations",
        kind: HostKind::Function,
        edge: HostEdge::UserCode,
    },
    HostEntry {
        name: "provider_usages",
        kind: HostKind::Function,
        edge: HostEdge::UserCode,
    },
    HostEntry {
        name: "requires_arguments",
        kind: HostKind::Function,
        edge: HostEdge::UserCode,
    },
    HostEntry {
        name: "restore_python_root_modules",
        kind: HostKind::Function,
        edge: HostEdge::UserCode,
    },
    HostEntry {
        name: "source_line",
        kind: HostKind::Function,
        edge: HostEdge::UserCode,
    },
    HostEntry {
        name: "validate_event_exporter_signature",
        kind: HostKind::Function,
        edge: HostEdge::UserCode,
    },
    HostEntry {
        name: "validate_discovered_inputs",
        kind: HostKind::Function,
        edge: HostEdge::UserCode,
    },
    HostEntry {
        name: "discover_runtime_extensions",
        kind: HostKind::Function,
        edge: HostEdge::UserCode,
    },
    HostEntry {
        name: "run_with_macro_bridge",
        kind: HostKind::Function,
        edge: HostEdge::UserCode,
    },
    HostEntry {
        name: "macro_store_module_paths",
        kind: HostKind::Function,
        edge: HostEdge::UserCode,
    },
    HostEntry {
        name: "active_macro_bridge",
        kind: HostKind::Function,
        edge: HostEdge::UserCode,
    },
    HostEntry {
        name: "load_custom_materializations",
        kind: HostKind::Function,
        edge: HostEdge::UserCode,
    },
    HostEntry {
        name: "python_node_dependency_key",
        kind: HostKind::Function,
        edge: HostEdge::UserCode,
    },
    HostEntry {
        name: "python_node_identity",
        kind: HostKind::Function,
        edge: HostEdge::UserCode,
    },
    HostEntry {
        name: "build_hook_identities",
        kind: HostKind::Function,
        edge: HostEdge::UserCode,
    },
    HostEntry {
        name: "PythonNodeKind",
        kind: HostKind::Class,
        edge: HostEdge::UserCode,
    },
    HostEntry {
        name: "PythonNodeStatus",
        kind: HostKind::Class,
        edge: HostEdge::UserCode,
    },
    HostEntry {
        name: "ProviderInputError",
        kind: HostKind::Exception,
        edge: HostEdge::UserCode,
    },
    HostEntry {
        name: "build_provider_session",
        kind: HostKind::Function,
        edge: HostEdge::UserCode,
    },
    HostEntry {
        name: "read_asset_definition",
        kind: HostKind::Function,
        edge: HostEdge::UserCode,
    },
    HostEntry {
        name: "read_audit_factory_definition",
        kind: HostKind::Function,
        edge: HostEdge::UserCode,
    },
    HostEntry {
        name: "read_check_definition",
        kind: HostKind::Function,
        edge: HostEdge::UserCode,
    },
    HostEntry {
        name: "read_factory_definition",
        kind: HostKind::Function,
        edge: HostEdge::UserCode,
    },
    HostEntry {
        name: "read_hook_definition",
        kind: HostKind::Function,
        edge: HostEdge::UserCode,
    },
    HostEntry {
        name: "read_loader_definition",
        kind: HostKind::Function,
        edge: HostEdge::UserCode,
    },
    HostEntry {
        name: "read_task_definition",
        kind: HostKind::Function,
        edge: HostEdge::UserCode,
    },
    HostEntry {
        name: "AuditCase",
        kind: HostKind::Class,
        edge: HostEdge::UserCode,
    },
    HostEntry {
        name: "RELATION_RENDERER",
        kind: HostKind::Value,
        edge: HostEdge::UserCode,
    },
    HostEntry {
        name: "SqlResourceRef",
        kind: HostKind::Class,
        edge: HostEdge::UserCode,
    },
    HostEntry {
        name: "PythonCheckSeverity",
        kind: HostKind::Class,
        edge: HostEdge::UserCode,
    },
    HostEntry {
        name: "SqlResourceRefKind",
        kind: HostKind::Class,
        edge: HostEdge::UserCode,
    },
    HostEntry {
        name: "load_custom_rules",
        kind: HostKind::Function,
        edge: HostEdge::UserCode,
    },
    HostEntry {
        name: "custom_rule_payloads",
        kind: HostKind::Function,
        edge: HostEdge::UserCode,
    },
    HostEntry {
        name: "EventExporterInputError",
        kind: HostKind::Exception,
        edge: HostEdge::UserCode,
    },
    HostEntry {
        name: "EventExporterStateError",
        kind: HostKind::Exception,
        edge: HostEdge::UserCode,
    },
    HostEntry {
        name: "invoke_lifecycle_event_sink",
        kind: HostKind::Function,
        edge: HostEdge::UserCode,
    },
    HostEntry {
        name: "run_command_output_sinks",
        kind: HostKind::Function,
        edge: HostEdge::UserCode,
    },
    HostEntry {
        name: "get_lifecycle_event_sink_definition",
        kind: HostKind::Function,
        edge: HostEdge::UserCode,
    },
    HostEntry {
        name: "invoke_recall_sink",
        kind: HostKind::Function,
        edge: HostEdge::UserCode,
    },
];
