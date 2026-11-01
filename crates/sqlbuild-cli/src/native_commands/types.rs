//! Private interpreter protocol shapes owned by the location supervisor.

use chrono::{DateTime, Utc};
use sqlbuild_orchestration::snapshots::models::SnapshotRecord;

/// Publication metadata and the earliest calendar refresh deadline.
pub(crate) type LocationPublication = (SnapshotRecord, Option<DateTime<Utc>>);
