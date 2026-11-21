//! Find the `@udf(...)` or `@table_fn(...)` function of a Python function file without importing it.

use ruff_python_ast::{ExprCall, Stmt, StmtFunctionDef};

use crate::cpython_tree::main::parse_cpython_module::parse_cpython_module;
use crate::cpython_tree::models::ParsedSource;
use crate::udf_functions::_helpers::dataframes::parameter_dataframe;
use crate::udf_functions::_helpers::decorators::{docstring, is_udf_import, udf_call, udf_keyword};
use crate::udf_functions::models::{FunctionDecorator, UdfFunction, UdfKeyword, UdfShape};

/// The `@udf(...)` or `@table_fn(...)` function of `contents`, or `None` when it does not parse.
#[must_use]
pub fn udf_function(contents: &str, python_version: (u8, u8)) -> Option<UdfShape> {
    let parsed: ParsedSource = parse_cpython_module(contents, python_version)?;
    let mut matches: Vec<(usize, &StmtFunctionDef, (&ExprCall, FunctionDecorator))> = Vec::new();
    for (index, statement) in parsed.module.body.iter().enumerate() {
        let Stmt::FunctionDef(function) = statement else {
            continue;
        };
        matches.extend(
            function
                .decorator_list
                .iter()
                .filter_map(udf_call)
                .map(|call| (index, function, call)),
        );
    }
    let (statement_index, function, (call, decorator)) = match matches.as_slice() {
        [] => return Some(UdfShape::Missing),
        [single] => *single,
        _ => return Some(UdfShape::Multiple),
    };
    let keywords: Vec<UdfKeyword> = call
        .arguments
        .keywords
        .iter()
        .map(|keyword| udf_keyword(keyword, &parsed.lines))
        .collect();
    Some(UdfShape::Single(UdfFunction {
        name: function.name.as_str().to_owned(),
        decorator,
        dataframe: parameter_dataframe(&parsed.module.body, function),
        keywords,
        has_positional_arguments: !call.arguments.args.is_empty(),
        docstring: docstring(function),
        statement_index,
        removed_decorators: function
            .decorator_list
            .iter()
            .enumerate()
            .filter(|(_, decorator)| udf_call(decorator).is_some())
            .map(|(index, _)| index)
            .collect(),
        removed_statements: parsed
            .module
            .body
            .iter()
            .enumerate()
            .filter(|(_, statement)| is_udf_import(statement))
            .map(|(index, _)| index)
            .collect(),
    }))
}
