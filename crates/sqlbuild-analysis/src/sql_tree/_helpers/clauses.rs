//! Clauses SQLQuack keeps as statement fields rather than expressions: CTEs, joins, a query's other clauses and a window's.

use sqlquack_syntax::Expression;
use sqlquack_syntax::expressions::names::models::Identifier;
use sqlquack_syntax::expressions::queries::models::{
    Cte, From, Join, JoinKind, OrderBy, Ordered, Over, Select, Where, With,
};
use sqlquack_syntax::tokens::models::Span;

use crate::sql_tree::constants::{
    CTE_KIND, FROM_KIND, GROUP_BY_KIND, HAVING_KIND, JOIN_KIND, LIMIT_KIND, ORDER_BY_KIND,
    PARTITION_BY_KIND, PROJECTION_KIND, QUALIFY_KIND, WHERE_KIND, WINDOW_ORDER_BY_KIND,
};

/// One clause of a statement: its kind, its span, its name, and the expressions it owns.
pub(crate) struct Clause<'ast> {
    pub(crate) kind: &'static str,
    pub(crate) span: Option<Span>,
    pub(crate) name: Option<&'ast Identifier>,
    pub(crate) children: Vec<&'ast Expression>,
    /// Whether a clause without its own span takes its statement's, as the clauses the parser records no span for do.
    pub(crate) spans_statement: bool,
}

/// The clauses `expression` holds directly: CTEs, then joins, then the rest in the order they are written.
pub(crate) fn clauses(expression: &Expression) -> Vec<Clause<'_>> {
    let mut found: Vec<Clause<'_>> = ctes_and_joins(expression);
    found.extend(written_clauses(expression));
    found
}

/// The CTEs and explicit joins `expression` holds directly, CTEs first.
fn ctes_and_joins(expression: &Expression) -> Vec<Clause<'_>> {
    let (with, joins): (Option<&With>, Vec<&[Join]>) = match expression {
        Expression::Select(select) => (select.with.as_ref(), vec![select.joins.as_slice()]),
        Expression::Union(union) => (union.with.as_ref(), Vec::new()),
        Expression::Intersect(intersect) => (intersect.with.as_ref(), Vec::new()),
        Expression::Except(except) => (except.with.as_ref(), Vec::new()),
        Expression::Insert(insert) => (insert.with.as_ref(), Vec::new()),
        Expression::Update(update) => (
            update.with.as_ref(),
            vec![update.table_joins.as_slice(), update.from_joins.as_slice()],
        ),
        Expression::Delete(delete) => (delete.with.as_ref(), vec![delete.joins.as_slice()]),
        Expression::JoinedTable(joined) => (None, vec![joined.joins.as_slice()]),
        _ => (None, Vec::new()),
    };
    let ctes = with
        .into_iter()
        .flat_map(|with| with.ctes.iter())
        .map(cte_clause);
    let joins = joins
        .into_iter()
        .flatten()
        .filter(|join| !is_comma_item(join))
        .map(join_clause);
    ctes.chain(joins).collect()
}

fn cte_clause(cte: &Cte) -> Clause<'_> {
    Clause {
        kind: CTE_KIND,
        span: cte.span.0,
        name: Some(&cte.alias),
        children: vec![&cte.this],
        spans_statement: false,
    }
}

fn join_clause(join: &Join) -> Clause<'_> {
    let mut children: Vec<&Expression> = vec![&join.this];
    children.extend(join.on.as_ref());
    children.extend(join.match_condition.as_ref());
    children.extend(join.pivots.iter());
    Clause {
        kind: JOIN_KIND,
        span: join.span.0,
        name: None,
        children,
        spans_statement: false,
    }
}

/// A comma-separated FROM item after an explicit join, which the shared parser keeps as a join.
fn is_comma_item(join: &Join) -> bool {
    join.kind == JoinKind::Implicit
}

/// A query's, a DML statement's or a window's other clauses, in the order they are written.
fn written_clauses(expression: &Expression) -> Vec<Clause<'_>> {
    match expression {
        Expression::Select(select) => select_clauses(select),
        Expression::Union(union) => tail_clauses(union.order_by.as_ref(), union.limit.as_deref()),
        Expression::Intersect(intersect) => {
            tail_clauses(intersect.order_by.as_ref(), intersect.limit.as_deref())
        }
        Expression::Except(except) => {
            tail_clauses(except.order_by.as_ref(), except.limit.as_deref())
        }
        Expression::Update(update) => change_clauses(
            update.where_clause.as_ref(),
            update.order_by.as_ref(),
            update.limit.as_ref(),
        ),
        Expression::Delete(delete) => change_clauses(
            delete.where_clause.as_ref(),
            delete.order_by.as_ref(),
            delete.limit.as_ref(),
        ),
        Expression::WindowFunction(window) => window_clauses(&window.over),
        _ => Vec::new(),
    }
}

fn select_clauses(select: &Select) -> Vec<Clause<'_>> {
    let mut found: Vec<Clause<'_>> =
        vec![owning(PROJECTION_KIND, select.expressions.iter().collect())];
    found.extend(
        select
            .from
            .iter()
            .map(|from| owning(FROM_KIND, from_sources(from, &select.joins))),
    );
    found.extend(
        select
            .where_clause
            .iter()
            .map(|clause| owning(WHERE_KIND, vec![&clause.this])),
    );
    found.extend(
        select
            .group_by
            .iter()
            .map(|group_by| owning(GROUP_BY_KIND, group_by.expressions.iter().collect())),
    );
    found.extend(
        select
            .having
            .iter()
            .map(|having| owning(HAVING_KIND, vec![&having.this])),
    );
    found.extend(
        select
            .qualify
            .iter()
            .map(|qualify| owning(QUALIFY_KIND, vec![&qualify.this])),
    );
    found.extend(tail_clauses(
        select.order_by.as_ref(),
        select.limit.as_ref().map(|limit| &limit.this),
    ));
    found
}

/// An `UPDATE`'s or `DELETE`'s `WHERE`, `ORDER BY` and `LIMIT`, where written.
fn change_clauses<'ast>(
    where_clause: Option<&'ast Where>,
    order_by: Option<&'ast OrderBy>,
    limit: Option<&'ast Expression>,
) -> Vec<Clause<'ast>> {
    let filtered = where_clause.map(|clause| owning(WHERE_KIND, vec![&clause.this]));
    filtered
        .into_iter()
        .chain(tail_clauses(order_by, limit))
        .collect()
}

/// The `FROM` sources, then each comma-separated source written after an explicit join.
fn from_sources<'ast>(from: &'ast From, joins: &'ast [Join]) -> Vec<&'ast Expression> {
    let commas = joins.iter().filter(|join| is_comma_item(join));
    from.expressions
        .iter()
        .chain(commas.map(|join| &join.this))
        .collect()
}

/// The expressions sorted by, without their directions.
fn sort_keys(keys: &[Ordered]) -> Vec<&Expression> {
    keys.iter().map(|key| &key.this).collect()
}

/// `ORDER BY` and `LIMIT`, where written.
fn tail_clauses<'ast>(
    order_by: Option<&'ast OrderBy>,
    limit: Option<&'ast Expression>,
) -> Vec<Clause<'ast>> {
    let ordered = order_by.map(|order_by| owning(ORDER_BY_KIND, sort_keys(&order_by.expressions)));
    let limited = limit.map(|limit| owning(LIMIT_KIND, vec![limit]));
    ordered.into_iter().chain(limited).collect()
}

/// A window's `PARTITION BY` and `ORDER BY`, where written.
fn window_clauses(over: &Over) -> Vec<Clause<'_>> {
    let partition = (!over.partition_by.is_empty())
        .then(|| owning(PARTITION_BY_KIND, over.partition_by.iter().collect()));
    let order = (!over.order_by.is_empty())
        .then(|| owning(WINDOW_ORDER_BY_KIND, sort_keys(&over.order_by)));
    partition.into_iter().chain(order).collect()
}

/// A clause the parser records no span for, owning `children`.
fn owning<'ast>(kind: &'static str, children: Vec<&'ast Expression>) -> Clause<'ast> {
    Clause {
        kind,
        span: None,
        name: None,
        children,
        spans_statement: true,
    }
}
