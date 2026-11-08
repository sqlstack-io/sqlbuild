//! What a person may do, reported for the UI to show or hide actions; every request is still authorized.

use serde::Serialize;
use uuid::Uuid;

use crate::credentials::models::Action;

/// One location target the person can view, with the actions allowed on at least one of its nodes.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, utoipa::ToSchema)]
pub struct LocationAccess {
    pub location_id: Uuid,
    pub name: String,
    pub target: String,
    pub actions: Vec<Action>,
}

/// The person's name, the actions allowed server-wide, and the location targets they can view.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, utoipa::ToSchema)]
pub struct PersonAccess {
    pub principal_id: Uuid,
    pub name: String,
    pub actions: Vec<Action>,
    pub locations: Vec<LocationAccess>,
}

/// What a target's role overview is built from, read in one transaction.
#[derive(Debug)]
pub(crate) struct RoleFacts {
    pub(crate) custom: Vec<crate::store::types::Row>,
    pub(crate) holders: std::collections::BTreeMap<String, u64>,
    pub(crate) descriptions: std::collections::BTreeMap<String, String>,
}

#[derive(Debug, serde::Deserialize)]
pub(crate) struct RoleHolderCount {
    pub(crate) role_key: String,
    pub(crate) holders: u64,
}

#[derive(Debug, serde::Deserialize)]
pub(crate) struct CurrentSnapshotId {
    pub(crate) snapshot_id: Uuid,
}
