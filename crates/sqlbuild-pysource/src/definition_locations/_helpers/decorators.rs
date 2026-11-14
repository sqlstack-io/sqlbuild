//! How a decorator expression is named.

use ruff_python_ast::Expr;

/// The decorator's last name segment, looking through a call: `udf`, `sqlbuild.udf` and `udf(...)` all name `udf`.
pub(crate) fn decorator_name(expression: &Expr) -> Option<&str> {
    let target: &Expr = match expression {
        Expr::Call(call) => &call.func,
        _ => expression,
    };
    match target {
        Expr::Name(name) => Some(name.id.as_str()),
        Expr::Attribute(attribute) => Some(attribute.attr.as_str()),
        _ => None,
    }
}
