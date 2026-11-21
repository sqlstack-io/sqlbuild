//! The node name of one item of an asset's set: `<asset>[<item id>]`.

use crate::discovered_sets::constants::PLACEHOLDER_ITEM;

/// `asset[item]`; with no item, the placeholder `asset[*]` of a set that was never discovered.
pub fn item_node_name(asset: &str, item: Option<&str>) -> String {
    format!("{asset}[{}]", item.unwrap_or(PLACEHOLDER_ITEM))
}
