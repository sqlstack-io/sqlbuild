//! Planning-time source freshness: observe, compare with latest state and map downstream impact.

use std::collections::{HashMap, HashSet};

use pyo3::prelude::{
    Bound, PyAny, PyAnyMethods, PyErr, PyModule, PyModuleMethods, PyResult, Python,
};
use pyo3::types::{PyDict, PyDictMethods, PyFrozenSet, PySet, PySetMethods, PyTuple};
use pyo3::{pyfunction, wrap_pyfunction};
use sqlbuild_analysis::assembly::project::types::ObjectKey;
use sqlbuild_analysis::planner_identity::constants::{MODEL_RESOURCE, SOURCE_RESOURCE};
use sqlbuild_core::graph_walk::main::breadth_first_reach::breadth_first_reach;
use sqlbuild_host::entries::models::AdapterHost;
use sqlbuild_state::freshness::main::configuration_error::configuration_error;

use crate::bindings::_helpers::boundary::python_calls::KeywordArguments;
use crate::bindings::_helpers::freshness::exception_types::SourceFreshnessObservationError;
use crate::bindings::_helpers::freshness::observation::{
    Observer, observation_record, observation_source, observe_adapter_batch, observe_configured,
    strategy, unknown,
};
use crate::bindings::_helpers::freshness::state_io::{StateRead, read_latest};
use crate::bindings::_helpers::freshness::support::text;
use crate::bindings::_helpers::native_host::host;
use crate::bindings::_helpers::planner_scope::retained::scope_data;
use crate::bindings::_helpers::records::construct::{enum_member, new_record_with};
use crate::bindings::_helpers::records::fields::field as native_field;
use crate::bindings::_helpers::records::fields::mapping_get;
use crate::bindings::_helpers::records::fields::mapping_get_or;
use crate::bindings::_helpers::records::ids::RecordId;
use crate::bindings::_helpers::state_rows::freshness::{age_policy, equivalent};

const ADAPTER_STRATEGY: &str = "adapter";
const NO_CONFIG_MESSAGE: &str = "no freshness config and adapter metadata unavailable";

/// The message of an `AdapterUserError` or a source freshness observation error, if it is one.
fn handled_message<'py>(py: Python<'py>, error: &PyErr) -> PyResult<Option<Bound<'py, PyAny>>> {
    let value: Bound<'py, PyAny> = error.value(py).clone().into_any();
    if value.is_instance(&host::entry(py, AdapterHost::AdapterUserError)?)? {
        return native_field(&value, "message").map(Some);
    }
    if error.is_instance_of::<SourceFreshnessObservationError>(py) {
        return Ok(Some(value.str()?.into_any()));
    }
    Ok(None)
}

fn freshness_strategy_is_adapter(source: &Bound<'_, PyAny>) -> PyResult<bool> {
    Ok(strategy(source)?.as_deref() == Some(ADAPTER_STRATEGY))
}

/// Latest previous records by identity across the state schemas, newest observation winning.
fn previous_records<'py>(
    arguments: &KeywordArguments<'py>,
    (adapter, connection): (&Bound<'py, PyAny>, &Bound<'py, PyAny>),
    source_names: Option<&[String]>,
) -> PyResult<Bound<'py, PyDict>> {
    let py: Python<'py> = adapter.py();
    let read: StateRead<'py> = StateRead {
        connection: connection.clone(),
        execute: native_field(adapter, "execute")?,
        render_qualified_name: arguments.value("render_qualified_name")?,
        render_read_latest_sql: native_field(adapter, "render_read_latest_source_freshness_sql")?,
    };
    let database: Bound<'py, PyAny> = arguments.value("state_database")?;
    let exists_by_schema: Bound<'py, PyAny> = arguments.value("state_table_exists_by_schema")?;
    let merged: Bound<'py, PyDict> = PyDict::new(py);
    for schema in arguments.value("state_schemas")?.try_iter()? {
        let schema: Bound<'py, PyAny> = schema?;
        let exists: bool = mapping_get_or(&exists_by_schema, &schema, false)?.is_truthy()?;
        let previous: Bound<'py, PyAny> =
            read_latest(&read, (&database, &schema), exists, source_names)?;
        for (identity, candidate) in native_field(&previous, "records")?
            .downcast::<PyDict>()?
            .iter()
        {
            let replace: bool = match merged.get_item(&identity)? {
                None => true,
                Some(current) => native_field(&candidate, "observed_at")?
                    .gt(native_field(&current, "observed_at")?)?,
            };
            if replace {
                merged.set_item(identity, candidate)?;
            }
        }
    }
    Ok(merged)
}

/// Records sorted by `str(record.identity)`, stably.
fn sorted_by_identity<'py>(
    records: impl Iterator<Item = Bound<'py, PyAny>>,
) -> PyResult<Vec<Bound<'py, PyAny>>> {
    let mut keyed: Vec<(String, Bound<'py, PyAny>)> = records
        .map(|record| Ok((native_field(&record, "identity")?.str()?.extract()?, record)))
        .collect::<PyResult<_>>()?;
    keyed.sort_by(|left, right| left.0.cmp(&right.0));
    Ok(keyed.into_iter().map(|(_, record)| record).collect())
}

fn sorted_dict<'py>(py: Python<'py>, items: &Bound<'py, PyDict>) -> PyResult<Bound<'py, PyDict>> {
    let mut keyed: Vec<(String, Bound<'py, PyAny>)> = items
        .iter()
        .map(|(name, value)| Ok((name.extract()?, value)))
        .collect::<PyResult<_>>()?;
    keyed.sort_by(|left, right| left.0.cmp(&right.0));
    let sorted: Bound<'py, PyDict> = PyDict::new(py);
    for (name, value) in keyed {
        sorted.set_item(name, value)?;
    }
    Ok(sorted)
}

/// The adapter batch, or every source unknown with the batch failure's message.
fn adapter_batch<'py>(
    observer: &Observer<'py>,
    sources: &[Bound<'py, PyAny>],
) -> PyResult<Bound<'py, PyAny>> {
    let py: Python<'py> = observer.adapter.py();
    let error: PyErr = match observe_adapter_batch(observer, sources) {
        Ok(batch) => return Ok(batch),
        Err(error) => error,
    };
    let Some(message) = handled_message(py, &error)? else {
        return Err(error);
    };
    let unknown_sources: Bound<'py, PyDict> = PyDict::new(py);
    for source in sources {
        let name: Bound<'py, PyAny> = native_field(source, "name")?;
        unknown_sources.set_item(&name, unknown(&name, ("ERROR", message.clone()))?)?;
    }
    new_record_with(
        py,
        RecordId::AdapterSourceFreshnessBatch,
        &[("unknown", unknown_sources.into_any())],
    )
}

/// Observe direct source freshness and compare it to latest append-only state.
#[pyfunction]
#[pyo3(signature = (**options))]
pub(crate) fn build_direct_source_freshness_planning_result<'py>(
    py: Python<'py>,
    options: Option<Bound<'py, PyDict>>,
) -> PyResult<Bound<'py, PyAny>> {
    let arguments: KeywordArguments<'py> = KeywordArguments::read(
        py,
        "build_direct_source_freshness_planning_result",
        options,
        (
            &[
                "adapter",
                "connection",
                "sources",
                "state_database",
                "state_schemas",
                "observed_at",
                "run_id",
                "render_qualified_name",
                "state_table_exists_by_schema",
            ],
            &["filter_previous_to_sources"],
        ),
    )?;
    let observer: Observer<'py> = Observer {
        adapter: arguments.value("adapter")?,
        connection: arguments.value("connection")?,
        observed_at: arguments.value("observed_at")?,
    };
    let run_id: Bound<'py, PyAny> = arguments.value("run_id")?;
    let sources: Vec<Bound<'py, PyAny>> = arguments
        .value("sources")?
        .try_iter()?
        .collect::<PyResult<_>>()?;
    let source_names: Option<Vec<String>> =
        if arguments.value("filter_previous_to_sources")?.is_truthy()? {
            let mut names: Vec<String> = sources
                .iter()
                .map(|source| native_field(source, "name")?.extract())
                .collect::<PyResult<_>>()?;
            names.sort();
            Some(names)
        } else {
            None
        };
    let previous: Bound<'py, PyDict> = previous_records(
        &arguments,
        (&observer.adapter, &observer.connection),
        source_names.as_deref(),
    )?;
    let unknown_sources: Bound<'py, PyDict> = PyDict::new(py);
    let observed_sources: Bound<'py, PyDict> = PyDict::new(py);
    let mut adapter_sources: Vec<Bound<'py, PyAny>> = Vec::new();
    for source in &sources {
        if native_field(source, "managed")?.is_truthy()? {
            continue;
        }
        let name: Bound<'py, PyAny> = native_field(source, "name")?;
        let Some(observed) = observation_source(&observer.adapter, source)? else {
            unknown_sources.set_item(
                &name,
                unknown(&name, ("NO_FRESHNESS_CONFIG", text(py, NO_CONFIG_MESSAGE)))?,
            )?;
            continue;
        };
        observed_sources.set_item(&name, &observed)?;
        if freshness_strategy_is_adapter(&observed)? {
            adapter_sources.push(observed);
        }
    }
    let batch: Bound<'py, PyAny> = adapter_batch(&observer, &adapter_sources)?;
    unknown_sources.update(native_field(&batch, "unknown")?.downcast()?)?;
    let observations: Bound<'py, PyAny> = native_field(&batch, "observations")?;
    let mut observed_records: Vec<Bound<'py, PyAny>> = Vec::new();
    let changed: Bound<'py, PySet> = PySet::empty(py)?;
    let unchanged: Bound<'py, PySet> = PySet::empty(py)?;
    let age_statuses: Bound<'py, PyDict> = PyDict::new(py);
    for (name, source) in observed_sources.iter() {
        let observation: Bound<'py, PyAny> = if freshness_strategy_is_adapter(&source)? {
            let observation: Bound<'py, PyAny> = mapping_get(&observations, &name)?;
            if observation.is_none() {
                continue;
            }
            observation
        } else {
            match observe_configured(&observer, &source) {
                Ok(observation) => observation,
                Err(error) => {
                    let Some(message) = handled_message(py, &error)? else {
                        return Err(error);
                    };
                    let is_observation_error: bool = !error
                        .value(py)
                        .is_instance(&host::entry(py, AdapterHost::AdapterUserError)?)?;
                    if is_observation_error && configuration_error(&message.extract::<String>()?) {
                        return Err(error);
                    }
                    unknown_sources.set_item(&name, unknown(&name, ("ERROR", message))?)?;
                    continue;
                }
            }
        };
        let record: Bound<'py, PyAny> = observation_record(&observation, &source, &run_id)?;
        observed_records.push(record.clone());
        let freshness: Bound<'py, PyAny> = native_field(&source, "freshness")?;
        let (policy, lag_tolerance) = if freshness.is_none() {
            (py.None().into_bound(py), None)
        } else {
            (
                native_field(&freshness, "age_policy")?,
                native_field(&freshness, "lag_tolerance")?.extract::<Option<String>>()?,
            )
        };
        let identity: Bound<'py, PyAny> = native_field(&record, "identity")?;
        if let Some(status) = age_policy(
            &policy,
            &native_field(&observation, "data_version")?,
            &native_field(&observation, "observed_at")?,
        )? {
            age_statuses.set_item(&identity, status)?;
        }
        match previous.get_item(&identity)? {
            None => changed.add(&identity)?,
            Some(previous_record) => {
                if equivalent(&previous_record, &record, lag_tolerance.as_deref())? {
                    unchanged.add(&identity)?;
                } else {
                    changed.add(&identity)?;
                }
            }
        }
    }
    let unknown_names: Bound<'py, PyDict> = sorted_dict(py, &unknown_sources)?;
    new_record_with(
        py,
        RecordId::DirectSourceFreshnessPlanningResult,
        &[
            (
                "observed_records",
                PyTuple::new(py, sorted_by_identity(observed_records.into_iter())?)?.into_any(),
            ),
            (
                "previous_records",
                PyTuple::new(py, sorted_by_identity(previous.values().into_iter())?)?.into_any(),
            ),
            ("changed_identities", frozen(&changed)?),
            ("unchanged_identities", frozen(&unchanged)?),
            (
                "unknown_source_names",
                PyTuple::new(py, unknown_names.keys())?.into_any(),
            ),
            ("unknown_sources", unknown_names.into_any()),
            ("age_statuses", age_statuses.into_any()),
        ],
    )
}

/// `frozenset(items)`, keeping CPython's copied set layout and iteration order.
fn frozen<'py>(items: &Bound<'py, PySet>) -> PyResult<Bound<'py, PyAny>> {
    items.py().get_type::<PyFrozenSet>().call1((items,))
}

fn downstream_models(
    source_name: String,
    downstream_deps: &HashMap<&ObjectKey, &Vec<ObjectKey>>,
) -> HashSet<String> {
    let key = (SOURCE_RESOURCE.to_owned(), source_name);
    let mut visited: HashSet<&ObjectKey> = HashSet::new();
    let Ok(()) = breadth_first_reach([&key], None, &mut visited, |node| {
        Ok(downstream_deps
            .get(node)
            .map_or(&[][..], |deps| deps.as_slice()))
    });
    visited
        .into_iter()
        .filter(|key| key.0 == MODEL_RESOURCE)
        .map(|key| key.1.clone())
        .collect()
}

/// Map changed and unknown source freshness roots to downstream model names.
#[pyfunction]
#[pyo3(signature = (*, source_freshness, scope))]
pub(crate) fn build_direct_source_freshness_propagation_result<'py>(
    source_freshness: &Bound<'py, PyAny>,
    scope: &Bound<'py, PyAny>,
) -> PyResult<Bound<'py, PyAny>> {
    let py: Python<'py> = scope.py();
    let scope = scope_data(scope)?;
    let downstream_deps: HashMap<&ObjectKey, &Vec<ObjectKey>> = scope
        .downstream
        .iter()
        .map(|(key, deps)| (key, deps))
        .collect();
    let changed: Bound<'py, PyDict> = PyDict::new(py);
    let unknown_sources: Bound<'py, PyDict> = PyDict::new(py);
    let errors: Bound<'py, PyDict> = PyDict::new(py);
    let mut stale: HashSet<String> = HashSet::new();
    let mut blocked: HashSet<String> = HashSet::new();
    for identity in native_field(source_freshness, "changed_identities")?.try_iter()? {
        let identity: Bound<'py, PyAny> = identity?;
        let models = downstream_models(
            native_field(&identity, "source_name")?.extract()?,
            &downstream_deps,
        );
        stale.extend(models.iter().cloned());
        changed.set_item(identity, PyFrozenSet::new(py, models)?)?;
    }
    for name in native_field(source_freshness, "unknown_source_names")?.try_iter()? {
        let name: Bound<'py, PyAny> = name?;
        let models = downstream_models(name.extract()?, &downstream_deps);
        stale.extend(models.iter().cloned());
        unknown_sources.set_item(name, PyFrozenSet::new(py, models)?)?;
    }
    let error_status: Bound<'py, PyAny> =
        enum_member(py, RecordId::SourceFreshnessAgeStatus, "ERROR")?;
    for (identity, status) in native_field(source_freshness, "age_statuses")?
        .downcast::<PyDict>()?
        .iter()
    {
        if status.ne(&error_status)? {
            continue;
        }
        let models = downstream_models(
            native_field(&identity, "source_name")?.extract()?,
            &downstream_deps,
        );
        blocked.extend(models.iter().cloned());
        errors.set_item(identity, PyFrozenSet::new(py, models)?)?;
    }
    new_record_with(
        py,
        RecordId::DirectSourceFreshnessPropagationResult,
        &[
            ("changed_source_model_names", changed.into_any()),
            ("unknown_source_model_names", unknown_sources.into_any()),
            ("error_source_model_names", errors.into_any()),
            ("stale_model_names", PyFrozenSet::new(py, stale)?.into_any()),
            (
                "blocked_model_names",
                PyFrozenSet::new(py, blocked)?.into_any(),
            ),
        ],
    )
}

pub(crate) fn register(module: &Bound<'_, PyModule>) -> PyResult<()> {
    module.add_function(wrap_pyfunction!(
        build_direct_source_freshness_planning_result,
        module
    )?)?;
    module.add_function(wrap_pyfunction!(
        build_direct_source_freshness_propagation_result,
        module
    )?)?;
    Ok(())
}
