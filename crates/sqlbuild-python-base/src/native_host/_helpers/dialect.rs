//! Which adapter attributes a plan's SQL dialect may read.

/// Adapter attributes that only write SQL or describe the dialect, by prefix.
const SYNTAX_PREFIXES: [&str; 3] = ["render_", "_render_", "supports_"];
/// Adapter attributes that only describe the dialect or its defaults; none takes a connection.
const SYNTAX_ATTRIBUTES: [&str; 34] = [
    "adapter_name",
    "allows_implicit_managed_write_schema",
    "build_cursor_filter",
    "collection_rendering",
    "cursor_type_range",
    "default_database",
    "default_promotion_strategy",
    "default_schema",
    "default_table_promotion_mode",
    "expression_inference_profile",
    "format_row_diff_decimal_sql",
    "lists_relations_authoritatively",
    "max_identifier_length",
    "max_statement_size",
    "maximum_identifier_length",
    "normalize_row_diff_numeric_type",
    "persists_python_functions",
    "python_functions_inherit_default_namespace",
    "relation_grants_supported",
    "relation_names_match",
    "requires_derived_table_aliases",
    "retention_state_from_relation",
    "sql_analysis_dialect",
    "sql_analysis_dialect_name",
    "sql_lexical_syntax",
    "star_exclude_keyword",
    "state_timestamp_type",
    "table_fn",
    "table_type_entries",
    "table_type_downgrade",
    "untyped_column_type",
    "value_renderer",
    "views_bind_to_relation_identity",
    "views_read_with_reader_access",
];

/// Whether `name` only writes SQL or describes the dialect.
pub(crate) fn syntax(name: &str) -> bool {
    SYNTAX_PREFIXES
        .iter()
        .any(|prefix| name.starts_with(prefix))
        || SYNTAX_ATTRIBUTES.contains(&name)
}
