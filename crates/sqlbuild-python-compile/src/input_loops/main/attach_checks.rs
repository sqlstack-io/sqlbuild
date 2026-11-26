//! Attach every SQL test case, scenario and audit, then report every resource diagnostic.

use crate::input_loops::_helpers::resources::attachment::TEST_TIMING_KEY;
use crate::input_loops::_helpers::resources::attachment::audit_file_row;
use crate::input_loops::_helpers::resources::attachment::audit_settings;
use crate::input_loops::_helpers::resources::attachment::diagnostic_settings;
use crate::input_loops::_helpers::resources::attachment::items;
use crate::input_loops::_helpers::resources::attachment::scenario_row;
use crate::input_loops::_helpers::resources::attachment::sql_test_settings;
use crate::input_loops::_helpers::resources::attachment::test_file_row;
use crate::input_loops::_helpers::resources::resource_attachment::attach_audits;
use crate::input_loops::_helpers::resources::resource_attachment::attach_sql_tests;
use crate::input_loops::_helpers::resources::resource_diagnostics::resource_diagnostics;
use pyo3::prelude::{Bound, PyAny, PyAnyMethods, PyResult, Python};
use pyo3::pyfunction;
use pyo3::types::{PyDict, PyTuple};
use sqlbuild_host::compile_context::main::record_compile_timing::record_compile_timing;
use sqlbuild_python_base::boundary::models::KeywordArguments;
use sqlbuild_python_project::compiled_project::models::DiagnosticSettings;
use sqlbuild_python_project::compiled_project::models::NativeCompiledProject;

/// Attach every SQL test case, scenario and audit, then report every resource diagnostic.
#[pyfunction]
#[pyo3(signature = (**options))]
pub fn attach_checks<'py>(
    py: Python<'py>,
    options: Option<Bound<'py, PyDict>>,
) -> PyResult<Bound<'py, PyTuple>> {
    let arguments: KeywordArguments<'py> = KeywordArguments::read(
        py,
        "attach_checks",
        options,
        (
            &[
                "discovered_inputs",
                "native_project",
                "settings",
                "scan_cache",
                "source_inputs",
            ],
            &[],
        ),
    )?;
    let discovered_inputs: &Bound<'py, PyAny> = &arguments.value("discovered_inputs")?;
    let native_project: &Bound<'py, NativeCompiledProject> =
        &arguments.value("native_project")?.cast_into()?;
    let (effective_settings, value_renderer, syntax): (
        Bound<'py, PyAny>,
        Bound<'py, PyAny>,
        Bound<'py, PyAny>,
    ) = arguments.value("settings")?.extract()?;
    let scan_cache: &Bound<'py, PyAny> = &arguments.value("scan_cache")?;
    let source_inputs: &Bound<'py, PyAny> = &arguments.value("source_inputs")?;
    let (tests, scenarios) = record_compile_timing(TEST_TIMING_KEY, || -> PyResult<_> {
        let attached = attach_sql_tests(
            native_project,
            items(discovered_inputs, "test_files")?
                .into_iter()
                .map(test_file_row)
                .collect::<PyResult<_>>()?,
            items(discovered_inputs, "scenario_files")?
                .into_iter()
                .map(scenario_row)
                .collect::<PyResult<_>>()?,
            sql_test_settings(py, (&value_renderer, &syntax, scan_cache))?.extract()?,
        )?;
        scan_cache.call_method0("save")?;
        Ok(attached)
    })?;
    let (audits, audit_diagnostics) = attach_audits(
        native_project,
        items(discovered_inputs, "audit_files")?
            .into_iter()
            .map(audit_file_row)
            .collect::<PyResult<_>>()?,
        audit_settings(py, &effective_settings)?.extract()?,
    )?;
    let settings: DiagnosticSettings =
        diagnostic_settings(discovered_inputs, source_inputs)?.extract()?;
    let (orphans, descriptions, cursors) =
        resource_diagnostics(&native_project.borrow(), py, &settings)?;
    let diagnostics: Vec<Bound<'py, PyAny>> = audit_diagnostics
        .into_iter()
        .chain(orphans)
        .chain(descriptions)
        .chain(cursors)
        .collect();
    PyTuple::new(
        py,
        [
            PyTuple::new(py, tests)?.into_any(),
            PyTuple::new(py, scenarios)?.into_any(),
            PyTuple::new(py, audits)?.into_any(),
            PyTuple::new(py, diagnostics)?.into_any(),
        ],
    )
}
