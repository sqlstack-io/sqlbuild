//! The compile-and-plan pipeline of execution commands: compile, rules gate, then planning.

use pyo3::prelude::{Bound, PyAny, PyAnyMethods, PyModule, PyModuleMethods, PyResult, Python};
use pyo3::types::{PyDict, PySet, PySetMethods};
use pyo3::{IntoPyObject, PyErr, pyfunction, wrap_pyfunction};

use crate::bindings::_helpers::boundary::python_calls::{KeywordArguments, keywords};
use crate::bindings::_helpers::compile_attachment::effective_config::build_effective_connection_config;
use crate::bindings::_helpers::compile_driver::phases::monotonic;
use crate::bindings::_helpers::compile_driver::pipeline_entries::compile_analysis_selection;
use crate::bindings::_helpers::compile_driver::project::build_compiled_project;
use crate::bindings::_helpers::diagnostics::trackers::current_tracker;
use crate::bindings::_helpers::records::construct::error_class;
use crate::bindings::_helpers::records::construct::{enum_member, new_record, new_record_with};
use crate::bindings::_helpers::records::fields::field as native_field;
use crate::bindings::_helpers::records::fields::replaced;
use crate::bindings::_helpers::records::ids::{ErrorId, RecordId};

/// The current build's phase timing tracker, when one is recording.
pub(crate) fn timing_tracker(py: Python<'_>) -> PyResult<Bound<'_, PyAny>> {
    current_tracker(py)
}

/// Report `message` through `on_progress` when a callback is set.
pub(crate) fn progress(on_progress: &Bound<'_, PyAny>, message: &str) -> PyResult<()> {
    if !on_progress.is_none() {
        on_progress.call1((message,))?;
    }
    Ok(())
}

/// Compile a project without opening a warehouse planning connection.
#[pyfunction]
#[pyo3(signature = (**options))]
pub(crate) fn compile_project_phase<'py>(
    py: Python<'py>,
    options: Option<Bound<'py, PyDict>>,
) -> PyResult<Bound<'py, PyAny>> {
    let arguments: KeywordArguments<'py> = KeywordArguments::read(
        py,
        "compile_project_phase",
        options,
        (
            &["discovered_inputs", "adapter"],
            &["options", "on_progress"],
        ),
    )?;
    compiled_phase(
        (
            &arguments.value("discovered_inputs")?,
            &arguments.value("adapter")?,
        ),
        &arguments.value("options")?,
        &arguments.value("on_progress")?,
        None,
    )
}

/// `compile_project_phase` with resolved arguments.
pub(crate) fn compiled_phase<'py>(
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

/// Raise one `CompileInputError` listing every compile error the command must stop on.
fn reject_compile_errors(
    project: &Bound<'_, PyAny>,
    options: &Bound<'_, PyAny>,
    on_progress: &Bound<'_, PyAny>,
) -> PyResult<()> {
    let py: Python<'_> = project.py();
    let plan_sql_tests: bool = native_field(options, "plan_sql_tests")?.is_truthy()?;
    let sql_test: Bound<'_, PyAny> = enum_member(py, RecordId::CompiledResourceType, "SQL_TEST")?;
    let mut errors: Vec<Bound<'_, PyAny>> = Vec::new();
    for diagnostic in native_field(project, "diagnostics")?.try_iter()? {
        let diagnostic: Bound<'_, PyAny> = diagnostic?;
        if native_field(&diagnostic, "is_error")?.is_truthy()?
            && (plan_sql_tests || native_field(&diagnostic, "resource_type")?.ne(&sql_test)?)
        {
            errors.push(diagnostic);
        }
    }
    let Some(first) = errors.first() else {
        return Ok(());
    };
    progress(on_progress, "Project compilation failed.")?;
    let mut lines: Vec<String> = Vec::with_capacity(errors.len());
    let helps: Bound<'_, PySet> = PySet::empty(py)?;
    for diagnostic in &errors {
        let or_one = |name: &str| -> PyResult<String> {
            let value: Bound<'_, PyAny> = native_field(diagnostic, name)?;
            Ok(if value.is_truthy()? {
                value.str()?.to_string()
            } else {
                "1".to_owned()
            })
        };
        lines.push(format!(
            "[{}] {}:{}:{}: {}",
            native_field(diagnostic, "code")?.str()?,
            native_field(diagnostic, "path")?.str()?,
            or_one("line")?,
            or_one("column")?,
            native_field(diagnostic, "message")?.str()?,
        ));
        helps.add(native_field(diagnostic, "help")?)?;
    }
    let help: Bound<'_, PyAny> = if helps.len() == 1 {
        helps.pop().unwrap_or_else(|| py.None().into_bound(py))
    } else {
        py.None().into_bound(py)
    };
    let error: Bound<'_, PyAny> = error_class(py, ErrorId::CompileInput)?.call(
        (lines.join("\n"),),
        Some(&keywords(
            py,
            &[("code", native_field(first, "code")?), ("help", help)],
        )?),
    )?;
    Err(PyErr::from_value(error))
}

pub(crate) fn register(module: &Bound<'_, PyModule>) -> PyResult<()> {
    module.add_function(wrap_pyfunction!(compile_project_phase, module)?)?;
    Ok(())
}
