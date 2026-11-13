//! Private interpreter protocol shapes owned by the location supervisor.

use chrono::{DateTime, Utc};
use sqlbuild_orchestration::snapshots::models::SnapshotRecord;
use sqlbuild_orchestration::warehouse_metadata::models::{WarehouseHistoryPage, WarehouseStateLocation};

use crate::native_commands::models::LocationMetadataSync;

/// Publication metadata and the earliest calendar refresh deadline.
pub(crate) type LocationPublication = (
    SnapshotRecord,
    Option<DateTime<Utc>>,
    Vec<WarehouseStateLocation>,
);

/// A read page with its missing rows filled in, or the sync turn's early result.
pub(crate) type KeyedPage =
    Result<(LocationMetadataSync, Option<WarehouseHistoryPage>), Option<LocationMetadataSync>>;
