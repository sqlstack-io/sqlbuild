//! Contract codes, config values and the fixed help texts Python emits.

pub(crate) const MISSING_COLUMN_CODE: &str = "K001";
pub(crate) const TYPE_MISMATCH_CODE: &str = "K002";
pub(crate) const UNKNOWN_TYPE_CODE: &str = "K003";
pub(crate) const NULLABILITY_MISMATCH_CODE: &str = "K004";
pub(crate) const EXTRA_COLUMN_CODE: &str = "K005";
pub(crate) const MISSING_DECLARATIONS_CODE: &str = "K006";
pub(crate) const DYNAMIC_OUTPUT_NOT_PROVEN_CODE: &str = "K011";
pub(crate) const PROMOTION_CONFLICT_CODE: &str = "K011";
pub(crate) const ENUM_LITERAL_CODE: &str = "K013";
/// Dialects whose default string comparison ignores case or trailing spaces, so K013 is skipped.
pub(crate) const CASE_INSENSITIVE_DIALECTS: [&str; 1] = ["tsql"];

pub const CONTRACT_ENFORCED: &str = "enforced";
pub(crate) const CONTRACT_NONE: &str = "none";
pub(crate) const STAGED_LIFECYCLE_MATERIALIZATIONS: [&str; 2] = ["table", "incremental"];
pub(crate) const MICROBATCH_INCREMENTAL_MODE: &str = "microbatch";
pub(crate) const IMMEDIATE_PROMOTION_MODE: &str = "immediate";

pub(crate) const NO_COMPILER_EVIDENCE: &str = "no compiler evidence";
pub(crate) const DYNAMIC_OUTPUT_NOT_PROVEN_HELP: &str = "project one supported runtime-dynamic pivot through a wildcard output boundary and declare every generated family";
pub(crate) const DYNAMIC_FAMILY_UNKNOWN_TYPE_HELP: &str = "establish an authoritative type on the pivot value column or CAST the type-preserving aggregate input explicitly";
pub(crate) const DYNAMIC_FAMILY_TYPE_MISMATCH_HELP: &str =
    "correct the family type or the pivot aggregate input type";
pub(crate) const MISSING_DECLARATIONS_HELP: &str = "add MODEL(columns (...)), declare a proven MODEL(dynamic_columns (...)) family, or set contract none for this model";
pub(crate) const EXTRA_COLUMN_NAMED_SCHEMA_HELP: &str =
    "add the column to the named SCHEMA or remove it from the SELECT list";
pub(crate) const EXTRA_COLUMN_HELP: &str =
    "add the column to MODEL(columns) or remove it from the SELECT list";
pub(crate) const NULLABILITY_HELP: &str =
    "use COALESCE, filter nulls explicitly, or remove the non-null contract";
pub(crate) const NULLABLE_OUTPUT_MESSAGE: &str = "output expression proven nullable";
pub(crate) const UNPROVEN_TYPE_OUTPUT_MESSAGE: &str = "output expression with unproven type";
pub(crate) const UNKNOWN_TYPE_HELP: &str =
    "add an explicit CAST if this declared type should be checked statically";
pub(crate) const TYPE_MISMATCH_HELP: &str =
    "change the declared type or cast the expression explicitly";
pub(crate) const NAMED_SCHEMA_DECLARATION: &str = "the named SCHEMA declaration";
pub(crate) const MODEL_COLUMNS_DECLARATION: &str = "MODEL(columns (...))";
pub(crate) const PROMOTION_REMOVE_CONTRACT_HELP: &str = "or remove the model's enforced contract: `contract enforced` in its MODEL header, or `contract = \"enforced\"` in the [defaults] or [path_defaults] entry that applies to it";

pub(crate) const SETTING_SNIPPET_INDENT: &str = "            ";
pub(crate) const ADDITIONAL_HELP_SEPARATOR: &str = "\n  = help: ";
/// The dialect that folds unquoted identifiers to upper case.
pub(crate) const UPPER_FOLDING_DIALECT: &str = "snowflake";
