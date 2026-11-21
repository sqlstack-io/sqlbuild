//! Reference call prefixes and the contexts Python names in reference errors.

use crate::sql_references::types::ReferencePrefix;

/// Call prefixes with their kinds, in Python's matching order.
pub(crate) const REFERENCE_PREFIXES: [ReferencePrefix; 6] = [
    (b"__dbt_ref(", "dbt_ref"),
    (b"__table_fn(", "table_fn"),
    (b"__source(", "source"),
    (b"__seed(", "seed"),
    (b"__udf(", "udf"),
    (b"__ref(", "ref"),
];
pub(crate) const DBT_REFERENCE_KIND: &str = "dbt_ref";
pub(crate) const TABLE_FUNCTION_REFERENCE_KIND: &str = "table_fn";
pub(crate) const REFERENCE_CONTEXT: &str = "SQL reference";
pub(crate) const TABLE_FUNCTION_CALL_CONTEXT: &str = "SQL table function call";
/// Characters that may start a comment or quoted text under a supported syntax.
pub(crate) const NON_CODE_START_BYTES: &[u8] = b"-/#'\"`$";
/// Non-code openers whose text an argument keeps; comments become one space.
pub(crate) const QUOTE_BYTES: &[u8] = b"'\"`$";
/// The shortest argument whose quotes Python strips: the two quotes.
pub(crate) const QUOTED_NAME_MINIMUM_BYTES: usize = 2;
/// Quotes Python strips from an authored reference name.
pub(crate) const NAME_QUOTE_BYTES: &[u8] = b"'\"";
/// Python's placeholder names for a rejected call's corrected form, by kind.
pub(crate) const PLACEHOLDER_NAMES: [(&str, &[&str]); 6] = [
    ("ref", &["model_name"]),
    ("source", &["source_name"]),
    ("seed", &["seed_name"]),
    ("udf", &["function_name"]),
    ("table_fn", &["function_name"]),
    ("dbt_ref", &["package_name", "model_name"]),
];

/// The words that open a partition clause inside `OVER (`, in order.
pub(crate) const PARTITION_CLAUSE_WORDS: [&str; 2] = ["PARTITION", "BY"];
