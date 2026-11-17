use crate::sql_tree::models::{NodeExtent, NodeName, SqlTreeNode};

const SHAPE_KINDS: [&str; 5] = ["select", "cte", "join", "subquery", "star"];
/// How a name's fact reads unquoted, then quoted.
const QUOTING: [&str; 2] = ["", " quoted"];

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

/// Each node as `kind line:column`, then what it names or holds, then its end and the source text it covers.
pub(crate) fn facts(sql: &str, nodes: &[SqlTreeNode]) -> Vec<String> {
    nodes
        .iter()
        .map(|node| {
            format!(
                "{} {}:{}{}{}{}",
                node.kind,
                node.line,
                node.column,
                node.name.as_ref().map(name_fact).unwrap_or_default(),
                node.literal
                    .as_ref()
                    .map(|literal| format!(" {}={}", literal.literal_type, literal.value))
                    .unwrap_or_default(),
                node.extent
                    .as_ref()
                    .map(|extent| extent_fact(sql, extent))
                    .unwrap_or_default(),
            )
        })
        .collect()
}

fn name_fact(name: &NodeName) -> String {
    let quoted: &str = QUOTING[usize::from(name.quoted)];
    let qualifier: String = name
        .qualifier
        .first()
        .map(|_| format!(" in={}", name.qualifier.join(".")))
        .unwrap_or_default();
    format!(" name={}{quoted}{qualifier}", name.name)
}

fn extent_fact(sql: &str, extent: &NodeExtent) -> String {
    let (start, end) = extent.span;
    let text: String = sql.chars().skip(start).take(end - start).collect();
    format!(" to {}:{} {text:?}", extent.end_line, extent.end_column)
}
