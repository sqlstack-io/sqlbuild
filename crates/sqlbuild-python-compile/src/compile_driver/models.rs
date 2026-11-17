//! Public records of compile driver.

use pyo3::prelude::Bound;
use pyo3::prelude::Py;
use pyo3::prelude::PyAny;
use pyo3::prelude::PyResult;
use sqlbuild_python_base::boundary::models::KeywordArguments;

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
