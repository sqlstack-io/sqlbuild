//! Polyglot expression kinds and SQLBuild reference spellings read by fast lineage.

pub(crate) const STAR_COLUMN_NAME: &str = "*";
pub(crate) const KIND_ALIAS: &str = "alias";
pub(crate) const KIND_COLUMN: &str = "column";
pub(crate) const KIND_SELECT: &str = "select";
pub(crate) const KIND_TABLE: &str = "table";
pub(crate) const KIND_UNION: &str = "union";
pub(crate) const SET_OPERATION_KINDS: &[&str] = &["union", "intersect", "except"];
pub(crate) const CAST_KINDS: &[&str] = &["cast", "try_cast"];
pub(crate) const AGGREGATE_KINDS: &[&str] = &[
    "avg",
    "count",
    "max",
    "min",
    "sum",
    "array_agg",
    "string_agg",
];
pub(crate) const PAYLOAD_COLUMN: &str = "column";
pub(crate) const PAYLOAD_NAME: &str = "name";
pub(crate) const PAYLOAD_TABLE: &str = "table";
pub(crate) const SET_OPERATION_SIDES: [&str; 2] = ["left", "right"];
pub(crate) const REFERENCE_PREFIX: &str = "__";
pub(crate) const UDF_CALL: &str = "__udf(";
pub(crate) const PHYSICAL_RESOURCE_PREFIX: &str = "__sqlbuild_";
pub(crate) const PHYSICAL_NAME_REPLACEMENT: &str = "__";
/// The column type Python's schema mapping gives a column without one.
pub(crate) const UNKNOWN_COLUMN_TYPE: &str = "UNKNOWN";
/// Rich lineage workers, matching the compile's analysis pool.
pub(crate) const RICH_LINEAGE_WORKERS: usize = 4;
/// Polyglot analysis recurses deeply on long set-operation chains.
pub(crate) const RICH_LINEAGE_WORKER_STACK_BYTES: usize = 16 * 1024 * 1024;
/// Suffixes (lowercased) of the authored files the relation lineage fingerprint hashes.
pub(crate) const FINGERPRINT_SUFFIXES: &[&str] = &[".csv", ".py", ".sql", ".toml", ".yaml", ".yml"];
/// File names hashed wherever they appear, whatever their suffix.
pub(crate) const FINGERPRINT_ROOT_FILES: &[&str] = &[".gitignore", ".sqlbuildignore"];
/// Bound retained file contents while preserving parallel reads and ordered hashing.
pub(crate) const FINGERPRINT_SCAN_BATCH_SIZE: usize = 64;
/// Top-level entries the fingerprint never reads.
pub(crate) const FINGERPRINT_EXCLUDED_ROOTS: &[&str] = &[
    ".git",
    ".mypy_cache",
    ".pytest_cache",
    ".ruff_cache",
    ".tox",
    ".venv",
    "node_modules",
    "target",
    "venv",
];
/// A path part excluded at any depth.
pub(crate) const FINGERPRINT_EXCLUDED_PART: &str = "__pycache__";
pub(crate) const ENVIRONMENT_MARKER: &[u8] = b"ENV:";
pub(crate) const DYNAMIC_CONTEXT_MARKER: &[u8] = b"CTX:";
/// Suffixes whose dynamic context marker makes the graph uncacheable.
pub(crate) const DYNAMIC_CONTEXT_SUFFIXES: &[&str] = &[".py", ".toml"];
pub(crate) const MISSING_ENVIRONMENT_VALUE: &str = "<missing>";
/// A name directly followed by a byte from here on makes the graph uncacheable.
pub(crate) const NON_ASCII_BYTE_START: u8 = 128;
