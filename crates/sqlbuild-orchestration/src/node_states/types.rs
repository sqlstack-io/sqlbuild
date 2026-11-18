//! Shared shapes of the node states cache.

use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use uuid::Uuid;

use crate::node_states::models::TargetCache;

/// Each held target's view, keyed by location and target hash.
pub(crate) type TargetCaches = Arc<Mutex<HashMap<(Uuid, String), Arc<TargetCache>>>>;
