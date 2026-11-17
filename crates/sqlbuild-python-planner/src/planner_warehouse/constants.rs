//! Public constants of planner warehouse.

use sqlbuild_host::entries::models::{AdapterHost, ExecutorHost};
use sqlbuild_python_base::records::types::RecordId;

pub const RUN_BOUNDED: AdapterHost = AdapterHost::RunBoundedInspections;

pub(crate) const COLUMN_INFO: AdapterHost = AdapterHost::ColumnInfo;

pub(crate) const RELATION_LOOKUP: AdapterHost = AdapterHost::RelationLookup;

pub(crate) const LOG_DEBUG_EVENT: ExecutorHost = ExecutorHost::LogDebugEvent;

pub(crate) const RUN_SCOPE: ExecutorHost = ExecutorHost::RunScope;

pub(crate) const SQL_HOOK_ENTRY: RecordId = RecordId::SqlHookEntry;

pub(crate) const CURSOR_ALGEBRA_ERROR: (&str, &str) =
    ("sqlbuild.cursor_algebra.exceptions", "CursorAlgebraError");

pub(crate) const DATE_VALUE: RecordId = RecordId::DateValue;

pub(crate) const PLANNER_LOGGER: &str = "sqlbuild.planner";

pub(crate) const METADATA_NAME_FILTER_LIMIT: usize = 250;

pub(crate) const FUNCTION_NODE_TYPES: [&str; 2] = ["udf", "table_fn"];

pub(crate) const WILDCARD_TABLE_CHARACTER: &str = "*";

pub(crate) const MIGRATE_FROM_CONFIG_KEY: &str = "migrate_from";

pub(crate) const MODEL_HOOK_CONFIG_KEYS: [&str; 2] = ["pre_hooks", "post_hooks"];

pub(crate) const CURSOR_TYPE_TIMESTAMP: &str = "timestamp";

pub(crate) const CURSOR_TYPE_INTEGER: &str = "integer";

pub(crate) const MISSING_SOURCE_TABLE_CODE: &str = "S405";

pub(crate) const WATERMARK_CONTRACT_CODE: &str = "S302";

/// Why a plan preview leaves a source's freshness unknown; its plan lists freshness as unknown.
pub const PREVIEW_MESSAGE: &str = "a plan preview does not observe source freshness";

/// Items a preview always reports as known only once the run plans live.
pub const PREVIEW_UNKNOWNS: [&str; 5] = [
    "source freshness",
    "retention",
    "column rename support",
    "sources outside mirrored schemas",
    "recorded migrations",
];

/// What a preview's cursor reads leave to the run.
pub const CURSOR_INPUTS: &str = "cursor inputs";

/// Cursor reads one `UNION ALL` statement answers, so a plan runs few statements, each small.
pub(crate) const CURSOR_BATCH_SIZE: usize = 50;
