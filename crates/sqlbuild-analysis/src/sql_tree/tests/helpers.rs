use crate::sql_tree::models::SqlTreeNode;

const SHAPE_KINDS: [&str; 5] = ["select", "cte", "join", "subquery", "star"];

/// The listed kinds' nodes with their parent's kind, sorted by position, since child order is SQLQuack's.
pub(crate) fn shape(
    nodes: &[SqlTreeNode],
) -> Vec<(&'static str, usize, usize, Option<&'static str>)> {
    let mut shape: Vec<_> = nodes
        .iter()
        .filter(|node| SHAPE_KINDS.contains(&node.kind))
        .map(|node| {
            let parent = node.parent.map(|parent| nodes[parent].kind);
            (node.kind, node.line, node.column, parent)
        })
        .collect();
    shape.sort_by_key(|(kind, line, column, _)| (*line, *column, *kind));
    shape
}
