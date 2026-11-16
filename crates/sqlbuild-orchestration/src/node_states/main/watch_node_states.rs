//! A change stream subscribes before the target's view loads, so no state change after its first refetch is missed.

use tokio::sync::broadcast;
use uuid::Uuid;

use crate::node_states::_helpers::views::current_view;
use crate::node_states::models::{NodeStateCache, NodeStatusUpdate};
use crate::store::errors::ControlError;

pub(crate) async fn watch_node_states(
    cache: &NodeStateCache,
    scope: (Uuid, &str),
) -> Result<broadcast::Receiver<NodeStatusUpdate>, ControlError> {
    let updates = cache.updates.subscribe();
    current_view(cache, scope, None).await?;
    Ok(updates)
}
