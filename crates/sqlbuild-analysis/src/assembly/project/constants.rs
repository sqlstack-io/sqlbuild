//! Python's resource, reference and target constants the project assembly reproduces.

pub(crate) const MODEL_RESOURCE: &str = "model";
pub(crate) const SOURCE_KIND: &str = "source";
pub(crate) const SEED_KIND: &str = "seed";
pub(crate) const DBT_REF_KIND: &str = "dbt_ref";
pub(crate) const UDF_KIND: &str = "udf";
pub(crate) const TABLE_FUNCTION_KIND: &str = "table_fn";
/// The reference kinds whose dependency keeps the reference's kind as its resource type.
pub(crate) const KIND_RESOURCES: [&str; 4] =
    [SOURCE_KIND, SEED_KIND, UDF_KIND, TABLE_FUNCTION_KIND];
/// `AttachedAuditTargetKind` values, each also its resource type.
pub(crate) const ATTACHED_AUDIT_KINDS: [&str; 3] = [MODEL_RESOURCE, SOURCE_KIND, SEED_KIND];
pub(crate) const PRESERVE_TARGET_VALUE: &str = "preserve";
pub(crate) const TEMPLATE_OPEN_TOKEN: &str = "${";
pub(crate) const PYTHON_NONE: &str = "None";
pub(crate) const PYTHON_TRUE: &str = "True";
pub(crate) const PYTHON_FALSE: &str = "False";
pub(crate) const MODEL_NAME_CONTEXT: &str = "model.name";
pub(crate) const MODEL_DATABASE_CONTEXT: &str = "model.database";
pub(crate) const MODEL_SCHEMA_CONTEXT: &str = "model.schema";
pub(crate) const MODEL_ALIAS_CONTEXT: &str = "model.alias";
pub(crate) const DESTINATION_DATABASE_CONTEXT: &str = "destination.database";
pub(crate) const DESTINATION_SCHEMA_CONTEXT: &str = "destination.schema";
pub(crate) const DESTINATION_TABLE_CONTEXT: &str = "destination.table";
pub(crate) const DESTINATION_QUALIFIED_CONTEXT: &str = "destination.qualified";
/// `render_project_var_text` of booleans inside a larger template string.
pub(crate) const PROJECT_VAR_TRUE: &str = "true";
pub(crate) const PROJECT_VAR_FALSE: &str = "false";
/// Python's message when `validate` rejects SQL without naming an error.
pub(crate) const INVALID_SQL: &str = "invalid SQL";
/// The `context_label`s Python's target expansions name in their errors.
pub(crate) const MANAGED_SOURCE_TARGET_LABEL: &str = "managed source target";
pub(crate) const DEFAULT_SEED_DATABASE_LABEL: &str = "default seed database";
pub(crate) const DEFAULT_SEED_SCHEMA_LABEL: &str = "default seed schema";
pub(crate) const TARGET_DATABASE_LABEL: &str = "target database";
pub(crate) const TARGET_SCHEMA_LABEL: &str = "target schema";
pub(crate) const DATABASE_DIMENSION: &str = "database";
pub(crate) const SCHEMA_DIMENSION: &str = "schema";
/// An `ENV:` name the request did not look up, which the request builder's scan always finds.
pub(crate) const UNLISTED_ENVIRONMENT: &str =
    "a target template read an environment variable the request did not list";
/// `AttachedAuditTargetKind`'s name in the `ValueError` its constructor raises.
pub(crate) const AUDIT_TARGET_KIND_ENUM: &str = "AttachedAuditTargetKind";
/// The prefix of an environment read in a target template.
pub(crate) const ENVIRONMENT_READ_PREFIX: &str = "ENV:";
/// Characters that end an `ENV:` name besides Python whitespace.
pub(crate) const ENVIRONMENT_NAME_TERMINATORS: [char; 6] = ['(', ')', ',', '\'', '"', '}'];
/// The MODEL header opt-out help of SQL the parser rejects.
pub(crate) const MODEL_OPT_OUT_HELP: &str = "if this SQL is valid for your warehouse, skip SQL \
    analysis for this model, add this to the MODEL header:\n            MODEL (\n              \
    sql_analysis false,\n              ...\n            );\n          and please report the SQL \
    so the parser can support it";
/// The report-only help of rejected SQL that no model header owns.
pub(crate) const REPORT_ONLY_HELP: &str =
    "if this SQL is valid for your warehouse, please report it so the parser can support it";
/// The project-wide and single-run opt-out helps, each after its own help label.
pub(crate) const PROJECT_AND_RUN_OPT_OUT_HELP: &str = "\n  = help: SQL analysis is on for this \
    project; to turn it off for every model, set this in sqlbuild_project.toml:\n            \
    [settings]\n            sql_analysis = false\n  = help: to skip SQL analysis for one run, \
    use `--no-sql-analysis`";
