pub(crate) const UNIQUE_AUDIT: &str = "unique";
pub(crate) const NOT_NULL_AUDIT: &str = "not_null";
pub(crate) const INCREMENTAL_MATERIALIZATION: &str = "incremental";
pub(crate) const MICROBATCH_MODE: &str = "microbatch";
pub(crate) const KEYED_DELETE_INSERT_STRATEGY: &str = "delete_insert";
pub(crate) const KEY_PRESERVING_STRATEGIES: [&str; 2] = ["merge", KEYED_DELETE_INSERT_STRATEGY];
pub(crate) const RELATION_KIND_REF: &str = "ref";
pub(crate) const RELATION_KIND_SEED: &str = "seed";
