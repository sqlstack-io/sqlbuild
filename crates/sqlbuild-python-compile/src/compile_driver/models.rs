//! Public records of compile driver.

use crate::compile_driver::_helpers::setup::model_free_locations::model_free_locations;
use pyo3::IntoPyObjectExt;
use pyo3::prelude::Bound;
use pyo3::prelude::Py;
use pyo3::prelude::PyAny;
use pyo3::prelude::PyResult;
use pyo3::prelude::Python;
use pyo3::types::PyBool;
use sqlbuild_analysis::assembly::target_checks::constants::PRESERVE_TARGET_VALUE;
use sqlbuild_analysis::job_settings::models::NodeJobSettings;
use sqlbuild_python_base::boundary::models::KeywordArguments;
use sqlbuild_python_base::records::main::construct::new_record_with::new_record_with;
use sqlbuild_python_base::records::types::RecordId;
use std::collections::BTreeMap;

/// Semantic analysis cache outcome counts for one invocation.
#[derive(Debug)]
pub struct AnalysisCacheCounts {
    pub batch_hits: usize,
    pub entry_hits: usize,
    pub misses: usize,
    pub bypasses: usize,
}

/// What one full compile reports for reuse recording.
#[derive(Debug)]
pub struct CommandOutcome<'py> {
    pub exit_code: i64,
    pub compile_cache_enabled: bool,
    pub artifacts_written: bool,
    pub dag_artifact_path: Bound<'py, PyAny>,
}

/// The arguments of one compile input attachment; unset flags are `False`, other options `None`.
#[derive(Debug)]
pub struct InputsRequest {
    pub discovered_inputs: Py<PyAny>,
    pub adapter_context: Py<PyAny>,
    pub selected_target: Py<PyAny>,
    pub cli_vars: Py<PyAny>,
    pub run_id: Py<PyAny>,
    pub no_sql_validation: Py<PyAny>,
    pub defer_model_sql_validation: Py<PyAny>,
    pub external_sql_reference_resolver: Py<PyAny>,
    pub resolved_connection: Py<PyAny>,
    pub no_cache: Py<PyAny>,
}

impl InputsRequest {
    /// The request `build_compile_inputs` was called with.
    pub(crate) fn read(arguments: &KeywordArguments<'_>) -> PyResult<Self> {
        Ok(Self {
            discovered_inputs: arguments.value("discovered_inputs")?.unbind(),
            adapter_context: arguments.value("adapter_context")?.unbind(),
            selected_target: arguments.value("selected_target")?.unbind(),
            cli_vars: arguments.value("cli_vars")?.unbind(),
            run_id: arguments.value("run_id")?.unbind(),
            no_sql_validation: arguments.value_or_false("no_sql_validation")?.unbind(),
            defer_model_sql_validation: arguments
                .value_or_false("defer_model_sql_validation")?
                .unbind(),
            external_sql_reference_resolver: arguments
                .value("external_sql_reference_resolver")?
                .unbind(),
            resolved_connection: arguments.value("resolved_connection")?.unbind(),
            no_cache: arguments.value_or_false("no_cache")?.unbind(),
        })
    }
}

/// The active target's locations as SQLBuild resolves them (default: unset); `preserve` stays as written until `record`.
#[derive(Debug, Clone, Default)]
pub struct TargetSettingsValues {
    pub(crate) name: Option<String>,
    pub(crate) database: Option<String>,
    pub(crate) schema: Option<String>,
    pub(crate) loader_schema: Option<String>,
}

impl TargetSettingsValues {
    /// The resolved schema managed loader data goes to.
    #[must_use]
    pub fn loader_schema(&self) -> Option<&str> {
        self.loader_schema.as_deref()
    }

    /// Python nodes' `(database, schema)` under these settings, as `model_free_locations` picks.
    pub fn node_locations<'py>(
        &self,
        adapter: &Bound<'py, PyAny>,
    ) -> PyResult<(Bound<'py, PyAny>, Bound<'py, PyAny>)> {
        let py: Python<'py> = adapter.py();
        self.node_locations_from(
            (
                &self.database.as_deref().into_bound_py_any(py)?,
                &self.schema.as_deref().into_bound_py_any(py)?,
            ),
            adapter,
        )
    }

    /// `node_locations` from a caller-resolved `(database, schema)`, such as `sqb load`'s, which applies CLI vars.
    pub fn node_locations_from<'py>(
        &self,
        (database, schema): (&Bound<'py, PyAny>, &Bound<'py, PyAny>),
        adapter: &Bound<'py, PyAny>,
    ) -> PyResult<(Bound<'py, PyAny>, Bound<'py, PyAny>)> {
        model_free_locations(
            (database.clone(), schema.clone()),
            self.loader_schema(),
            adapter,
        )
    }

    /// The `TargetSettings` record: a `preserve` location reads `None` with its flag set.
    pub fn record<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        let located = |value: &Option<String>| -> PyResult<Bound<'py, PyAny>> {
            value
                .as_deref()
                .filter(|value| *value != PRESERVE_TARGET_VALUE)
                .into_bound_py_any(py)
        };
        let preserved = |value: &Option<String>| -> Bound<'py, PyAny> {
            PyBool::new(py, value.as_deref() == Some(PRESERVE_TARGET_VALUE))
                .to_owned()
                .into_any()
        };
        new_record_with(
            py,
            RecordId::TargetSettings,
            &[
                ("name", self.name.clone().into_bound_py_any(py)?),
                ("database", located(&self.database)?),
                ("schema", located(&self.schema)?),
                ("loader_schema", located(&self.loader_schema)?),
                ("database_preserved", preserved(&self.database)),
                ("schema_preserved", preserved(&self.schema)),
            ],
        )
    }

    /// The `ctx` a provider's `setup` receives in a command with these settings.
    pub fn setup_context<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        new_record_with(
            py,
            RecordId::ProviderSetupContext,
            &[("target_config", self.record(py)?)],
        )
    }
}

/// What the planned build takes from its named jobs, or nothing when it names none.
#[derive(Debug)]
pub struct PlannedJobSettings {
    pub(crate) settings: NodeJobSettings,
    /// The named jobs that set `ignore_audit_failures = true` for nodes they select.
    pub(crate) ignoring_jobs: Vec<String>,
    /// The named jobs that set `full_refresh = true` for models they select.
    pub(crate) refreshing_jobs: Vec<String>,
    /// Each named job whose `warehouse` its own nodes run on, and that warehouse.
    pub(crate) job_warehouses: BTreeMap<String, String>,
}
