//! Raise when the selected target's schema is `preserve` and a loader or Python node has no schema.

use crate::compile_attachment::main::resolved_target::resolved_target;
use crate::compile_driver::_helpers::setup::targets::planner_error;
use pyo3::prelude::{Bound, PyAny, PyAnyMethods, PyResult, Python};
use sqlbuild_analysis::assembly::target_checks::constants::{
    MANAGED_WRITE_SCHEMA_CODE, PRESERVE_TARGET_VALUE, PRESERVED_NODE_RESULTS_CODE,
};
use sqlbuild_analysis::assembly::target_checks::errors::TargetError;
use sqlbuild_analysis::assembly::target_checks::main::target_error_message::target_error_message;
use sqlbuild_core::text::main::python_strip::python_strip;
use sqlbuild_python_base::boundary::main::optional_text::optional_text;
use sqlbuild_python_base::records::main::fields::field::field as native_field;
use std::collections::HashSet;

/// The discovered inputs' Python node functions, each kind of which keeps node results.
const PYTHON_NODE_FIELDS: [&str; 4] = [
    "loader_functions",
    "task_functions",
    "asset_functions",
    "check_functions",
];

/// Raise when the selected target's schema is `preserve` and one of `loaders` names no schema, or
/// the project has Python nodes and the target names no loader schema to keep their results in.
pub fn preserved_loader_schemas(
    discovered: &Bound<'_, PyAny>,
    selected_target: &Bound<'_, PyAny>,
    loaders: &HashSet<String>,
) -> PyResult<()> {
    let py: Python<'_> = discovered.py();
    let (target_name, target_config) = resolved_target(discovered, selected_target)?;
    if target_name.is_none() {
        return Ok(());
    }
    if !optional_text(&native_field(&target_config, "schema")?)?
        .is_some_and(|schema| python_strip(&schema) == PRESERVE_TARGET_VALUE)
    {
        return Ok(());
    }
    for loader in native_field(discovered, "loader_functions")?.try_iter()? {
        let loader: Bound<'_, PyAny> = loader?;
        let name: String = native_field(&loader, "name")?.extract()?;
        let destination: Option<String> = optional_text(&native_field(&loader, "destination")?)?;
        if !loaders.contains(&name) || destination.as_deref().is_some_and(|d| d.contains('.')) {
            continue;
        }
        let table: String = destination.unwrap_or_else(|| format!("__loader__{name}"));
        return Err(planner_error(
            py,
            target_error_message(&TargetError::PreservedLoaderSchema(name)),
            MANAGED_WRITE_SCHEMA_CODE,
            Some(format!(
                "to choose where it writes, set its destination in its @loader decorator:\n            \
                 @loader(destination=\"my_schema.{table}\")"
            )),
        ));
    }
    let loader_schema: Option<String> =
        optional_text(&native_field(&target_config, "loader_schema")?)?
            .map(|schema| python_strip(&schema).to_owned())
            .filter(|schema| !schema.is_empty() && schema != PRESERVE_TARGET_VALUE);
    if loader_schema.is_some() || !has_python_nodes(discovered)? {
        return Ok(());
    }
    let target: String = target_name.extract()?;
    Err(planner_error(
        py,
        target_error_message(&TargetError::PreservedNodeResults(target.clone())),
        PRESERVED_NODE_RESULTS_CODE,
        Some(format!(
            "to keep Python node results in a schema, set this in sqlbuild_project.toml:\n            \
             [targets.{target}]\n            \
             loader_schema = \"my_schema\""
        )),
    ))
}

/// Whether the project declares any loader, task, asset or check written in Python.
fn has_python_nodes(discovered: &Bound<'_, PyAny>) -> PyResult<bool> {
    for field in PYTHON_NODE_FIELDS {
        if native_field(discovered, field)?.len()? > 0 {
            return Ok(true);
        }
    }
    Ok(false)
}
