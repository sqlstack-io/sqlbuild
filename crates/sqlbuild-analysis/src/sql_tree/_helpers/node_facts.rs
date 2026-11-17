//! What a node names or holds: identifier, column, alias, table and CTE names, and literal values.

use sqlquack_syntax::Expression;
use sqlquack_syntax::expressions::names::models::{Identifier, Literal, TableRef};
use sqlquack_syntax::tokens::models::Span;

use crate::relation_markers::constants::MARKER_NAME_PREFIX;
use crate::sql_tree::_helpers::source_map::SourceMap;
use crate::sql_tree::models::{NodeLiteral, NodeName};

/// The name `expression` carries, its qualifier outermost first; a relation marker's name is its call.
pub(crate) fn node_name(expression: &Expression, map: &SourceMap<'_>) -> Option<NodeName> {
    let (name, qualifier): (&Identifier, Vec<&Identifier>) = match expression {
        Expression::Identifier(identifier) => (identifier, Vec::new()),
        Expression::Column(column) => (&column.name, column.table.iter().collect()),
        Expression::Alias(alias) => (&alias.alias, Vec::new()),
        Expression::Table(table) => (
            &table.name,
            table.catalog.iter().chain(table.schema.iter()).collect(),
        ),
        _ => return None,
    };
    Some(named(name, &qualifier, map))
}

/// `name` and its qualifier as written, marker names read back as their calls.
pub(crate) fn named(name: &Identifier, qualifier: &[&Identifier], map: &SourceMap<'_>) -> NodeName {
    let (text, quoted) = written(name, map);
    NodeName {
        name: text,
        quoted,
        qualifier: qualifier.iter().map(|part| written(part, map).0).collect(),
    }
}

/// The value and snake_case type of a literal node.
pub(crate) fn node_literal(expression: &Expression) -> Option<NodeLiteral> {
    let Expression::Literal(literal) = expression else {
        return None;
    };
    Some(NodeLiteral {
        value: literal.value_str().to_owned(),
        literal_type: literal_type(literal),
    })
}

/// A table's source range: from its outermost qualifier to the end of its name, without its alias.
pub(crate) fn table_span(table: &TableRef) -> Option<Span> {
    let end: Span = table.name.span?;
    let first: &Identifier = table
        .catalog
        .as_ref()
        .or(table.schema.as_ref())
        .unwrap_or(&table.name);
    let qualified = [table.catalog.as_ref(), table.schema.as_ref()];
    if qualified.iter().flatten().any(|part| part.span.is_none()) {
        return None;
    }
    first.span.map(|start| start.through(end))
}

/// An identifier's text and quoting; a marker name reads as the call it stands for, unquoted.
fn written(identifier: &Identifier, map: &SourceMap<'_>) -> (String, bool) {
    match marker_call(identifier, map) {
        Some(call) => (call.to_owned(), false),
        None => (identifier.name.clone(), identifier.quoted),
    }
}

/// The call an unquoted marker name stands for.
fn marker_call<'source>(identifier: &Identifier, map: &SourceMap<'source>) -> Option<&'source str> {
    if identifier.quoted {
        return None;
    }
    let index: &str = identifier.name.strip_prefix(MARKER_NAME_PREFIX)?;
    match index.parse::<usize>() {
        Ok(index) => map.calls.get(index).copied(),
        Err(_not_a_marker) => None,
    }
}

fn literal_type(literal: &Literal) -> &'static str {
    match literal {
        Literal::String(_) => "string",
        Literal::Number(_) => "number",
        Literal::HexString(_) => "hex_string",
        Literal::HexNumber(_) => "hex_number",
        Literal::BitString(_) => "bit_string",
        Literal::ByteString(_) => "byte_string",
        Literal::NationalString(_) => "national_string",
        Literal::Date(_) => "date",
        Literal::Time(_) => "time",
        Literal::Timestamp(_) => "timestamp",
        Literal::Datetime(_) => "datetime",
        Literal::TripleQuotedString(_, _) => "triple_quoted_string",
        Literal::EscapeString(_) => "escape_string",
        Literal::DollarString(_) => "dollar_string",
        Literal::RawString(_) => "raw_string",
    }
}
