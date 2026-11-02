//! Stage capture document keys, noise masks and the per-type encoding tables.

/// The key that replaces a shared subtree with its digest.
pub const SHARED_MARKER: &str = "__shared__";
/// The key of the capture root in a document with shared nodes.
pub const ROOT_KEY: &str = "__capture__";
/// The key of the shared nodes of a document.
pub const SHARED_NODES_KEY: &str = "__shared_nodes__";
/// Characters of compact JSON from which a subtree is shared.
pub const SHARED_MIN_CHARS: usize = 1024;
/// Keys a mapping must avoid to be written as a JSON object.
pub const RESERVED_KEYS: [&str; 3] = ["__shared__", "__capture__", "__shared_nodes__"];
/// The text that replaces a compile invocation id.
pub const INVOCATION_ID_MASK: &str = "<invocation-id>";
/// The prefix of a native store namespace suffix such as `-native-v1`.
pub const ENGINE_NAMESPACE_PREFIX: &str = "-native-v";
/// The prefix of a refactor staging folder name.
pub const REFACTOR_STAGING_PREFIX: &str = "sqb-refactor-";
/// The text that replaces a refactor staging folder name.
pub const REFACTOR_STAGING_MASK: &str = "sqb-refactor-<staging>";
/// Invocation id digits before the `T`, digits before the `Z`, and hexadecimal digits after `_`.
pub const INVOCATION_ID_PARTS: (usize, usize, usize) = (8, 6, 12);
/// `__slots__` entries that are not instance attributes.
pub const SKIPPED_SLOTS: [&str; 2] = ["__dict__", "__weakref__"];
/// Attributes whose mappings are captured without their order, by type.
pub const UNORDERED_ATTRIBUTES: [(&str, &[&str]); 1] = [(
    "sqlbuild._native:BindingCatalog",
    &["analysis_shapes", "expression_shapes", "schemas"],
)];
/// The attributes captured, in order, for each native type.
pub const PROJECTED_ATTRIBUTES: [(&str, &[&str]); 15] = [
    (
        "sqlbuild._native:DbtCompileReferenceResolver",
        &["_dbt_manifest"],
    ),
    (
        "sqlbuild._native:BindingCatalog",
        &[
            "schemas",
            "dialect",
            "quoted_ignore_case",
            "analysis_shapes",
            "expression_shapes",
            "native",
        ],
    ),
    (
        "sqlbuild._native:DiscoveredSqlModelFile",
        &[
            "file_path",
            "relative_path",
            "contents",
            "header_values",
            "header_column_locations",
            "output_column_locations",
            "query_sql",
            "enum_declarations",
            "constant_declarations",
            "extract_implicit_alias_columns",
        ],
    ),
    (
        "sqlbuild._native:NativeAuditInput",
        &[
            "audit_file",
            "audit_block",
            "sql_body",
            "evaluation_mode",
            "measurement_contract",
            "thresholds",
            "minimum_samples",
            "measure_sql",
            "evidence_sql",
            "evidence_limit",
            "references",
            "attached_target_kind",
            "attached_target_name",
            "attached_column_name",
            "severity",
            "run_scope",
            "always_run",
            "description",
            "meta",
            "declaration_usages",
            "name",
        ],
    ),
    (
        "sqlbuild._native:NativeCompiledAudit",
        &[
            "key",
            "scope_deps",
            "name",
            "definition_name",
            "audit_file",
            "audit_block",
            "sql_body",
            "evaluation_mode",
            "measurement_contract",
            "thresholds",
            "minimum_samples",
            "measure_sql",
            "evidence_sql",
            "evidence_limit",
            "references",
            "attached_target_kind",
            "attached_target_name",
            "attached_column_name",
            "severity",
            "run_scope",
            "always_run",
            "description",
            "meta",
        ],
    ),
    (
        "sqlbuild._native:NativeFunctionInput",
        &[
            "function_file",
            "name",
            "arguments",
            "returns",
            "body_sql",
            "return_columns",
            "references",
            "database",
            "schema",
            "logical_database",
            "logical_schema",
            "fingerprint_database",
            "fingerprint_schema",
            "fingerprint_logical_database",
            "fingerprint_logical_schema",
            "language",
            "runtime_version",
            "entry_point",
            "packages",
            "declaration_usages",
            "tags",
            "description",
        ],
    ),
    (
        "sqlbuild._native:NativeCompiledFunction",
        &[
            "key",
            "deps",
            "name",
            "relative_path",
            "arguments",
            "returns",
            "body_sql",
            "destination",
            "fingerprint_destination",
            "return_columns",
            "references",
            "language",
            "source_file_path",
            "runtime_version",
            "entry_point",
            "packages",
            "tags",
            "description",
        ],
    ),
    (
        "sqlbuild._native:NativeSourceInput",
        &["source_entry", "source_file", "declaration_usages"],
    ),
    (
        "sqlbuild._native:NativeCompiledSource",
        &["key", "deps", "name", "source_entry", "source_file"],
    ),
    (
        "sqlbuild._native:NativeSeedInput",
        &["seed_file", "schema_entry", "schema_file"],
    ),
    (
        "sqlbuild._native:NativeCompiledSeed",
        &[
            "key",
            "deps",
            "name",
            "seed_file",
            "schema_entry",
            "schema_file",
            "destination",
            "external",
        ],
    ),
    (
        "sqlbuild._native:NativeInputsView",
        &[
            "project_config",
            "local_config",
            "discovered_inputs",
            "sql_lexical_syntax",
            "run_id",
            "effective_target_name",
            "effective_target",
            "compile_cache_dir",
            "effective_connection",
            "effective_settings",
            "no_sql_validation",
            "effective_vars",
            "macro_context",
            "loaded_macros",
            "public_enums",
            "public_constants",
            "model_inputs",
            "seed_inputs",
            "source_inputs",
            "sql_function_inputs",
            "test_inputs",
            "scenario_inputs",
            "audit_inputs",
            "diagnostics",
            "external_sql_reference_resolver",
            "scope_index",
            "declaration_scope",
        ],
    ),
    (
        "sqlbuild._native:NativeProjectView",
        &[
            "run_id",
            "effective_target_name",
            "effective_connection",
            "effective_vars",
            "binding_catalog",
            "effective_target_database",
            "effective_target_schema",
            "sql_analysis_dialect",
            "sql_lexical_syntax",
            "compile_cache_dir",
            "settings",
            "scenario",
            "models",
            "sources",
            "seeds",
            "functions",
            "audits",
            "sql_tests",
            "sql_scenarios",
            "loader_functions",
            "hook_functions",
            "enforce_explicit_references",
            "sql_hook_files",
            "materialization_files",
            "public_enums",
            "public_constants",
            "loaded_macros",
            "diagnostics",
            "unmatched_literal_sql_relations",
            "external_sql_reference_resolver",
            "scope_index",
            "sql_expansions",
            "python_execution_inputs",
        ],
    ),
    (
        "sqlbuild._native:NativeCompiledModel",
        &[
            "key",
            "deps",
            "name",
            "relative_path",
            "query_sql",
            "config",
            "destination",
            "references",
            "schema_entry",
            "inferred_columns",
            "fast_lineage_columns",
            "fast_lineage_has_star",
            "fast_lineage_star_resolved",
            "authored_sql",
            "authored_query_sql",
            "output_column_locations",
            "extract_implicit_alias_columns",
            "macro_deps",
            "enum_declarations",
            "constant_declarations",
            "enum_columns",
            "binding_diagnostics",
            "binding_validated",
            "dynamic_column_contract",
            "unchecked_output_columns",
            "rejected_sql_analysis_opt_out",
        ],
    ),
    (
        "sqlbuild._native:NativeModelInput",
        &[
            "model_file",
            "config",
            "query_sql",
            "macro_source_sql",
            "references",
            "schema_entry",
            "schema_file",
            "sql_validation_enabled",
            "rejected_sql_analysis_opt_out",
            "enum_declarations",
            "constant_declarations",
            "enum_columns",
            "macro_deps",
            "macro_usages",
            "declaration_usages",
            "sql_expansion",
            "lint_expansion",
        ],
    ),
];
/// Attributes left out of the capture, by type.
pub const OMITTED_ATTRIBUTES: [(&str, &[&str]); 6] = [
    ("sqlbuild.compiler.scopes.models:ScopeLookup", &["native"]),
    (
        "sqlbuild.compiler.discovery.models:DiscoveredProjectInputs",
        &["native_session"],
    ),
    (
        "sqlbuild.compiler.compile.models:DeclarationScopeResolver",
        &["native_contexts"],
    ),
    (
        "sqlbuild.compiler.compile.models:DeclarationScopeBuild",
        &["sql_test_scans"],
    ),
    (
        "sqlbuild.compiler.compile.models:CompileProjectInputs",
        &["native_project"],
    ),
    (
        "sqlbuild.compiler.compile.models:CompiledProject",
        &["lineage_graph", "native_project", "native_session"],
    ),
];
/// The Python model type a native type is captured as.
pub const CANONICAL_TYPES: [(&str, &str); 11] = [
    (
        "sqlbuild._native:DbtCompileReferenceResolver",
        "sqlbuild.integrations.dbt.classes.dbt_compile_reference_resolver:DbtCompileReferenceResolver",
    ),
    (
        "sqlbuild._native:BindingCatalog",
        "sqlbuild.compiler.sql_analysis.classes.binding_catalog:BindingCatalog",
    ),
    (
        "sqlbuild._native:DiscoveredSqlModelFile",
        "sqlbuild.compiler.discovery.models:DiscoveredSqlModelFile",
    ),
    (
        "sqlbuild._native:NativeInputsView",
        "sqlbuild.compiler.compile.models:CompileProjectInputs",
    ),
    (
        "sqlbuild._native:NativeProjectView",
        "sqlbuild.compiler.compile.models:CompiledProject",
    ),
    (
        "sqlbuild._native:NativeModelInput",
        "sqlbuild.compiler.compile.models:CompileModelInput",
    ),
    (
        "sqlbuild._native:NativeCompiledModel",
        "sqlbuild.compiler.compile.models:CompiledModel",
    ),
    (
        "sqlbuild._native:NativeCompiledSource",
        "sqlbuild.compiler.compile.models:CompiledSource",
    ),
    (
        "sqlbuild._native:NativeCompiledSeed",
        "sqlbuild.compiler.compile.models:CompiledSeed",
    ),
    (
        "sqlbuild._native:NativeCompiledFunction",
        "sqlbuild.compiler.compile.models:CompiledFunction",
    ),
    (
        "sqlbuild._native:NativeCompiledAudit",
        "sqlbuild.compiler.compile.models:CompiledAudit",
    ),
];
/// Sequence types captured as their items.
pub const DECODED_SEQUENCES: [&str; 1] = ["sqlbuild.compiler.compile.models:CompactLineageFacts"];
