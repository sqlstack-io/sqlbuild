//! Raise when the selected target's schema is `preserve` and a loader or Python node has no schema.

use crate::compile_attachment::main::resolved_target::resolved_target;
use crate::compile_driver::_helpers::setup::node_results::require_node_results_schema;
use crate::compile_driver::_helpers::setup::targets::planner_error;
use pyo3::prelude::{Bound, PyAny, PyAnyMethods, PyResult, Python};
use sqlbuild_analysis::assembly::target_checks::constants::{
    MANAGED_WRITE_SCHEMA_CODE, PRESERVE_TARGET_VALUE,
};
use sqlbuild_analysis::assembly::target_checks::errors::TargetError;
use sqlbuild_analysis::assembly::target_checks::main::target_error_message::target_error_message;
use sqlbuild_core::text::main::python_strip::python_strip;
use sqlbuild_python_base::boundary::main::optional_text::optional_text;
use sqlbuild_python_base::records::main::fields::field::field as native_field;
use std::collections::HashSet;

/// Raise when a `preserve` target leaves `loaders` or Python node results without a schema, after `effective_vars`.
pub(crate) fn preserved_loader_schemas(
    discovered: &Bound<'_, PyAny>,
    (selected_target, effective_vars): (&Bound<'_, PyAny>, &Bound<'_, PyAny>),
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
    require_node_results_schema(discovered, (&target_name, &target_config), effective_vars)
}
