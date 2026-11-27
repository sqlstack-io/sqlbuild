//! Check a load as SQLLoad reads it, with the column types of the sources it fills, without connecting anywhere.

use sqlload::project::errors::ProjectError;
use sqlload::project::main::parse_python_project::parse_python_project;

use crate::loads::_helpers::problems::problem;
use crate::loads::_helpers::project::check_project;
use crate::loads::_helpers::settings::{at, refuse_warehouse_connection, with_source_columns};
use crate::loads::models::{LoadCheck, LoadDeclaration, LoadProblem};

/// `load`'s settings with its sources' column types, as SQLLoad reads them, or why it can't be used.
pub fn check_load(check: &LoadCheck, load: &LoadDeclaration) -> Result<toml::Table, LoadProblem> {
    refuse_warehouse_connection(check, load)?;
    let settings: toml::Table = with_source_columns(load)?;
    let (text, sessions) = check_project(check, Some((&load.name, settings.clone())))
        .map_err(|message| at(&[], message))?;
    match parse_python_project(&text, &|_| None, (&[], sessions)) {
        Ok(_) => Ok(settings),
        Err(ProjectError::Syntax(message)) => Err(problem(&load.name, &message)),
        Err(ProjectError::Invalid(problems)) => Err(problem(&load.name, &problems.join("\n"))),
        Err(error) => Err(problem(&load.name, &error.to_string())),
    }
}
