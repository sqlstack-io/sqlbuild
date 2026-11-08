//! Join and CTE clauses, which SQLQuack keeps as statement fields rather than expressions.

use sqlquack_syntax::Expression;
use sqlquack_syntax::expressions::queries::models::{Cte, Join, JoinKind, With};
use sqlquack_syntax::tokens::models::Span;

use crate::sql_tree::constants::{CTE_KIND, JOIN_KIND};

/// One clause of a statement: its kind, its span, and the expressions it owns.
pub(crate) struct Clause<'ast> {
    pub(crate) kind: &'static str,
    pub(crate) span: Option<Span>,
    pub(crate) children: Vec<&'ast Expression>,
}

/// The CTEs and explicit joins `expression` holds directly, CTEs first.
pub(crate) fn clauses(expression: &Expression) -> Vec<Clause<'_>> {
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
        children: vec![&cte.this],
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
        children,
    }
}

/// A comma-separated FROM item after an explicit join, which the shared parser keeps as a join.
fn is_comma_item(join: &Join) -> bool {
    join.kind == JoinKind::Implicit
}
