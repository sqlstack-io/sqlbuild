//! Nodes of a rule's SQL syntax tree.

/// One node: its SQLQuack kind, where it starts and ends, what it names or holds, and its parent's index in pre-order.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SqlTreeNode {
    /// SQLQuack's snake_case node kind, such as `select`, `subquery`, `star`, `join` or `cte`.
    pub kind: &'static str,
    /// 1-based line of the node's first character; a node without a span takes its parent's.
    pub line: usize,
    /// 1-based column, in characters, of the node's first character.
    pub column: usize,
    /// The parent's index; `None` for a statement.
    pub parent: Option<usize>,
    /// The source text the node covers; `None` when the parser recorded no span for it.
    pub extent: Option<NodeExtent>,
    /// The name an identifier, column, alias or table node carries.
    pub name: Option<NodeName>,
    /// The value a literal node holds.
    pub literal: Option<NodeLiteral>,
}

/// Where a node's source text ends, and the character range it covers in the source.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct NodeExtent {
    /// 1-based line of the position just after the node's last character.
    pub end_line: usize,
    /// 1-based column, in characters, of the position just after the node's last character.
    pub end_column: usize,
    /// The node's start and exclusive end as character offsets into the source.
    pub span: (usize, usize),
}

/// A name as written; a relation marker's name is its call, such as `__ref("orders")`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct NodeName {
    /// The name without quote characters.
    pub name: String,
    /// Whether the source quoted the name.
    pub quoted: bool,
    /// The names before it, outermost first, such as `["db", "sales"]` in `db.sales.orders`.
    pub qualifier: Vec<String>,
}

/// A literal's value as written, without quotes, and its SQLQuack type, such as `string` or `number`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct NodeLiteral {
    /// The value's text.
    pub value: String,
    /// The snake_case literal type.
    pub literal_type: &'static str,
}

/// The SQLQuack parser that reads a dialect's SQL for rules.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum TreeParser {
    /// `sqlquack_syntax_postgres`'s native PostgreSQL parser.
    Postgres,
    /// `sqlquack_syntax`'s shared parser for one dialect, recording node spans.
    Shared(sqlquack_syntax::Dialect),
}
