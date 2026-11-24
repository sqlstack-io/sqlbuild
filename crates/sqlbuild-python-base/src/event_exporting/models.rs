//! Public records of event exporting.

use crate::boundary::main::call_with::call_with;
use crate::boundary::main::context_variable::context_variable;
use crate::boundary::models::KeywordArguments;
use crate::event_exporting::_helpers::scope::CURRENT_SCOPE;
use crate::event_exporting::_helpers::scope::ProviderSessionCloser;
use crate::event_exporting::_helpers::scope::ScopeState;
use crate::event_exporting::_helpers::scope::bindings;
use crate::event_exporting::_helpers::scope::optional;
use crate::event_exporting::_helpers::scope::providers_missing_secrets;
use crate::event_exporting::_helpers::scope::teardown_failures;
use crate::event_exporting::_helpers::scope::tuple_or_empty;
use crate::event_exporting::_helpers::values::accounting_repr;
use crate::event_exporting::_helpers::values::bound_exporter_arguments;
use crate::event_exporting::_helpers::values::counts_repr;
use crate::event_exporting::_helpers::values::python_repr;
use crate::event_exporting::_helpers::values::severity_from;
use crate::event_exporting::_helpers::values::severity_object;
use crate::event_exporting::_helpers::values::text_repr;
use crate::event_exporting::constants::SCOPE_CONTEXT_VARIABLE;
use crate::native_host::main::host_entry::host_entry;
use crate::records::main::fields::field::field as native_field;
use pyo3::exceptions::PyRuntimeError;
use pyo3::prelude::Bound;
use pyo3::prelude::Py;
use pyo3::prelude::PyAny;
use pyo3::prelude::PyAnyMethods;
use pyo3::prelude::PyResult;
use pyo3::prelude::Python;
use pyo3::prelude::pyclass;
use pyo3::prelude::pymethods;
use pyo3::types::PyBool;
use pyo3::types::PyDict;
use pyo3::types::PyFrozenSet;
use pyo3::types::PyTuple;
use sqlbuild_core::event_export::constants::EVENT_EXPORT_KINDS;
use sqlbuild_core::event_export::models::ExportSeverity;
use sqlbuild_core::event_export::models::ExportSnapshot;
use sqlbuild_core::event_export::models::ExporterCounts;
use sqlbuild_core::event_export::models::ExporterFilter;
use sqlbuild_host::entries::models::UserCodeHost;
use std::collections::HashMap;
use std::sync::Mutex;
use std::sync::MutexGuard;

/// Frozen attempt counters for one exporter or their aggregate.
#[pyclass(module = "sqlbuild._native", frozen, eq, hash)]
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct EventExporterCounts {
    pub counts: ExporterCounts,
}

/// Frozen accounting snapshot for one named exporter.
#[pyclass(module = "sqlbuild._native", frozen, eq, hash)]
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct EventExporterAccounting {
    pub(crate) exporter_name: String,
    pub(crate) counts: ExporterCounts,
}

/// Final or point-in-time best-effort delivery accounting.
#[pyclass(module = "sqlbuild._native", frozen, eq, hash)]
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct EventExportSummary {
    pub snapshot: ExportSnapshot,
}

/// Sanitized exporter failure information safe for diagnostics.
#[pyclass(module = "sqlbuild._native", frozen, eq, hash)]
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct EventExporterFailure {
    pub exporter_name: String,
    pub error_type: String,
    pub event_kind: String,
    pub event_severity: ExportSeverity,
}

/// One lifecycle-event sink bound to command-scoped provider instances.
#[pyclass(module = "sqlbuild._native", frozen)]
#[derive(Debug)]
pub struct BoundEventExporter {
    pub name: String,
    pub function: Py<PyAny>,
    pub provider_arguments: Py<PyAny>,
    pub filter: ExporterFilter,
}

#[pymethods]
impl EventExporterCounts {
    #[getter]
    pub fn accepted(&self) -> u64 {
        self.counts.accepted
    }

    #[getter]
    pub fn filtered(&self) -> u64 {
        self.counts.filtered
    }

    #[getter]
    pub fn delivered(&self) -> u64 {
        self.counts.delivered
    }

    #[getter]
    pub fn failed(&self) -> u64 {
        self.counts.failed
    }

    #[getter]
    pub fn dropped(&self) -> u64 {
        self.counts.dropped
    }

    pub(crate) fn __repr__(&self) -> String {
        counts_repr(&self.counts)
    }
}

#[pymethods]
impl EventExporterAccounting {
    #[getter]
    pub fn exporter_name(&self) -> &str {
        &self.exporter_name
    }

    #[getter]
    pub fn counts(&self) -> EventExporterCounts {
        EventExporterCounts {
            counts: self.counts,
        }
    }

    pub(crate) fn __repr__(&self, py: Python<'_>) -> PyResult<String> {
        accounting_repr(py, (&self.exporter_name, &self.counts))
    }
}

#[pymethods]
impl EventExportSummary {
    #[getter]
    pub fn aggregate(&self) -> EventExporterCounts {
        EventExporterCounts {
            counts: self.snapshot.aggregate,
        }
    }

    #[getter]
    pub fn per_exporter<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyTuple>> {
        let items: Vec<EventExporterAccounting> = self
            .snapshot
            .per_exporter
            .iter()
            .map(|(name, counts)| EventExporterAccounting {
                exporter_name: name.clone(),
                counts: *counts,
            })
            .collect();
        PyTuple::new(py, items)
    }

    #[getter]
    pub fn queue_depth(&self) -> usize {
        self.snapshot.queue_depth
    }

    #[getter]
    pub fn queue_capacity(&self) -> usize {
        self.snapshot.queue_capacity
    }

    #[getter]
    pub fn flush_complete(&self) -> bool {
        self.snapshot.flush_complete
    }

    #[getter]
    pub fn accepted(&self) -> u64 {
        self.snapshot.aggregate.accepted
    }

    #[getter]
    pub fn filtered(&self) -> u64 {
        self.snapshot.aggregate.filtered
    }

    #[getter]
    pub fn delivered(&self) -> u64 {
        self.snapshot.aggregate.delivered
    }

    #[getter]
    pub fn failed(&self) -> u64 {
        self.snapshot.aggregate.failed
    }

    #[getter]
    pub fn dropped(&self) -> u64 {
        self.snapshot.aggregate.dropped
    }

    pub(crate) fn __repr__(&self, py: Python<'_>) -> PyResult<String> {
        let per_exporter: Vec<String> = self
            .snapshot
            .per_exporter
            .iter()
            .map(|(name, counts)| accounting_repr(py, (name, counts)))
            .collect::<PyResult<Vec<String>>>()?;
        let per_exporter: String = match per_exporter.as_slice() {
            [single] => format!("({single},)"),
            items => format!("({})", items.join(", ")),
        };
        Ok(format!(
            "EventExportSummary(aggregate={}, per_exporter={per_exporter}, queue_depth={}, \
             queue_capacity={}, flush_complete={})",
            counts_repr(&self.snapshot.aggregate),
            self.snapshot.queue_depth,
            self.snapshot.queue_capacity,
            if self.snapshot.flush_complete {
                "True"
            } else {
                "False"
            }
        ))
    }
}

#[pymethods]
impl EventExporterFailure {
    #[getter]
    pub fn exporter_name(&self) -> &str {
        &self.exporter_name
    }

    #[getter]
    pub fn error_type(&self) -> &str {
        &self.error_type
    }

    #[getter]
    pub fn event_kind(&self) -> &str {
        &self.event_kind
    }

    #[getter]
    pub fn event_severity<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        severity_object(py, self.event_severity)
    }

    pub(crate) fn __repr__(&self, py: Python<'_>) -> PyResult<String> {
        Ok(format!(
            "EventExporterFailure(exporter_name={}, error_type={}, event_kind={}, event_severity={})",
            text_repr(py, &self.exporter_name)?,
            text_repr(py, &self.error_type)?,
            text_repr(py, &self.event_kind)?,
            python_repr(&severity_object(py, self.event_severity)?)?
        ))
    }
}

#[pymethods]
impl BoundEventExporter {
    #[new]
    #[pyo3(signature = (*arguments, **options))]
    pub(crate) fn new(
        arguments: &Bound<'_, PyTuple>,
        options: Option<&Bound<'_, PyDict>>,
    ) -> PyResult<Self> {
        let [
            name,
            function,
            provider_arguments,
            event_kinds,
            min_severity,
        ] = bound_exporter_arguments(arguments, options)?;
        let (Some(name), Some(function), Some(provider_arguments)) =
            (name, function, provider_arguments)
        else {
            return Err(pyo3::exceptions::PyTypeError::new_err(
                "BoundEventExporter() missing required arguments",
            ));
        };
        Self::create(
            name.extract()?,
            (function.unbind(), provider_arguments.unbind()),
            (event_kinds.as_ref(), min_severity.as_ref()),
        )
    }

    #[getter]
    pub fn name(&self) -> &str {
        &self.name
    }

    #[getter]
    pub fn function(&self, py: Python<'_>) -> Py<PyAny> {
        self.function.clone_ref(py)
    }

    #[getter]
    pub fn provider_arguments(&self, py: Python<'_>) -> Py<PyAny> {
        self.provider_arguments.clone_ref(py)
    }

    #[getter]
    pub fn event_kinds<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyFrozenSet>> {
        PyFrozenSet::new(py, self.filter.event_kinds.iter())
    }

    #[getter]
    pub fn min_severity<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        severity_object(py, self.filter.min_severity)
    }
}

impl BoundEventExporter {
    /// A sink bound to `provider_arguments`, filtered by `event_kinds` and `min_severity` when given.
    pub fn create(
        name: String,
        (function, provider_arguments): (Py<PyAny>, Py<PyAny>),
        (event_kinds, min_severity): (Option<&Bound<'_, PyAny>>, Option<&Bound<'_, PyAny>>),
    ) -> PyResult<Self> {
        let event_kinds = match event_kinds {
            Some(kinds) => kinds
                .try_iter()?
                .map(|kind| kind.and_then(|kind| kind.extract::<String>()))
                .collect::<PyResult<_>>()?,
            None => EVENT_EXPORT_KINDS
                .iter()
                .map(|kind| (*kind).to_owned())
                .collect(),
        };
        let min_severity: ExportSeverity = match min_severity {
            Some(severity) => severity_from(severity)?,
            None => ExportSeverity::Debug,
        };
        Ok(Self {
            name,
            function,
            provider_arguments,
            filter: ExporterFilter {
                event_kinds,
                min_severity,
            },
        })
    }
}

impl EventExportSummary {
    /// A Python summary of `snapshot`.
    pub fn object(py: Python<'_>, snapshot: ExportSnapshot) -> PyResult<Py<Self>> {
        Py::new(py, Self { snapshot })
    }
}

/// Bind discovered typed sinks and own their command-scoped provider session.
#[pyclass(module = "sqlbuild._native", frozen)]
#[derive(Debug)]
pub struct EventExporterCommandScope {
    pub(crate) dispatcher: Py<PyAny>,
    pub(crate) state: Mutex<ScopeState>,
}

/// Installs one command-owned exporter scope while entered.
#[pyclass(module = "sqlbuild._native", frozen)]
#[derive(Debug)]
pub struct EventExporterScopeContext {
    pub(crate) scope: Py<PyAny>,
    pub(crate) token: Mutex<Option<Py<PyAny>>>,
}

impl EventExporterCommandScope {
    pub(crate) fn locked(&self) -> PyResult<MutexGuard<'_, ScopeState>> {
        self.state
            .lock()
            .map_err(|_| PyRuntimeError::new_err("event exporter scope lock is poisoned"))
    }

    /// Bind discovered sinks over one command-scoped provider session, once.
    pub(crate) fn bind_extensions<'py>(
        &self,
        py: Python<'py>,
        (project_dir, setup_context): (Option<Bound<'py, PyAny>>, Bound<'py, PyAny>),
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
            &host_entry(py, UserCodeHost::BuildProviderSession)?,
            &[
                ("discovered_providers", providers),
                ("setup_context", setup_context),
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

    pub(crate) fn bind_dispatcher(
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
    pub fn new(dispatcher: Py<PyAny>) -> Self {
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
                teardown_failures: Vec::new(),
            }),
        }
    }

    #[getter]
    pub fn dispatcher(&self, py: Python<'_>) -> Py<PyAny> {
        self.dispatcher.clone_ref(py)
    }

    #[getter]
    pub fn provider_session(&self, py: Python<'_>) -> PyResult<Py<PyAny>> {
        Ok(optional(py, self.locked()?.provider_session.as_ref()))
    }

    #[getter]
    pub fn project_dir(&self, py: Python<'_>) -> PyResult<Py<PyAny>> {
        Ok(optional(py, self.locked()?.project_dir.as_ref()))
    }

    #[getter]
    pub fn providers(&self, py: Python<'_>) -> PyResult<Py<PyAny>> {
        Ok(tuple_or_empty(py, self.locked()?.providers.as_ref()))
    }

    #[getter]
    pub fn event_exporters(&self, py: Python<'_>) -> PyResult<Py<PyAny>> {
        Ok(tuple_or_empty(py, self.locked()?.event_exporters.as_ref()))
    }

    #[getter]
    pub fn command_output_sinks(&self, py: Python<'_>) -> PyResult<Py<PyAny>> {
        Ok(tuple_or_empty(
            py,
            self.locked()?.command_output_sinks.as_ref(),
        ))
    }

    /// Bind once after normal project discovery has validated declarations.
    pub(crate) fn configure(
        &self,
        py: Python<'_>,
        discovered_inputs: &Bound<'_, PyAny>,
    ) -> PyResult<()> {
        self.bind_extensions(
            py,
            (None, py.None().into_bound(py)),
            (
                native_field(discovered_inputs, "providers")?,
                native_field(discovered_inputs, "event_exporters")?,
                native_field(discovered_inputs, "command_output_sinks")?,
            ),
        )
    }

    /// Bind declarations discovered at the outer command startup boundary.
    #[pyo3(signature = (**options))]
    pub(crate) fn configure_extensions(
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
                &["project_dir", "command_output_sinks", "setup_context"],
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
            (
                (!project_dir.is_none()).then_some(project_dir),
                options.value("setup_context")?,
            ),
            (
                options.value("providers")?,
                options.value("event_exporters")?,
                command_output_sinks,
            ),
        )
    }

    /// Return startup declarations only for the same project root.
    #[pyo3(signature = (*, project_dir))]
    pub(crate) fn cached_extensions(
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
    pub(crate) fn close(&self, py: Python<'_>) -> PyResult<Py<PyAny>> {
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
        if let Err(error) = self
            .dispatcher
            .bind(py)
            .call_method1("finalize_when_idle", (closer,))
        {
            self.locked()?.teardown_failures = teardown_failures(py, &error);
        }
        Ok(summary)
    }

    /// Each sink provider whose teardown raised when the scope closed, with its error type.
    pub fn teardown_failures(&self) -> PyResult<Vec<String>> {
        Ok(self.locked()?.teardown_failures.clone())
    }

    /// Reuse bound sink functions and provider instances for output records.
    pub(crate) fn export_output(&self, py: Python<'_>, records: &Bound<'_, PyAny>) -> PyResult<()> {
        self.dispatcher
            .bind(py)
            .call_method1("export_output", (records,))?;
        Ok(())
    }
}

#[pymethods]
impl EventExporterScopeContext {
    pub(crate) fn __enter__(&self, py: Python<'_>) -> PyResult<Py<PyAny>> {
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
    pub(crate) fn __exit__(
        &self,
        py: Python<'_>,
        _exc_info: &Bound<'_, PyTuple>,
    ) -> PyResult<bool> {
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
