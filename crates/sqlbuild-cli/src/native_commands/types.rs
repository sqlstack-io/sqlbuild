//! Private interpreter protocol shapes owned by the location supervisor.

use crate::native_commands::models::{LocationMetadataSync, LocationWatch};
use sqlbuild_orchestration::platform::models::{LocationRuleFailure, MissingProviderSecrets};
use sqlbuild_orchestration::warehouse_metadata::models::WarehouseStateLocation;

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

/// The providers a generation shows missing secrets, and the node keys that need each.
pub(crate) type GenerationSecrets = (
    Vec<MissingProviderSecrets>,
    std::collections::BTreeMap<String, Vec<String>>,
);

/// A published generation's refresh deadline, state locations and providers missing secrets, or the rule findings that failed it.
pub(crate) type LoadOutcome = Result<
    (
        Option<tokio::time::Instant>,
        Vec<WarehouseStateLocation>,
        GenerationSecrets,
    ),
    Option<LocationRuleFailure>,
>;

/// A plan document and the model diffs it left out, by model, which the server stores apart.
pub(crate) type LocationPlan = (
    serde_json::Value,
    std::collections::BTreeMap<String, sqlbuild_orchestration::platform::models::PlanModelDiff>,
);

/// The frozen nodes a plan request selects, and the jobs it pins with their nodes and settings.
pub(crate) type PlanSelection<'a> = (
    &'a std::collections::BTreeSet<String>,
    &'a std::collections::BTreeMap<String, sqlbuild_orchestration::runs::models::PinnedJob>,
);

/// A plan request's settings, and the names of every setting its command gives, `warehouse` included.
pub(crate) type PlanSettings<'a> = (
    &'a std::collections::BTreeMap<String, serde_json::Value>,
    &'a std::collections::BTreeSet<String>,
);
