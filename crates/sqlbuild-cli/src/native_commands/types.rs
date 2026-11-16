//! Private interpreter protocol shapes owned by the location supervisor.

use chrono::{DateTime, Utc};
use sqlbuild_orchestration::snapshots::models::SnapshotRecord;
use sqlbuild_orchestration::warehouse_metadata::models::WarehouseStateLocation;

use crate::native_commands::models::{LocationMetadataSync, LocationWatch};

/// Publication metadata and the earliest calendar refresh deadline.
pub(crate) type LocationPublication = (
    SnapshotRecord,
    Option<DateTime<Utc>>,
    Vec<WarehouseStateLocation>,
);

/// A settled publication: the watch without its local databases, its metadata sync, and the sync deferred until the warehouse connects.
pub(crate) type Settled = (
    LocationWatch,
    Option<LocationMetadataSync>,
    Option<(Vec<WarehouseStateLocation>, i64)>,
);

/// A compile's outcome: whether it published, its next calendar refresh, and the warehouse state it reads.
pub(crate) type CompileOutcome = (
    bool,
    Option<tokio::time::Instant>,
    Vec<WarehouseStateLocation>,
);

/// The next compile, if a generation number was issued, with the metadata sync and the inputs it saw.
pub(crate) type NextGeneration = (
    Option<(CompileOutcome, i64)>,
    Option<LocationMetadataSync>,
    Option<Vec<sqlbuild_cache::project_snapshot::models::PathStamp>>,
);
