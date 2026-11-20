//! The dataframe library a function's first parameter annotation names, through module imports.

use ruff_python_ast::{Alias, Expr, Stmt, StmtFunctionDef};

use crate::udf_functions::constants::{DATAFRAME_CLASS, PANDAS_MODULE, POLARS_MODULE};
use crate::udf_functions::models::DataFrameLibrary;

/// `pd.DataFrame` after `import pandas as pd`, or `Frame` after `from polars import DataFrame as Frame`.
pub(crate) fn parameter_dataframe(
    body: &[Stmt],
    function: &StmtFunctionDef,
) -> Option<DataFrameLibrary> {
    let parameters = &function.parameters;
    let first = parameters
        .posonlyargs
        .iter()
        .chain(&parameters.args)
        .next()?;
    match first.parameter.annotation.as_deref()? {
        Expr::Attribute(attribute) if attribute.attr.as_str() == DATAFRAME_CLASS => {
            let Expr::Name(module) = attribute.value.as_ref() else {
                return None;
            };
            imported_module(body, module.id.as_str())
        }
        Expr::Name(name) => imported_class(body, name.id.as_str()),
        _ => None,
    }
}

/// The library `import pandas` or `import pandas as <alias>` binds to `alias`.
fn imported_module(body: &[Stmt], alias: &str) -> Option<DataFrameLibrary> {
    body.iter()
        .find_map(|statement| import_library(statement, alias))
}

/// The library one `import` statement binds to `alias`.
fn import_library(statement: &Stmt, alias: &str) -> Option<DataFrameLibrary> {
    let Stmt::Import(import) = statement else {
        return None;
    };
    import
        .names
        .iter()
        .find_map(|name| module_library(name, alias))
}

/// The library one `import` name binds to `alias`.
fn module_library(name: &Alias, alias: &str) -> Option<DataFrameLibrary> {
    if bound_name(name) != alias {
        return None;
    }
    library(name.name.as_str())
}

/// The library `from pandas import DataFrame` or `... import DataFrame as <alias>` binds to `alias`.
fn imported_class(body: &[Stmt], alias: &str) -> Option<DataFrameLibrary> {
    body.iter()
        .find_map(|statement| import_from_library(statement, alias))
}

/// The library one `from ... import` statement binds to `alias`.
fn import_from_library(statement: &Stmt, alias: &str) -> Option<DataFrameLibrary> {
    let Stmt::ImportFrom(import) = statement else {
        return None;
    };
    let module: &str = import.module.as_ref()?.as_str();
    import
        .names
        .iter()
        .find_map(|name| class_library(name, module, alias))
}

/// The library one `from <module> import` name binds to `alias` when it is the frame class.
fn class_library(name: &Alias, module: &str, alias: &str) -> Option<DataFrameLibrary> {
    if bound_name(name) != alias || name.name.as_str() != DATAFRAME_CLASS {
        return None;
    }
    library(module)
}

/// The name an imported alias binds in the module.
fn bound_name(name: &Alias) -> &str {
    name.asname.as_ref().unwrap_or(&name.name).as_str()
}

fn library(module: &str) -> Option<DataFrameLibrary> {
    match module {
        PANDAS_MODULE => Some(DataFrameLibrary::Pandas),
        POLARS_MODULE => Some(DataFrameLibrary::Polars),
        _ => None,
    }
}
