pub const SQL_WILDCARD: &str = "*";
pub const TIMESTAMP_TYPE: &str = "TIMESTAMP";
pub const OPEN_PAREN: &str = "(";
pub const CLOSE_PAREN: &str = ")";
/// The error text every caught native panic carries across the boundary.
pub const PANIC_MESSAGE: &str = "NativeCompilerError: native SQL compilation panicked";
/// The prefix every native internal failure carries, so the boundary raises `NativeCompilerError`.
pub const NATIVE_FAILURE_PREFIX: &str = "NativeCompilerError: ";
/// File name of the project configuration at the project root.
pub const PROJECT_CONFIG_FILENAME: &str = "sqlbuild_project.toml";
/// Project configuration section holding settings.
pub const SETTINGS_SECTION: &str = "settings";
/// Settings key selecting how table builds are promoted.
pub const TABLE_PROMOTION_MODE_SETTING_KEY: &str = "table_promotion_mode";
/// Table promotion mode that builds into a staging relation before swapping.
pub const STAGED_PROMOTION_MODE: &str = "staged";
/// Project directory holding Python node modules.
pub const PYTHON_NODE_ROOT: &str = "python";
/// Stem of a Python package's initializer module.
pub const PYTHON_INIT_MODULE_STEM: &str = "__init__";
/// State table of model, seed and hook fingerprints.
pub const FINGERPRINT_TABLE_NAME: &str = "_sqlbuild_fingerprints";
/// State table of observed source freshness.
pub const SOURCE_FRESHNESS_TABLE_NAME: &str = "_sqlbuild_source_freshness";
/// State table of the inputs each model and seed execution consumed.
pub const CONSUMPTION_TABLE_NAME: &str = "_sqlbuild_consumption";
/// State table of model migration events.
pub const MIGRATION_TABLE_NAME: &str = "_sqlbuild_migrations";
/// State table of column migration events.
pub const COLUMN_MIGRATION_TABLE_NAME: &str = "_sqlbuild_column_migrations";
/// State table of old-name view events.
pub const OLD_NAME_VIEW_TABLE_NAME: &str = "_sqlbuild_old_name_views";
/// A zero-length cursor duration.
pub const ZERO_DAY_DURATION: &str = "0d";
/// Node key of a common table expression in the polyglot SQL AST.
pub const CTE_AST_KIND: &str = "cte";
