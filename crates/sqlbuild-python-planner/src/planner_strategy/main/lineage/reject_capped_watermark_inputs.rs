//! Reject capped producers feeding a microbatch model's watermark inputs.

use crate::planner_strategy::_helpers::lineage::ProjectLineage;
use crate::planner_strategy::_helpers::lineage::lineage_error;
use crate::planner_strategy::_helpers::lineage::microbatch_flag;
use crate::planner_strategy::_helpers::lineage::references;
use crate::planner_strategy::_helpers::lineage::watermark_names;
use crate::planner_strategy::main::values::input_error::input_error;
use pyo3::prelude::{Bound, PyAny, PyAnyMethods, PyResult, Python};
use pyo3::types::PyDict;
use sqlbuild_analysis::planner_strategy::main::capped_watermark_input::capped_watermark_input;
use sqlbuild_python_base::records::types::ErrorId;
use sqlbuild_python_project::compiled_views::main::compiled_models::model_name::model_name;
use sqlbuild_python_project::native_host::models::ModelLookup;

/// Reject capped producers feeding a microbatch model's watermark inputs.
pub fn reject_capped_watermark_inputs<'py>(
    model: &Bound<'py, PyAny>,
    models_by_name: ModelLookup,
    functions_by_name: Option<Bound<'py, PyAny>>,
) -> PyResult<()> {
    let py: Python<'py> = model.py();
    if !microbatch_flag(model)? {
        return Ok(());
    }
    let functions_by_name: Bound<'py, PyAny> = match functions_by_name {
        Some(functions) if functions.is_truthy()? => functions,
        _ => PyDict::new(py).into_any(),
    };
    let graph = ProjectLineage {
        models_by_name,
        functions_by_name,
    };
    let name: String = model_name(model)?;
    let inputs: Vec<String> = watermark_names(model)?;
    match capped_watermark_input(&graph, (&name, references(model)?), inputs) {
        Ok(None) => Ok(()),
        Ok(Some(capped)) => Err(input_error(
            py,
            ErrorId::CompileInput,
            &capped.message(),
            None,
        )),
        Err(failure) => Err(lineage_error(py, failure)),
    }
}
