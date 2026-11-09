//! Versions, kinds and keys of the static DAG artifact.

/// The artifact's `version`.
pub(crate) const DAG_VERSION: i64 = 1;
/// Keys whose empty collections are written instead of dropped.
pub(crate) const REQUIRED_COLLECTION_KEYS: [&str; 3] = ["nodes", "edges", "checks"];
/// `json.dumps(..., indent=2)`.
pub(crate) const DAG_INDENT: usize = 2;
pub(crate) const SOURCE_KIND: &str = "source";
pub(crate) const LOADER_KIND: &str = "loader";
pub(crate) const SEED_KIND: &str = "seed";
pub(crate) const MODEL_KIND: &str = "model";
pub(crate) const UDF_KIND: &str = "udf";
pub(crate) const TABLE_FUNCTION_KIND: &str = "table_fn";
pub(crate) const ASSET_KIND: &str = "asset";
pub(crate) const CHECK_KIND: &str = "check";
pub(crate) const SQL_TEST_KIND: &str = "sql_test";
pub(crate) const AUDIT_KIND: &str = "audit";
pub(crate) const SCENARIO_KIND: &str = "scenario";
pub(crate) const PYTHON_CHECK_KIND: &str = "python_check";
pub(crate) const DEFAULT_MATERIALIZATION_PYTHON_ASSET: &str = "python_asset";
pub(crate) const TABLE_RETURN_KIND: &str = "table";
pub(crate) const SCALAR_RETURN_KIND: &str = "scalar";
/// The meta of a loader node no source declares.
pub(crate) const LOADER_NODE_META_KEY: &str = "sqlbuild_loader_node";
/// The destination of a loader without one.
pub(crate) const LOADER_DESTINATION_PREFIX: &str = "__loader__";
/// The most parts a loader destination may have.
pub(crate) const LOADER_MAX_PARTS: usize = 3;
pub(crate) const SELECTOR_KIND_NAME: &str = "name";
pub(crate) const SELECTOR_KIND_TAG: &str = "tag";
pub(crate) const SELECTOR_KIND_PATH: &str = "path";
pub(crate) const SELECTOR_KIND_CHECK: &str = "check";
pub(crate) const SELECTOR_KIND_TEST: &str = "test";
pub(crate) const SELECTOR_KIND_JOB: &str = "job";
pub(crate) const SELECTOR_KIND_SEPARATOR: &str = ":";
pub(crate) const SELECTOR_MODELS_PREFIX: &str = "models/";
/// The orchestrator kind every SQLBuild asset carries.
pub(crate) const SQLBUILD_ASSET_KIND: &str = "sqlbuild";
