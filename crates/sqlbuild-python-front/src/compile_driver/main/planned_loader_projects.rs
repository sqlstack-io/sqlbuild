//! For each `@loader`, the SQLLoad project its engine run starts from and the Python source it runs as.

use pyo3::exceptions::PyValueError;
use pyo3::prelude::{Bound, PyAny, PyAnyMethods, PyResult, Python};
use pyo3::types::{PyDict, PyDictMethods};
use sqlbuild_load::loads::main::loader_source::loader_source;
use sqlbuild_load::loads::main::run_project_text::run_project_text;
use sqlbuild_load::loads::models::LoadTarget;
use sqlbuild_python_base::records::main::fields::field::field as native_field;

use crate::compile_driver::_helpers::load_projects::{WAREHOUSE_SESSION, named_connection};
use crate::compile_driver::_helpers::loaders::loader_declaration;
use crate::compile_driver::constants::{LOADER_SCHEMA, LOADER_TARGET};
use crate::compile_driver::main::project_load_warehouse::project_load_warehouse;
use crate::compile_driver::models::ProjectConfiguration;

/// `{loader: (project text, Python source JSON)}` for each `@loader`, into `target` (its name and default schema) through the warehouse session.
pub fn planned_loader_projects<'py>(
    discovered: &Bound<'py, PyAny>,
    project: &Bound<'py, PyAny>,
    (target, schema): (Option<&str>, Option<&str>),
) -> PyResult<Bound<'py, PyDict>> {
    let py: Python<'py> = discovered.py();
    let projects: Bound<'py, PyDict> = PyDict::new(py);
    let loaders: Bound<'py, PyAny> = native_field(discovered, "loader_functions")?;
    if !loaders.is_truthy()? {
        return Ok(projects);
    }
    let effective_vars: Bound<'py, PyAny> = native_field(project, "effective_vars")?;
    let (project_dir, project_config, local_config) = (
        native_field(discovered, "project_dir")?,
        native_field(discovered, "project_config")?,
        native_field(discovered, "local_config")?,
    );
    let config = ProjectConfiguration {
        project_dir: &project_dir,
        project_config: &project_config,
        local_config: &local_config,
    };
    let source_files: Bound<'py, PyAny> = native_field(discovered, "source_files")?;
    let warehouse = project_load_warehouse(&config)?.map_err(PyValueError::new_err)?;
    let target = LoadTarget {
        name: target.unwrap_or(LOADER_TARGET).to_owned(),
        session: WAREHOUSE_SESSION.to_owned(),
        loader_schema: Some(schema.unwrap_or(LOADER_SCHEMA).to_owned()),
    };
    for loader in loaders.try_iter()? {
        let (declaration, _) = loader_declaration(&loader?, &source_files)?;
        let connections = named_connection(
            &config,
            effective_vars.cast()?,
            declaration.connection.as_deref(),
        )?;
        let text: String =
            run_project_text(None, connections, &target).map_err(PyValueError::new_err)?;
        let source = loader_source(&declaration, warehouse)
            .map_err(|problem| PyValueError::new_err(problem.message))?;
        projects.set_item(&declaration.name, (text, source.to_string()))?;
    }
    Ok(projects)
}
