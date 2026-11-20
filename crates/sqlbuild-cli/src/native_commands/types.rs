//! Private interpreter protocol shapes owned by the location supervisor.

use crate::native_commands::models::{LocationMetadataSync, LocationWatch};
use chrono::{DateTime, Utc};
use sqlbuild_orchestration::platform::models::{LocationRuleFailure, MissingProviderSecrets};
use sqlbuild_orchestration::snapshots::models::SnapshotRecord;
use sqlbuild_orchestration::warehouse_metadata::models::WarehouseStateLocation;

/// Publication metadata and the earliest calendar refresh deadline.
pub(crate) type LocationPublication = (
    SnapshotRecord,
    Option<DateTime<Utc>>,
    Vec<WarehouseStateLocation>,
    Vec<MissingProviderSecrets>,
);

/// A settled publication: the watch without its local databases, its metadata sync, and the sync deferred until the warehouse connects.
pub(crate) type Settled = (
    LocationWatch,
    Option<LocationMetadataSync>,
    Option<(Vec<WarehouseStateLocation>, i64)>,
);

/// The next compile, if a generation number was issued, with the metadata sync and the inputs it saw.
pub(crate) type NextGeneration = (
    Option<(LoadOutcome, i64)>,
    Option<LocationMetadataSync>,
    Option<Vec<sqlbuild_cache::project_snapshot::models::PathStamp>>,
);

/// A published generation's refresh deadline, state locations and providers missing secrets, or the rule findings that failed it.
pub(crate) type LoadOutcome = Result<
    (
        Option<tokio::time::Instant>,
        Vec<WarehouseStateLocation>,
        Vec<MissingProviderSecrets>,
    ),
    Option<LocationRuleFailure>,
>;

/// A plan document and the model diffs it left out, by model, which the server stores apart.
pub(crate) type LocationPlan = (
    serde_json::Value,
    std::collections::BTreeMap<String, sqlbuild_orchestration::platform::models::PlanModelDiff>,
);
