//! Model files that `[defaults]`, `[path_defaults]` or a model schema describe.

use crate::discovery::_helpers::records::description_inputs::discover_model_description_inputs;
use crate::model_config::main::model_nodes::externally_described_models::externally_described_models as described_models;
use crate::render::main::declarations::public_model_schema_index::public_model_schema_index;
use pyo3::prelude::{Bound, PyAny, PyResult, Python};
use pyo3::types::{PyDict, PyFrozenSet};

/// Model files that `[defaults]`, `[path_defaults]` or a model schema describe.
pub fn externally_described_models<'py>(
    project_dir: &Bound<'py, PyAny>,
) -> PyResult<Bound<'py, PyFrozenSet>> {
    let py: Python<'py> = project_dir.py();
    let inputs: Bound<'py, PyAny> = discover_model_description_inputs(project_dir)?;
    let schemas: Bound<'py, PyDict> = public_model_schema_index(inputs.clone())?;
    PyFrozenSet::new(py, described_models(inputs, schemas)?)
}
