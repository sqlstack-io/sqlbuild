//! Public constants of planner strategy.

pub const AUDIT_TYPES_MODULE: &str = "sqlbuild.compiler.auditing.types";

pub const COMPILE_TYPES_MODULE: &str = "sqlbuild.compiler.compile.types";

pub const MODEL_ATTACHMENT: &str = "model";

pub const SOURCE_ATTACHMENT: &str = "source";

pub const INCREMENTAL_MATERIALIZATION: &str = "incremental";

pub const MICROBATCH_MODE: &str = "microbatch";

pub const CUSTOM_MATERIALIZATION: &str = "custom";

pub const ENFORCED_CONTRACT: &str = "enforced";

pub const TIMESTAMP_CURSOR: &str = "timestamp";

pub const MODEL_REFERENCE: &str = "ref";

pub const SEED_REFERENCE: &str = "seed";

pub const SOURCE_REFERENCE: &str = "source";

pub const DDL_SEPARATOR: &str = ";\n\n";

/// The lineage confidence whose reads fixtures must provide.
pub(crate) const HIGH_CONFIDENCE: &str = "high";

/// The lineage transform kind that passes an input column's type through.
pub(crate) const DIRECT_TRANSFORM: &str = "direct";

/// Resource type values keying fixture relations.
pub const MODEL_REFERENCE_KIND: &str = "model";

pub(crate) const SOURCE_REFERENCE_KIND: &str = "source";

pub(crate) const SEED_REFERENCE_KIND: &str = "seed";

/// The SQL analysis dialect whose adapter lays tables out by the physical storage keys.
pub const PHYSICAL_STORAGE_DIALECT: &str = "clickhouse";

/// The error code for physical storage keys on an adapter without them.
pub const PHYSICAL_STORAGE_UNSUPPORTED_CODE: &str = "A011";

/// The error code for `replace_key` on an adapter that cannot replace groups of rows.
pub const GROUP_REPLACEMENT_UNSUPPORTED_CODE: &str = "A012";

/// Plan action values that create the model's table anew or leave it untouched.
pub const TABLE_RECREATING_ACTIONS: [&str; 2] = ["create_table", "skip"];
