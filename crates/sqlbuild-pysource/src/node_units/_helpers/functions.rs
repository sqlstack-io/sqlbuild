//! A function's decorators, signature and body shape, and the decorated functions inside it.

use ruff_python_ast::visitor::{Visitor, walk_expr};
use ruff_python_ast::{Decorator, Expr, Parameter, Stmt, StmtFunctionDef};
use ruff_python_parser::{Mode, ParseOptions};
use ruff_text_size::Ranged;

use crate::cpython_tree::main::walk_breadth_first::walk_breadth_first;
use crate::cpython_tree::models::CpythonNode;
use crate::node_units::_helpers::positions::{UnitText, unit_position};
use crate::node_units::constants::{
    DEPENDS_ON_KEYWORD, MAIN_FUNCTION, NONE_NAME, OPTIONAL_NAME, UNION_NAMES,
};
use crate::node_units::models::{UnitDecorator, UnitFunction, UnitParameter};

/// The shape of `function` and of every decorated function inside it.
pub(crate) fn unit_function(function: &StmtFunctionDef, source: &UnitText<'_>) -> UnitFunction {
    let parameters = &function.parameters;
    let body: Vec<CpythonNode<'_>> =
        walk_breadth_first(function.body.iter().map(CpythonNode::Stmt).collect());
    UnitFunction {
        name: function.name.to_string(),
        position: unit_position(source, function.name.range().start()),
        decorators: function
            .decorator_list
            .iter()
            .filter_map(decorator)
            .collect(),
        positional: parameters
            .posonlyargs
            .iter()
            .chain(&parameters.args)
            .map(|with_default| parameter(&with_default.parameter))
            .collect(),
        variadic: parameters.vararg.is_some(),
        keyword_only: parameters
            .kwonlyargs
            .iter()
            .map(|with_default| parameter(&with_default.parameter))
            .collect(),
        keywords_variadic: parameters.kwarg.is_some(),
        returns: function.returns.as_deref().map(union_names),
        docstring: leading_docstring(&function.body),
        returns_directly: matches!(without_docstring(&function.body), [Stmt::Return(_)]),
        delegation: delegation(&function.body),
        inner: body
            .iter()
            .filter_map(|node| match node {
                CpythonNode::Stmt(Stmt::FunctionDef(inner)) if !inner.decorator_list.is_empty() => {
                    Some(unit_function(inner, source))
                }
                _ => None,
            })
            .collect(),
        loops: body
            .iter()
            .filter_map(|node| match node {
                CpythonNode::Stmt(Stmt::For(loop_statement)) => {
                    Some(loaded_names(&loop_statement.iter))
                }
                _ => None,
            })
            .collect(),
    }
}

/// Whether `body` opens with a string expression statement.
pub(crate) fn leading_docstring(body: &[Stmt]) -> bool {
    matches!(body.first(), Some(Stmt::Expr(statement)) if statement.value.is_string_literal_expr())
}

/// `body` without its leading docstring.
fn without_docstring(body: &[Stmt]) -> &[Stmt] {
    if leading_docstring(body) {
        &body[1..]
    } else {
        body
    }
}

/// For a body that is exactly `return main(...)` after any docstring, whether every argument is a named keyword.
fn delegation(body: &[Stmt]) -> Option<bool> {
    let statements: &[Stmt] = if leading_docstring(body) {
        &body[1..]
    } else {
        body
    };
    let [Stmt::Return(returned)] = statements else {
        return None;
    };
    let Some(Expr::Call(call)) = returned.value.as_deref() else {
        return None;
    };
    let Expr::Name(function) = call.func.as_ref() else {
        return None;
    };
    (function.id.as_str() == MAIN_FUNCTION).then(|| {
        call.arguments.args.is_empty()
            && call
                .arguments
                .keywords
                .iter()
                .all(|keyword| keyword.arg.is_some())
    })
}

/// A decorator by its last name, with its keywords and the names its `depends_on` value reads.
fn decorator(decorator: &Decorator) -> Option<UnitDecorator> {
    let (target, call) = match &decorator.expression {
        Expr::Call(call) => (call.func.as_ref(), Some(call)),
        other => (other, None),
    };
    let name: String = last_name(target)?;
    let keywords = call
        .map(|call| call.arguments.keywords.as_ref())
        .unwrap_or_default();
    Some(UnitDecorator {
        name,
        keywords: keywords
            .iter()
            .filter_map(|keyword| keyword.arg.as_ref().map(ToString::to_string))
            .collect(),
        depends_on: keywords
            .iter()
            .filter(|keyword| {
                keyword
                    .arg
                    .as_ref()
                    .is_some_and(|arg| arg.as_str() == DEPENDS_ON_KEYWORD)
            })
            .flat_map(|keyword| walk_breadth_first(vec![CpythonNode::Expr(&keyword.value)]))
            .filter_map(|node| match node {
                CpythonNode::Expr(Expr::Name(name)) => Some(name.id.to_string()),
                _ => None,
            })
            .collect(),
    })
}

fn parameter(parameter: &Parameter) -> UnitParameter {
    UnitParameter {
        name: parameter.name.to_string(),
        annotation: parameter.annotation.as_deref().and_then(last_name),
    }
}

/// The last name of each member of an annotation written as `A | B`, `Optional[A]` or `Union[A, B]`.
fn union_names(annotation: &Expr) -> Vec<String> {
    match annotation {
        Expr::BinOp(union) if union.op.is_bit_or() => {
            let mut names: Vec<String> = union_names(&union.left);
            names.extend(union_names(&union.right));
            names
        }
        Expr::NoneLiteral(_) => vec![NONE_NAME.to_owned()],
        Expr::StringLiteral(literal) => string_expression(literal.value.to_str())
            .map_or_else(Vec::new, |expression| union_names(&expression)),
        Expr::Subscript(subscript)
            if last_name(&subscript.value)
                .is_some_and(|name| UNION_NAMES.contains(&name.as_str())) =>
        {
            let mut names: Vec<String> = match subscript.slice.as_ref() {
                Expr::Tuple(members) => members.elts.iter().flat_map(union_names).collect(),
                member => union_names(member),
            };
            if last_name(&subscript.value).as_deref() == Some(OPTIONAL_NAME) {
                names.push(NONE_NAME.to_owned());
            }
            names
        }
        other => last_name(other).into_iter().collect(),
    }
}

/// The last name of a name, an attribute chain, or the subscripted base of either.
pub(crate) fn last_name(expression: &Expr) -> Option<String> {
    match expression {
        Expr::Name(name) => Some(name.id.to_string()),
        Expr::Attribute(attribute) => Some(attribute.attr.to_string()),
        Expr::Subscript(subscript) => last_name(&subscript.value),
        Expr::StringLiteral(literal) => last_name(&string_expression(literal.value.to_str())?),
        _ => None,
    }
}

/// The expression a string annotation holds, if it parses.
fn string_expression(text: &str) -> Option<Expr> {
    let parsed =
        ruff_python_parser::parse_unchecked(text.trim(), ParseOptions::from(Mode::Expression));
    if !parsed.has_no_syntax_errors() {
        return None;
    }
    Some(*parsed.try_into_expression()?.into_syntax().body)
}

/// Every name `expression` loads, in source order: `enumerate` and `SPECS` for `enumerate(SPECS.items())`.
fn loaded_names(expression: &Expr) -> Vec<String> {
    let mut names = LoadedNames::default();
    names.visit_expr(expression);
    names.names
}

#[derive(Default)]
struct LoadedNames {
    names: Vec<String>,
}

impl<'a> Visitor<'a> for LoadedNames {
    fn visit_expr(&mut self, expression: &'a Expr) {
        if let Expr::Name(name) = expression {
            self.names.push(name.id.to_string());
        }
        walk_expr(self, expression);
    }
}
