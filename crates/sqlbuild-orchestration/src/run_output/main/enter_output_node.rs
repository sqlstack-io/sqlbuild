//! Name the node whose code runs on this thread, so the output it prints is filed under it.

use crate::run_output::_helpers::worker::upload::OUTPUT_NODE;

/// Returns the node named before, to restore when the node's code returns; `None` files output under the run.
pub fn enter_output_node(node_key: Option<String>) -> Option<String> {
    OUTPUT_NODE.with(|node| node.replace(node_key))
}
