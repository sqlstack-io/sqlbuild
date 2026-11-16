//! FLATTEN facts: the element each FLATTEN reads, and the whole elements a select item copies.

use sqlquack_syntax::Expression;
use sqlquack_syntax::expressions::functions::models::Function;
use sqlquack_syntax::expressions::names::models::Literal;

use crate::query_facts::models::{FlattenInput, WholeValue};

const VALUE_COLUMN: &str = "value";

/// Functions whose result embeds each argument whole.
const EMBEDDING_FUNCTIONS: [&str; 5] = [
    "object_construct",
    "object_construct_keep_null",
    "object_insert",
    "array_construct",
    "array_construct_compact",
];

/// Conversions whose result is their first argument whole, in another type.
const CONVERSION_FUNCTIONS: [&str; 6] = [
    "to_json",
    "to_char",
    "to_variant",
    "to_object",
    "to_array",
    "to_varchar",
];

/// The `FLATTEN` call a FROM item runs, as `LATERAL FLATTEN(...)` or `TABLE(FLATTEN(...))`.
pub(crate) fn flatten_call(expression: &Expression) -> Option<&Function> {
    match expression {
        Expression::Lateral(lateral) => flatten_call(&lateral.this),
        Expression::Function(function) if function.name.eq_ignore_ascii_case("table") => {
            function.args.first().and_then(flatten_call)
        }
        Expression::Function(function) if function.name.eq_ignore_ascii_case("flatten") => {
            Some(function)
        }
        _ => None,
    }
}

/// The column path a `FLATTEN` reads, such as `o.value:lines`, when its input is one.
pub(crate) fn flatten_input(function: &Function) -> Option<FlattenInput> {
    let input: &Expression = function.args.iter().find_map(|arg| match arg {
        Expression::NamedArgument(named) => named
            .name
            .name
            .eq_ignore_ascii_case("input")
            .then_some(&named.value),
        positional => Some(positional),
    })?;
    path_root(input)
}

/// The column `expression` reads a path of, with the path's first key.
fn path_root(expression: &Expression) -> Option<FlattenInput> {
    let (base, key): (&Expression, Option<String>) = match expression {
        Expression::Column(column) => {
            return Some(FlattenInput {
                qualifier: column.table.as_ref()?.name.to_ascii_lowercase(),
                column: column.name.name.to_ascii_lowercase(),
                key: None,
            });
        }
        Expression::Cast(cast) => return path_root(&cast.this),
        Expression::JSONExtract(extract) => (&extract.this, literal_key(&extract.expression)),
        Expression::Subscript(subscript) => (&subscript.this, literal_key(&subscript.index)),
        Expression::Function(function) if is_path_function(function) => (
            function.args.first()?,
            function.args.get(1).and_then(literal_key),
        ),
        _ => return None,
    };
    let mut root: FlattenInput = path_root(base)?;
    if root.key.is_none() {
        root.key = key;
    }
    Some(root)
}

fn is_path_function(function: &Function) -> bool {
    ["get", "get_path"]
        .iter()
        .any(|name| function.name.eq_ignore_ascii_case(name))
}

/// The first key of a path literal such as `'lines.sku'` or `'lines'`.
fn literal_key(expression: &Expression) -> Option<String> {
    let Expression::Literal(literal) = expression else {
        return None;
    };
    let Literal::String(path) = literal.as_ref() else {
        return None;
    };
    let key: &str = path
        .trim_start_matches('$')
        .trim_start_matches('.')
        .split(['.', '['])
        .next()?;
    (!key.is_empty()).then(|| key.trim_matches('"').to_ascii_lowercase())
}

/// Every `<alias>.value` that `expression` copies whole, with the keys `OBJECT_DELETE` removes.
pub(crate) fn whole_values(expression: &Expression) -> Vec<WholeValue> {
    let mut found: Vec<WholeValue> = Vec::new();
    let mut pending: Vec<(&Expression, Vec<String>)> = vec![(expression, Vec::new())];
    while let Some((node, removed)) = pending.pop() {
        match node {
            Expression::Column(column) if column.name.name.eq_ignore_ascii_case(VALUE_COLUMN) => {
                if let Some(table) = &column.table {
                    found.push(WholeValue {
                        qualifier: table.name.to_ascii_lowercase(),
                        removed,
                    });
                }
            }
            Expression::Paren(paren) => pending.push((&paren.this, removed)),
            Expression::Cast(cast) | Expression::TryCast(cast) | Expression::SafeCast(cast) => {
                pending.push((&cast.this, removed));
            }
            Expression::Function(function)
                if CONVERSION_FUNCTIONS
                    .iter()
                    .any(|name| function.name.eq_ignore_ascii_case(name)) =>
            {
                if let Some(value) = function.args.first() {
                    pending.push((value, removed));
                }
            }
            Expression::Function(function)
                if function.name.eq_ignore_ascii_case("object_delete") =>
            {
                let mut args = function.args.iter();
                if let Some(object) = args.next() {
                    pending.push((object, args.filter_map(literal_key).collect()));
                }
            }
            Expression::Function(function)
                if EMBEDDING_FUNCTIONS
                    .iter()
                    .any(|name| function.name.eq_ignore_ascii_case(name)) =>
            {
                pending.extend(function.args.iter().map(|arg| (arg, Vec::new())));
            }
            _ => {}
        }
    }
    found
}
