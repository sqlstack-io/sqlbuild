//! The SQL expansion context compiler-integrated lint expands authored bodies with.

use pyo3::prelude::{Bound, PyAny, PyAnyMethods, PyErr, PyResult, Python};
use pyo3::pyfunction;
use pyo3::types::{PyBool, PyDict};
use sqlbuild_host::entries::models::AdapterHost;

use crate::compile_attachment::main::present::present;
use crate::compile_attachment::main::sql_expansion_inputs::sql_expansion_inputs;
use crate::compile_attachment::models::ExpansionSources;
use crate::compile_attachment::models::ExpansionVars;
use crate::compile_driver::main::stages::discover_project_inputs::discover_project_inputs;
use sqlbuild_python_base::boundary::main::call_with::call_with;
use sqlbuild_python_base::boundary::main::keywords::keywords;
use sqlbuild_python_base::boundary::models::KeywordArguments;
use sqlbuild_python_base::native_host::main::host_entry::host_entry;
use sqlbuild_python_base::records::main::construct::error_class::error_class;
use sqlbuild_python_base::records::main::fields::field::field as native_field;
use sqlbuild_python_base::records::types::ErrorId;
use sqlbuild_python_front::contracts::main::resolve_effective_adapter_name::resolve_effective_adapter_name;
use sqlbuild_python_front::render::models::SqlExpansionInputs;

const COMPILE_FIRST_MESSAGE: &str = "compiler-integrated Rules check the SQL your project \
    actually produces, so the project must compile first";

/// Build the lint expansion context, reporting compile failures as `ProjectCompileError`.
#[pyfunction]
#[pyo3(signature = (**options))]
pub(crate) fn build_lint_expansion_context<'py>(
    py: Python<'py>,
    options: Option<Bound<'py, PyDict>>,
) -> PyResult<Bound<'py, PyAny>> {
    let arguments: KeywordArguments<'py> = KeywordArguments::read(
        py,
        "build_lint_expansion_context",
        options,
        (
            &["project_dir"],
            &[
                "value_renderer",
                "discovered_inputs",
                "declaration_scope",
                "static_declaration_scope",
            ],
        ),
    )?;
    let renderer: Bound<'py, PyAny> = arguments.value("value_renderer")?;
    lint_expansion_inputs(LintExpansionSources {
        vars: ExpansionVars::ProjectDefault(None),
        project_dir: arguments.value("project_dir")?,
        value_renderer: renderer.is_truthy()?.then_some(renderer),
        discovered_inputs: present(arguments.value("discovered_inputs")?),
        declaration_scope: present(arguments.value("declaration_scope")?),
        static_declaration_scope: present(arguments.value("static_declaration_scope")?),
    })?
    .record(py)
}

/// Where lint's expansion inputs come from; a missing renderer is the project's adapter.
pub(crate) struct LintExpansionSources<'py> {
    pub(crate) project_dir: Bound<'py, PyAny>,
    pub(crate) value_renderer: Option<Bound<'py, PyAny>>,
    /// The target and vars lint expands with.
    pub(crate) vars: ExpansionVars<'py>,
    pub(crate) discovered_inputs: Option<Bound<'py, PyAny>>,
    pub(crate) declaration_scope: Option<Bound<'py, PyAny>>,
    pub(crate) static_declaration_scope: Option<Bound<'py, PyAny>>,
}

/// Lint's expansion inputs, reporting compile failures as `ProjectCompileError`.
pub(crate) fn lint_expansion_inputs(
    sources: LintExpansionSources<'_>,
) -> PyResult<SqlExpansionInputs> {
    let py: Python<'_> = sources.project_dir.py();
    expansion_inputs(sources).or_else(|error| Err(compile_first_error(py, error)?))
}

fn expansion_inputs(sources: LintExpansionSources<'_>) -> PyResult<SqlExpansionInputs> {
    let py: Python<'_> = sources.project_dir.py();
    let project_dir: Bound<'_, PyAny> = sources.project_dir;
    let discovered: Bound<'_, PyAny> = match sources.discovered_inputs {
        Some(discovered) => discovered,
        None => {
            let no: Bound<'_, PyAny> = PyBool::new(py, false).to_owned().into_any();
            discover_project_inputs(
                py,
                Some(keywords(
                    py,
                    &[
                        ("project_dir", project_dir.clone()),
                        ("sql_analysis_enabled_override", no.clone()),
                        ("extract_output_column_locations", no),
                    ],
                )?),
            )?
        }
    };
    let renderer: Bound<'_, PyAny> = match sources.value_renderer {
        Some(renderer) => renderer,
        None => {
            let adapter_name: Bound<'_, PyAny> = resolve_effective_adapter_name(
                &native_field(&discovered, "project_config")?,
                &native_field(&discovered, "local_config")?,
            )?;
            call_with(
                &host_entry(py, AdapterHost::ResolveAdapter)?,
                &[
                    ("adapter_name", adapter_name),
                    ("project_dir", project_dir.clone()),
                ],
            )?
        }
    };
    sql_expansion_inputs(ExpansionSources {
        project_dir,
        value_renderer: renderer,
        vars: sources.vars,
        discovered_inputs: Some(discovered),
        declaration_scope: sources.declaration_scope,
        static_declaration_scope: sources.static_declaration_scope,
    })
}

/// Adapter, compile and discovery errors as `ProjectCompileError`; any other error unchanged.
fn compile_first_error(py: Python<'_>, error: PyErr) -> PyResult<PyErr> {
    let value: Bound<'_, PyAny> = error.value(py).clone().into_any();
    let reported: bool = value.is_instance(&host_entry(py, AdapterHost::AdapterUserError)?)?
        || value.is_instance(&error_class(py, ErrorId::CompileInput)?)?
        || value.is_instance(&error_class(py, ErrorId::Discovery)?)?;
    if !reported {
        return Ok(error);
    }
    let message: String = format!("{COMPILE_FIRST_MESSAGE}: {}", value.str()?);
    let raised: PyErr =
        PyErr::from_value(error_class(py, ErrorId::ProjectCompile)?.call1((message,))?);
    raised.set_cause(py, Some(error));
    Ok(raised)
}
