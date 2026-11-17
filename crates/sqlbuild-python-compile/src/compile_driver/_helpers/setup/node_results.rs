//! The schema a `preserve` target keeps Python node results in.

use crate::compile_driver::_helpers::setup::targets::planner_error;
use pyo3::prelude::{Bound, PyAny, PyAnyMethods, PyResult};
use sqlbuild_analysis::assembly::target_checks::constants::{
    PRESERVE_TARGET_VALUE, PRESERVED_NODE_RESULTS_CODE,
};
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

/// Raise when a `preserve` target has Python nodes but no loader schema to keep their results in.
pub(crate) fn require_node_results_schema(
    discovered: &Bound<'_, PyAny>,
    (target_name, target_config): (&Bound<'_, PyAny>, &Bound<'_, PyAny>),
) -> PyResult<()> {
    let loader_schema: Option<String> =
        optional_text(&native_field(target_config, "loader_schema")?)?
            .map(|schema| python_strip(&schema).to_owned())
            .filter(|schema| !schema.is_empty() && schema != PRESERVE_TARGET_VALUE);
    if loader_schema.is_some() || python_node_count(discovered)? == 0 {
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

/// How many loaders, tasks, assets and checks the project declares in Python.
fn python_node_count(discovered: &Bound<'_, PyAny>) -> PyResult<usize> {
    PYTHON_NODE_FIELDS
        .iter()
        .map(|field| native_field(discovered, field)?.len())
        .sum()
}
