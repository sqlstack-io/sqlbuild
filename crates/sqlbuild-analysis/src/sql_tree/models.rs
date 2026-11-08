//! Nodes of a rule's SQL syntax tree.

/// One node: its SQLQuack kind, where it starts, and its parent's index in pre-order.
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
}

/// The SQLQuack parser that reads a dialect's SQL for rules.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum TreeParser {
    /// `sqlquack_syntax_postgres`'s native PostgreSQL parser.
    Postgres,
    /// `sqlquack_syntax`'s shared parser for one dialect, recording node spans.
    Shared(sqlquack_syntax::Dialect),
}
