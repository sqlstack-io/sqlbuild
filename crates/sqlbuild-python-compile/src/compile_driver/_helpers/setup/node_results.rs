//! The schema a `preserve` target keeps Python node results in.

use crate::compile_driver::_helpers::setup::model_free_locations::kept_loader_schema;
use crate::compile_driver::_helpers::setup::targets::planner_error;
use crate::compile_driver::main::setup::expanded_loader_namespace::expanded_loader_namespace;
use pyo3::exceptions::PyException;
use pyo3::prelude::{Bound, PyAny, PyAnyMethods, PyResult};
use pyo3::types::{PyDict, PyDictMethods};
use sqlbuild_analysis::assembly::target_checks::constants::PRESERVED_NODE_RESULTS_CODE;
use sqlbuild_analysis::assembly::target_checks::errors::TargetError;
use sqlbuild_analysis::assembly::target_checks::main::target_error_message::target_error_message;
use sqlbuild_core::text::main::python_strip::python_strip;
use sqlbuild_python_base::boundary::main::optional_text::optional_text;
use sqlbuild_python_base::records::main::fields::field::field as native_field;

/// The discovered inputs' Python node functions, each kind of which keeps node results.
const PYTHON_NODE_FIELDS: [&str; 4] = [
    "loader_functions",
    "task_functions",
    "asset_functions",
    "check_functions",
];

/// Raise when a `preserve` target has Python nodes but no usable loader schema, after `effective_vars`, to keep their results in.
pub(crate) fn require_node_results_schema(
    discovered: &Bound<'_, PyAny>,
    (target_name, target_config): (&Bound<'_, PyAny>, &Bound<'_, PyAny>),
    effective_vars: &Bound<'_, PyAny>,
) -> PyResult<()> {
    let loader_schema: Option<String> = resolved_loader_schema(target_config, effective_vars)?;
    if kept_loader_schema(loader_schema.as_deref()).is_some() || python_node_count(discovered)? == 0
    {
        return Ok(());
    }
    let target: String = target_name.extract()?;
    Err(planner_error(
        discovered.py(),
        target_error_message(&TargetError::PreservedNodeResults(target.clone())),
        PRESERVED_NODE_RESULTS_CODE,
        Some(format!(
            "to keep Python node results in a schema, set this in sqlbuild_project.toml:\n            \
             [targets.{target}]\n            \
             loader_schema = \"my_schema\""
        )),
    ))
}

/// The target's `loader_schema` as runtime resolves it: vars expanded, `None` when it cannot resolve outside a model.
fn resolved_loader_schema(
    target_config: &Bound<'_, PyAny>,
    effective_vars: &Bound<'_, PyAny>,
) -> PyResult<Option<String>> {
    let Some(schema) = optional_text(&native_field(target_config, "loader_schema")?)? else {
        return Ok(None);
    };
    let py = target_config.py();
    let namespace: Bound<'_, PyDict> = PyDict::new(py);
    namespace.set_item("schema", python_strip(&schema))?;
    match expanded_loader_namespace(namespace.as_any(), effective_vars) {
        Ok(expanded) => {
            let schema: Bound<'_, PyAny> = expanded.get_item("schema")?;
            Ok((!schema.is_none())
                .then(|| schema.str()?.extract())
                .transpose()?)
        }
        Err(error) if error.is_instance_of::<PyException>(py) => Ok(None),
        Err(error) => Err(error),
    }
}

/// How many loaders, tasks, assets and checks the project declares in Python.
pub(crate) fn python_node_count(discovered: &Bound<'_, PyAny>) -> PyResult<usize> {
    PYTHON_NODE_FIELDS
        .iter()
        .map(|field| native_field(discovered, field)?.len())
        .sum()
}
