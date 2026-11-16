//! `compile_project_phase` with resolved arguments.

use crate::compile_driver::_helpers::setup::pipeline::reject_compile_errors;
use crate::compile_driver::_helpers::setup::pipeline_entries::compile_analysis_selection;
use crate::compile_driver::main::setup::build_compiled_project::build_compiled_project;
use crate::compile_driver::main::setup::progress::progress;
use crate::compile_driver::main::setup::timing_tracker::timing_tracker;
use crate::compile_driver::main::stages::monotonic::monotonic;
use pyo3::IntoPyObject;
use pyo3::prelude::{Bound, PyAny, PyAnyMethods, PyResult, Python};
use sqlbuild_python_base::boundary::main::keywords::keywords;
use sqlbuild_python_base::records::main::construct::new_record::new_record;
use sqlbuild_python_base::records::main::construct::new_record_with::new_record_with;
use sqlbuild_python_base::records::main::fields::field::field as native_field;
use sqlbuild_python_base::records::main::fields::replaced::replaced;
use sqlbuild_python_base::records::types::RecordId;
use sqlbuild_python_front::compile_attachment::main::build_effective_connection_config::build_effective_connection_config;

/// `compile_project_phase` with resolved arguments.
pub fn compiled_phase<'py>(
    (discovered, adapter): (&Bound<'py, PyAny>, &Bound<'py, PyAny>),
    options: &Bound<'py, PyAny>,
    on_progress: &Bound<'py, PyAny>,
    on_inputs_ready: Option<&Bound<'py, PyAny>>,
) -> PyResult<Bound<'py, PyAny>> {
    let py: Python<'py> = discovered.py();
    let options: Bound<'py, PyAny> = if options.is_truthy()? {
        options.clone()
    } else {
        new_record(py, RecordId::CompilePipelineOptions, (), None)?
    };
    let configured: Bound<'py, PyAny> = native_field(&options, "connection_config")?;
    let connection_config: Bound<'py, PyAny> = if configured.is_none() {
        build_effective_connection_config(
            discovered,
            Some(native_field(&options, "selected_target")?),
            Some(native_field(&options, "cli_vars")?),
            false,
        )?
    } else {
        configured
    };
    progress(on_progress, "Compiling project...")?;
    let started: f64 = monotonic(py)?;
    let compiled: PyResult<Bound<'py, PyAny>> = (|| {
        let selection: Bound<'py, PyAny> = compile_analysis_selection(&options, discovered)?;
        let selection: Bound<'py, PyAny> = match on_inputs_ready {
            Some(hook) => replaced(&selection, &[("on_inputs_ready", hook.clone())])?,
            None => selection,
        };
        build_compiled_project(
            py,
            Some(keywords(
                py,
                &[
                    ("discovered_inputs", discovered.clone()),
                    ("adapter", adapter.clone()),
                    (
                        "selected_target",
                        native_field(&options, "selected_target")?,
                    ),
                    (
                        "no_sql_validation",
                        native_field(&options, "no_sql_validation")?,
                    ),
                    ("cli_vars", native_field(&options, "cli_vars")?),
                    (
                        "external_sql_reference_resolver",
                        native_field(&options, "external_sql_reference_resolver")?,
                    ),
                    ("resolved_connection", connection_config.clone()),
                    ("analysis_selection", selection),
                    (
                        "source_lexical_syntax",
                        native_field(&options, "source_lexical_syntax")?,
                    ),
                ],
            )?),
        )
    })();
    let compile_seconds: f64 = monotonic(py)? - started;
    let tracker: Bound<'py, PyAny> = timing_tracker(py)?;
    if !tracker.is_none() {
        tracker.setattr("compile_seconds", compile_seconds)?;
    }
    let project: Bound<'py, PyAny> = compiled?;
    reject_compile_errors(&project, &options, on_progress)?;
    progress(
        on_progress,
        &format!("Compiled project. ({compile_seconds:.2}s)"),
    )?;
    new_record_with(
        py,
        RecordId::CompiledProjectPhaseResult,
        &[
            ("project", project),
            ("connection_config", connection_config),
            (
                "compile_seconds",
                compile_seconds.into_pyobject(py)?.into_any(),
            ),
        ],
    )
}
