//! `EventExporterCommandScope`: typed sink binding, its provider session and the active command scope.

use crate::event_exporting::models::EventExporterCommandScope;
use crate::event_exporting::models::EventExporterScopeContext;
use std::collections::HashMap;
use std::sync::Mutex;

use pyo3::prelude::{
    Bound, Py, PyAny, PyAnyMethods, PyModule, PyModuleMethods, PyResult, Python, pyclass,
    pyfunction, pymethods, wrap_pyfunction,
};
use pyo3::sync::PyOnceLock;
use pyo3::types::{PyBool, PyDict, PyDictMethods, PyTuple};
use sqlbuild_host::entries::models::ExecutorHost;

use crate::boundary::main::call_with::call_with;
use crate::boundary::main::context_variable::context_variable;
use crate::event_exporting::constants::SCOPE_CONTEXT_VARIABLE;
use crate::event_exporting::models::BoundEventExporter;
use crate::native_host::main::host_entry::host_entry;
use crate::native_host::main::write_console::write_console;
use crate::orchestration_reads::main::provider_state::provider_state;
use crate::orchestration_reads::models::ProviderState;
use crate::records::main::fields::field::field as native_field;
use crate::records::main::fields::mapping_get::mapping_get;

pub(crate) static CURRENT_SCOPE: PyOnceLock<Py<PyAny>> = PyOnceLock::new();

/// Discovered extensions and the provider session one command bound.
#[derive(Debug)]
pub(crate) struct ScopeState {
    pub(crate) configured: bool,
    pub(crate) provider_close_registered: bool,
    pub(crate) provider_session: Option<Py<PyAny>>,
    pub(crate) project_dir: Option<Py<PyAny>>,
    pub(crate) providers: Option<Py<PyAny>>,
    pub(crate) event_exporters: Option<Py<PyAny>>,
    pub(crate) command_output_sinks: Option<Py<PyAny>>,
}

/// Releases and force-closes a provider session on the closing thread once sink delivery is idle.
#[pyclass(module = "sqlbuild._native", frozen)]
pub(crate) struct ProviderSessionCloser {
    pub(crate) session: Py<PyAny>,
}

pub(crate) fn empty_tuple(py: Python<'_>) -> Py<PyAny> {
    PyTuple::empty(py).into_any().unbind()
}

pub(crate) fn tuple_or_empty(py: Python<'_>, value: Option<&Py<PyAny>>) -> Py<PyAny> {
    value.map_or_else(|| empty_tuple(py), |value| value.clone_ref(py))
}

pub(crate) fn optional(py: Python<'_>, value: Option<&Py<PyAny>>) -> Py<PyAny> {
    value.map_or_else(|| py.None(), |value| value.clone_ref(py))
}

/// `{usage.parameter_name: session.get(usage.provider_name)}` for one declaration.
fn provider_arguments<'py>(
    session: &Bound<'py, PyAny>,
    declaration: &Bound<'py, PyAny>,
) -> PyResult<Bound<'py, PyDict>> {
    let arguments: Bound<'py, PyDict> = PyDict::new(session.py());
    for usage in native_field(declaration, "provider_usages")?.try_iter()? {
        let usage: Bound<'py, PyAny> = usage?;
        arguments.set_item(
            native_field(&usage, "parameter_name")?,
            mapping_get(session, native_field(&usage, "provider_name")?)?,
        )?;
    }
    Ok(arguments)
}

/// Each discovered provider whose required secret settings are unset, with their variables.
pub(crate) fn providers_missing_secrets(
    providers: &Bound<'_, PyAny>,
) -> PyResult<HashMap<String, Vec<String>>> {
    let mut missing: HashMap<String, Vec<String>> = HashMap::new();
    for provider in providers.try_iter()? {
        let provider: Bound<'_, PyAny> = provider?;
        if let ProviderState::MissingSecrets(variables) =
            provider_state(&native_field(&provider, "provider_class")?)?
        {
            missing.insert(native_field(&provider, "name")?.extract()?, variables);
        }
    }
    Ok(missing)
}

/// Whether `declaration` can bind; when a provider it takes is missing secrets, a notice says it is left unbound.
fn bindable(
    declaration: &Bound<'_, PyAny>,
    missing: &HashMap<String, Vec<String>>,
) -> PyResult<bool> {
    for usage in native_field(declaration, "provider_usages")?.try_iter()? {
        let provider: String = native_field(&usage?, "provider_name")?.extract()?;
        if let Some(variables) = missing.get(&provider) {
            write_console(
                declaration.py(),
                "stderr",
                &format!(
                    "sink '{}' is not bound: provider '{provider}' is missing {}\n",
                    native_field(declaration, "name")?.str()?,
                    variables.join(", ")
                ),
            )?;
            return Ok(false);
        }
    }
    Ok(true)
}

/// Bound lifecycle exporters and command-output sinks over `session`, except those using `missing` providers.
pub(crate) fn bindings<'py>(
    session: &Bound<'py, PyAny>,
    (event_exporters, command_output_sinks): (&Bound<'py, PyAny>, &Bound<'py, PyAny>),
    missing: &HashMap<String, Vec<String>>,
) -> PyResult<(Bound<'py, PyTuple>, Bound<'py, PyTuple>)> {
    let py: Python<'py> = session.py();
    let mut exporters: Vec<Bound<'py, PyAny>> = Vec::new();
    for exporter in event_exporters.try_iter()? {
        let exporter: Bound<'py, PyAny> = exporter?;
        if !bindable(&exporter, missing)? {
            continue;
        }
        let arguments: Bound<'py, PyDict> = provider_arguments(session, &exporter)?;
        exporters.push(
            Bound::new(
                py,
                BoundEventExporter::create(
                    native_field(&exporter, "name")?.extract()?,
                    (
                        native_field(&exporter, "function")?.unbind(),
                        arguments.into_any().unbind(),
                    ),
                    (
                        Some(&native_field(&exporter, "event_kinds")?),
                        Some(&native_field(&exporter, "min_severity")?),
                    ),
                )?,
            )?
            .into_any(),
        );
    }
    let sink_class: Bound<'py, PyAny> = host_entry(py, ExecutorHost::BoundCommandOutputSink)?;
    let mut sinks: Vec<Bound<'py, PyAny>> = Vec::new();
    for sink in command_output_sinks.try_iter()? {
        let sink: Bound<'py, PyAny> = sink?;
        if !bindable(&sink, missing)? {
            continue;
        }
        let arguments: Bound<'py, PyDict> = provider_arguments(session, &sink)?;
        sinks.push(call_with(
            &sink_class,
            &[
                ("name", native_field(&sink, "name")?),
                ("function", native_field(&sink, "function")?),
                ("provider_arguments", arguments.into_any()),
                ("streams", native_field(&sink, "streams")?),
            ],
        )?);
    }
    Ok((PyTuple::new(py, exporters)?, PyTuple::new(py, sinks)?))
}

#[pymethods]
impl ProviderSessionCloser {
    pub(crate) fn __call__(&self, py: Python<'_>) -> PyResult<()> {
        let session: &Bound<'_, PyAny> = self.session.bind(py);
        let released = session.call_method0("release");
        call_with(
            &native_field(session, "close")?,
            &[("force", PyBool::new(py, true).to_owned().into_any())],
        )?;
        released?;
        Ok(())
    }
}

/// Install one command-owned exporter scope.
#[pyfunction]
pub(crate) fn event_exporter_command_scope(scope: Py<PyAny>) -> EventExporterScopeContext {
    EventExporterScopeContext {
        scope,
        token: Mutex::new(None),
    }
}

/// The active command-owned exporter scope, or `None`.
pub(crate) fn current_scope(py: Python<'_>) -> PyResult<Bound<'_, PyAny>> {
    context_variable(py, &CURRENT_SCOPE, SCOPE_CONTEXT_VARIABLE)?.call_method0("get")
}

/// Return the active command-owned exporter scope, if any.
#[pyfunction]
fn current_event_exporter_command_scope(py: Python<'_>) -> PyResult<Bound<'_, PyAny>> {
    current_scope(py)
}

/// Configure the active exporter scope from validated discovery inputs.
pub(crate) fn configure_discovered_event_exporters(discovered: &Bound<'_, PyAny>) -> PyResult<()> {
    let scope: Bound<'_, PyAny> = current_scope(discovered.py())?;
    if !scope.is_none() {
        scope.call_method1("configure", (discovered,))?;
    }
    Ok(())
}

/// Return startup-discovered extensions for the active project root.
#[pyfunction]
#[pyo3(signature = (*, project_dir))]
fn cached_event_exporter_extensions<'py>(
    py: Python<'py>,
    project_dir: &Bound<'py, PyAny>,
) -> PyResult<Bound<'py, PyAny>> {
    let scope: Bound<'py, PyAny> = current_scope(py)?;
    if scope.is_none() {
        return Ok(scope);
    }
    call_with(
        &native_field(&scope, "cached_extensions")?,
        &[("project_dir", project_dir.clone())],
    )
}

pub(crate) fn register(module: &Bound<'_, PyModule>) -> PyResult<()> {
    module.add_class::<EventExporterCommandScope>()?;
    module.add_class::<ProviderSessionCloser>()?;
    module.add_class::<EventExporterScopeContext>()?;
    module.add_function(wrap_pyfunction!(event_exporter_command_scope, module)?)?;
    module.add_function(wrap_pyfunction!(
        current_event_exporter_command_scope,
        module
    )?)?;
    module.add_function(wrap_pyfunction!(cached_event_exporter_extensions, module)?)
}
