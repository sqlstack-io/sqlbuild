//! Non-definition metadata excluded by the planner's version identity, and fingerprint dialect names.

pub const AUDIT_GATE_METADATA_KEY: &str = "audit_gate";
pub const MIGRATION_FINGERPRINT_METADATA_KEY: &str = "migration_fingerprint";
pub const DECLARED_COLUMNS_METADATA_KEY: &str = "declared_columns_hash";
pub const LOCAL_FUNCTION_HASHES_METADATA_KEY: &str = "local_function_hashes";
/// Metadata key of the model config entries in version identity.
pub const CONFIG_METADATA_KEY: &str = "config";
/// Config key of a maintained model's refresh mode.
pub const REFRESH_MODE_CONFIG_KEY: &str = "refresh_mode";
/// The refresh mode of a maintained model that declares none; writing it out changes nothing.
pub const DEFAULT_REFRESH_MODE: &str = "incremental";
/// The fingerprint dialect when a project names none.
pub(crate) const GENERIC_FINGERPRINT_DIALECT: &str = "generic";
/// Python's `NATIVE_DIALECT_ALIASES`.
pub(crate) const FINGERPRINT_DIALECT_ALIASES: [(&str, &str); 3] = [
    ("postgres", "postgresql"),
    ("motherduck", "duckdb"),
    ("sqlserver", "tsql"),
];
