//! The facts one refactoring plan of `requests` reads.

use crate::refactor::_helpers::refactor_facts::GENERIC_DIALECT;
use crate::refactor::_helpers::refactor_facts::authored_files;
use crate::refactor::_helpers::refactor_facts::discovered_files;
use crate::refactor::_helpers::refactor_facts::move_keeps_name;
use crate::refactor::_helpers::refactor_facts::python_locations;
use crate::refactor::_helpers::refactor_facts::resource_columns;
use crate::refactor::_helpers::refactor_facts::unit_tests;
use crate::refactor::main::model_facts::model_facts;
use pyo3::prelude::{Bound, PyAny, PyAnyMethods, PyResult};
use sqlbuild_python_base::records::main::fields::field::field as native_field;
use sqlbuild_refactor::refactoring::models::{
    DiscoveredFile, ManualLocation, RefactorFacts, RefactorRequest,
};

/// The facts one refactoring plan of `requests` reads.
pub fn refactor_facts(
    project: &Bound<'_, PyAny>,
    requests: &[RefactorRequest],
) -> PyResult<RefactorFacts> {
    let mut locations: Vec<ManualLocation> = Vec::new();
    for request in requests {
        if !move_keeps_name(request) {
            locations.extend(python_locations(project, request)?);
        }
    }
    let compiled: Bound<'_, PyAny> = native_field(&native_field(project, "graph")?, "project")?;
    let discovered: Bound<'_, PyAny> = native_field(project, "discovered")?;
    let dialect: Option<String> = native_field(&compiled, "sql_analysis_dialect")?.extract()?;
    let mut yaml_files: Vec<DiscoveredFile> =
        discovered_files(&native_field(&discovered, "source_files")?)?;
    yaml_files.extend(discovered_files(&native_field(
        &discovered,
        "schema_files",
    )?)?);
    Ok(RefactorFacts {
        project_dir: native_field(project, "project_dir")?.str()?.extract()?,
        dialect: dialect
            .filter(|dialect| !dialect.is_empty())
            .unwrap_or_else(|| GENERIC_DIALECT.to_owned()),
        models: model_facts(project)?.unwrap_or_default(),
        sources: resource_columns(&native_field(&compiled, "sources")?, "source_entry")?,
        seeds: resource_columns(&native_field(&compiled, "seeds")?, "schema_entry")?,
        model_files: discovered_files(&native_field(&discovered, "model_files")?)?,
        authored_files: authored_files(&discovered)?,
        yaml_files,
        unit_tests: unit_tests(&compiled)?,
        python_locations: locations,
    })
}
