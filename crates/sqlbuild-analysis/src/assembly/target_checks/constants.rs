//! Error codes and values of the target namespace checks.

/// A target, resource or loader schema that keeps the logical schema.
pub const PRESERVE_TARGET_VALUE: &str = "preserve";
/// Execution needs an explicit database and schema for every model and seed.
pub const MISSING_TARGET_PARTS_CODE: &str = "S101";
/// Two targets write managed loader data to one namespace.
pub const LOADER_COLLISION_CODE: &str = "S102";
/// A managed write has no explicit physical schema.
pub const MANAGED_WRITE_SCHEMA_CODE: &str = "S103";
/// A named target chooses no schema strategy.
pub const NAMED_TARGET_SCHEMA_CODE: &str = "S104";
/// A target that keeps each model's schema has no schema for Python node results.
pub const PRESERVED_NODE_RESULTS_CODE: &str = "S105";
pub(crate) const DATABASE_PART: &str = "database";
pub(crate) const SCHEMA_PART: &str = "schema";
