//! Layouts of the compile and discovery value records hosted natively.

use sqlbuild_host::entries::models::{FacadeHost, UserCodeHost};

use crate::bindings::_helpers::records::ids::RecordId;
use crate::bindings::_helpers::records::models::{FieldDefault, FieldSpec, RecordSpec};

const R_RELATED_LOCATION: RecordSpec = RecordSpec {
    name: "RelatedLocation",
    doc: "A secondary authored location that adds context to a diagnostic.",
    fields: &[
        FieldSpec::required("label"),
        FieldSpec::required("location"),
        FieldSpec::new("message", FieldDefault::None),
    ],
};
const R_FIXTURE_COLUMN_INFERENCE: RecordSpec = RecordSpec {
    name: "FixtureColumnInference",
    doc: "Inferred fixture columns plus outputs that are explicitly bare NULL literals.",
    fields: &[
        FieldSpec::required("columns"),
        FieldSpec::required("null_literal_names"),
        FieldSpec::required("quoted_names"),
    ],
};
const R_DYNAMIC_COLUMN_FAMILY_PROOF: RecordSpec = RecordSpec {
    name: "DynamicColumnFamilyProof",
    doc: "Compiler evidence for one declared runtime-generated column family.",
    fields: &[
        FieldSpec::required("name"),
        FieldSpec::new("inferred_type", FieldDefault::None),
    ],
};
const R_DYNAMIC_COLUMN_CONTRACT_PROOF: RecordSpec = RecordSpec {
    name: "DynamicColumnContractProof",
    doc: "Closed output-shape evidence for a dynamic column contract.",
    fields: &[
        FieldSpec::required("output_proven"),
        FieldSpec::new("fixed_columns", FieldDefault::EmptyTuple),
        FieldSpec::new("families", FieldDefault::EmptyTuple),
        FieldSpec::new("input_relations", FieldDefault::EmptyTuple),
        FieldSpec::new("failure_reason", FieldDefault::None),
        FieldSpec::new("bare_dynamic_pivot", FieldDefault::False),
    ],
};
const R_COMPILE_MODEL_CONFIG: RecordSpec = RecordSpec {
    name: "CompileModelConfig",
    doc: "Pre-semantic effective config layers attached to a model input.",
    fields: &[
        FieldSpec::new("values", FieldDefault::EmptyDict),
        FieldSpec::new("model_header_keys", FieldDefault::EmptyTuple),
        FieldSpec::new("matched_path_default", FieldDefault::None),
        FieldSpec::new("logical_schema", FieldDefault::None),
        FieldSpec::new("layer_schema", FieldDefault::None),
        FieldSpec::new("logical_database", FieldDefault::None),
        FieldSpec::new(
            "time_travel_retention",
            FieldDefault::NativeCall(RecordId::ResolvedTimeTravelRetention),
        ),
        FieldSpec::new(
            "table_type",
            FieldDefault::NativeCall(RecordId::ResolvedTableType),
        ),
    ],
};
const R_CURSOR_INPUT_ROLES: RecordSpec = RecordSpec {
    name: "CursorInputRoles",
    doc: "Effective filter and watermark inputs for one compiled model.",
    fields: &[
        FieldSpec::required("filter_inputs"),
        FieldSpec::required("watermark_inputs"),
    ],
};
const R_DECLARATION_RUNTIME_PROJECTION: RecordSpec = RecordSpec {
    name: "DeclarationRuntimeProjection",
    doc: "Process-only declaration values keyed by canonical static identity.",
    fields: &[FieldSpec::new("declarations", FieldDefault::EmptyDict)],
};
const R_DECLARATION_SCOPE_RESOLVER: RecordSpec = RecordSpec {
    name: "DeclarationScopeResolver",
    doc: "Process-only lookup state used to project visible runtime declarations.",
    fields: &[
        FieldSpec::required("project_dir"),
        FieldSpec::required("lookup"),
        FieldSpec::required("projection"),
        FieldSpec {
            name: "native_contexts",
            default: FieldDefault::Required,
            compared: false,
            shown: false,
        },
        FieldSpec::new("resource_specific", FieldDefault::None),
        FieldSpec {
            name: "contexts_by_directory",
            default: FieldDefault::EmptyDict,
            compared: false,
            shown: false,
        },
    ],
};
const R_DECLARATION_SCOPE_BUILD: RecordSpec = RecordSpec {
    name: "DeclarationScopeBuild",
    doc: "Static artifact and process-local values shared by compile attachment phases.",
    fields: &[
        FieldSpec::required("loaded_macros"),
        FieldSpec::required("index"),
        FieldSpec::required("resolver"),
        FieldSpec::required("sql_test_scans"),
    ],
};
const R_MACRO_EXPANSION_RESULT: RecordSpec = RecordSpec {
    name: "MacroExpansionResult",
    doc: "Expanded SQL and resolved macro usage facts from one authored string.",
    fields: &[
        FieldSpec::required("sql"),
        FieldSpec::new("spans", FieldDefault::EmptyTuple),
        FieldSpec::new("dependencies", FieldDefault::EmptyTuple),
        FieldSpec::new("usages", FieldDefault::EmptyTuple),
        FieldSpec::new("argument_references", FieldDefault::EmptyTuple),
    ],
};
const R_VISIBLE_MODEL_DECLARATIONS: RecordSpec = RecordSpec {
    name: "VisibleModelDeclarations",
    doc: "One model's visible declarations and macros, as the native model loop resolves them.",
    fields: &[
        FieldSpec::required("local_enums"),
        FieldSpec::required("local_constants"),
        FieldSpec::required("enums"),
        FieldSpec::required("constants"),
        FieldSpec::required("inaccessible_enums"),
        FieldSpec::required("inaccessible_constants"),
        FieldSpec::required("enum_visibility"),
        FieldSpec::required("constant_visibility"),
        FieldSpec::required("macros"),
        FieldSpec::required("macro_records"),
        FieldSpec::required("inaccessible_macros"),
    ],
};
const R_DECLARATION_EXPANSION_CONTEXT: RecordSpec = RecordSpec {
    name: "DeclarationExpansionContext",
    doc: "Declarations and adapter rendering used by one authored SQL owner.",
    fields: &[
        FieldSpec::required("declarations"),
        FieldSpec::required("value_renderer"),
        FieldSpec::required("collection_rendering"),
        FieldSpec::new("resolver", FieldDefault::None),
    ],
};
const R_COMPILE_ADAPTER_CONTEXT: RecordSpec = RecordSpec {
    name: "CompileAdapterContext",
    doc: "Cycle-free adapter behavior required while building compile inputs.",
    fields: &[
        FieldSpec::required("value_renderer"),
        FieldSpec::required("collection_rendering"),
        FieldSpec::required("python_functions_inherit_default_namespace"),
        FieldSpec::required("sql_lexical_syntax"),
    ],
};
const R_INVALID_SQL_REFERENCE_CALL: RecordSpec = RecordSpec {
    name: "InvalidSqlReferenceCall",
    doc: "One reference call whose arguments compile could not replace with a relation.",
    fields: &[
        FieldSpec::required("ref_kind"),
        FieldSpec::required("call"),
        FieldSpec::required("start"),
        FieldSpec::required("message"),
        FieldSpec::required("help"),
        FieldSpec::required("corrected_call"),
    ],
};
const R_SQL_REFERENCE_SCAN: RecordSpec = RecordSpec {
    name: "SqlReferenceScan",
    doc: "Valid references and rejected reference calls found in one SQL text.",
    fields: &[
        FieldSpec::required("references"),
        FieldSpec::new("invalid_calls", FieldDefault::EmptyTuple),
    ],
};
const R_COMPILED_LINEAGE_COLUMN_FACT: RecordSpec = RecordSpec {
    name: "CompiledLineageColumnFact",
    doc: "Compact output column lineage fact extracted during SQL analysis.",
    fields: &[
        FieldSpec::required("output_column"),
        FieldSpec::new("upstream_columns", FieldDefault::EmptyTuple),
        FieldSpec::new(
            "transform_kind",
            FieldDefault::NativeMember(RecordId::ColumnTransformKind, "UNKNOWN"),
        ),
        FieldSpec::new(
            "confidence",
            FieldDefault::NativeMember(RecordId::ColumnLineageConfidence, "UNKNOWN"),
        ),
    ],
};
const R_POLYGLOT_ANALYSIS_RESULT: RecordSpec = RecordSpec {
    name: "PolyglotAnalysisResult",
    doc: "Outcome of one Polyglot column and lineage analysis pass.",
    fields: &[
        FieldSpec::required("analysis_succeeded"),
        FieldSpec::new("columns", FieldDefault::None),
        FieldSpec::new("lineage_columns", FieldDefault::EmptyTuple),
        FieldSpec::new("has_star", FieldDefault::False),
        FieldSpec::new("star_resolved", FieldDefault::False),
        FieldSpec::new("binding_diagnostics", FieldDefault::EmptyTuple),
        FieldSpec::new("binding_validated", FieldDefault::False),
    ],
};
const R_ANALYSIS_CACHE_CONTEXT: RecordSpec = RecordSpec {
    name: "AnalysisCacheContext",
    doc: "Shared project-local analysis cache identity for one compile invocation.",
    fields: &[
        FieldSpec::required("root"),
        FieldSpec::required("shared_fingerprint"),
        FieldSpec::new("signature_namespace", FieldDefault::Text("default")),
    ],
};
const R_COMPILE_ANALYSIS_SELECTION: RecordSpec = RecordSpec {
    name: "CompileAnalysisSelection",
    doc: "Deep-analysis selection and invocation-local preparation observer.",
    fields: &[
        FieldSpec::new("select", FieldDefault::EmptyTuple),
        FieldSpec::new("exclude", FieldDefault::EmptyTuple),
        FieldSpec::new("auto_load_sources", FieldDefault::False),
        FieldSpec::new("no_cache", FieldDefault::False),
        FieldSpec {
            name: "on_inputs_ready",
            default: FieldDefault::None,
            compared: false,
            shown: false,
        },
    ],
};
const R_COMPILE_SQL_SCENARIO_CTE: RecordSpec = RecordSpec {
    name: "CompileSqlScenarioCte",
    doc: "One top-level SQL-native scenario CTE extracted after macro expansion.",
    fields: &[FieldSpec::required("name"), FieldSpec::required("sql_body")],
};
const R_FUNCTION_ARGUMENT: RecordSpec = RecordSpec {
    name: "FunctionArgument",
    doc: "One SQL function argument with an adapter-native type string.",
    fields: &[FieldSpec::required("name"), FieldSpec::required("type")],
};
const R_FUNCTION_RETURN_COLUMN: RecordSpec = RecordSpec {
    name: "FunctionReturnColumn",
    doc: "One named table-function return column with an adapter-native type string.",
    fields: &[FieldSpec::required("name"), FieldSpec::required("type")],
};
const R_HAND_BUILT_ANALYSIS: RecordSpec = RecordSpec {
    name: "HandBuiltAnalysis",
    doc: "A hand-built model input's session position, fallback columns and standalone proof.",
    fields: &[
        FieldSpec::required("session"),
        FieldSpec::required("fallback"),
        FieldSpec::required("standalone_proof"),
    ],
};
const R_MODEL_SQL_ANALYSIS: RecordSpec = RecordSpec {
    name: "ModelSqlAnalysis",
    doc: "Completed model analysis and its in-memory binding proof.",
    fields: &[
        FieldSpec::required("polyglot_analysis"),
        FieldSpec::required("placeholders"),
        FieldSpec::new("cached", FieldDefault::False),
        FieldSpec::new("fused_binding_validated", FieldDefault::False),
        FieldSpec::new("cleaned_sql", FieldDefault::None),
        FieldSpec::new("validated_schema", FieldDefault::None),
        FieldSpec::new("dynamic_column_contract", FieldDefault::None),
    ],
};
const R_COMPILE_SQL_SCENARIO_INPUT: RecordSpec = RecordSpec {
    name: "CompileSqlScenarioInput",
    doc: "One discovered SQL-native scenario with compile-time SQL expansion applied.",
    fields: &[
        FieldSpec::required("scenario_file"),
        FieldSpec::required("sql_body"),
        FieldSpec::new("authored_ctes", FieldDefault::EmptyTuple),
        FieldSpec::new("expected_ctes", FieldDefault::EmptyTuple),
        FieldSpec::new("assertion_ctes", FieldDefault::EmptyTuple),
        FieldSpec::new("source_fixture_names", FieldDefault::EmptyTuple),
        FieldSpec::new("ref_fixture_names", FieldDefault::EmptyTuple),
        FieldSpec::new("seed_fixture_names", FieldDefault::EmptyTuple),
        FieldSpec::new("dbt_ref_fixture_names", FieldDefault::EmptyTuple),
        FieldSpec::new("expected_model_names", FieldDefault::EmptyTuple),
        FieldSpec::new("assertion_names", FieldDefault::EmptyTuple),
        FieldSpec::new("assertion_target_model_names", FieldDefault::EmptyTuple),
        FieldSpec::new("target_model_names", FieldDefault::EmptyTuple),
        FieldSpec::new("declaration_usages", FieldDefault::EmptyTuple),
    ],
};
const R_COMPILE_PROJECT_INPUTS: RecordSpec = RecordSpec {
    name: "CompileProjectInputs",
    doc: "Attached project view used as the first pre-semantic compile input snapshot.",
    fields: &[
        FieldSpec::required("project_config"),
        FieldSpec::required("local_config"),
        FieldSpec::required("discovered_inputs"),
        FieldSpec::required("sql_lexical_syntax"),
        FieldSpec::new("run_id", FieldDefault::Text("")),
        FieldSpec::new("effective_target_name", FieldDefault::None),
        FieldSpec::new("effective_target", FieldDefault::None),
        FieldSpec::new("compile_cache_dir", FieldDefault::None),
        FieldSpec::new("effective_connection", FieldDefault::EmptyDict),
        FieldSpec::new(
            "effective_settings",
            FieldDefault::NativeCall(RecordId::SettingsConfig),
        ),
        FieldSpec::new("no_sql_validation", FieldDefault::False),
        FieldSpec::new("effective_vars", FieldDefault::EmptyDict),
        FieldSpec {
            name: "macro_context",
            default: FieldDefault::None,
            compared: false,
            shown: false,
        },
        FieldSpec::new("loaded_macros", FieldDefault::EmptyDict),
        FieldSpec::new("public_enums", FieldDefault::EmptyDict),
        FieldSpec::new("public_constants", FieldDefault::EmptyDict),
        FieldSpec::new("model_inputs", FieldDefault::EmptyTuple),
        FieldSpec::new("seed_inputs", FieldDefault::EmptyTuple),
        FieldSpec::new("source_inputs", FieldDefault::EmptyTuple),
        FieldSpec::new("sql_function_inputs", FieldDefault::EmptyTuple),
        FieldSpec::new("test_inputs", FieldDefault::EmptyTuple),
        FieldSpec::new("scenario_inputs", FieldDefault::EmptyTuple),
        FieldSpec::new("audit_inputs", FieldDefault::EmptyTuple),
        FieldSpec::new("diagnostics", FieldDefault::EmptyTuple),
        FieldSpec::new("external_sql_reference_resolver", FieldDefault::None),
        FieldSpec::new(
            "scope_index",
            FieldDefault::NativeCall(RecordId::ScopeIndex),
        ),
        FieldSpec {
            name: "declaration_scope",
            default: FieldDefault::None,
            compared: false,
            shown: false,
        },
        FieldSpec {
            name: "native_project",
            default: FieldDefault::None,
            compared: false,
            shown: false,
        },
    ],
};
const R_COMPILED_SOURCE: RecordSpec = RecordSpec {
    name: "CompiledSource",
    doc: "Planner-ready compiled source metadata.",
    fields: &[
        FieldSpec::required("key"),
        FieldSpec::required("deps"),
        FieldSpec::required("name"),
        FieldSpec::required("source_entry"),
        FieldSpec::required("source_file"),
    ],
};
const R_COMPILED_SEED: RecordSpec = RecordSpec {
    name: "CompiledSeed",
    doc: "Planner-ready compiled seed metadata.",
    fields: &[
        FieldSpec::required("key"),
        FieldSpec::required("deps"),
        FieldSpec::required("name"),
        FieldSpec::required("seed_file"),
        FieldSpec::required("schema_entry"),
        FieldSpec::required("schema_file"),
        FieldSpec::required("destination"),
        FieldSpec::new("external", FieldDefault::False),
    ],
};
const R_COMPILED_FUNCTION: RecordSpec = RecordSpec {
    name: "CompiledFunction",
    doc: "Planner-ready compiled SQL function metadata.",
    fields: &[
        FieldSpec::required("key"),
        FieldSpec::required("deps"),
        FieldSpec::required("name"),
        FieldSpec::required("relative_path"),
        FieldSpec::required("arguments"),
        FieldSpec::required("returns"),
        FieldSpec::required("body_sql"),
        FieldSpec::required("destination"),
        FieldSpec::required("fingerprint_destination"),
        FieldSpec::new("return_columns", FieldDefault::EmptyTuple),
        FieldSpec::new("references", FieldDefault::EmptyTuple),
        FieldSpec::new(
            "language",
            FieldDefault::HostMember(FacadeHost::FunctionLanguage.id(), "SQL"),
        ),
        FieldSpec::new("source_file_path", FieldDefault::None),
        FieldSpec::new("runtime_version", FieldDefault::None),
        FieldSpec::new("entry_point", FieldDefault::None),
        FieldSpec::new("packages", FieldDefault::EmptyTuple),
        FieldSpec::new("tags", FieldDefault::EmptyTuple),
        FieldSpec::new("description", FieldDefault::None),
    ],
};
const R_PYTHON_SQL_REFERENCE_REPORT: RecordSpec = RecordSpec {
    name: "PythonSqlReferenceReport",
    doc: "P008 diagnostics plus literal SQL relations that named no project relation.",
    fields: &[
        FieldSpec::new("diagnostics", FieldDefault::EmptyTuple),
        FieldSpec::new("unmatched", FieldDefault::EmptyTuple),
    ],
};
const R_COMPILE_SQL_TEST_CTE: RecordSpec = RecordSpec {
    name: "CompileSqlTestCte",
    doc: "One top-level SQL-native test CTE extracted after macro expansion.",
    fields: &[FieldSpec::required("name"), FieldSpec::required("sql_body")],
};
const R_COMPILE_MODEL_SQL_TEST_INPUT_PAYLOAD: RecordSpec = RecordSpec {
    name: "CompileModelSqlTestInputPayload",
    doc: "Model-mode SQL test compile payload.",
    fields: &[
        FieldSpec::new("authored_ctes", FieldDefault::EmptyTuple),
        FieldSpec::new("macro_mocks", FieldDefault::EmptyDict),
        FieldSpec::new("mock_model_names", FieldDefault::EmptyTuple),
        FieldSpec::new("mock_source_names", FieldDefault::EmptyTuple),
        FieldSpec::new("mock_seed_names", FieldDefault::EmptyTuple),
        FieldSpec::new("mock_dbt_ref_names", FieldDefault::EmptyTuple),
        FieldSpec::new("mock_table_function_names", FieldDefault::EmptyTuple),
        FieldSpec::new("expected_ctes", FieldDefault::EmptyTuple),
        FieldSpec::new("expected_model_names", FieldDefault::EmptyTuple),
        FieldSpec::new("assertion_ctes", FieldDefault::EmptyTuple),
        FieldSpec::new("assertion_names", FieldDefault::EmptyTuple),
        FieldSpec::new("assertion_target_model_names", FieldDefault::EmptyTuple),
        FieldSpec::new("read_helper_names", FieldDefault::EmptyTuple),
        FieldSpec::new("reference_target_model_names", FieldDefault::EmptyTuple),
    ],
};
const R_COMPILE_DIRECT_LOGIC_SQL_TEST_INPUT_PAYLOAD: RecordSpec = RecordSpec {
    name: "CompileDirectLogicSqlTestInputPayload",
    doc: "Direct-logic SQL test compile payload.",
    fields: &[
        FieldSpec::required("actual_cte"),
        FieldSpec::required("expected_cte"),
        FieldSpec::required("mode"),
        FieldSpec::new("helper_ctes", FieldDefault::EmptyTuple),
        FieldSpec::new("tested_resource_names", FieldDefault::EmptyTuple),
    ],
};
const R_COMPILE_SQL_TEST_INPUT: RecordSpec = RecordSpec {
    name: "CompileSqlTestInput",
    doc: "One discovered SQL-native test block with compile-time SQL expansion applied.",
    fields: &[
        FieldSpec::required("test_file"),
        FieldSpec::required("test_block"),
        FieldSpec::required("sql_body"),
        FieldSpec::new(
            "mode",
            FieldDefault::HostMember(FacadeHost::SqlTestMode.id(), "MODEL"),
        ),
        FieldSpec::new(
            "payload",
            FieldDefault::NativeCall(RecordId::CompileModelSqlTestInputPayload),
        ),
        FieldSpec::new("declaration_usages", FieldDefault::EmptyTuple),
        FieldSpec::new("parent_name", FieldDefault::None),
        FieldSpec::new("case_name", FieldDefault::None),
        FieldSpec::new("case_index", FieldDefault::None),
        FieldSpec::new("parameter_schema", FieldDefault::EmptyTuple),
        FieldSpec::new("parameter_values", FieldDefault::EmptyTuple),
    ],
};
const R_COMPILED_MODEL_SQL_TEST_PAYLOAD: RecordSpec = RecordSpec {
    name: "CompiledModelSqlTestPayload",
    doc: "Compiled model-mode SQL test payload.",
    fields: &[
        FieldSpec::new("authored_ctes", FieldDefault::EmptyTuple),
        FieldSpec::new("macro_mocks", FieldDefault::EmptyDict),
        FieldSpec::new("model_query_overrides", FieldDefault::EmptyDict),
        FieldSpec::new("mock_model_names", FieldDefault::EmptyTuple),
        FieldSpec::new("mock_source_names", FieldDefault::EmptyTuple),
        FieldSpec::new("mock_seed_names", FieldDefault::EmptyTuple),
        FieldSpec::new("mock_dbt_ref_names", FieldDefault::EmptyTuple),
        FieldSpec::new("mock_table_function_names", FieldDefault::EmptyTuple),
        FieldSpec::new("expected_ctes", FieldDefault::EmptyTuple),
        FieldSpec::new("expected_model_names", FieldDefault::EmptyTuple),
        FieldSpec::new("assertion_ctes", FieldDefault::EmptyTuple),
        FieldSpec::new("assertion_names", FieldDefault::EmptyTuple),
    ],
};
const R_SQL_EXPANSION_CONTEXT: RecordSpec = RecordSpec {
    name: "SqlExpansionContext",
    doc: "Everything needed to expand one project's authored SQL bodies.",
    fields: &[
        FieldSpec::required("effective_vars"),
        FieldSpec::required("loaded_macros"),
        FieldSpec::required("macro_context"),
        FieldSpec::required("enums"),
        FieldSpec::required("constants"),
        FieldSpec::required("value_renderer"),
        FieldSpec::required("collection_rendering"),
        FieldSpec::new("local_declarations", FieldDefault::EmptyDict),
        FieldSpec::new("declaration_resolver", FieldDefault::None),
    ],
};
const R_EXPANSION_SPAN: RecordSpec = RecordSpec {
    name: "ExpansionSpan",
    doc: "One substituted region, pairing its source range with its rendered range.",
    fields: &[
        FieldSpec::required("source_start"),
        FieldSpec::required("source_end"),
        FieldSpec::required("output_start"),
        FieldSpec::required("output_end"),
    ],
};
const R_COMPILED_SQL_EXPANSION: RecordSpec = RecordSpec {
    name: "CompiledSqlExpansion",
    doc: "Process-local authored-to-expanded SQL evidence shared with compiler checks.",
    fields: &[
        FieldSpec::required("authored_sql"),
        FieldSpec::required("expanded_sql"),
        FieldSpec::required("passes"),
    ],
};
const R_MAPPED_OFFSET: RecordSpec = RecordSpec {
    name: "MappedOffset",
    doc: "An offset in rendered SQL resolved back onto the text that produced it.",
    fields: &[
        FieldSpec::required("offset"),
        FieldSpec::required("generated"),
    ],
};
const R_COMPILED_DIRECT_LOGIC_SQL_TEST_PAYLOAD: RecordSpec = RecordSpec {
    name: "CompiledDirectLogicSqlTestPayload",
    doc: "Compiled direct-logic SQL test payload.",
    fields: &[
        FieldSpec::required("actual_cte"),
        FieldSpec::required("expected_cte"),
        FieldSpec::new(
            "mode",
            FieldDefault::HostMember(FacadeHost::SqlTestMode.id(), "MODEL"),
        ),
        FieldSpec::new("helper_ctes", FieldDefault::EmptyTuple),
        FieldSpec::new("tested_resource_names", FieldDefault::EmptyTuple),
    ],
};
const R_COMPILED_SQL_TEST_RESOURCE: RecordSpec = RecordSpec {
    name: "CompiledSqlTestResource",
    doc: "One direct SQL test resource with its authored resource kind.",
    fields: &[FieldSpec::required("kind"), FieldSpec::required("name")],
};
const R_NATIVE_MODEL_CONFIG_SESSION: RecordSpec = RecordSpec {
    name: "NativeModelConfigSession",
    doc: "The native config builder, validator, and inherited storage policies for one compile.",
    fields: &[
        FieldSpec::required("path_defaults"),
        FieldSpec::required("builder"),
        FieldSpec::required("validator"),
        FieldSpec::required("inherited_storage"),
    ],
};
const R_NATIVE_MODEL_CONFIG_INPUTS: RecordSpec = RecordSpec {
    name: "NativeModelConfigInputs",
    doc: "The project and run inputs every model's native config build reads.",
    fields: &[
        FieldSpec::required("project_config"),
        FieldSpec::required("target_config"),
        FieldSpec::required("effective_vars"),
        FieldSpec::required("effective_target_name"),
        FieldSpec::required("run_id"),
        FieldSpec::required("microbatch_concurrency"),
    ],
};
const R_MODEL_VALIDATION_REQUEST: RecordSpec = RecordSpec {
    name: "ModelValidationRequest",
    doc: "One model's effective config and the facts its validators read.",
    fields: &[
        FieldSpec::required("model_file"),
        FieldSpec::required("config"),
        FieldSpec::required("references"),
        FieldSpec::required("declared_columns"),
        FieldSpec::required("query_sql"),
    ],
};
const R_NAMED_SQL_HOOK_ENTRY: RecordSpec = RecordSpec {
    name: "NamedSqlHookEntry",
    doc: "An unresolved invocation of a discovered SQL hook resource.",
    fields: &[FieldSpec::required("name"), FieldSpec::required("kwargs")],
};
const R_DISCOVERED_SQL_FUNCTION_FILE: RecordSpec = RecordSpec {
    name: "DiscoveredSqlFunctionFile",
    doc: "A discovered SQL function file and its raw contents.",
    fields: &[
        FieldSpec::required("file_path"),
        FieldSpec::required("relative_path"),
        FieldSpec::required("contents"),
        FieldSpec::required("header_values"),
        FieldSpec::required("body_sql"),
    ],
};
const R_DISCOVERED_SQL_HOOK_FILE: RecordSpec = RecordSpec {
    name: "DiscoveredSqlHookFile",
    doc: "A discovered named SQL lifecycle hook resource.",
    fields: &[
        FieldSpec::required("file_path"),
        FieldSpec::required("relative_path"),
        FieldSpec::required("contents"),
        FieldSpec::required("header_values"),
        FieldSpec::required("sql_body"),
        FieldSpec::required("name"),
        FieldSpec::new("description", FieldDefault::None),
        FieldSpec::new(
            "scope_kind",
            FieldDefault::NativeMember(RecordId::ScopeKind, "GLOBAL"),
        ),
        FieldSpec::new("ownership_root", FieldDefault::None),
        FieldSpec::new("owning_path", FieldDefault::None),
        FieldSpec::new("declaration_root", FieldDefault::None),
    ],
};
const R_ENUM_MEMBER: RecordSpec = RecordSpec {
    name: "EnumMember",
    doc: "One named member in a SQLBuild enum declaration.",
    fields: &[FieldSpec::required("name"), FieldSpec::required("value")],
};
const R_DISCOVERED_ENUM_FILE: RecordSpec = RecordSpec {
    name: "DiscoveredEnumFile",
    doc: "A public SQL file containing one or more enum declarations.",
    fields: &[
        FieldSpec::required("file_path"),
        FieldSpec::required("relative_path"),
        FieldSpec::required("contents"),
        FieldSpec::required("declarations"),
        FieldSpec::new(
            "scope_kind",
            FieldDefault::NativeMember(RecordId::ScopeKind, "GLOBAL"),
        ),
        FieldSpec::new("ownership_root", FieldDefault::None),
        FieldSpec::new("owning_path", FieldDefault::None),
        FieldSpec::new("declaration_root", FieldDefault::None),
    ],
};
const R_DISCOVERED_CONSTANT_FILE: RecordSpec = RecordSpec {
    name: "DiscoveredConstantFile",
    doc: "A public SQL file containing one or more constant declarations.",
    fields: &[
        FieldSpec::required("file_path"),
        FieldSpec::required("relative_path"),
        FieldSpec::required("contents"),
        FieldSpec::required("declarations"),
        FieldSpec::new(
            "scope_kind",
            FieldDefault::NativeMember(RecordId::ScopeKind, "GLOBAL"),
        ),
        FieldSpec::new("ownership_root", FieldDefault::None),
        FieldSpec::new("owning_path", FieldDefault::None),
        FieldSpec::new("declaration_root", FieldDefault::None),
    ],
};
const R_MODEL_SCHEMA_DECLARATION: RecordSpec = RecordSpec {
    name: "ModelSchemaDeclaration",
    doc: "One public reusable model column schema declaration.",
    fields: &[
        FieldSpec::required("name"),
        FieldSpec::required("columns"),
        FieldSpec::required("relative_path"),
        FieldSpec::new("extends", FieldDefault::None),
        FieldSpec::new("description", FieldDefault::None),
    ],
};
const R_DISCOVERED_MODEL_SCHEMA_FILE: RecordSpec = RecordSpec {
    name: "DiscoveredModelSchemaFile",
    doc: "A public SQL file containing reusable model schemas.",
    fields: &[
        FieldSpec::required("file_path"),
        FieldSpec::required("relative_path"),
        FieldSpec::required("contents"),
        FieldSpec::required("declarations"),
        FieldSpec::new(
            "scope_kind",
            FieldDefault::NativeMember(RecordId::ScopeKind, "GLOBAL"),
        ),
        FieldSpec::new("ownership_root", FieldDefault::None),
        FieldSpec::new("owning_path", FieldDefault::None),
        FieldSpec::new("declaration_root", FieldDefault::None),
    ],
};
const R_DISCOVERED_PYTHON_FUNCTION_FILE: RecordSpec = RecordSpec {
    name: "DiscoveredPythonFunctionFile",
    doc: "A discovered Python function file and its parsed UDF metadata.",
    fields: &[
        FieldSpec::required("file_path"),
        FieldSpec::required("relative_path"),
        FieldSpec::required("contents"),
        FieldSpec::required("header_values"),
        FieldSpec::required("entry_point"),
        FieldSpec::required("body_python"),
    ],
};
const R_DISCOVERED_SCHEMA_FILE: RecordSpec = RecordSpec {
    name: "DiscoveredSchemaFile",
    doc: "A discovered schema.yml file and its raw contents.",
    fields: &[
        FieldSpec::required("file_path"),
        FieldSpec::required("relative_path"),
        FieldSpec::required("contents"),
        FieldSpec::required("model_entries"),
        FieldSpec::required("seed_entries"),
    ],
};
const R_DISCOVERED_SEED_FILE: RecordSpec = RecordSpec {
    name: "DiscoveredSeedFile",
    doc: "A discovered seed file.",
    fields: &[
        FieldSpec::required("file_path"),
        FieldSpec::required("relative_path"),
    ],
};
const R_DISCOVERED_SQL_TEST_CASE: RecordSpec = RecordSpec {
    name: "DiscoveredSqlTestCase",
    doc: "One ordered named case belonging to a SQL-native test template.",
    fields: &[
        FieldSpec::required("name"),
        FieldSpec::required("values"),
        FieldSpec::required("case_index"),
    ],
};
const R_DISCOVERED_SQL_TEST_BLOCK: RecordSpec = RecordSpec {
    name: "DiscoveredSqlTestBlock",
    doc: "One raw TEST(...) block discovered from a SQL-native test file.",
    fields: &[
        FieldSpec::required("test_index"),
        FieldSpec::required("header_values"),
        FieldSpec::required("sql_body"),
        FieldSpec::new("name", FieldDefault::None),
        FieldSpec::new(
            "mode",
            FieldDefault::HostMember(FacadeHost::SqlTestMode.id(), "MODEL"),
        ),
        FieldSpec::new("parameters", FieldDefault::EmptyTuple),
        FieldSpec::new("cases", FieldDefault::EmptyTuple),
        FieldSpec::new("cursor_start", FieldDefault::None),
        FieldSpec::new("cursor_end", FieldDefault::None),
        FieldSpec::new("sql_body_span", FieldDefault::None),
    ],
};
const R_DISCOVERED_AUDIT_FILE: RecordSpec = RecordSpec {
    name: "DiscoveredAuditFile",
    doc: "A discovered audit SQL file and its raw contents.",
    fields: &[
        FieldSpec::required("file_path"),
        FieldSpec::required("relative_path"),
        FieldSpec::required("contents"),
        FieldSpec::required("blocks"),
        FieldSpec::new(
            "declaration_kind",
            FieldDefault::NativeMember(RecordId::DeclarationKind, "AUDIT"),
        ),
        FieldSpec::new(
            "scope_kind",
            FieldDefault::NativeMember(RecordId::ScopeKind, "GLOBAL"),
        ),
        FieldSpec::new("ownership_root", FieldDefault::None),
        FieldSpec::new("owning_path", FieldDefault::None),
        FieldSpec::new("declaration_root", FieldDefault::None),
    ],
};
const R_DISCOVERED_AUDIT_BLOCK: RecordSpec = RecordSpec {
    name: "DiscoveredAuditBlock",
    doc: "One raw AUDIT(...) block discovered from a SQL audit file.",
    fields: &[
        FieldSpec::required("audit_index"),
        FieldSpec::required("header_values"),
        FieldSpec::required("sql_body"),
        FieldSpec::new("name", FieldDefault::None),
        FieldSpec::new(
            "evaluation_mode",
            FieldDefault::HostMember(FacadeHost::AuditEvaluationMode.id(), "VIOLATIONS"),
        ),
        FieldSpec::new("measurement_contract", FieldDefault::None),
        FieldSpec::new("measure_sql", FieldDefault::None),
        FieldSpec::new("evidence_sql", FieldDefault::None),
    ],
};
const R_DISCOVERED_ADAPTER_FILE: RecordSpec = RecordSpec {
    name: "DiscoveredAdapterFile",
    doc: "A detected project adapter Python file.",
    fields: &[
        FieldSpec::required("file_path"),
        FieldSpec::required("relative_path"),
    ],
};
const R_DISCOVERY_FILE_FAULT: RecordSpec = RecordSpec {
    name: "DiscoveryFileFault",
    doc: "One project-relative authored file fault captured during tolerant discovery.",
    fields: &[FieldSpec::required("path"), FieldSpec::required("message")],
};
const R_TOLERANT_SCOPE_DISCOVERY: RecordSpec = RecordSpec {
    name: "TolerantScopeDiscovery",
    doc: "Parsed scope inputs and independently categorized tolerant discovery faults.",
    fields: &[
        FieldSpec::required("discovered_inputs"),
        FieldSpec::new("resource_faults", FieldDefault::EmptyTuple),
        FieldSpec::new("declaration_faults", FieldDefault::EmptyTuple),
        FieldSpec::new("relationship_faults", FieldDefault::EmptyTuple),
        FieldSpec::new("config_faults", FieldDefault::EmptyTuple),
    ],
};
const R_SQL_HOOK_ENTRY: RecordSpec = RecordSpec {
    name: "SqlHookEntry",
    doc: "A model lifecycle hook that executes SQL.",
    fields: &[
        FieldSpec::required("statement"),
        FieldSpec::new("name", FieldDefault::None),
        FieldSpec::new("relative_path", FieldDefault::None),
        FieldSpec::new("definition_sql", FieldDefault::None),
        FieldSpec::new("kwargs", FieldDefault::None),
        FieldSpec::new("description", FieldDefault::None),
        FieldSpec::new("reads", FieldDefault::EmptyTuple),
    ],
};
const R_PYTHON_HOOK_ENTRY: RecordSpec = RecordSpec {
    name: "PythonHookEntry",
    doc: "A model lifecycle hook that invokes a discovered Python hook.",
    fields: &[FieldSpec::required("name"), FieldSpec::required("kwargs")],
};
const R_ENUM_DECLARATION: RecordSpec = RecordSpec {
    name: "EnumDeclaration",
    doc: "One validated public or model-local enum declaration.",
    fields: &[
        FieldSpec::required("name"),
        FieldSpec::required("members"),
        FieldSpec::required("scalar_type"),
        FieldSpec::required("relative_path"),
        FieldSpec::new("model_name", FieldDefault::None),
    ],
};
const R_CONSTANT_DECLARATION: RecordSpec = RecordSpec {
    name: "ConstantDeclaration",
    doc: "One validated public or model-local constant declaration.",
    fields: &[
        FieldSpec::required("name"),
        FieldSpec::required("value"),
        FieldSpec::required("relative_path"),
        FieldSpec::new("model_name", FieldDefault::None),
        FieldSpec::new("render_as", FieldDefault::None),
    ],
};
const R_DISCOVERED_SOURCE_FILE: RecordSpec = RecordSpec {
    name: "DiscoveredSourceFile",
    doc: "A discovered source declaration file and its raw contents.",
    fields: &[
        FieldSpec::required("file_path"),
        FieldSpec::required("relative_path"),
        FieldSpec::required("contents"),
        FieldSpec::required("source_entries"),
    ],
};
const R_DISCOVERED_SQL_TEST_FILE: RecordSpec = RecordSpec {
    name: "DiscoveredSqlTestFile",
    doc: "A discovered SQL-native test file and its raw contents.",
    fields: &[
        FieldSpec::required("file_path"),
        FieldSpec::required("relative_path"),
        FieldSpec::required("contents"),
        FieldSpec::required("blocks"),
        FieldSpec::new(
            "ownership_root",
            FieldDefault::CallText("pathlib", "Path", "tests/unit"),
        ),
    ],
};
const R_SQL_TEST_PARAMETER_DECLARATION: RecordSpec = RecordSpec {
    name: "SqlTestParameterDeclaration",
    doc: "One typed parameter declared by a SQL-native test template.",
    fields: &[
        FieldSpec::required("name"),
        FieldSpec::required("value_type"),
        FieldSpec::new("nullable", FieldDefault::False),
    ],
};
const R_DISCOVERED_SQL_SCENARIO_FILE: RecordSpec = RecordSpec {
    name: "DiscoveredSqlScenarioFile",
    doc: "A discovered SQL-native scenario file and its raw contents.",
    fields: &[
        FieldSpec::required("file_path"),
        FieldSpec::required("relative_path"),
        FieldSpec::required("contents"),
        FieldSpec::required("header_values"),
        FieldSpec::required("sql_body"),
        FieldSpec::required("name"),
        FieldSpec::new(
            "ownership_root",
            FieldDefault::CallText("pathlib", "Path", "tests/scenarios"),
        ),
        FieldSpec::new("sql_body_span", FieldDefault::None),
    ],
};
const R_DISCOVERED_MACRO_FILE: RecordSpec = RecordSpec {
    name: "DiscoveredMacroFile",
    doc: "A discovered project macro file and its raw contents.",
    fields: &[
        FieldSpec::required("file_path"),
        FieldSpec::required("relative_path"),
        FieldSpec::required("contents"),
        FieldSpec::new(
            "scope_kind",
            FieldDefault::NativeMember(RecordId::ScopeKind, "GLOBAL"),
        ),
        FieldSpec::new("ownership_root", FieldDefault::None),
        FieldSpec::new("owning_path", FieldDefault::None),
        FieldSpec::new("declaration_root", FieldDefault::None),
    ],
};
const R_DISCOVERED_MATERIALIZATION_FILE: RecordSpec = RecordSpec {
    name: "DiscoveredMaterializationFile",
    doc: "A discovered custom materialization Python file.",
    fields: &[
        FieldSpec::required("file_path"),
        FieldSpec::required("relative_path"),
        FieldSpec::required("name"),
        FieldSpec::new("provider_usages", FieldDefault::EmptyTuple),
    ],
};
const R_DISCOVERED_PROVIDER_USAGE: RecordSpec = RecordSpec {
    name: "DiscoveredProviderUsage",
    doc: "Framework-owned metadata for provider usage by a Python callable surface.",
    fields: &[
        FieldSpec::required("provider_name"),
        FieldSpec::required("parameter_name"),
        FieldSpec::new("annotation_class_name", FieldDefault::None),
        FieldSpec::new("annotation_module", FieldDefault::None),
    ],
};
const R_DISCOVERED_LOADER_FUNCTION: RecordSpec = RecordSpec {
    name: "DiscoveredLoaderFunction",
    doc: "A discovered project source loader function.",
    fields: &[
        FieldSpec::required("file_path"),
        FieldSpec::required("relative_path"),
        FieldSpec::required("name"),
        FieldSpec::required("function"),
        FieldSpec::new("depends_on", FieldDefault::EmptyTuple),
        FieldSpec::new("destination", FieldDefault::None),
        FieldSpec::new("write_strategy", FieldDefault::None),
        FieldSpec::new("cursor_column", FieldDefault::None),
        FieldSpec::new("unique_key", FieldDefault::EmptyTuple),
        FieldSpec::new("columns", FieldDefault::EmptyTuple),
        FieldSpec::new("contract", FieldDefault::None),
        FieldSpec::new("description", FieldDefault::None),
        FieldSpec::new(
            "connection_mode",
            FieldDefault::NativeMember(RecordId::LoaderConnectionMode, "SQLBUILD"),
        ),
        FieldSpec::new("provider_usages", FieldDefault::EmptyTuple),
        FieldSpec {
            name: "identity",
            default: FieldDefault::None,
            compared: false,
            shown: false,
        },
    ],
};
const R_DISCOVERED_TASK_FUNCTION: RecordSpec = RecordSpec {
    name: "DiscoveredTaskFunction",
    doc: "A discovered project task function.",
    fields: &[
        FieldSpec::required("file_path"),
        FieldSpec::required("relative_path"),
        FieldSpec::required("name"),
        FieldSpec::required("function"),
        FieldSpec::new("depends_on", FieldDefault::EmptyTuple),
        FieldSpec::new("tags", FieldDefault::EmptyTuple),
        FieldSpec::new("group", FieldDefault::None),
        FieldSpec::new("description", FieldDefault::None),
        FieldSpec::new("meta", FieldDefault::None),
        FieldSpec::new("retry", FieldDefault::None),
        FieldSpec::new("provider_usages", FieldDefault::EmptyTuple),
        FieldSpec {
            name: "identity",
            default: FieldDefault::None,
            compared: false,
            shown: false,
        },
    ],
};
const R_DISCOVERED_ASSET_FUNCTION: RecordSpec = RecordSpec {
    name: "DiscoveredAssetFunction",
    doc: "A discovered project asset function.",
    fields: &[
        FieldSpec::required("file_path"),
        FieldSpec::required("relative_path"),
        FieldSpec::required("name"),
        FieldSpec::required("function"),
        FieldSpec::new("depends_on", FieldDefault::EmptyTuple),
        FieldSpec::new("tags", FieldDefault::EmptyTuple),
        FieldSpec::new("group", FieldDefault::None),
        FieldSpec::new("description", FieldDefault::None),
        FieldSpec::new("meta", FieldDefault::None),
        FieldSpec::new("columns", FieldDefault::EmptyTuple),
        FieldSpec::new("column_lineage", FieldDefault::None),
        FieldSpec::new("retry", FieldDefault::None),
        FieldSpec::new("provider_usages", FieldDefault::EmptyTuple),
        FieldSpec {
            name: "identity",
            default: FieldDefault::None,
            compared: false,
            shown: false,
        },
    ],
};
const R_DISCOVERED_CHECK_FUNCTION: RecordSpec = RecordSpec {
    name: "DiscoveredCheckFunction",
    doc: "A discovered project check function.",
    fields: &[
        FieldSpec::required("file_path"),
        FieldSpec::required("relative_path"),
        FieldSpec::required("name"),
        FieldSpec::required("function"),
        FieldSpec::required("depends_on"),
        FieldSpec::new(
            "severity",
            FieldDefault::HostMember(UserCodeHost::PythonCheckSeverity.id(), "ERROR"),
        ),
        FieldSpec::new("tags", FieldDefault::EmptyTuple),
        FieldSpec::new("group", FieldDefault::None),
        FieldSpec::new("description", FieldDefault::None),
        FieldSpec::new("meta", FieldDefault::None),
        FieldSpec::new("provider_usages", FieldDefault::EmptyTuple),
        FieldSpec {
            name: "identity",
            default: FieldDefault::None,
            compared: false,
            shown: false,
        },
    ],
};
const R_DISCOVERED_HOOK_FUNCTION: RecordSpec = RecordSpec {
    name: "DiscoveredHookFunction",
    doc: "A discovered project model lifecycle hook function.",
    fields: &[
        FieldSpec::required("file_path"),
        FieldSpec::required("relative_path"),
        FieldSpec::required("name"),
        FieldSpec::required("function"),
        FieldSpec::new("description", FieldDefault::None),
        FieldSpec::new("reads", FieldDefault::EmptyTuple),
        FieldSpec::new("provider_usages", FieldDefault::EmptyTuple),
        FieldSpec::new(
            "scope_kind",
            FieldDefault::NativeMember(RecordId::ScopeKind, "GLOBAL"),
        ),
        FieldSpec::new("ownership_root", FieldDefault::None),
        FieldSpec::new("owning_path", FieldDefault::None),
        FieldSpec::new("declaration_root", FieldDefault::None),
    ],
};
const R_DISCOVERED_AUDIT_FACTORY: RecordSpec = RecordSpec {
    name: "DiscoveredAuditFactory",
    doc: "A discovered audit factory and its normalized cases.",
    fields: &[
        FieldSpec::required("name"),
        FieldSpec::required("function"),
        FieldSpec::required("file_path"),
        FieldSpec::required("relative_path"),
        FieldSpec::required("line"),
        FieldSpec::new("cases", FieldDefault::EmptyTuple),
    ],
};
const R_DISCOVERED_EVENT_EXPORTER_DECLARATION: RecordSpec = RecordSpec {
    name: "DiscoveredEventExporterDeclaration",
    doc: "A discovered exporter declaration before provider binding.",
    fields: &[
        FieldSpec::required("file_path"),
        FieldSpec::required("relative_path"),
        FieldSpec::required("name"),
        FieldSpec::required("function"),
        FieldSpec::required("event_kinds"),
        FieldSpec::required("min_severity"),
    ],
};
const R_DISCOVERED_EVENT_EXPORTER: RecordSpec = RecordSpec {
    name: "DiscoveredEventExporter",
    doc: "A discovered canonical lifecycle event exporter function.",
    fields: &[
        FieldSpec::required("file_path"),
        FieldSpec::required("relative_path"),
        FieldSpec::required("name"),
        FieldSpec::required("function"),
        FieldSpec::new(
            "event_kinds",
            FieldDefault::TextFrozenset(&[
                "audit",
                "invocation",
                "microbatch",
                "operation",
                "resource",
                "retry",
                "run",
                "statement",
            ]),
        ),
        FieldSpec::new(
            "min_severity",
            FieldDefault::NativeMember(RecordId::EventExportSeverity, "DEBUG"),
        ),
        FieldSpec::new("provider_usages", FieldDefault::EmptyTuple),
    ],
};
const R_DISCOVERED_COMMAND_OUTPUT_SINK_DECLARATION: RecordSpec = RecordSpec {
    name: "DiscoveredCommandOutputSinkDeclaration",
    doc: "A command-output sink declaration before provider binding.",
    fields: &[
        FieldSpec::required("file_path"),
        FieldSpec::required("relative_path"),
        FieldSpec::required("name"),
        FieldSpec::required("function"),
        FieldSpec::required("streams"),
    ],
};
const R_DISCOVERED_COMMAND_OUTPUT_SINK: RecordSpec = RecordSpec {
    name: "DiscoveredCommandOutputSink",
    doc: "A discovered command-output sink function.",
    fields: &[
        FieldSpec::required("file_path"),
        FieldSpec::required("relative_path"),
        FieldSpec::required("name"),
        FieldSpec::required("function"),
        FieldSpec::required("streams"),
        FieldSpec::new("provider_usages", FieldDefault::EmptyTuple),
    ],
};
const R_DISCOVERED_PROVIDER: RecordSpec = RecordSpec {
    name: "DiscoveredProvider",
    doc: "A discovered project provider class and validated settings object.",
    fields: &[
        FieldSpec::required("file_path"),
        FieldSpec::required("relative_path"),
        FieldSpec::required("name"),
        FieldSpec::required("provider_class"),
        FieldSpec::required("settings"),
    ],
};
const R_DISCOVERED_PYTHON_NODE_FUNCTIONS: RecordSpec = RecordSpec {
    name: "DiscoveredPythonNodeFunctions",
    doc: "Discovered project Python DAG node functions grouped by kind.",
    fields: &[
        FieldSpec::new("loaders", FieldDefault::EmptyTuple),
        FieldSpec::new("tasks", FieldDefault::EmptyTuple),
        FieldSpec::new("assets", FieldDefault::EmptyTuple),
        FieldSpec::new("checks", FieldDefault::EmptyTuple),
        FieldSpec::new("audit_factories", FieldDefault::EmptyTuple),
    ],
};
const R_DISCOVERED_PROJECT_INPUTS: RecordSpec = RecordSpec {
    name: "DiscoveredProjectInputs",
    doc: "All raw project inputs discovered from disk before semantic resolution.",
    fields: &[
        FieldSpec::required("project_config"),
        FieldSpec::required("local_config"),
        FieldSpec::new("project_dir", FieldDefault::None),
        FieldSpec::new("model_files", FieldDefault::EmptyTuple),
        FieldSpec::new("enum_files", FieldDefault::EmptyTuple),
        FieldSpec::new("constant_files", FieldDefault::EmptyTuple),
        FieldSpec::new("model_schema_files", FieldDefault::EmptyTuple),
        FieldSpec::new("sql_function_files", FieldDefault::EmptyTuple),
        FieldSpec::new("sql_hook_files", FieldDefault::EmptyTuple),
        FieldSpec::new("python_function_files", FieldDefault::EmptyTuple),
        FieldSpec::new("schema_files", FieldDefault::EmptyTuple),
        FieldSpec::new("source_files", FieldDefault::EmptyTuple),
        FieldSpec::new("seed_files", FieldDefault::EmptyTuple),
        FieldSpec::new("test_files", FieldDefault::EmptyTuple),
        FieldSpec::new("scenario_files", FieldDefault::EmptyTuple),
        FieldSpec::new("audit_files", FieldDefault::EmptyTuple),
        FieldSpec::new("macro_files", FieldDefault::EmptyTuple),
        FieldSpec::new("materialization_files", FieldDefault::EmptyTuple),
        FieldSpec::new("loader_functions", FieldDefault::EmptyTuple),
        FieldSpec::new("task_functions", FieldDefault::EmptyTuple),
        FieldSpec::new("asset_functions", FieldDefault::EmptyTuple),
        FieldSpec::new("check_functions", FieldDefault::EmptyTuple),
        FieldSpec::new("audit_factories", FieldDefault::EmptyTuple),
        FieldSpec::new("hook_functions", FieldDefault::EmptyTuple),
        FieldSpec::new("event_exporters", FieldDefault::EmptyTuple),
        FieldSpec::new("command_output_sinks", FieldDefault::EmptyTuple),
        FieldSpec::new("providers", FieldDefault::EmptyTuple),
        FieldSpec::new("adapter_file", FieldDefault::None),
        FieldSpec {
            name: "native_session",
            default: FieldDefault::None,
            compared: false,
            shown: false,
        },
    ],
};
const R_DISCOVERED_RUNTIME_EXTENSIONS: RecordSpec = RecordSpec {
    name: "DiscoveredRuntimeExtensions",
    doc: "Command-startup providers, typed sinks, and lifecycle delivery settings.",
    fields: &[
        FieldSpec::new("providers", FieldDefault::EmptyTuple),
        FieldSpec::new("event_exporters", FieldDefault::EmptyTuple),
        FieldSpec::new("command_output_sinks", FieldDefault::EmptyTuple),
        FieldSpec::new("lifecycle_shutdown_timeout_seconds", FieldDefault::None),
        FieldSpec::new("recall_lifecycle_sink", FieldDefault::None),
        FieldSpec::new("recall_output_sink", FieldDefault::None),
    ],
};
const R_COMPILER_DIAGNOSTIC: RecordSpec = RecordSpec {
    name: "CompilerDiagnostic",
    doc: "One project diagnostic produced by compile-time checks.",
    fields: &[
        FieldSpec::required("phase"),
        FieldSpec::required("severity"),
        FieldSpec::required("code"),
        FieldSpec::required("message"),
        FieldSpec::new("resource_type", FieldDefault::None),
        FieldSpec::new("resource_name", FieldDefault::None),
        FieldSpec::new("column_name", FieldDefault::None),
        FieldSpec::new("path", FieldDefault::None),
        FieldSpec::new("line", FieldDefault::None),
        FieldSpec::new("column", FieldDefault::None),
        FieldSpec::new("location", FieldDefault::None),
        FieldSpec::new("related_locations", FieldDefault::EmptyTuple),
        FieldSpec::new("help", FieldDefault::None),
        FieldSpec::new("notes", FieldDefault::EmptyTuple),
        FieldSpec::new("affected_rules", FieldDefault::EmptyTuple),
    ],
};
const R_INFERRED_COLUMN: RecordSpec = RecordSpec {
    name: "InferredColumn",
    doc: "One output column inferred from query SQL via sql_analysis parsing.",
    fields: &[
        FieldSpec::required("name"),
        FieldSpec::new("type", FieldDefault::None),
        FieldSpec::new(
            "nullability",
            FieldDefault::NativeMember(RecordId::InferredNullability, "UNKNOWN"),
        ),
    ],
};
const R_LOADED_MACRO: RecordSpec = RecordSpec {
    name: "LoadedMacro",
    doc: "One loaded project macro available for compile-time SQL expansion.",
    fields: &[
        FieldSpec::required("name"),
        FieldSpec::required("file_path"),
        FieldSpec::required("relative_path"),
        FieldSpec::required("raw_source"),
        FieldSpec::required("function"),
        FieldSpec::new("dependencies", FieldDefault::EmptyTuple),
        FieldSpec::new("language", FieldDefault::Text("python")),
    ],
};
const R_STATIC_MACRO_EXPORT: RecordSpec = RecordSpec {
    name: "StaticMacroExport",
    doc: "Value-free AST inventory for one project-owned macro export.",
    fields: &[
        FieldSpec::required("name"),
        FieldSpec::required("relative_path"),
        FieldSpec::required("parameters"),
        FieldSpec::required("line"),
        FieldSpec::required("source_digest"),
        FieldSpec::new("dependencies", FieldDefault::EmptyTuple),
    ],
};
const R_STATIC_MACRO_FAULT: RecordSpec = RecordSpec {
    name: "StaticMacroFault",
    doc: "One value-free static macro inventory fault.",
    fields: &[
        FieldSpec::required("relative_path"),
        FieldSpec::required("message"),
    ],
};
const R_STATIC_MACRO_INVENTORY: RecordSpec = RecordSpec {
    name: "StaticMacroInventory",
    doc: "AST macro exports and faults collected without module execution.",
    fields: &[
        FieldSpec::new("exports", FieldDefault::EmptyTuple),
        FieldSpec::new("faults", FieldDefault::EmptyTuple),
    ],
};
const R_MACRO_CONTEXT: RecordSpec = RecordSpec {
    name: "MacroContext",
    doc: "Expose target settings and caller-visible declarations to a Python SQL macro.",
    fields: &[
        FieldSpec::required("adapter_name"),
        FieldSpec::required("sql_analysis_enabled"),
        FieldSpec::required("target_name"),
        FieldSpec::new("vars", FieldDefault::EmptyDict),
        FieldSpec::new("constants", FieldDefault::EmptyDict),
        FieldSpec::new("enums", FieldDefault::EmptyDict),
        FieldSpec {
            name: "_value_renderer",
            default: FieldDefault::None,
            compared: false,
            shown: false,
        },
        FieldSpec {
            name: "_collection_rendering",
            default: FieldDefault::NativeMember(RecordId::CollectionRendering, "VALUE_LIST"),
            compared: false,
            shown: false,
        },
        FieldSpec {
            name: "_constant_declarations",
            default: FieldDefault::EmptyDict,
            compared: false,
            shown: false,
        },
        FieldSpec {
            name: "_enforce_explicit_references",
            default: FieldDefault::True,
            compared: false,
            shown: false,
        },
        FieldSpec {
            name: "_lexical_syntax",
            default: FieldDefault::None,
            compared: false,
            shown: false,
        },
    ],
};
const R_DECLARATION_RESOLUTION_CONTEXT: RecordSpec = RecordSpec {
    name: "DeclarationResolutionContext",
    doc: "Declarations visible during one authored SQL expansion.",
    fields: &[
        FieldSpec::new("enums", FieldDefault::EmptyDict),
        FieldSpec::new("constants", FieldDefault::EmptyDict),
        FieldSpec::new("inaccessible_enums", FieldDefault::EmptyDict),
        FieldSpec::new("inaccessible_constants", FieldDefault::EmptyDict),
        FieldSpec::new("enum_visibility", FieldDefault::EmptyDict),
        FieldSpec::new("constant_visibility", FieldDefault::EmptyDict),
        FieldSpec::new("macros", FieldDefault::EmptyDict),
        FieldSpec::new("macro_records", FieldDefault::EmptyDict),
        FieldSpec::new("macro_visibility", FieldDefault::EmptyDict),
        FieldSpec::new("inaccessible_macros", FieldDefault::EmptyDict),
        FieldSpec::new("consumer", FieldDefault::None),
    ],
};
const R_MODEL_INPUT_BUILD_CONTEXT: RecordSpec = RecordSpec {
    name: "ModelInputBuildContext",
    doc: "Run-constant config and macros for building model compile inputs.",
    fields: &[
        FieldSpec::required("effective_vars"),
        FieldSpec::required("effective_settings"),
        FieldSpec::required("target_config"),
        FieldSpec::required("effective_target_name"),
        FieldSpec::required("run_id"),
        FieldSpec::required("macro_context"),
        FieldSpec::required("loaded_macros"),
        FieldSpec::required("value_renderer"),
        FieldSpec::required("collection_rendering"),
        FieldSpec::required("sql_lexical_syntax"),
        FieldSpec::new("public_enums", FieldDefault::EmptyDict),
        FieldSpec::new("public_constants", FieldDefault::EmptyDict),
        FieldSpec::new("public_model_schemas", FieldDefault::EmptyDict),
        FieldSpec::new("declaration_resolver", FieldDefault::None),
    ],
};
const R_COMPILE_SQL_REFERENCE: RecordSpec = RecordSpec {
    name: "CompileSqlReference",
    doc: "One logical SQL reference discovered from compiled SQL text.",
    fields: &[
        FieldSpec::required("ref_kind"),
        FieldSpec::required("ref_name"),
        FieldSpec::new("ref_package", FieldDefault::None),
        FieldSpec::new("call_argument_count", FieldDefault::None),
    ],
};
const R_COMPILED_LINEAGE_SOURCE_FACT: RecordSpec = RecordSpec {
    name: "CompiledLineageSourceFact",
    doc: "Compact upstream column fact extracted during SQL analysis.",
    fields: &[
        FieldSpec::required("resource_type"),
        FieldSpec::required("resource_name"),
        FieldSpec::required("column_name"),
    ],
};
const R_COMPACT_LINEAGE_FACTS: RecordSpec = RecordSpec {
    name: "CompactLineageFacts",
    doc: "Indexed native lineage rows with lazy object projection.",
    fields: &[
        FieldSpec::required("string_pool"),
        FieldSpec::required("rows"),
        FieldSpec::new("resource_name_indexes", FieldDefault::EmptyDict),
        FieldSpec::new("resource_names", FieldDefault::EmptyDict),
    ],
};
const R_COMPILED_OBJECT_KEY: RecordSpec = RecordSpec {
    name: "CompiledObjectKey",
    doc: "Stable logical identity for one compiled resource or external dependency.",
    fields: &[
        FieldSpec::required("resource_type"),
        FieldSpec::required("name"),
    ],
};
const R_COMPILE_MODEL_INPUT: RecordSpec = RecordSpec {
    name: "CompileModelInput",
    doc: "One discovered model file with its attached schema metadata, if any.",
    fields: &[
        FieldSpec::required("model_file"),
        FieldSpec::new(
            "config",
            FieldDefault::NativeCall(RecordId::CompileModelConfig),
        ),
        FieldSpec::new("query_sql", FieldDefault::Text("")),
        FieldSpec::new("macro_source_sql", FieldDefault::Text("")),
        FieldSpec::new("references", FieldDefault::EmptyTuple),
        FieldSpec::new("schema_entry", FieldDefault::None),
        FieldSpec::new("schema_file", FieldDefault::None),
        FieldSpec::new("sql_validation_enabled", FieldDefault::False),
        FieldSpec::new("rejected_sql_analysis_opt_out", FieldDefault::None),
        FieldSpec::new("enum_declarations", FieldDefault::EmptyTuple),
        FieldSpec::new("constant_declarations", FieldDefault::EmptyTuple),
        FieldSpec::new("enum_columns", FieldDefault::EmptyDict),
        FieldSpec::new("macro_deps", FieldDefault::EmptyTuple),
        FieldSpec::new("macro_usages", FieldDefault::EmptyTuple),
        FieldSpec::new("declaration_usages", FieldDefault::EmptyTuple),
        FieldSpec {
            name: "sql_expansion",
            default: FieldDefault::None,
            compared: false,
            shown: false,
        },
        FieldSpec {
            name: "lint_expansion",
            default: FieldDefault::None,
            compared: false,
            shown: false,
        },
    ],
};
const R_COMPILED_RELATION_LOCATION: RecordSpec = RecordSpec {
    name: "CompiledRelationLocation",
    doc: "Logical and physical relation location resolved during compile.",
    fields: &[
        FieldSpec::required("database"),
        FieldSpec::required("schema"),
        FieldSpec::required("name"),
        FieldSpec::required("qualified_name"),
        FieldSpec::new("logical_schema", FieldDefault::None),
        FieldSpec::new("logical_database", FieldDefault::None),
    ],
};
const R_COMPILED_MODEL: RecordSpec = RecordSpec {
    name: "CompiledModel",
    doc: "Planner-ready compiled model metadata.",
    fields: &[
        FieldSpec::required("key"),
        FieldSpec::required("deps"),
        FieldSpec::required("name"),
        FieldSpec::required("relative_path"),
        FieldSpec::required("query_sql"),
        FieldSpec::required("config"),
        FieldSpec::required("destination"),
        FieldSpec::new("references", FieldDefault::EmptyTuple),
        FieldSpec::new("schema_entry", FieldDefault::None),
        FieldSpec::new("inferred_columns", FieldDefault::None),
        FieldSpec::new("fast_lineage_columns", FieldDefault::None),
        FieldSpec::new("fast_lineage_has_star", FieldDefault::False),
        FieldSpec::new("fast_lineage_star_resolved", FieldDefault::False),
        FieldSpec::new("authored_sql", FieldDefault::Text("")),
        FieldSpec::new("authored_query_sql", FieldDefault::Text("")),
        FieldSpec::new("output_column_locations", FieldDefault::EmptyDict),
        FieldSpec::new("extract_implicit_alias_columns", FieldDefault::True),
        FieldSpec::new("macro_deps", FieldDefault::EmptyTuple),
        FieldSpec::new("enum_declarations", FieldDefault::EmptyTuple),
        FieldSpec::new("constant_declarations", FieldDefault::EmptyTuple),
        FieldSpec::new("enum_columns", FieldDefault::EmptyDict),
        FieldSpec::new("binding_diagnostics", FieldDefault::EmptyTuple),
        FieldSpec::new("binding_validated", FieldDefault::False),
        FieldSpec::new("dynamic_column_contract", FieldDefault::None),
        FieldSpec::new("unchecked_output_columns", FieldDefault::EmptyFrozenset),
        FieldSpec::new("rejected_sql_analysis_opt_out", FieldDefault::None),
    ],
};
const R_COMPILED_AUDIT: RecordSpec = RecordSpec {
    name: "CompiledAudit",
    doc: "Compiled audit metadata selected by scope dependencies.",
    fields: &[
        FieldSpec::required("key"),
        FieldSpec::required("scope_deps"),
        FieldSpec::required("name"),
        FieldSpec::required("definition_name"),
        FieldSpec::required("audit_file"),
        FieldSpec::required("audit_block"),
        FieldSpec::required("sql_body"),
        FieldSpec::new(
            "evaluation_mode",
            FieldDefault::HostMember(FacadeHost::AuditEvaluationMode.id(), "VIOLATIONS"),
        ),
        FieldSpec::new("measurement_contract", FieldDefault::None),
        FieldSpec::new("thresholds", FieldDefault::None),
        FieldSpec::new("minimum_samples", FieldDefault::None),
        FieldSpec::new("measure_sql", FieldDefault::None),
        FieldSpec::new("evidence_sql", FieldDefault::None),
        FieldSpec::new("evidence_limit", FieldDefault::None),
        FieldSpec::new("references", FieldDefault::EmptyTuple),
        FieldSpec::new("attached_target_kind", FieldDefault::None),
        FieldSpec::new("attached_target_name", FieldDefault::None),
        FieldSpec::new("attached_column_name", FieldDefault::None),
        FieldSpec::new("severity", FieldDefault::None),
        FieldSpec::new("run_scope", FieldDefault::None),
        FieldSpec::new("always_run", FieldDefault::False),
        FieldSpec::new("description", FieldDefault::None),
        FieldSpec::new("meta", FieldDefault::None),
    ],
};
const R_COMPILED_SQL_SCENARIO: RecordSpec = RecordSpec {
    name: "CompiledSqlScenario",
    doc: "Compiled SQL-native scenario metadata selected by inferred graph targets.",
    fields: &[
        FieldSpec::required("key"),
        FieldSpec::required("name"),
        FieldSpec::required("scenario_file"),
        FieldSpec::required("sql_body"),
        FieldSpec::new("authored_ctes", FieldDefault::EmptyTuple),
        FieldSpec::new("expected_ctes", FieldDefault::EmptyTuple),
        FieldSpec::new("assertion_ctes", FieldDefault::EmptyTuple),
        FieldSpec::new("source_fixture_names", FieldDefault::EmptyTuple),
        FieldSpec::new("ref_fixture_names", FieldDefault::EmptyTuple),
        FieldSpec::new("seed_fixture_names", FieldDefault::EmptyTuple),
        FieldSpec::new("dbt_ref_fixture_names", FieldDefault::EmptyTuple),
        FieldSpec::new("expected_model_names", FieldDefault::EmptyTuple),
        FieldSpec::new("assertion_names", FieldDefault::EmptyTuple),
        FieldSpec::new("assertion_target_model_names", FieldDefault::EmptyTuple),
        FieldSpec::new("target_model_names", FieldDefault::EmptyTuple),
        FieldSpec::new("source_path", FieldDefault::None),
        FieldSpec::new("ownership_root", FieldDefault::None),
    ],
};
const R_COMPILED_PROJECT: RecordSpec = RecordSpec {
    name: "CompiledProject",
    doc: "Planner-ready whole-project compile output.",
    fields: &[
        FieldSpec::required("run_id"),
        FieldSpec::required("effective_target_name"),
        FieldSpec::required("effective_connection"),
        FieldSpec::required("effective_vars"),
        FieldSpec {
            name: "binding_catalog",
            default: FieldDefault::None,
            compared: false,
            shown: false,
        },
        FieldSpec::new("effective_target_database", FieldDefault::None),
        FieldSpec::new("effective_target_schema", FieldDefault::None),
        FieldSpec::new("sql_analysis_dialect", FieldDefault::None),
        FieldSpec::new(
            "sql_lexical_syntax",
            FieldDefault::NativeCall(RecordId::SqlLexicalSyntax),
        ),
        FieldSpec::new("compile_cache_dir", FieldDefault::None),
        FieldSpec::new(
            "settings",
            FieldDefault::NativeCall(RecordId::SettingsConfig),
        ),
        FieldSpec::new(
            "scenario",
            FieldDefault::NativeCall(RecordId::ScenarioConfig),
        ),
        FieldSpec::new("models", FieldDefault::EmptyTuple),
        FieldSpec::new("sources", FieldDefault::EmptyTuple),
        FieldSpec::new("seeds", FieldDefault::EmptyTuple),
        FieldSpec::new("functions", FieldDefault::EmptyTuple),
        FieldSpec::new("audits", FieldDefault::EmptyTuple),
        FieldSpec::new("sql_tests", FieldDefault::EmptyTuple),
        FieldSpec::new("sql_scenarios", FieldDefault::EmptyTuple),
        FieldSpec::new("loader_functions", FieldDefault::EmptyTuple),
        FieldSpec::new("hook_functions", FieldDefault::EmptyTuple),
        FieldSpec::new("enforce_explicit_references", FieldDefault::True),
        FieldSpec::new("sql_hook_files", FieldDefault::EmptyTuple),
        FieldSpec::new("materialization_files", FieldDefault::EmptyTuple),
        FieldSpec::new("public_enums", FieldDefault::EmptyDict),
        FieldSpec::new("public_constants", FieldDefault::EmptyDict),
        FieldSpec::new("loaded_macros", FieldDefault::EmptyDict),
        FieldSpec::new("diagnostics", FieldDefault::EmptyTuple),
        FieldSpec::new("unmatched_literal_sql_relations", FieldDefault::EmptyTuple),
        FieldSpec::new("external_sql_reference_resolver", FieldDefault::None),
        FieldSpec::new(
            "scope_index",
            FieldDefault::NativeCall(RecordId::ScopeIndex),
        ),
        FieldSpec {
            name: "sql_expansions",
            default: FieldDefault::EmptyDict,
            compared: false,
            shown: false,
        },
        FieldSpec {
            name: "native_session",
            default: FieldDefault::None,
            compared: false,
            shown: false,
        },
        FieldSpec {
            name: "native_project",
            default: FieldDefault::None,
            compared: false,
            shown: false,
        },
        FieldSpec::new("python_execution_inputs", FieldDefault::EmptyDict),
    ],
};
const R_COMPILED_SQL_TEST: RecordSpec = RecordSpec {
    name: "CompiledSqlTest",
    doc: "Compiled SQL-native unit test metadata selected by expected targets.",
    fields: &[
        FieldSpec::required("key"),
        FieldSpec::required("scope_deps"),
        FieldSpec::required("name"),
        FieldSpec::required("test_file"),
        FieldSpec::required("test_block"),
        FieldSpec::required("sql_body"),
        FieldSpec::new(
            "mode",
            FieldDefault::HostMember(FacadeHost::SqlTestMode.id(), "MODEL"),
        ),
        FieldSpec::new(
            "payload",
            FieldDefault::NativeCall(RecordId::CompiledModelSqlTestPayload),
        ),
        FieldSpec::new("source_path", FieldDefault::None),
        FieldSpec::new("ownership_root", FieldDefault::None),
        FieldSpec::new("block_index", FieldDefault::None),
        FieldSpec::new("explicit_name", FieldDefault::None),
        FieldSpec::new("parent_name", FieldDefault::None),
        FieldSpec::new("case_name", FieldDefault::None),
        FieldSpec::new("case_index", FieldDefault::None),
        FieldSpec::new("case_fingerprint", FieldDefault::None),
        FieldSpec::new("parameter_schema", FieldDefault::EmptyTuple),
        FieldSpec::new("parameter_values", FieldDefault::EmptyTuple),
        FieldSpec::new("expected_model_names", FieldDefault::EmptyTuple),
        FieldSpec::new("assertion_names", FieldDefault::EmptyTuple),
        FieldSpec::new("assertion_target_model_names", FieldDefault::EmptyTuple),
        FieldSpec::new("read_helper_names", FieldDefault::EmptyTuple),
        FieldSpec::new("reference_target_model_names", FieldDefault::EmptyTuple),
        FieldSpec::new("target_model_names", FieldDefault::EmptyTuple),
        FieldSpec::new("tested_resources", FieldDefault::EmptyTuple),
    ],
};

/// The compile and discovery value records.
pub(crate) const COMPILE_VALUE_RECORDS: &[RecordSpec] = &[
    R_RELATED_LOCATION,
    R_FIXTURE_COLUMN_INFERENCE,
    R_DYNAMIC_COLUMN_FAMILY_PROOF,
    R_DYNAMIC_COLUMN_CONTRACT_PROOF,
    R_COMPILE_MODEL_CONFIG,
    R_CURSOR_INPUT_ROLES,
    R_DECLARATION_RUNTIME_PROJECTION,
    R_DECLARATION_SCOPE_RESOLVER,
    R_DECLARATION_SCOPE_BUILD,
    R_MACRO_EXPANSION_RESULT,
    R_VISIBLE_MODEL_DECLARATIONS,
    R_DECLARATION_EXPANSION_CONTEXT,
    R_COMPILE_ADAPTER_CONTEXT,
    R_INVALID_SQL_REFERENCE_CALL,
    R_SQL_REFERENCE_SCAN,
    R_COMPILED_LINEAGE_COLUMN_FACT,
    R_POLYGLOT_ANALYSIS_RESULT,
    R_ANALYSIS_CACHE_CONTEXT,
    R_COMPILE_ANALYSIS_SELECTION,
    R_COMPILE_SQL_SCENARIO_CTE,
    R_FUNCTION_ARGUMENT,
    R_FUNCTION_RETURN_COLUMN,
    R_HAND_BUILT_ANALYSIS,
    R_MODEL_SQL_ANALYSIS,
    R_COMPILE_SQL_SCENARIO_INPUT,
    R_COMPILE_PROJECT_INPUTS,
    R_COMPILED_SOURCE,
    R_COMPILED_SEED,
    R_COMPILED_FUNCTION,
    R_PYTHON_SQL_REFERENCE_REPORT,
    R_COMPILE_SQL_TEST_CTE,
    R_COMPILE_MODEL_SQL_TEST_INPUT_PAYLOAD,
    R_COMPILE_DIRECT_LOGIC_SQL_TEST_INPUT_PAYLOAD,
    R_COMPILE_SQL_TEST_INPUT,
    R_COMPILED_MODEL_SQL_TEST_PAYLOAD,
    R_SQL_EXPANSION_CONTEXT,
    R_EXPANSION_SPAN,
    R_COMPILED_SQL_EXPANSION,
    R_MAPPED_OFFSET,
    R_COMPILED_DIRECT_LOGIC_SQL_TEST_PAYLOAD,
    R_COMPILED_SQL_TEST_RESOURCE,
    R_NATIVE_MODEL_CONFIG_SESSION,
    R_NATIVE_MODEL_CONFIG_INPUTS,
    R_MODEL_VALIDATION_REQUEST,
    R_NAMED_SQL_HOOK_ENTRY,
    R_DISCOVERED_SQL_FUNCTION_FILE,
    R_DISCOVERED_SQL_HOOK_FILE,
    R_ENUM_MEMBER,
    R_DISCOVERED_ENUM_FILE,
    R_DISCOVERED_CONSTANT_FILE,
    R_MODEL_SCHEMA_DECLARATION,
    R_DISCOVERED_MODEL_SCHEMA_FILE,
    R_DISCOVERED_PYTHON_FUNCTION_FILE,
    R_DISCOVERED_SCHEMA_FILE,
    R_DISCOVERED_SEED_FILE,
    R_DISCOVERED_SQL_TEST_CASE,
    R_DISCOVERED_SQL_TEST_BLOCK,
    R_DISCOVERED_AUDIT_FILE,
    R_DISCOVERED_AUDIT_BLOCK,
    R_DISCOVERED_ADAPTER_FILE,
    R_DISCOVERY_FILE_FAULT,
    R_TOLERANT_SCOPE_DISCOVERY,
    R_SQL_HOOK_ENTRY,
    R_PYTHON_HOOK_ENTRY,
    R_ENUM_DECLARATION,
    R_CONSTANT_DECLARATION,
    R_DISCOVERED_SOURCE_FILE,
    R_DISCOVERED_SQL_TEST_FILE,
    R_SQL_TEST_PARAMETER_DECLARATION,
    R_DISCOVERED_SQL_SCENARIO_FILE,
    R_DISCOVERED_MACRO_FILE,
    R_DISCOVERED_MATERIALIZATION_FILE,
    R_DISCOVERED_PROVIDER_USAGE,
    R_DISCOVERED_LOADER_FUNCTION,
    R_DISCOVERED_TASK_FUNCTION,
    R_DISCOVERED_ASSET_FUNCTION,
    R_DISCOVERED_CHECK_FUNCTION,
    R_DISCOVERED_HOOK_FUNCTION,
    R_DISCOVERED_AUDIT_FACTORY,
    R_DISCOVERED_EVENT_EXPORTER_DECLARATION,
    R_DISCOVERED_EVENT_EXPORTER,
    R_DISCOVERED_COMMAND_OUTPUT_SINK_DECLARATION,
    R_DISCOVERED_COMMAND_OUTPUT_SINK,
    R_DISCOVERED_PROVIDER,
    R_DISCOVERED_PYTHON_NODE_FUNCTIONS,
    R_DISCOVERED_PROJECT_INPUTS,
    R_DISCOVERED_RUNTIME_EXTENSIONS,
    R_COMPILER_DIAGNOSTIC,
    R_INFERRED_COLUMN,
    R_LOADED_MACRO,
    R_STATIC_MACRO_EXPORT,
    R_STATIC_MACRO_FAULT,
    R_STATIC_MACRO_INVENTORY,
    R_MACRO_CONTEXT,
    R_DECLARATION_RESOLUTION_CONTEXT,
    R_MODEL_INPUT_BUILD_CONTEXT,
    R_COMPILE_SQL_REFERENCE,
    R_COMPILED_LINEAGE_SOURCE_FACT,
    R_COMPACT_LINEAGE_FACTS,
    R_COMPILED_OBJECT_KEY,
    R_COMPILE_MODEL_INPUT,
    R_COMPILED_RELATION_LOCATION,
    R_COMPILED_MODEL,
    R_COMPILED_AUDIT,
    R_COMPILED_SQL_SCENARIO,
    R_COMPILED_PROJECT,
    R_COMPILED_SQL_TEST,
];
