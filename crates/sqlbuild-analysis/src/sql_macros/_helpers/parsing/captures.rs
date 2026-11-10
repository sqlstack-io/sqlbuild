//! Plain `COLUMNS` slot reads inside a subquery, and the subquery relations their bare names meet.

use sqlquack_syntax::expressions::queries::models::Select;
use sqlquack_syntax::{Expression, ExpressionWalk};

use crate::analysis_route::types::NativeDialect;

use crate::sql_macros::_helpers::parsing::body::{
    ParsedBody, marked_text, parsed, slot_placeholder, slot_placeholder_name,
};
use crate::sql_macros::_helpers::parsing::parameters::{ParameterReference, placeholder_parameter};
use crate::sql_macros::_helpers::parsing::slots::SlotRead;
use crate::sql_macros::models::{ArgumentShape, RenderedArgument, SlotCapture, SqlMacroDefinition};

/// The relations one query reads, and whether it is a subquery of an expression.
struct Scope {
    sources: Vec<Option<String>>,
    subquery: bool,
}

/// The placeholder columns found, each with the relations of its enclosing subqueries.
#[derive(Default)]
struct Captures {
    scopes: Vec<Scope>,
    found: Vec<(String, Vec<Option<String>>)>,
}

/// The name a `FROM` or `JOIN` item reads, `@parameter` for a relation parameter.
fn source_names(source: &Expression) -> Vec<Option<String>> {
    match source {
        Expression::Table(table) => {
            let name: &str = &table.name.name;
            let shown: String = match placeholder_parameter(name) {
                Some(parameter) => format!("@{parameter}"),
                None => match &table.schema {
                    Some(schema) => format!("{}.{name}", schema.name),
                    None => name.to_owned(),
                },
            };
            vec![Some(shown)]
        }
        Expression::JoinedTable(joined) => {
            let mut names: Vec<Option<String>> = source_names(&joined.left);
            for join in &joined.joins {
                names.extend(source_names(&join.this));
            }
            names
        }
        Expression::Paren(paren) => source_names(&paren.this),
        _ => vec![None],
    }
}

/// Whether `child` of `select` is a query of its own: a CTE body, or a non-`LATERAL` `FROM` item.
fn own_scope(select: &Select, child: &Expression) -> bool {
    let lateral: bool = matches!(child, Expression::Subquery(subquery) if subquery.lateral);
    let (cte, source) = child_kind(select, child);
    cte || (source && !lateral)
}

/// Whether `child` of `select` is a CTE body, and whether it is a `FROM` or `JOIN` item.
fn child_kind(select: &Select, child: &Expression) -> (bool, bool) {
    let source: bool = select
        .from
        .iter()
        .flat_map(|from| &from.expressions)
        .chain(select.joins.iter().map(|join| &join.this))
        .any(|known| std::ptr::eq(known, child));
    let cte: bool = select
        .with
        .iter()
        .flat_map(|with| &with.ctes)
        .any(|cte| std::ptr::eq(&cte.this, child));
    (cte, source)
}

/// Whether `child` of `select` can reach its output: a CTE body or any `FROM` item, `LATERAL` too.
fn output_scope(select: &Select, child: &Expression) -> bool {
    let (cte, source) = child_kind(select, child);
    cte || source
}

/// The selects whose output names reach the query's output: all but those inside expressions.
pub(crate) fn output_selects(root: &Expression) -> Vec<&Select> {
    selects_under(root, false)
}

/// The output selects at and under `node`, which stands inside an expression if `in_expression`.
fn selects_under(node: &Expression, in_expression: bool) -> Vec<&Select> {
    let mut found: Vec<&Select> = Vec::new();
    let select: Option<&Select> = match node {
        Expression::Select(select) => Some(select),
        _ => None,
    };
    if let Some(select) = select
        && !in_expression
    {
        found.push(select);
    }
    for child in node.children() {
        let inside: bool =
            in_expression || select.is_some_and(|select| !output_scope(select, child));
        found.extend(selects_under(child, inside));
    }
    found
}

impl Captures {
    fn visit(&mut self, node: &Expression, in_expression: bool) {
        if let Expression::Select(select) = node {
            self.select(node, select, in_expression);
            return;
        }
        if let Some(name) = slot_placeholder_name(node) {
            self.record(name);
        }
        for child in node.children() {
            self.visit(child, in_expression);
        }
    }

    fn select(&mut self, node: &Expression, select: &Select, subquery: bool) {
        let sources: Vec<&Expression> = select
            .from
            .iter()
            .flat_map(|from| &from.expressions)
            .chain(select.joins.iter().map(|join| &join.this))
            .collect();
        self.scopes.push(Scope {
            sources: sources
                .iter()
                .flat_map(|source| source_names(source))
                .collect(),
            subquery,
        });
        for child in node.children() {
            self.visit(child, !own_scope(select, child));
        }
        self.scopes.pop();
    }

    /// Records a placeholder with the relations of the subqueries between it and its query.
    fn record(&mut self, name: &str) {
        let mut sources: Vec<Option<String>> = Vec::new();
        for scope in self.scopes.iter().rev() {
            if !scope.subquery {
                break;
            }
            sources.extend(scope.sources.iter().cloned());
        }
        if !sources.is_empty() {
            self.found.push((name.to_owned(), sources));
        }
    }
}

/// Each plain group read of `body` inside a subquery, with the relations that subquery reads.
pub(crate) fn slot_captures(
    (parser, definition): (&dyn NativeDialect, &SqlMacroDefinition),
    (body, references, reads): (&str, &[ParameterReference], &[SlotRead]),
    arguments: &[RenderedArgument],
) -> Vec<SlotCapture> {
    let argument_of = |relation: &str| {
        arguments.iter().find(|argument| {
            definition.parameters[argument.parameter]
                .name
                .eq_ignore_ascii_case(relation)
        })
    };
    let marked: Vec<&SlotRead> = reads
        .iter()
        .filter(|read| read.group && read.prefix.is_none() && !read.in_using)
        .filter(|read| {
            argument_of(&read.relation).is_some_and(|argument| {
                !matches!(argument.shape, ArgumentShape::ParameterReference { .. })
            })
        })
        .collect();
    if marked.is_empty() {
        return Vec::new();
    }
    let text: String = marked_text((body, references), &marked);
    let Ok(ParsedBody { select }) = parsed(parser, definition.form, &text) else {
        return Vec::new();
    };
    let root: Expression = Expression::Select(select);
    let mut captures: Captures = Captures::default();
    captures.visit(&root, false);
    let mut found: Vec<SlotCapture> = Vec::new();
    for (index, read) in marked.iter().enumerate() {
        let Some(argument) = argument_of(&read.relation) else {
            continue;
        };
        let Some(mapping) = argument
            .slots
            .iter()
            .find(|mapping| mapping.slot.eq_ignore_ascii_case(&read.slot))
        else {
            continue;
        };
        let name: String = slot_placeholder(index);
        let passed: &str = argument.sql.trim();
        for (_, sources) in captures.found.iter().filter(|(found, _)| *found == name) {
            let mut shown: Vec<Option<String>> = Vec::with_capacity(sources.len());
            for source in sources {
                let source: Option<String> = source
                    .as_ref()
                    .map(|source| shown_source(source, definition, arguments));
                if source
                    .as_deref()
                    .is_some_and(|source| source.trim().eq_ignore_ascii_case(passed))
                {
                    continue;
                }
                shown.push(source);
            }
            if shown.is_empty() {
                continue;
            }
            found.push(SlotCapture {
                relation: read.relation.clone(),
                slot: read.slot.clone(),
                columns: mapping.columns.clone(),
                sources: shown,
            });
        }
    }
    found
}

/// A source as the call reads it: a relation parameter as the relation passed for it.
fn shown_source(
    source: &str,
    definition: &SqlMacroDefinition,
    arguments: &[RenderedArgument],
) -> String {
    let Some(parameter) = source.strip_prefix('@') else {
        return source.to_owned();
    };
    arguments
        .iter()
        .find(|argument| {
            definition.parameters[argument.parameter]
                .name
                .eq_ignore_ascii_case(parameter)
        })
        .map_or_else(|| source.to_owned(), |argument| argument.sql.clone())
}
