//! Expected-output column names read syntactically from an `__expected__` CTE body.

use std::collections::HashSet;

use sqlquack_syntax::expressions::names::models::Identifier;
use sqlquack_syntax::expressions::queries::models::Select;
use sqlquack_syntax::{Expression, Parsed};

use crate::analysis_route::types::NativeDialect;

/// Return the authored output column identifiers of `sql`, or `None` when they are not explicit.
pub(crate) fn expected_columns(sql: &str, parser: &dyn NativeDialect) -> Option<Vec<String>> {
    let mut statements = match parser.parse(sql) {
        Ok(Parsed { statements, gaps }) if gaps.is_empty() => statements,
        _ => return None,
    };
    if statements.len() != 1 {
        return None;
    }
    let statement = statements.pop()?;
    let source = AuthoredSource::new(sql);
    let columns: Vec<String> = query_columns(&statement, &source)?;
    let mut seen: HashSet<String> = HashSet::new();
    if columns.is_empty()
        || !columns
            .iter()
            .all(|column| seen.insert(normalized_identifier(column)))
    {
        return None;
    }
    Some(columns)
}

/// Fold an authored identifier to the case-insensitive name used for column matching.
pub(crate) fn normalized_identifier(identifier: &str) -> String {
    let unquoted = match identifier.chars().next() {
        Some('"') => identifier.trim_matches('"').replace("\"\"", "\""),
        Some('`') => identifier.trim_matches('`').replace("``", "`"),
        Some('[') => identifier
            .trim_start_matches('[')
            .trim_end_matches(']')
            .replace("]]", "]"),
        _ => identifier.to_string(),
    };
    unquoted.to_lowercase()
}

struct AuthoredSource<'a> {
    sql: &'a str,
    char_offsets: Vec<usize>,
}

impl<'a> AuthoredSource<'a> {
    fn new(sql: &'a str) -> Self {
        let mut char_offsets: Vec<usize> = sql.char_indices().map(|(offset, _)| offset).collect();
        char_offsets.push(sql.len());
        Self { sql, char_offsets }
    }

    /// Return the identifier exactly as written, including its quoting.
    fn identifier(&self, identifier: &Identifier) -> Option<String> {
        let Some(span) = identifier.span.as_ref() else {
            return (!identifier.quoted).then(|| identifier.name.clone());
        };
        let start = *self.char_offsets.get(span.start)?;
        let end = *self.char_offsets.get(span.end)?;
        let text = self.sql.get(start..end)?;
        let consistent = if identifier.quoted {
            text.chars().count() >= identifier.name.chars().count() + 2
        } else {
            text == identifier.name
        };
        consistent.then(|| text.to_string())
    }
}

fn query_columns(expression: &Expression, source: &AuthoredSource<'_>) -> Option<Vec<String>> {
    match expression {
        Expression::Select(select) => select_columns(select, source),
        Expression::Union(union) => query_columns(&union.left, source),
        Expression::Intersect(intersect) => query_columns(&intersect.left, source),
        Expression::Except(except) => query_columns(&except.left, source),
        Expression::Paren(paren) => query_columns(&paren.this, source),
        Expression::Subquery(subquery) if subquery.column_aliases.is_empty() => {
            query_columns(&subquery.this, source)
        }
        Expression::Values(values) => identifiers(&values.column_aliases, source),
        _ => None,
    }
}

fn select_columns(select: &Select, source: &AuthoredSource<'_>) -> Option<Vec<String>> {
    if let [Expression::Star(star)] = select.expressions.as_slice() {
        if star.table.is_some()
            || star.except.is_some()
            || star.replace.is_some()
            || star.rename.is_some()
            || !select.joins.is_empty()
        {
            return None;
        }
        let from = select.from.as_ref()?;
        let [relation] = from.expressions.as_slice() else {
            return None;
        };
        return aliased_values_columns(relation, source);
    }
    select
        .expressions
        .iter()
        .map(|projection| projection_column(projection, source))
        .collect()
}

fn aliased_values_columns(
    relation: &Expression,
    source: &AuthoredSource<'_>,
) -> Option<Vec<String>> {
    match relation {
        Expression::Subquery(subquery) if matches!(subquery.this, Expression::Values(_)) => {
            identifiers(&subquery.column_aliases, source)
        }
        Expression::Values(values) => identifiers(&values.column_aliases, source),
        _ => None,
    }
}

fn projection_column(projection: &Expression, source: &AuthoredSource<'_>) -> Option<String> {
    match projection {
        Expression::Alias(alias) if alias.column_aliases.is_empty() => {
            source.identifier(&alias.alias)
        }
        Expression::Column(column) => source.identifier(&column.name),
        _ => None,
    }
}

fn identifiers(identifiers: &[Identifier], source: &AuthoredSource<'_>) -> Option<Vec<String>> {
    if identifiers.is_empty() {
        return None;
    }
    identifiers
        .iter()
        .map(|identifier| source.identifier(identifier))
        .collect()
}
