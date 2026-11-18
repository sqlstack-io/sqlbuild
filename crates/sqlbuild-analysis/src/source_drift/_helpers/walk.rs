//! The walk deciding whether new columns on some relations could change a query's result.

use std::collections::HashSet;

use sqlbuild_core::text::main::python_casefold::python_casefold;
use sqlquack_syntax::expressions::queries::models::{JoinKind, Select, With};
use sqlquack_syntax::{Expression, ExpressionWalk};

use crate::source_drift::constants::{COUNT_FUNCTION, OPAQUE_KINDS, SHARED_SCOPE_RELATIONS};

/// The relations that gained columns and the names they gained, casefolded.
pub(crate) struct Drift<'a> {
    pub(crate) relations: &'a HashSet<String>,
    pub(crate) added: &'a HashSet<String>,
}

/// One query scope's visible relations: whether each exposes a drifted relation's columns.
type Visible = Vec<bool>;

/// CTE names in scope and whether each exposes a drifted relation's columns, innermost last.
type Ctes = Vec<(String, bool)>;

pub(crate) struct Walk<'a> {
    drift: Drift<'a>,
    /// Set once anything shows the result could change.
    pub(crate) affected: bool,
}

impl<'a> Walk<'a> {
    pub(crate) fn new(drift: Drift<'a>) -> Self {
        Self {
            drift,
            affected: false,
        }
    }

    /// Mark `statement` affected when new columns could change its result.
    pub(crate) fn statement(&mut self, statement: &Expression) {
        self.names(statement);
        if self.query(statement, &Vec::new(), &Vec::new()) {
            self.affected = true;
        }
    }

    /// Any construct whose meaning depends on every column, and any name a new column could take.
    fn names(&mut self, statement: &Expression) {
        for node in statement.dfs() {
            let name: Option<&str> = match node {
                Expression::Column(column) => Some(column.name.name.as_str()),
                Expression::Identifier(identifier) => Some(identifier.name.as_str()),
                Expression::Pivot(_)
                | Expression::Unpivot(_)
                | Expression::PipeOperator(_)
                | Expression::With(_)
                | Expression::Cte(_) => {
                    self.affected = true;
                    None
                }
                _ if OPAQUE_KINDS.contains(&node.variant_name()) => {
                    self.affected = true;
                    None
                }
                _ => None,
            };
            if name.is_some_and(|name| self.drift.added.contains(&python_casefold(name))) {
                self.affected = true;
            }
        }
    }

    /// Whether `query`'s output columns include a drifted relation's columns, walking its scopes.
    fn query(&mut self, query: &Expression, ctes: &Ctes, outer: &Visible) -> bool {
        match query {
            Expression::Select(select) => self.select((query, select), ctes, outer),
            Expression::Union(union) => self.set_operation(
                (&union.left, &union.right),
                union.with.as_ref(),
                ctes,
                outer,
            ),
            Expression::Intersect(intersect) => self.set_operation(
                (&intersect.left, &intersect.right),
                intersect.with.as_ref(),
                ctes,
                outer,
            ),
            Expression::Except(except) => self.set_operation(
                (&except.left, &except.right),
                except.with.as_ref(),
                ctes,
                outer,
            ),
            Expression::Subquery(subquery) => self.query(&subquery.this, ctes, outer),
            Expression::Paren(paren) => self.query(&paren.this, ctes, outer),
            other => {
                let mut exposes: bool = false;
                for nested in nested_queries(other) {
                    exposes |= self.query(nested, ctes, outer);
                }
                exposes
            }
        }
    }

    /// A set operation pairs columns by position, so a branch exposing new columns changes it.
    fn set_operation(
        &mut self,
        (left, right): (&Expression, &Expression),
        with: Option<&With>,
        ctes: &Ctes,
        outer: &Visible,
    ) -> bool {
        let ctes: Ctes = self.with(with, ctes, outer);
        let exposes: bool = self.query(left, &ctes, outer) | self.query(right, &ctes, outer);
        if exposes {
            self.affected = true;
        }
        exposes
    }

    fn with(&mut self, with: Option<&With>, ctes: &Ctes, outer: &Visible) -> Ctes {
        let mut scoped: Ctes = ctes.clone();
        for cte in with.map(|with| with.ctes.as_slice()).unwrap_or_default() {
            let exposes: bool = self.query(&cte.this, &scoped, outer);
            scoped.push((python_casefold(&cte.alias.name), exposes));
        }
        scoped
    }

    fn select(
        &mut self,
        (node, select): (&Expression, &Select),
        ctes: &Ctes,
        outer: &Visible,
    ) -> bool {
        let ctes: Ctes = self.with(select.with.as_ref(), ctes, outer);
        let mut walked: HashSet<*const Expression> = select
            .with
            .iter()
            .flat_map(|with| with.ctes.iter())
            .map(|cte| std::ptr::from_ref(&cte.this))
            .collect();
        let mut local: Vec<(Option<String>, bool)> = Vec::new();
        let relations = select
            .from
            .iter()
            .flat_map(|from| from.expressions.iter())
            .chain(select.joins.iter().map(|join| &join.this));
        for relation in relations {
            walked.insert(std::ptr::from_ref(relation));
            local.extend(self.relation(relation, &ctes, outer));
        }
        let drifted_here: bool = local.iter().any(|(_, exposes)| *exposes);
        for join in &select.joins {
            let natural: bool = matches!(
                join.kind,
                JoinKind::Natural
                    | JoinKind::NaturalLeft
                    | JoinKind::NaturalRight
                    | JoinKind::NaturalFull
            );
            let using_added: bool = join
                .using
                .iter()
                .any(|name| self.drift.added.contains(&python_casefold(&name.name)));
            if (natural && drifted_here) || using_added {
                self.affected = true;
            }
        }
        let visible: Visible = outer
            .iter()
            .copied()
            .chain(local.iter().map(|(_, exposes)| *exposes))
            .collect();
        let shared: bool =
            visible.len() >= SHARED_SCOPE_RELATIONS && visible.iter().any(|exposes| *exposes);
        let mut star: bool = false;
        let mut unqualified: bool = false;
        let mut nested: Vec<&Expression> = Vec::new();
        let mut pending: Vec<(&Expression, bool)> = Vec::new();
        for item in &select.expressions {
            pending.push((item, true));
        }
        let projected: HashSet<*const Expression> =
            select.expressions.iter().map(std::ptr::from_ref).collect();
        for child in node.children() {
            if !walked.contains(&std::ptr::from_ref(child))
                && !projected.contains(&std::ptr::from_ref(child))
            {
                pending.push((child, false));
            }
        }
        while let Some((node, in_projection)) = pending.pop() {
            match node {
                Expression::Select(_)
                | Expression::Union(_)
                | Expression::Intersect(_)
                | Expression::Except(_)
                | Expression::Subquery(_) => nested.push(node),
                Expression::Star(expanded) if in_projection => {
                    star |= match &expanded.table {
                        None => drifted_here,
                        Some(qualifier) => {
                            let qualifier: String = python_casefold(&qualifier.name);
                            local
                                .iter()
                                .find(|(alias, _)| alias.as_deref() == Some(qualifier.as_str()))
                                .is_none_or(|(_, exposes)| *exposes)
                        }
                    };
                }
                Expression::BracedWildcard(_) if in_projection => star |= drifted_here,
                Expression::Count(_)
                | Expression::AggregateFunction(_)
                | Expression::Function(_)
                    if counts_rows(node) =>
                {
                    pending.extend(
                        node.children()
                            .into_iter()
                            .filter(|child| !matches!(child, Expression::Star(_)))
                            .map(|child| (child, in_projection)),
                    );
                }
                Expression::Column(column) if column.table.is_none() => unqualified = true,
                _ => pending.extend(
                    node.children()
                        .into_iter()
                        .map(|child| (child, in_projection)),
                ),
            }
        }
        if shared && unqualified {
            self.affected = true;
        }
        for query in nested {
            if self.query(query, &ctes, &visible) {
                self.affected = true;
            }
        }
        star
    }

    /// Whether each relation a `FROM` item brings into scope exposes a drifted relation's columns.
    fn relation(
        &mut self,
        relation: &Expression,
        ctes: &Ctes,
        outer: &Visible,
    ) -> Vec<(Option<String>, bool)> {
        match relation {
            Expression::Table(table) => {
                let alias: String =
                    python_casefold(&table.alias.as_ref().unwrap_or(&table.name).name);
                let name: String = python_casefold(&table.name.name);
                let qualified: String = python_casefold(
                    &[&table.catalog, &table.schema]
                        .into_iter()
                        .flatten()
                        .map(|part| part.name.as_str())
                        .chain([table.name.name.as_str()])
                        .collect::<Vec<&str>>()
                        .join("."),
                );
                let unqualified: bool = table.schema.is_none() && table.catalog.is_none();
                let cte: Option<bool> = unqualified
                    .then(|| {
                        ctes.iter()
                            .rev()
                            .find(|(cte, _)| *cte == name)
                            .map(|(_, exposes)| *exposes)
                    })
                    .flatten();
                vec![(
                    Some(alias),
                    cte.unwrap_or_else(|| {
                        self.drift.relations.contains(&name)
                            || self.drift.relations.contains(&qualified)
                    }),
                )]
            }
            Expression::Subquery(subquery) => vec![(
                subquery
                    .alias
                    .as_ref()
                    .map(|alias| python_casefold(&alias.name)),
                self.query(&subquery.this, ctes, outer),
            )],
            other => {
                let mut found: Vec<(Option<String>, bool)> = Vec::new();
                let mut pending: Vec<&Expression> = other.children();
                while let Some(node) = pending.pop() {
                    match node {
                        Expression::Table(_) | Expression::Subquery(_) => {
                            found.extend(self.relation(node, ctes, outer));
                        }
                        Expression::Select(_)
                        | Expression::Union(_)
                        | Expression::Intersect(_)
                        | Expression::Except(_) => {
                            found.push((None, self.query(node, ctes, outer)))
                        }
                        _ => pending.extend(node.children()),
                    }
                }
                if found.is_empty() {
                    found.push((None, false));
                }
                found
            }
        }
    }
}

/// Whether `node` is `COUNT`, whose `*` counts rows rather than listing columns.
fn counts_rows(node: &Expression) -> bool {
    match node {
        Expression::Count(_) => true,
        Expression::AggregateFunction(call) => call.name.eq_ignore_ascii_case(COUNT_FUNCTION),
        Expression::Function(call) => call.name.eq_ignore_ascii_case(COUNT_FUNCTION),
        _ => false,
    }
}

/// The outermost queries nested anywhere inside `expression`.
fn nested_queries(expression: &Expression) -> Vec<&Expression> {
    let mut found: Vec<&Expression> = Vec::new();
    let mut pending: Vec<&Expression> = expression.children();
    while let Some(node) = pending.pop() {
        match node {
            Expression::Select(_)
            | Expression::Union(_)
            | Expression::Intersect(_)
            | Expression::Except(_)
            | Expression::Subquery(_) => found.push(node),
            _ => pending.extend(node.children()),
        }
    }
    found
}
