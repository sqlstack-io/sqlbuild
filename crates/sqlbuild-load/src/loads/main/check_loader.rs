//! Check a `@loader`'s options as SQLLoad reads a Python table's, without connecting anywhere.

use sqlload::project::errors::ProjectError;
use sqlload::project::main::parse_python_project::parse_python_project;
use sqlload::python_sources::models::PythonSource;

use crate::loads::_helpers::loaders::{
    loader_problem, python_source, refused_connection, refused_option, table_settings,
};
use crate::loads::_helpers::project::check_project;
use crate::loads::models::{LoadCheck, LoadProblem, LoaderDeclaration};

/// `loader`'s table settings as SQLLoad reads them, with the columns SQLBuild gives it, or why they can't be used.
pub fn check_loader(
    check: &LoadCheck,
    loader: &LoaderDeclaration,
) -> Result<toml::Table, LoadProblem> {
    refused_option(loader)?;
    refused_connection(check, loader)?;
    let table: toml::Table = table_settings(loader);
    let source: PythonSource = python_source(loader, &table)?;
    let (text, sessions) = check_project(check, None).map_err(|message| LoadProblem {
        message,
        path: Vec::new(),
    })?;
    match parse_python_project(&text, &|_| None, (&[source], sessions)) {
        Ok(_) => Ok(table),
        Err(ProjectError::Syntax(message)) => Err(loader_problem(loader, &message)),
        Err(ProjectError::Invalid(problems)) => Err(loader_problem(loader, &problems.join("\n"))),
        Err(error) => Err(loader_problem(loader, &error.to_string())),
    }
}
