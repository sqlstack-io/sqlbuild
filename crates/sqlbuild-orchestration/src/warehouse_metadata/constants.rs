//! Limits bound each authenticated warehouse observation transaction.

pub const MAX_HISTORY_RECORDS: usize = 100;
pub(crate) const CATALOG_MIRROR: &str = "sqb_warehouse_catalog";
pub(crate) const DEFAULT_STATISTICS_RETENTION_DAYS: u16 = 90;
/// Column-lineage node definitions read per query, well under every backend's bound parameter limit.
pub(crate) const LINEAGE_DEFINITION_BATCH: usize = 1000;
