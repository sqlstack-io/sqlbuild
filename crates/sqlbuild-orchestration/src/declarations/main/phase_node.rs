//! Which group member nodes run in a Python or load phase of the member's build.

use crate::declarations::constants::PHASE_NODE_KINDS;

/// Whether a node runs in a Python or load phase of its member's build, where waiting would hold the member's other nodes back.
#[must_use]
pub fn phase_node(key: &str) -> bool {
    key.split_once(':')
        .is_some_and(|(kind, _)| PHASE_NODE_KINDS.contains(&kind))
}
