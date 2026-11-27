//! `loader_projects`: each `@loader`'s compiled SQLLoad project, for runtimes built outside a command.

use pyo3::prelude::{Bound, PyAny, PyModule, PyModuleMethods, PyResult, pyfunction};
use pyo3::types::PyDict;
use pyo3::wrap_pyfunction;
use sqlbuild_python_front::compile_driver::main::planned_loader_projects::planned_loader_projects;

/// `{loader: (project text, Python source JSON)}` of `discovered_inputs`'s loaders, into `target` with its default `schema`.
#[pyfunction]
#[pyo3(signature = (*, discovered_inputs, project, target = None, schema = None))]
fn loader_projects<'py>(
    discovered_inputs: &Bound<'py, PyAny>,
    project: &Bound<'py, PyAny>,
    target: Option<&str>,
    schema: Option<&str>,
) -> PyResult<Bound<'py, PyDict>> {
    planned_loader_projects(discovered_inputs, project, (target, schema))
}

pub(crate) fn register(module: &Bound<'_, PyModule>) -> PyResult<()> {
    module.add_function(wrap_pyfunction!(loader_projects, module)?)
}
