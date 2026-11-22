use crate::sql_tree::models::{NodeExtent, NodeName, SqlTreeNode};

const SHAPE_KINDS: [&str; 5] = ["select", "cte", "join", "subquery", "star"];
/// The clause kinds the rule tree adds for a query's and a window's clauses.
const CLAUSE_KINDS: [&str; 10] = [
    "projection",
    "from",
    "where",
    "group_by",
    "having",
    "qualify",
    "order_by",
    "limit",
    "partition_by",
    "window_order_by",
];
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

/// As `facts`, leaving out the clause nodes, which carry no names, literals or spans of their own.
pub(crate) fn expression_facts(sql: &str, nodes: &[SqlTreeNode]) -> Vec<String> {
    facts(sql, nodes)
        .into_iter()
        .zip(nodes)
        .filter(|(_, node)| !CLAUSE_KINDS.contains(&node.kind))
        .map(|(fact, _)| fact)
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

/// Each clause node, in tree order, with its direct children as `kind line:column` sorted by position.
pub(crate) fn clauses(nodes: &[SqlTreeNode]) -> Vec<(&'static str, Vec<String>)> {
    nodes
        .iter()
        .enumerate()
        .filter(|(_, node)| CLAUSE_KINDS.contains(&node.kind))
        .map(|(index, node)| {
            let mut children: Vec<&SqlTreeNode> = nodes
                .iter()
                .filter(|child| child.parent == Some(index))
                .collect();
            children.sort_by_key(|child| (child.line, child.column));
            let described: Vec<String> = children
                .iter()
                .map(|child| format!("{} {}:{}", child.kind, child.line, child.column))
                .collect();
            (node.kind, described)
        })
        .collect()
}

/// The kinds of the first statement's direct children, in tree order.
pub(crate) fn statement_children(nodes: &[SqlTreeNode]) -> Vec<&'static str> {
    nodes
        .iter()
        .filter(|node| node.parent == Some(0))
        .map(|node| node.kind)
        .collect()
}
