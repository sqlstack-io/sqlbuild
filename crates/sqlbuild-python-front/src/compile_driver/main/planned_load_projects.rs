//! For each load a plan runs, the SQLLoad project it runs as and the session it writes through.

use pyo3::prelude::{Bound, PyAny, PyAnyMethods, PyResult, Python};
use pyo3::types::{PyDict, PyDictMethods};
use sqlbuild_load::loads::models::LoadTarget;
use sqlbuild_python_base::records::main::fields::field::field as native_field;

use crate::compile_driver::_helpers::load_projects::{WAREHOUSE_SESSION, load_project_text};
use crate::compile_driver::models::ProjectConfiguration;

/// `{load: (project text, session)}` for each load `plan` runs, into `target` with its `loader_schema`.
pub fn planned_load_projects<'py>(
    discovered: &Bound<'py, PyAny>,
    (project, plan): (&Bound<'py, PyAny>, &Bound<'py, PyAny>),
    (target, loader_schema): (&str, Option<&str>),
) -> PyResult<Bound<'py, PyDict>> {
    let py: Python<'py> = discovered.py();
    let projects: Bound<'py, PyDict> = PyDict::new(py);
    let planned: Vec<String> = native_field(plan, "load_entries")?
        .try_iter()?
        .map(|entry| native_field(&entry?, "name")?.extract::<String>())
        .collect::<PyResult<_>>()?;
    if planned.is_empty() {
        return Ok(projects);
    }
    let effective_vars: Bound<'py, PyAny> = native_field(project, "effective_vars")?;
    let (project_dir, project_config, local_config) = (
        native_field(discovered, "project_dir")?,
        native_field(discovered, "project_config")?,
        native_field(discovered, "local_config")?,
    );
    for load in native_field(project, "loads")?.try_iter()? {
        let load: Bound<'py, PyAny> = load?;
        let name: String = native_field(&load, "name")?.extract()?;
        if !planned.contains(&name) {
            continue;
        }
        let text: String = load_project_text(
            &ProjectConfiguration {
                project_dir: &project_dir,
                project_config: &project_config,
                local_config: &local_config,
            },
            effective_vars.cast()?,
            (&name, &native_field(&load, "settings")?),
            &LoadTarget {
                name: target.to_owned(),
                session: WAREHOUSE_SESSION.to_owned(),
                loader_schema: loader_schema.map(str::to_owned),
            },
        )?;
        projects.set_item(&name, (text, WAREHOUSE_SESSION))?;
    }
    Ok(projects)
}
