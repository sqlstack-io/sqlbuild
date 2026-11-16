//! Public constants of compile driver.

pub const EXPRESSION_SHAPES: &str = "infer_native_expression_source_shapes";

pub const EXPRESSION_SHAPE_BATCHES: &str = "expression_shape_batches";

pub const EXPRESSION_SHAPES_INFERRED: &str = "expression_shapes_inferred";

pub const COLUMN_LINEAGE: &str = "build_project_column_lineage";

pub const MODEL_CONTRACTS: &str = "evaluate_model_contracts";

pub const PROJECT_DISCOVERIES: &str = "discover_project_inputs";

pub const MODEL_ANALYSIS: &str = "analyze_native_model_sql";

pub const SEMANTIC_COMPLETION: &str = "complete_semantic_project";

pub const SQL_TEST_ASSEMBLY: &str = "assemble_native_sql_tests";

pub const PROMOTION_CONFLICTS: &str = "promotion_conflict_diagnostics";

pub const PIVOT_PROOFS: &str = "pivot_proofs";

pub const SESSION_PIVOT_PROOFS: &str = "session_pivot_proofs";

pub const DECLARATION_SCOPES: &str = "build_declaration_scope";

pub const LINEAGE_GRAPH_BUILDS: &str = "lineage_graph_builds";

pub const PROJECT_GRAPHS: &str = "build_project_graph";

pub const FORMAT_LINT_BATCHES: &str = "format_lint_batches";

pub const FORMAT_LINT_FILES: &str = "format_lint_files";

pub const FORMAT_CACHE_HITS: &str = "format_cache_hits";

pub const FORMAT_CACHE_MISSES: &str = "format_cache_misses";

pub const SQL_TEST_ARTIFACT_PLANS: &str = "plan_compiled_sql_test_artifacts";

pub const SQL_TEST_MODEL_CHAINS: &str = "sql_test_model_chain_names_by_key";

pub const BACKGROUND_SQL_TEST_PLANS: &str = "background_sql_test_plans";

/// Models whose header columns are reused from the native header parse.
pub const NATIVE_HEADER_COLUMNS: &str = "native_header_columns";

/// Models whose header columns are parsed again from their config values.
pub const PARSED_HEADER_COLUMNS: &str = "parsed_header_columns";

/// Config values converted into typed nodes for a key a reader asked for.
pub const TYPED_CONFIG_READS: &str = "typed_config_reads";

/// Reads of a model or project that is not a native compiled view, read by attribute instead.
pub const COMPILED_VIEW_FALLBACKS: &str = "compiled_view_fallbacks";
