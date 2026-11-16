//! A person reads every current node's state and its version, with the same authorization as the node list.

use uuid::Uuid;

use crate::credentials::models::{AuditContext, DefaultRole};
use crate::node_states::_helpers::authorized::authorized_view;
use crate::node_states::_helpers::bodies::states_json;
use crate::node_states::models::{CachedJson, NodeStateCache};
use crate::store::errors::ControlError;

pub async fn read_person_node_states(
    cache: &NodeStateCache,
    credentials: (&AuditContext, Uuid),
    scope: (Uuid, &str),
    default_role: DefaultRole,
) -> Result<CachedJson, ControlError> {
    states_json(&authorized_view(cache, credentials, scope, default_role).await?).await
}
