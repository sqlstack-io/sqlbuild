//! A person lists the current nodes with view on all of them, without their states, served from the held view and tagged with its snapshot.

use uuid::Uuid;

use crate::credentials::models::{AuditContext, DefaultRole};
use crate::node_states::_helpers::authorized::authorized_view;
use crate::node_states::_helpers::bodies::nodes_json;
use crate::node_states::models::{CachedJson, NodeStateCache};
use crate::store::errors::ControlError;

pub async fn list_person_catalog_nodes(
    cache: &NodeStateCache,
    credentials: (&AuditContext, Uuid),
    scope: (Uuid, &str),
    default_role: DefaultRole,
) -> Result<CachedJson, ControlError> {
    nodes_json(&authorized_view(cache, credentials, scope, default_role).await?).await
}
