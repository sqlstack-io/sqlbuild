//! Limits bound each authenticated warehouse observation transaction.

use crate::warehouse_metadata::models::WarehouseHistoryKind;

/// Statistics relations sampled per page, and other bounded statistics work; each sample is a query.
pub const MAX_HISTORY_RECORDS: usize = 100;
/// Rows of one state-table or catalog page; the reply's byte budget usually ends a page first.
pub const MAX_HISTORY_PAGE_RECORDS: usize = 2000;
/// Encoded records one page carries before the reader ends it, far below the API body bound.
pub const HISTORY_PAGE_BYTES: usize = 8 * 1024 * 1024;
pub(crate) const CATALOG_MIRROR: &str = "sqb_warehouse_catalog";
pub(crate) const DEFAULT_STATISTICS_RETENTION_DAYS: u16 = 90;
/// Builds and check results a SQLite store keeps per node or check unless configured.
pub const DEFAULT_SQLITE_STATE_HISTORY_VERSIONS: u16 = 5;
/// The PostgreSQL default matches the janitor's `direct_state_history_versions` default.
pub const DEFAULT_POSTGRES_STATE_HISTORY_VERSIONS: u16 = 20;
/// Mirrored state-history rows one pruning transaction deletes, so writers never wait long.
pub(crate) const STATE_HISTORY_PRUNE_BATCH: usize = 500;
/// Builds ranked past the limit within their node, in one state namespace; the newest always stays.
pub(crate) const PRUNE_FINGERPRINTS_SQL: &str = "DELETE FROM sqb_warehouse_fingerprints WHERE event_id IN (SELECT event_id FROM \
    (SELECT event_id, ROW_NUMBER() OVER (PARTITION BY location_id, target, state_database, state_schema, \
    node_type, node_name, target_database, target_schema, target_name ORDER BY observed_at DESC, event_id DESC) \
    AS version FROM sqb_warehouse_fingerprints) ranked WHERE version > $1 LIMIT $2) RETURNING definition_text_hash";

/// Check results ranked past the limit within their check binding, in one state namespace.
pub(crate) const PRUNE_AUDIT_RESULTS_SQL: &str = "DELETE FROM sqb_warehouse_audit_results WHERE event_id IN (SELECT event_id FROM \
    (SELECT event_id, ROW_NUMBER() OVER (PARTITION BY location_id, target, state_database, state_schema, \
    COALESCE(binding_key, audit_name) ORDER BY observed_at DESC, event_id DESC) AS version \
    FROM sqb_warehouse_audit_results) ranked WHERE version > $1 LIMIT $2) RETURNING event_id";
/// Free pages, as a share of the SQLite file, above which pruning returns the space to the disk.
pub(crate) const SQLITE_RECLAIM_FREE_SHARE: f64 = 0.25;
/// Free pages below this are not worth reclaiming, whatever their share.
pub(crate) const SQLITE_RECLAIM_MIN_FREE_PAGES: i64 = 1024;
/// Free pages one incremental vacuum step returns to the disk.
pub(crate) const SQLITE_RECLAIM_STEP_PAGES: i64 = 4096;
/// SQLite's `auto_vacuum` value for incremental mode.
pub(crate) const SQLITE_AUTO_VACUUM_INCREMENTAL: i64 = 2;
/// Column-lineage node definitions read per query, well under every backend's bound parameter limit.
pub(crate) const LINEAGE_DEFINITION_BATCH: usize = 1000;
/// Mirrored tables a plan preview needs synced cleanly; observations only refine it into knowns.
pub(crate) const PREVIEW_REQUIRED_TABLES: [WarehouseHistoryKind; 3] = [
    WarehouseHistoryKind::Catalog,
    WarehouseHistoryKind::Fingerprints,
    WarehouseHistoryKind::Consumption,
];

/// The target's newest runs read for node outcomes, and as many newest failed or warned runs; never other targets'.
pub(crate) const OUTCOME_RECENT_RUNS: i64 = 25;
/// How far back, in runs, the newest failed or warned runs are looked for.
pub(crate) const OUTCOME_FAILURE_WINDOW: i64 = 500;
/// Records one ingestion transaction applies before committing, so a page never holds the write lock long.
pub(crate) const INGESTION_CHUNK_RECORDS: usize = 200;

/// The audit result metadata field marking a failure its build let through.
pub(crate) const IGNORED_KEY: &str = "ignored";
/// The mirrored catalog of one location's target, with when each relation was observed.
pub(crate) const SYNCED_CATALOG_SQL: &str = "SELECT record_json, observed_at, scan_id, received_at FROM sqb_warehouse_catalog \
    WHERE location_id=$1 AND target=$2 ORDER BY state_database, state_schema, event_id";
/// The development locations of one target, by project path.
pub(crate) const DEVELOPMENT_LOCATIONS_SQL: &str =
    "SELECT project_path, location_id FROM sqb_development_locations WHERE target=$1";
