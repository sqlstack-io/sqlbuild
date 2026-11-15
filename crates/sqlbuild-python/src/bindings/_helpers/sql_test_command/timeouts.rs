//! Resolve the per-test and per-scenario time limit a run applies from CLI, target and settings.

use pyo3::exceptions::PyValueError;
use pyo3::prelude::{
    Bound, PyAny, PyAnyMethods, PyErr, PyModule, PyModuleMethods, PyResult, Python,
};
use pyo3::types::{PyDict, PyDictMethods};
use pyo3::{pyfunction, wrap_pyfunction};
use sqlbuild_model_config::execution_limits::main::parse_execution_limit::parse_execution_limit;
use sqlbuild_model_config::execution_limits::models::ExecutionLimit;

use crate::bindings::_helpers::contracts::spec_resolution::resolve_target_config;
use sqlbuild_python_base::records::main::construct::new_error::new_error;
use sqlbuild_python_base::records::main::fields::field::field as native_field;
use sqlbuild_python_base::records::types::ErrorId;

const INVALID_TIMEOUT_CODE: &str = "C418";
const LOCAL_FILE: &str = "sqlbuild_local.toml";
const DEFAULT_TEST_TIMEOUT: &str = "30s";

/// Which per-item limit a command resolves.
#[derive(Clone, Copy, Debug)]
pub(crate) enum TimeoutKind {
    Test,
    Scenario,
}

impl TimeoutKind {
    fn key(self) -> &'static str {
        match self {
            Self::Test => "test_timeout",
            Self::Scenario => "scenario_timeout",
        }
    }

    fn flag(self) -> &'static str {
        match self {
            Self::Test => "--test-timeout",
            Self::Scenario => "--scenario-timeout",
        }
    }
}

/// The run's limit for each item: text, whole seconds (`None` when disabled) and its source.
#[derive(Clone, Debug)]
pub(crate) struct RunTimeout {
    pub(crate) limit: Option<String>,
    pub(crate) seconds: Option<u64>,
    pub(crate) source: Option<String>,
    pub(crate) overrides_items: bool,
}

impl RunTimeout {
    /// The executor keyword arguments that carry this limit.
    pub(crate) fn keywords<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyDict>> {
        let keywords: Bound<'py, PyDict> = PyDict::new(py);
        keywords.set_item("timeout_limit", self.limit.as_deref())?;
        keywords.set_item("timeout_seconds", self.seconds)?;
        keywords.set_item("timeout_source", self.source.as_deref())?;
        keywords.set_item("timeout_overrides_items", self.overrides_items)?;
        Ok(keywords)
    }
}

fn invalid(py: Python<'_>, message: String) -> PyErr {
    new_error(py, ErrorId::CompileInput, message, INVALID_TIMEOUT_CODE)
}

fn limit_seconds(text: &str) -> Option<Option<u64>> {
    match parse_execution_limit(text)? {
        ExecutionLimit::Seconds(seconds) => Some(Some(seconds)),
        ExecutionLimit::Disabled => Some(None),
    }
}

/// The limit for `kind`: the CLI value, else the target's, else the project or local setting.
pub(crate) fn resolve_run_timeout(
    (project, discovered): (&Bound<'_, PyAny>, &Bound<'_, PyAny>),
    cli: Option<String>,
    kind: TimeoutKind,
) -> PyResult<RunTimeout> {
    let py: Python<'_> = project.py();
    if let Some(text) = cli {
        let Some(seconds) = limit_seconds(&text) else {
            return Err(invalid(
                py,
                format!(
                    "{} must be a positive fixed duration such as '30s' or '2m', or 'disabled'; \
                     got '{text}'",
                    kind.flag()
                ),
            ));
        };
        return Ok(RunTimeout {
            limit: Some(text),
            seconds,
            source: Some(kind.flag().to_owned()),
            overrides_items: true,
        });
    }
    let target_name: Bound<'_, PyAny> = native_field(project, "effective_target_name")?;
    if !target_name.is_none() {
        let target: Bound<'_, PyAny> = resolve_target_config(
            py,
            &native_field(discovered, "project_config")?,
            &native_field(discovered, "local_config")?,
            &target_name,
        )?;
        let limits: Bound<'_, PyAny> = native_field(&target, "execution_limits")?;
        let text: Option<String> = native_field(&limits, kind.key())?.extract()?;
        if let Some(text) = text {
            let name: String = target_name.extract()?;
            return Ok(RunTimeout {
                seconds: limit_seconds(&text).flatten(),
                limit: Some(text),
                source: Some(format!("[targets.{name}.execution_limits] {}", kind.key())),
                overrides_items: false,
            });
        }
    }
    let settings: Bound<'_, PyAny> = native_field(project, "settings")?;
    let text: Option<String> = native_field(&settings, kind.key())?.extract()?;
    let Some(text) = text else {
        return Ok(RunTimeout {
            limit: None,
            seconds: None,
            source: None,
            overrides_items: false,
        });
    };
    let local: bool = native_field(
        &native_field(discovered, "local_config")?,
        "setting_overrides",
    )?
    .contains(kind.key())?;
    let source: String = if local {
        format!("[settings] {} in {LOCAL_FILE}", kind.key())
    } else if matches!(kind, TimeoutKind::Test) && text == DEFAULT_TEST_TIMEOUT {
        format!("default {DEFAULT_TEST_TIMEOUT} ([settings] {})", kind.key())
    } else {
        format!("[settings] {}", kind.key())
    };
    Ok(RunTimeout {
        seconds: limit_seconds(&text).flatten(),
        limit: Some(text),
        source: Some(source),
        overrides_items: false,
    })
}

/// Whole seconds of an execution limit such as `30s`, or `None` for `disabled`.
#[pyfunction]
fn execution_limit_seconds(text: &str) -> PyResult<Option<u64>> {
    limit_seconds(text).ok_or_else(|| {
        PyValueError::new_err(format!(
            "'{text}' is not a positive fixed duration such as '30s' or 'disabled'"
        ))
    })
}

pub(crate) fn register(module: &Bound<'_, PyModule>) -> PyResult<()> {
    module.add_function(wrap_pyfunction!(execution_limit_seconds, module)?)
}
