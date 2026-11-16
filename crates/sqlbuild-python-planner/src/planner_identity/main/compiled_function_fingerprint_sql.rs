//! Stable function definition text used for change fingerprints.

use crate::planner_identity::_helpers::metadata::rendered_pairs;
use crate::planner_identity::_helpers::metadata::text_of;
use pyo3::prelude::{Bound, PyAny, PyAnyMethods, PyResult};
use sqlbuild_analysis::planner_identity::main::function_fingerprint_sql::function_fingerprint_sql;
use sqlbuild_analysis::planner_identity::models::FunctionFingerprintFields;
use sqlbuild_python_project::resource_views::main::function_reads::arguments::arguments;
use sqlbuild_python_project::resource_views::main::function_reads::body_sql::body_sql;
use sqlbuild_python_project::resource_views::main::function_reads::entry_point::entry_point;
use sqlbuild_python_project::resource_views::main::function_reads::packages::packages;
use sqlbuild_python_project::resource_views::main::function_reads::return_columns::return_columns;
use sqlbuild_python_project::resource_views::main::function_reads::returns::returns;
use sqlbuild_python_project::resource_views::main::function_reads::runtime_version::runtime_version;

/// Stable function definition text used for change fingerprints.
pub fn compiled_function_fingerprint_sql(function: &Bound<'_, PyAny>) -> PyResult<String> {
    Ok(function_fingerprint_sql(&FunctionFingerprintFields {
        language:
            sqlbuild_python_project::resource_views::main::function_reads::language::language(
                function,
            )?
            .str()?
            .extract()?,
        arguments: rendered_pairs(&arguments(function)?)?,
        returns: returns(function)?.str()?.extract()?,
        return_columns: rendered_pairs(&return_columns(function)?)?,
        runtime_version: text_of(runtime_version(function))?,
        entry_point: text_of(entry_point(function))?,
        packages: packages(function)?
            .try_iter()?
            .map(|package| package?.extract::<String>())
            .collect::<PyResult<_>>()?,
        body_sql: body_sql(function)?.extract()?,
    }))
}
