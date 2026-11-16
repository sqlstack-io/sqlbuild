//! Observe the current freshness of selected sources and compare it with previous state.

use crate::freshness::models::PreviousState;
use std::collections::{HashMap, HashSet};

use pyo3::exceptions::{PyException, PyValueError};
use pyo3::prelude::{Bound, PyAny, PyAnyMethods, PyErr, PyResult, Python};
use pyo3::pyfunction;
use pyo3::types::{PyDict, PyString};
use sqlbuild_core::freshness_text::main::freshness_json::freshness_json;

use sqlbuild_host::entries::models::ExecutorHost;

use crate::freshness::_helpers::observation::{
    observation_record, observation_source, observe_adapter_batch, observe_configured, strategy,
};
use crate::freshness::models::Observer;
use sqlbuild_core::freshness_text::main::freshness_status::freshness_status;
use sqlbuild_core::freshness_text::models::{
    FreshnessSourceResult, FreshnessStatus, StateComparison,
};
use sqlbuild_python_base::boundary::models::KeywordArguments;
use sqlbuild_python_base::native_host::main::host_entry::host_entry;
use sqlbuild_python_base::records::main::fields::field::field as native_field;
use sqlbuild_python_base::records::main::fields::mapping_get::mapping_get;
use sqlbuild_python_front::state_rows::main::event_rows::age_policy::age_policy;
use sqlbuild_python_front::state_rows::main::event_rows::equivalent::equivalent;

const ADAPTER_STRATEGY: &str = "adapter";
const UNAVAILABLE_REASON: &str = "unavailable";
const OBSERVATION_RUN_ID: &str = "freshness";
const NO_CONFIG_MESSAGE: &str = "no freshness config and adapter metadata unavailable";
const MISSING_STATE_MESSAGE: &str = "previous source freshness state missing";
const NOT_RETURNED_MESSAGE: &str = "freshness metadata was not returned";

fn optional_text(value: Bound<'_, PyAny>) -> PyResult<Option<String>> {
    if value.is_none() {
        return Ok(None);
    }
    Ok(Some(value.str()?.extract()?))
}

/// `str(exc)` of an `Exception`, or the error itself for anything else.
fn exception_message(py: Python<'_>, error: PyErr) -> PyResult<String> {
    if !error.is_instance_of::<PyException>(py) {
        return Err(error);
    }
    error.value(py).str()?.extract()
}

fn target_result(
    source: &Bound<'_, PyAny>,
    (name, status): (&str, FreshnessStatus),
    message: Option<String>,
) -> PyResult<FreshnessSourceResult> {
    let mut result: FreshnessSourceResult = FreshnessSourceResult::new(name, status);
    result.target_database = optional_text(native_field(source, "database")?)?;
    result.target_schema = optional_text(native_field(source, "schema")?)?;
    result.target_name = optional_text(native_field(source, "table")?)?;
    result.message = message;
    Ok(result)
}

fn previous_record<'py>(
    state: &PreviousState<'py>,
    record: &Bound<'py, PyAny>,
) -> PyResult<Option<Bound<'py, PyAny>>> {
    let Some(lookup) = &state.by_identity else {
        return Ok(None);
    };
    let found: Bound<'py, PyAny> = mapping_get(lookup, native_field(record, "identity")?)?;
    Ok(found.is_truthy()?.then_some(found))
}

/// The command result of one successful observation.
fn observed_result(
    (name, source, observation): (&str, &Bound<'_, PyAny>, &Bound<'_, PyAny>),
    state: &PreviousState<'_>,
) -> PyResult<FreshnessSourceResult> {
    let py: Python<'_> = source.py();
    let current: Bound<'_, PyAny> = observation_record(
        observation,
        source,
        &PyString::new(py, OBSERVATION_RUN_ID).into_any(),
    )?;
    let freshness: Bound<'_, PyAny> = native_field(source, "freshness")?;
    let (lag_tolerance, policy) = if freshness.is_none() {
        (py.None().into_bound(py), py.None().into_bound(py))
    } else {
        (
            native_field(&freshness, "lag_tolerance")?,
            native_field(&freshness, "age_policy")?,
        )
    };
    let age_status: Option<Bound<'_, PyAny>> = age_policy(
        &policy,
        &native_field(observation, "data_version")?,
        &native_field(observation, "observed_at")?,
    )?;
    let previous: Option<Bound<'_, PyAny>> = previous_record(state, &current)?;
    let comparison: StateComparison = match &previous {
        None if state.by_identity.is_some() => StateComparison::Missing,
        None => StateComparison::NotCompared,
        Some(previous) => {
            let tolerance: Option<String> = lag_tolerance.extract()?;
            if equivalent(previous, &current, tolerance.as_deref())? {
                StateComparison::Equivalent {
                    same_hash: native_field(previous, "data_version_hash")?
                        .eq(native_field(&current, "data_version_hash")?)?,
                }
            } else {
                StateComparison::Different
            }
        }
    };
    let mut result: FreshnessSourceResult =
        FreshnessSourceResult::new(name, freshness_status(comparison));
    result.strategy = optional_text(native_field(&current, "strategy")?)?;
    result.value_kind = optional_text(native_field(&current, "value_kind")?)?;
    result.current_data_version = optional_text(native_field(&current, "data_version")?)?;
    result.previous_data_version = match &previous {
        Some(previous) => optional_text(native_field(previous, "data_version")?)?,
        None => None,
    };
    result.lag_tolerance = optional_text(lag_tolerance)?;
    result.target_database = optional_text(native_field(&current, "target_database")?)?;
    result.target_schema = optional_text(native_field(&current, "target_schema")?)?;
    result.target_name = optional_text(native_field(&current, "target_name")?)?;
    if comparison == StateComparison::Missing {
        result.message = Some(MISSING_STATE_MESSAGE.to_owned());
    }
    result.age_status = match age_status {
        None => None,
        Some(status) => Some(native_field(&status, "value")?.extract()?),
    };
    Ok(result)
}

/// Adapter-strategy observations in one batch, or the message of the batch failure.
fn adapter_batch<'py>(
    observer: &Observer<'py>,
    sources: &[Bound<'py, PyAny>],
) -> PyResult<Result<Bound<'py, PyAny>, String>> {
    let py: Python<'py> = observer.adapter.py();
    match observe_adapter_batch(observer, sources) {
        Ok(batch) => Ok(Ok(batch)),
        Err(error) => Ok(Err(exception_message(py, error)?)),
    }
}

/// One adapter-strategy source's observation, or its unknown or error result.
fn adapter_observation<'py>(
    batch: &Result<Bound<'py, PyAny>, String>,
    (name, source): (&str, &Bound<'py, PyAny>),
) -> PyResult<Result<Bound<'py, PyAny>, FreshnessSourceResult>> {
    let batch: &Bound<'py, PyAny> = match batch {
        Ok(batch) => batch,
        Err(message) => {
            return Ok(Err(target_result(
                source,
                (name, FreshnessStatus::Error),
                Some(message.clone()),
            )?));
        }
    };
    let unknown: Bound<'py, PyAny> =
        native_field(batch, "unknown")?.call_method1("get", (name,))?;
    if !unknown.is_none() {
        let reason: String = native_field(&unknown, "reason")?.str()?.extract()?;
        let status: FreshnessStatus = if reason == UNAVAILABLE_REASON {
            FreshnessStatus::Unknown
        } else {
            FreshnessStatus::Error
        };
        return Ok(Err(target_result(
            source,
            (name, status),
            optional_text(native_field(&unknown, "message")?)?,
        )?));
    }
    let observation: Bound<'py, PyAny> =
        native_field(batch, "observations")?.call_method1("get", (name,))?;
    if observation.is_none() {
        return Ok(Err(target_result(
            source,
            (name, FreshnessStatus::Error),
            Some(NOT_RETURNED_MESSAGE.to_owned()),
        )?));
    }
    Ok(Ok(observation))
}

/// One configured-strategy source's observation, or its error result.
fn configured_observation<'py>(
    observer: &Observer<'py>,
    (name, source): (&str, &Bound<'py, PyAny>),
) -> PyResult<Result<Bound<'py, PyAny>, FreshnessSourceResult>> {
    let py: Python<'py> = source.py();
    match observe_configured(observer, source) {
        Ok(observation) => Ok(Ok(observation)),
        Err(error) => Ok(Err(target_result(
            source,
            (name, FreshnessStatus::Error),
            Some(exception_message(py, error)?),
        )?)),
    }
}

fn observe_sources(
    observer: &Observer<'_>,
    sources: &[Bound<'_, PyAny>],
    state: &PreviousState<'_>,
) -> PyResult<Vec<FreshnessSourceResult>> {
    let mut results: Vec<FreshnessSourceResult> = Vec::new();
    let mut observed: Vec<(String, Bound<'_, PyAny>)> = Vec::new();
    let mut positions: HashMap<String, usize> = HashMap::new();
    let mut adapter_sources: Vec<Bound<'_, PyAny>> = Vec::new();
    for source in sources {
        let name: String = native_field(source, "name")?.extract()?;
        let Some(observed_source) = observation_source(&observer.adapter, source)? else {
            results.push(target_result(
                source,
                (&name, FreshnessStatus::Unknown),
                Some(NO_CONFIG_MESSAGE.to_owned()),
            )?);
            continue;
        };
        if strategy(&observed_source)?.as_deref() == Some(ADAPTER_STRATEGY) {
            adapter_sources.push(observed_source.clone());
        }
        match positions.get(&name) {
            Some(&position) => observed[position].1 = observed_source,
            None => {
                positions.insert(name.clone(), observed.len());
                observed.push((name, observed_source));
            }
        }
    }
    let batch = adapter_batch(observer, &adapter_sources)?;
    for (name, source) in &observed {
        let observation = if strategy(source)?.as_deref() == Some(ADAPTER_STRATEGY) {
            adapter_observation(&batch, (name, source))?
        } else {
            configured_observation(observer, (name, source))?
        };
        match observation {
            Ok(observation) => results.push(observed_result((name, source, &observation), state)?),
            Err(result) => results.push(result),
        }
    }
    results.sort_by(|left, right| left.name.cmp(&right.name));
    Ok(results)
}

/// Observe the selected sources inside the run's observability scope.
pub(crate) fn observe_freshness(
    observer: &Observer<'_>,
    (sources, select, exclude): (&Bound<'_, PyAny>, &[String], &[String]),
    (run_id, state): (Option<String>, &PreviousState<'_>),
) -> PyResult<Vec<FreshnessSourceResult>> {
    let py: Python<'_> = observer.adapter.py();
    let selected: HashSet<&str> = select.iter().map(String::as_str).collect();
    let excluded: HashSet<&str> = exclude.iter().map(String::as_str).collect();
    let mut chosen: Vec<Bound<'_, PyAny>> = Vec::new();
    for source in sources.try_iter()? {
        let source: Bound<'_, PyAny> = source?;
        let name: String = native_field(&source, "name")?.extract()?;
        if (selected.is_empty() || selected.contains(name.as_str()))
            && !excluded.contains(name.as_str())
        {
            chosen.push(source);
        }
    }
    let Some(run_id) = run_id else {
        return observe_sources(observer, &chosen, state);
    };
    let scope: Bound<'_, PyAny> = host_entry(py, ExecutorHost::RunScope)?.call1((run_id,))?;
    scope.call_method0("__enter__")?;
    match observe_sources(observer, &chosen, state) {
        Ok(results) => {
            scope.call_method1("__exit__", (py.None(), py.None(), py.None()))?;
            Ok(results)
        }
        Err(error) => {
            scope.call_method1(
                "__exit__",
                (error.get_type(py), error.value(py), error.traceback(py)),
            )?;
            Err(error)
        }
    }
}

/// Observe source freshness and return the `sqb freshness --json` payload.
#[pyfunction]
#[pyo3(signature = (**options))]
pub(crate) fn observe_source_freshness_json(
    py: Python<'_>,
    options: Option<Bound<'_, PyDict>>,
) -> PyResult<String> {
    let arguments: KeywordArguments<'_> = KeywordArguments::read(
        py,
        "observe_source_freshness_json",
        options,
        (
            &["adapter", "connection", "sources", "observed_at"],
            &["select", "exclude", "run_id", "previous_records"],
        ),
    )?;
    let names = |name: &str| -> PyResult<Vec<String>> {
        let value: Bound<'_, PyAny> = arguments.value(name)?;
        if value.is_none() {
            return Ok(Vec::new());
        }
        value.extract()
    };
    let observer: Observer<'_> = Observer {
        adapter: arguments.value("adapter")?,
        connection: arguments.value("connection")?,
        observed_at: arguments.value("observed_at")?,
    };
    let previous: Bound<'_, PyAny> = arguments.value("previous_records")?;
    let state: PreviousState<'_> = PreviousState {
        by_identity: (!previous.is_none()).then_some(previous),
    };
    let results: Vec<FreshnessSourceResult> = observe_freshness(
        &observer,
        (
            &arguments.value("sources")?,
            &names("select")?,
            &names("exclude")?,
        ),
        (arguments.value("run_id")?.extract()?, &state),
    )?;
    freshness_json(&results).map_err(|error| PyValueError::new_err(format!("{error:?}")))
}
