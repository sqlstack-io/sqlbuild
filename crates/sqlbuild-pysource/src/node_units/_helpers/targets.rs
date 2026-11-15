//! The names assignments bind and the dotted names of class bases.

use ruff_python_ast::Expr;

/// The names an assignment's targets bind; empty when any target is an attribute or subscript.
pub(crate) fn assignment_names(targets: &[Expr]) -> Vec<String> {
    elements_names(targets).unwrap_or_default()
}

/// Each class base as dotted text, such as `paged.PagedApi`; empty for any other expression.
pub(crate) fn class_bases(bases: &[Expr]) -> Vec<String> {
    bases.iter().map(base_name).collect()
}

/// One class base as dotted text, or empty.
fn base_name(base: &Expr) -> String {
    dotted_name(base).unwrap_or_default()
}

/// The names one target binds through tuples, lists and starred targets; `None` for an attribute or subscript.
fn target_names(target: &Expr) -> Option<Vec<String>> {
    match target {
        Expr::Name(name) => Some(vec![name.id.to_string()]),
        Expr::Tuple(tuple) => elements_names(&tuple.elts),
        Expr::List(list) => elements_names(&list.elts),
        Expr::Starred(starred) => target_names(&starred.value),
        _ => None,
    }
}

/// The names several targets bind; `None` when any of them binds no plain name.
fn elements_names(elements: &[Expr]) -> Option<Vec<String>> {
    let names: Vec<Vec<String>> = elements.iter().map(target_names).collect::<Option<_>>()?;
    Some(names.concat())
}

/// A name or attribute chain as dotted text, seen through a subscript such as `Generic[T]`.
fn dotted_name(expression: &Expr) -> Option<String> {
    match expression {
        Expr::Name(name) => Some(name.id.to_string()),
        Expr::Attribute(attribute) => Some(format!(
            "{}.{}",
            dotted_name(&attribute.value)?,
            attribute.attr
        )),
        Expr::Subscript(subscript) => dotted_name(&subscript.value),
        _ => None,
    }
}
