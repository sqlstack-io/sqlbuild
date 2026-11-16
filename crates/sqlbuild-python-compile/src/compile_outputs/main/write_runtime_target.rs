//! Write executed model and function lifecycle SQL under `target/run`.

use crate::compile_outputs::_helpers::runtime_target::RUN_DIR;
use crate::compile_outputs::_helpers::runtime_target::entry_paths;
use crate::compile_outputs::_helpers::runtime_target::relative_posix;
use crate::compile_outputs::_helpers::runtime_target::write_text;
use pyo3::prelude::{Bound, PyAny, PyAnyMethods, PyResult};
use sqlbuild_analysis::target_paths::main::function_run_path::function_run_path;
use sqlbuild_analysis::target_paths::main::model_run_path::model_run_path;
use sqlbuild_analysis::target_paths::main::runtime_sql_text::runtime_sql_text;
use sqlbuild_host::models::{BuildExecutionResult, LifeCycleEvent, LifeCycleEventKind};
use sqlbuild_python_base::records::main::fields::field::field as native_field;
use std::collections::HashMap;

/// Write executed model and function lifecycle SQL under `target/run`.
pub fn write_runtime_target(
    target_dir: &Bound<'_, PyAny>,
    plan: &Bound<'_, PyAny>,
    result: &BuildExecutionResult<Bound<'_, PyAny>>,
) -> PyResult<()> {
    let run_dir: Bound<'_, PyAny> = target_dir.div(RUN_DIR)?;
    let models: HashMap<String, String> =
        entry_paths(&native_field(plan, "model_entries")?, |entry| {
            relative_posix(entry).map(|path| model_run_path(&path))
        })?;
    let functions: HashMap<String, String> =
        entry_paths(&native_field(plan, "function_entries")?, |entry| {
            Ok(function_run_path(
                &relative_posix(entry)?,
                &native_field(entry, "language")?
                    .getattr("value")?
                    .extract::<String>()?,
            ))
        })?;
    let models = result.model_results.iter().map(|model| {
        (
            models.get(&model.model_name),
            model.lifecycle_events.as_slice(),
        )
    });
    let functions = result.function_results.iter().map(|function| {
        (
            functions.get(&function.function_name),
            function.lifecycle_events.as_slice(),
        )
    });
    for (path, events) in models.chain(functions) {
        let statements: Vec<String> = events
            .iter()
            .filter(|event: &&LifeCycleEvent| event.kind == LifeCycleEventKind::Sql)
            .map(|event| event.content.clone())
            .collect();
        if let Some(path) = path.filter(|_| !statements.is_empty()) {
            write_text(&run_dir.div(path)?, &runtime_sql_text(&statements))?;
        }
    }
    Ok(())
}
