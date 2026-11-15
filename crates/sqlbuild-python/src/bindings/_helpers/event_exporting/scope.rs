//! `EventExporterCommandScope`: typed sink binding, its provider session and the active command scope.

use std::collections::HashMap;
use std::sync::{Mutex, MutexGuard};

use pyo3::exceptions::PyRuntimeError;
use pyo3::prelude::{
    Bound, Py, PyAny, PyAnyMethods, PyModule, PyModuleMethods, PyResult, Python, pyclass,
    pyfunction, pymethods, wrap_pyfunction,
};
use pyo3::sync::PyOnceLock;
use pyo3::types::{PyBool, PyDict, PyDictMethods, PyTuple};
use sqlbuild_host::entries::models::{ExecutorHost, UserCodeHost};

use crate::bindings::_helpers::boundary::python_calls::{
    KeywordArguments, call_with, context_variable,
};
use crate::bindings::_helpers::event_exporting::constants::SCOPE_CONTEXT_VARIABLE;
use crate::bindings::_helpers::event_exporting::models::BoundEventExporter;
use crate::bindings::_helpers::native_host::host;
use crate::bindings::_helpers::native_host::provider_settings::{ProviderState, provider_state};
use crate::bindings::_helpers::records::fields::field as native_field;
use crate::bindings::_helpers::records::fields::mapping_get;

static CURRENT_SCOPE: PyOnceLock<Py<PyAny>> = PyOnceLock::new();

/// Discovered extensions and the provider session one command bound.
struct ScopeState {
    configured: bool,
    provider_close_registered: bool,
    provider_session: Option<Py<PyAny>>,
    project_dir: Option<Py<PyAny>>,
    providers: Option<Py<PyAny>>,
    event_exporters: Option<Py<PyAny>>,
    command_output_sinks: Option<Py<PyAny>>,
}

/// Bind discovered typed sinks and own their command-scoped provider session.
#[pyclass(module = "sqlbuild._native", frozen)]
pub(crate) struct EventExporterCommandScope {
    dispatcher: Py<PyAny>,
    state: Mutex<ScopeState>,
}

/// Releases and force-closes a provider session on the closing thread once sink delivery is idle.
#[pyclass(module = "sqlbuild._native", frozen)]
pub(crate) struct ProviderSessionCloser {
    session: Py<PyAny>,
}

/// Installs one command-owned exporter scope while entered.
#[pyclass(module = "sqlbuild._native", frozen)]
pub(crate) struct EventExporterScopeContext {
    scope: Py<PyAny>,
    token: Mutex<Option<Py<PyAny>>>,
}

fn empty_tuple(py: Python<'_>) -> Py<PyAny> {
    PyTuple::empty(py).into_any().unbind()
}

fn optional(py: Python<'_>, value: Option<&Py<PyAny>>) -> Py<PyAny> {
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
fn providers_missing_secrets(
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
            host::write_console(
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
fn bindings<'py>(
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
    let sink_class: Bound<'py, PyAny> = host::entry(py, ExecutorHost::BoundCommandOutputSink)?;
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

impl EventExporterCommandScope {
    fn locked(&self) -> PyResult<MutexGuard<'_, ScopeState>> {
        self.state
            .lock()
            .map_err(|_| PyRuntimeError::new_err("event exporter scope lock is poisoned"))
    }

    /// Bind discovered sinks over one command-scoped provider session, once.
    fn bind_extensions<'py>(
        &self,
        py: Python<'py>,
        project_dir: Option<Bound<'py, PyAny>>,
        (providers, event_exporters, command_output_sinks): (
            Bound<'py, PyAny>,
            Bound<'py, PyAny>,
            Bound<'py, PyAny>,
        ),
    ) -> PyResult<()> {
        {
            let mut state = self.locked()?;
            if state.configured {
                return Ok(());
            }
            state.configured = true;
        }
        let project_dir: Option<Bound<'_, PyAny>> = project_dir
            .map(|project_dir| project_dir.call_method0("resolve"))
            .transpose()?;
        {
            let mut state = self.locked()?;
            state.project_dir = project_dir.map(Bound::unbind);
            state.providers = Some(providers.clone().unbind());
            state.event_exporters = Some(event_exporters.clone().unbind());
            state.command_output_sinks = Some(command_output_sinks.clone().unbind());
        }
        if !event_exporters.is_truthy()? && !command_output_sinks.is_truthy()? {
            let empty = PyTuple::empty(py).into_any();
            return self.bind_dispatcher(py, (empty.clone(), empty));
        }
        let missing: HashMap<String, Vec<String>> = providers_missing_secrets(&providers)?;
        let session: Bound<'_, PyAny> = call_with(
            &host::entry(py, UserCodeHost::BuildProviderSession)?,
            &[
                ("discovered_providers", providers),
                ("allow_shared", PyBool::new(py, false).to_owned().into_any()),
            ],
        )?;
        session.call_method0("hold_open")?;
        let (exporters, sinks) = match bindings(
            &session,
            (&event_exporters, &command_output_sinks),
            &missing,
        ) {
            Ok(bound) => bound,
            Err(error) => {
                session.call_method0("release")?;
                call_with(
                    &native_field(&session, "close")?,
                    &[("force", PyBool::new(py, true).to_owned().into_any())],
                )?;
                return Err(error);
            }
        };
        self.locked()?.provider_session = Some(session.unbind());
        self.bind_dispatcher(py, (exporters.into_any(), sinks.into_any()))
    }

    fn bind_dispatcher(
        &self,
        py: Python<'_>,
        (exporters, sinks): (Bound<'_, PyAny>, Bound<'_, PyAny>),
    ) -> PyResult<()> {
        let dispatcher: &Bound<'_, PyAny> = self.dispatcher.bind(py);
        dispatcher.call_method1("bind", (exporters,))?;
        dispatcher.call_method1("bind_command_output_sinks", (sinks,))?;
        Ok(())
    }
}

#[pymethods]
impl EventExporterCommandScope {
    #[new]
    #[pyo3(signature = (*, dispatcher))]
    pub(crate) fn new(dispatcher: Py<PyAny>) -> Self {
        Self {
            dispatcher,
            state: Mutex::new(ScopeState {
                configured: false,
                provider_close_registered: false,
                provider_session: None,
                project_dir: None,
                providers: None,
                event_exporters: None,
                command_output_sinks: None,
            }),
        }
    }

    #[getter]
    pub(crate) fn dispatcher(&self, py: Python<'_>) -> Py<PyAny> {
        self.dispatcher.clone_ref(py)
    }

    #[getter]
    pub(crate) fn provider_session(&self, py: Python<'_>) -> PyResult<Py<PyAny>> {
        Ok(optional(py, self.locked()?.provider_session.as_ref()))
    }

    #[getter]
    pub(crate) fn project_dir(&self, py: Python<'_>) -> PyResult<Py<PyAny>> {
        Ok(optional(py, self.locked()?.project_dir.as_ref()))
    }

    #[getter]
    pub(crate) fn providers(&self, py: Python<'_>) -> PyResult<Py<PyAny>> {
        let state = self.locked()?;
        Ok(state
            .providers
            .as_ref()
            .map_or_else(|| empty_tuple(py), |value| value.clone_ref(py)))
    }

    #[getter]
    pub(crate) fn event_exporters(&self, py: Python<'_>) -> PyResult<Py<PyAny>> {
        let state = self.locked()?;
        Ok(state
            .event_exporters
            .as_ref()
            .map_or_else(|| empty_tuple(py), |value| value.clone_ref(py)))
    }

    #[getter]
    pub(crate) fn command_output_sinks(&self, py: Python<'_>) -> PyResult<Py<PyAny>> {
        let state = self.locked()?;
        Ok(state
            .command_output_sinks
            .as_ref()
            .map_or_else(|| empty_tuple(py), |value| value.clone_ref(py)))
    }

    /// Bind once after normal project discovery has validated declarations.
    fn configure(&self, py: Python<'_>, discovered_inputs: &Bound<'_, PyAny>) -> PyResult<()> {
        self.bind_extensions(
            py,
            None,
            (
                native_field(discovered_inputs, "providers")?,
                native_field(discovered_inputs, "event_exporters")?,
                native_field(discovered_inputs, "command_output_sinks")?,
            ),
        )
    }

    /// Bind declarations discovered at the outer command startup boundary.
    #[pyo3(signature = (**options))]
    fn configure_extensions(
        &self,
        py: Python<'_>,
        options: Option<Bound<'_, PyDict>>,
    ) -> PyResult<()> {
        let options: KeywordArguments<'_> = KeywordArguments::read(
            py,
            "configure_extensions",
            options,
            (
                &["providers", "event_exporters"],
                &["project_dir", "command_output_sinks"],
            ),
        )?;
        let project_dir: Bound<'_, PyAny> = options.value("project_dir")?;
        let command_output_sinks: Bound<'_, PyAny> = options.value("command_output_sinks")?;
        let command_output_sinks: Bound<'_, PyAny> = if command_output_sinks.is_none() {
            PyTuple::empty(py).into_any()
        } else {
            command_output_sinks
        };
        self.bind_extensions(
            py,
            (!project_dir.is_none()).then_some(project_dir),
            (
                options.value("providers")?,
                options.value("event_exporters")?,
                command_output_sinks,
            ),
        )
    }

    /// Return startup declarations only for the same project root.
    #[pyo3(signature = (*, project_dir))]
    fn cached_extensions(
        &self,
        py: Python<'_>,
        project_dir: &Bound<'_, PyAny>,
    ) -> PyResult<Py<PyAny>> {
        let (own, providers, exporters, sinks) = {
            let state = self.locked()?;
            let Some(own) = state.project_dir.as_ref() else {
                return Ok(py.None());
            };
            (
                own.clone_ref(py),
                optional(py, state.providers.as_ref()),
                optional(py, state.event_exporters.as_ref()),
                optional(py, state.command_output_sinks.as_ref()),
            )
        };
        if project_dir.call_method0("resolve")?.ne(own.bind(py))? {
            return Ok(py.None());
        }
        Ok(PyTuple::new(py, [providers, exporters, sinks])?
            .into_any()
            .unbind())
    }

    /// Flush sink delivery before reverse-order provider teardown.
    fn close(&self, py: Python<'_>) -> PyResult<Py<PyAny>> {
        let summary: Py<PyAny> = self.dispatcher.bind(py).call_method0("shutdown")?.unbind();
        let session: Py<PyAny> = {
            let mut state = self.locked()?;
            let Some(session) = state
                .provider_session
                .as_ref()
                .map(|session| session.clone_ref(py))
            else {
                return Ok(summary);
            };
            if state.provider_close_registered {
                return Ok(summary);
            }
            state.provider_close_registered = true;
            session
        };
        let closer: Py<ProviderSessionCloser> = Py::new(py, ProviderSessionCloser { session })?;
        self.dispatcher
            .bind(py)
            .call_method1("finalize_when_idle", (closer,))?;
        Ok(summary)
    }

    /// Reuse bound sink functions and provider instances for output records.
    fn export_output(&self, py: Python<'_>, records: &Bound<'_, PyAny>) -> PyResult<()> {
        self.dispatcher
            .bind(py)
            .call_method1("export_output", (records,))?;
        Ok(())
    }
}

#[pymethods]
impl ProviderSessionCloser {
    fn __call__(&self, py: Python<'_>) -> PyResult<()> {
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

#[pymethods]
impl EventExporterScopeContext {
    fn __enter__(&self, py: Python<'_>) -> PyResult<Py<PyAny>> {
        let token = context_variable(py, &CURRENT_SCOPE, SCOPE_CONTEXT_VARIABLE)?
            .call_method1("set", (self.scope.bind(py),))?;
        *self
            .token
            .lock()
            .map_err(|_| PyRuntimeError::new_err("event exporter scope lock is poisoned"))? =
            Some(token.unbind());
        Ok(self.scope.clone_ref(py))
    }

    #[pyo3(signature = (*_exc_info))]
    fn __exit__(&self, py: Python<'_>, _exc_info: &Bound<'_, PyTuple>) -> PyResult<bool> {
        let token = self
            .token
            .lock()
            .map_err(|_| PyRuntimeError::new_err("event exporter scope lock is poisoned"))?
            .take();
        if let Some(token) = token {
            context_variable(py, &CURRENT_SCOPE, SCOPE_CONTEXT_VARIABLE)?
                .call_method1("reset", (token,))?;
        }
        Ok(false)
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
