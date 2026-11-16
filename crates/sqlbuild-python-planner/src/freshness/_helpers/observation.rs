//! Observe configured and adapter-metadata source freshness through adapter callbacks.

use crate::freshness::main::presumed_utc_datetime::presumed_utc_datetime;
use crate::freshness::models::Observer;
use pyo3::prelude::{Bound, PyAny, PyAnyMethods, PyResult, Python};
use pyo3::pyfunction;
use pyo3::types::{PyBool, PyDict, PyDictMethods, PyTuple};
use sqlbuild_host::entries::models::{AdapterHost, ExecutorHost};
use sqlbuild_state::freshness::main::single_value_problem::single_value_problem;
use sqlbuild_state::freshness::models::QueryShape;

use crate::freshness::_helpers::support::{
    OBSERVATION_ERROR, exception_text, freshness_error, presumed_utc_value, text,
};
use sqlbuild_python_base::boundary::main::call_with::call_with;
use sqlbuild_python_base::boundary::main::within::within;
use sqlbuild_python_base::native_host::main::host_entry::host_entry;
use sqlbuild_python_base::records::main::construct::enum_from_value::enum_from_value;
use sqlbuild_python_base::records::main::construct::enum_member::enum_member;
use sqlbuild_python_base::records::main::construct::new_record::new_record;
use sqlbuild_python_base::records::main::construct::new_record_with::new_record_with;
use sqlbuild_python_base::records::main::fields::field::field as native_field;
use sqlbuild_python_base::records::main::fields::mapping_get_or::mapping_get_or;
use sqlbuild_python_base::records::main::fields::replaced_keywords::replaced_keywords;
use sqlbuild_python_base::records::types::RecordId;
use sqlbuild_python_front::state_rows::main::event_rows::normalize::normalize;
use sqlbuild_python_front::state_rows::main::event_rows::version_hash::version_hash;
use sqlbuild_python_project::references::main::render_source_read::render_source_read;

const ADAPTER_STRATEGY: &str = "adapter";
const COLUMN_STRATEGY: &str = "column";
const DEFAULT_STRATEGY: &str = "sql";
const METADATA_OPERATION: &str = "source_freshness_metadata_observation";
const QUERY_OPERATION: &str = "source_freshness_query_observation";

pub(crate) fn observation_error(py: Python<'_>, message: &str) -> pyo3::PyErr {
    freshness_error(py, (OBSERVATION_ERROR, message), None)
}

/// The configured strategy value of a source, or `None` without freshness config.
pub(crate) fn strategy(source: &Bound<'_, PyAny>) -> PyResult<Option<String>> {
    let freshness: Bound<'_, PyAny> = native_field(source, "freshness")?;
    if freshness.is_none() {
        return Ok(None);
    }
    Ok(Some(
        native_field(&freshness, "strategy")?.str()?.extract()?,
    ))
}

/// The source as observed: itself with freshness config, an adapter-metadata copy, or none.
pub(crate) fn observation_source<'py>(
    adapter: &Bound<'py, PyAny>,
    source: &Bound<'py, PyAny>,
) -> PyResult<Option<Bound<'py, PyAny>>> {
    let py: Python<'py> = source.py();
    if !native_field(source, "freshness")?.is_none() {
        return Ok(Some(source.clone()));
    }
    if !native_field(source, "expression")?.is_none()
        || native_field(source, "table")?.is_none()
        || !adapter
            .call_method0("supports_table_freshness_metadata")?
            .is_truthy()?
    {
        return Ok(None);
    }
    let config: Bound<'py, PyAny> = new_record_with(
        py,
        RecordId::SourceFreshnessConfig,
        &[("strategy", adapter_strategy(py)?)],
    )?;
    replaced_source(source, config).map(Some)
}

fn replaced_source<'py>(
    source: &Bound<'py, PyAny>,
    config: Bound<'py, PyAny>,
) -> PyResult<Bound<'py, PyAny>> {
    let changes: Bound<'py, PyDict> = PyDict::new(source.py());
    changes.set_item("freshness", config)?;
    replaced_keywords(source, &changes)
}

fn adapter_strategy(py: Python<'_>) -> PyResult<Bound<'_, PyAny>> {
    enum_member(py, RecordId::SourceFreshnessStrategy, "ADAPTER")
}

/// `OperationLifecycle(operation_kind="freshness", ...)` for one observation phase.
fn lifecycle<'py>(
    py: Python<'py>,
    (operation_name, strategy): (&str, &str),
    item_count: Option<usize>,
) -> PyResult<Bound<'py, PyAny>> {
    let attributes: Bound<'py, PyAny> = call_with(
        &host_entry(py, ExecutorHost::OperationAttributes)?,
        &[
            ("phase", text(py, "observe")),
            ("strategy", text(py, strategy)),
            ("target_kind", text(py, "source")),
        ],
    )?;
    let mut options: Vec<(&str, Bound<'py, PyAny>)> = vec![
        ("operation_kind", text(py, "freshness")),
        ("operation_name", text(py, operation_name)),
    ];
    if let Some(item_count) = item_count {
        let metadata: Bound<'py, PyDict> = PyDict::new(py);
        metadata.set_item("item_count", item_count)?;
        options.push(("metadata", metadata.into_any()));
    }
    options.push(("attributes", attributes));
    call_with(&host_entry(py, ExecutorHost::OperationLifecycle)?, &options)
}

pub(crate) fn observation<'py>(
    source_name: &Bound<'py, PyAny>,
    (strategy, value_kind): (Bound<'py, PyAny>, Bound<'py, PyAny>),
    (data_version, observed_at): (Bound<'py, PyAny>, Bound<'py, PyAny>),
) -> PyResult<Bound<'py, PyAny>> {
    new_record_with(
        source_name.py(),
        RecordId::SourceFreshnessObservation,
        &[
            ("source_name", source_name.clone()),
            ("strategy", strategy),
            ("data_version", data_version),
            ("value_kind", value_kind),
            ("observed_at", observed_at),
        ],
    )
}

/// Observe one source freshness config and return a comparable data version.
pub(crate) fn observe_configured<'py>(
    observer: &Observer<'py>,
    source: &Bound<'py, PyAny>,
) -> PyResult<Bound<'py, PyAny>> {
    let py: Python<'py> = source.py();
    let strategy: Option<String> = strategy(source)?;
    let operation: &str = if strategy.as_deref() == Some(ADAPTER_STRATEGY) {
        METADATA_OPERATION
    } else {
        QUERY_OPERATION
    };
    let manager: Bound<'py, PyAny> = lifecycle(
        py,
        (operation, strategy.as_deref().unwrap_or(DEFAULT_STRATEGY)),
        None,
    )?;
    within(&manager, |_| {
        observe_configured_inner(observer, source, strategy.as_deref())
    })
}

fn observe_configured_inner<'py>(
    observer: &Observer<'py>,
    source: &Bound<'py, PyAny>,
    strategy: Option<&str>,
) -> PyResult<Bound<'py, PyAny>> {
    let py: Python<'py> = source.py();
    let name: Bound<'py, PyAny> = native_field(source, "name")?;
    let config: Bound<'py, PyAny> = native_field(source, "freshness")?;
    let Some(strategy) = strategy else {
        return Err(observation_error(
            py,
            &format!(
                "source '{}' does not define freshness configuration",
                name.str()?
            ),
        ));
    };
    if strategy == ADAPTER_STRATEGY {
        return observe_adapter_source(observer, source);
    }
    let value_kind: Bound<'py, PyAny> = native_field(&config, "value_kind")?;
    let sql: Bound<'py, PyAny> = if strategy == COLUMN_STRATEGY {
        let column: Bound<'py, PyAny> = native_field(&config, "column")?;
        if column.is_none() || value_kind.is_none() {
            return Err(observation_error(
                py,
                &format!(
                    "source '{}' has incomplete column freshness configuration",
                    name.str()?
                ),
            ));
        }
        let filter: Bound<'py, PyAny> = native_field(&config, "filter")?;
        let where_sql: String = if filter.is_none() {
            String::new()
        } else {
            format!(" WHERE {}", filter.str()?)
        };
        let relation: String = render_source_read(source, &observer.adapter)?;
        call_with(
            &native_field(&observer.adapter, "render_source_freshness_max_query")?,
            &[
                ("column", column),
                ("source_relation", text(py, &relation)),
                (
                    "source_is_subquery",
                    PyBool::new(py, !native_field(source, "expression")?.is_none())
                        .to_owned()
                        .into_any(),
                ),
                ("where_sql", text(py, &where_sql)),
            ],
        )?
    } else {
        let query: Bound<'py, PyAny> = native_field(&config, "query")?;
        if query.is_none() || value_kind.is_none() {
            return Err(observation_error(
                py,
                &format!(
                    "source '{}' has incomplete sql freshness configuration",
                    name.str()?
                ),
            ));
        }
        query
    };
    let data_version: Bound<'py, PyAny> =
        presumed_utc_value(&single_data_version(observer, &name, sql)?)?;
    observation(
        &name,
        (native_field(&config, "strategy")?, value_kind),
        (data_version, observer.observed_at.clone()),
    )
}

fn single_data_version<'py>(
    observer: &Observer<'py>,
    source_name: &Bound<'py, PyAny>,
    sql: Bound<'py, PyAny>,
) -> PyResult<Bound<'py, PyAny>> {
    let py: Python<'py> = source_name.py();
    let name: String = source_name.str()?.extract()?;
    let result: Bound<'py, PyAny> = match call_with(
        &native_field(&observer.adapter, "query")?,
        &[
            ("connection", observer.connection.clone()),
            ("sql", sql),
            ("limit", py.None().into_bound(py)),
        ],
    ) {
        Ok(result) => result,
        Err(error) => {
            let Some(message) = exception_text(py, &error)? else {
                return Err(error);
            };
            return Err(freshness_error(
                py,
                (
                    OBSERVATION_ERROR,
                    &format!("source '{name}' freshness query failed: {message}"),
                ),
                Some(error),
            ));
        }
    };
    let columns: usize = native_field(&result, "columns")?.len()?;
    let rows: Bound<'py, PyAny> = native_field(&result, "rows")?;
    let row_count: usize = rows.len()?;
    let first: Option<Bound<'py, PyAny>> = if columns == 1 && row_count == 1 {
        Some(rows.get_item(0)?.get_item(0)?)
    } else {
        None
    };
    let shape: QueryShape = QueryShape {
        columns,
        rows: row_count,
        first_value_null: first.as_ref().is_some_and(|value| value.is_none()),
    };
    if let Some(problem) = single_value_problem(&name, shape) {
        return Err(observation_error(py, &problem));
    }
    first.ok_or_else(|| observation_error(py, "freshness query returned no value"))
}

fn unsupported_message(
    adapter: &Bound<'_, PyAny>,
    source: &Bound<'_, PyAny>,
) -> PyResult<Option<String>> {
    if !adapter
        .call_method0("supports_table_freshness_metadata")?
        .is_truthy()?
    {
        return Ok(Some(format!(
            "adapter '{}' does not support table freshness metadata",
            native_field(adapter, "adapter_name")?.str()?
        )));
    }
    if !native_field(source, "expression")?.is_none() || native_field(source, "table")?.is_none() {
        return Ok(Some(format!(
            "source '{}' adapter freshness requires a physical table source",
            native_field(source, "name")?.str()?
        )));
    }
    Ok(None)
}

/// `SourceFreshnessUnknown(source_name=..., reason=<reason member>, message=...)`.
pub(crate) fn unknown<'py>(
    source_name: &Bound<'py, PyAny>,
    (reason, message): (&str, Bound<'py, PyAny>),
) -> PyResult<Bound<'py, PyAny>> {
    let py: Python<'py> = source_name.py();
    new_record_with(
        py,
        RecordId::SourceFreshnessUnknown,
        &[
            ("source_name", source_name.clone()),
            (
                "reason",
                enum_member(py, RecordId::SourceFreshnessUnknownReason, reason)?,
            ),
            ("message", message),
        ],
    )
}

/// The unknown outcome of unavailable metadata or a null data version, if any.
fn metadata_unknown<'py>(
    source_name: &Bound<'py, PyAny>,
    metadata: &Bound<'py, PyAny>,
) -> PyResult<Option<Bound<'py, PyAny>>> {
    let py: Python<'py> = source_name.py();
    let name: String = source_name.str()?.extract()?;
    let unavailable: Bound<'py, PyAny> =
        host_entry(py, AdapterHost::TableFreshnessStatus)?.getattr("UNAVAILABLE")?;
    if native_field(metadata, "status")?.eq(unavailable)? {
        let message: Bound<'py, PyAny> = native_field(metadata, "message")?;
        let message: String = if message.is_truthy()? {
            message.str()?.extract()?
        } else {
            format!("source '{name}' freshness metadata unavailable")
        };
        return unknown(source_name, ("UNAVAILABLE", text(py, &message))).map(Some);
    }
    if native_field(metadata, "data_version")?.is_none() {
        let message: String = format!("source '{name}' freshness data_version is null");
        return unknown(source_name, ("UNAVAILABLE", text(py, &message))).map(Some);
    }
    Ok(None)
}

fn metadata_observation<'py>(
    source_name: &Bound<'py, PyAny>,
    metadata: &Bound<'py, PyAny>,
    fallback_observed_at: &Bound<'py, PyAny>,
) -> PyResult<Bound<'py, PyAny>> {
    let py: Python<'py> = source_name.py();
    let observed_at: Bound<'py, PyAny> = native_field(metadata, "observed_at")?;
    let observed_at: Bound<'py, PyAny> = if observed_at.is_none() {
        fallback_observed_at.clone()
    } else {
        presumed_utc_datetime(&observed_at)?
    };
    observation(
        source_name,
        (
            adapter_strategy(py)?,
            enum_from_value(
                py,
                RecordId::SourceFreshnessValueKind,
                native_field(metadata, "value_kind")?,
            )?,
        ),
        (
            presumed_utc_value(&native_field(metadata, "data_version")?)?,
            observed_at,
        ),
    )
}

fn observe_adapter_source<'py>(
    observer: &Observer<'py>,
    source: &Bound<'py, PyAny>,
) -> PyResult<Bound<'py, PyAny>> {
    let py: Python<'py> = source.py();
    if let Some(message) = unsupported_message(&observer.adapter, source)? {
        return Err(observation_error(py, &message));
    }
    let metadata: Bound<'py, PyAny> = call_with(
        &native_field(&observer.adapter, "get_table_freshness_metadata")?,
        &[
            ("connection", observer.connection.clone()),
            ("database", native_field(source, "database")?),
            ("schema", native_field(source, "schema")?),
            ("name", native_field(source, "table")?),
        ],
    )?;
    let name: Bound<'py, PyAny> = native_field(source, "name")?;
    if let Some(unknown) = metadata_unknown(&name, &metadata)? {
        return Err(observation_error(
            py,
            &native_field(&unknown, "message")?
                .str()?
                .extract::<String>()?,
        ));
    }
    metadata_observation(&name, &metadata, &observer.observed_at)
}

/// Observe adapter metadata freshness per source, batching the warehouse lookups.
pub(crate) fn observe_adapter_batch<'py>(
    observer: &Observer<'py>,
    sources: &[Bound<'py, PyAny>],
) -> PyResult<Bound<'py, PyAny>> {
    let py: Python<'py> = observer.adapter.py();
    if sources.is_empty() {
        return new_record(py, RecordId::AdapterSourceFreshnessBatch, (), None);
    }
    let manager: Bound<'py, PyAny> = lifecycle(
        py,
        (METADATA_OPERATION, ADAPTER_STRATEGY),
        Some(sources.len()),
    )?;
    within(&manager, |entered| {
        let batch: Bound<'py, PyAny> = observe_adapter_batch_inner(observer, sources)?;
        let metadata: Bound<'py, PyDict> = PyDict::new(py);
        metadata.set_item("item_count", sources.len())?;
        metadata.set_item("unknown_count", native_field(&batch, "unknown")?.len()?)?;
        call_with(
            &native_field(entered, "completed")?,
            &[("metadata", metadata.into_any())],
        )?;
        Ok(batch)
    })
}

fn observe_adapter_batch_inner<'py>(
    observer: &Observer<'py>,
    sources: &[Bound<'py, PyAny>],
) -> PyResult<Bound<'py, PyAny>> {
    let py: Python<'py> = observer.adapter.py();
    let unknown_sources: Bound<'py, PyDict> = PyDict::new(py);
    let requests: Bound<'py, PyDict> = PyDict::new(py);
    let request_class: Bound<'py, PyAny> = host_entry(py, AdapterHost::TableFreshnessRequest)?;
    for source in sources {
        let name: Bound<'py, PyAny> = native_field(source, "name")?;
        if let Some(message) = unsupported_message(&observer.adapter, source)? {
            unknown_sources
                .set_item(&name, unknown(&name, ("UNAVAILABLE", text(py, &message)))?)?;
            continue;
        }
        requests.set_item(
            &name,
            call_with(
                &request_class,
                &[
                    ("database", native_field(source, "database")?),
                    ("schema", native_field(source, "schema")?),
                    ("name", native_field(source, "table")?.str()?.into_any()),
                ],
            )?,
        )?;
    }
    if requests.is_empty() {
        return new_record_with(
            py,
            RecordId::AdapterSourceFreshnessBatch,
            &[("unknown", unknown_sources.into_any())],
        );
    }
    let unique: Bound<'py, PyDict> = PyDict::new(py);
    for request in requests.values() {
        unique.set_item(request, py.None())?;
    }
    let metadata_by_request: Bound<'py, PyAny> = call_with(
        &native_field(&observer.adapter, "get_tables_freshness_metadata")?,
        &[
            ("connection", observer.connection.clone()),
            ("requests", PyTuple::new(py, unique.keys())?.into_any()),
        ],
    )?;
    let metadata_class: Bound<'py, PyAny> = host_entry(py, AdapterHost::TableFreshnessMetadata)?;
    let observations: Bound<'py, PyDict> = PyDict::new(py);
    for (name, request) in requests.iter() {
        let missing: Bound<'py, PyAny> = call_with(
            &native_field(&metadata_class, "unavailable")?,
            &[(
                "message",
                text(
                    py,
                    &format!(
                        "source '{}' freshness metadata was not returned",
                        name.str()?
                    ),
                ),
            )],
        )?;
        let metadata: Bound<'py, PyAny> = mapping_get_or(&metadata_by_request, request, missing)?;
        if let Some(unknown) = metadata_unknown(&name, &metadata)? {
            unknown_sources.set_item(&name, unknown)?;
            continue;
        }
        observations.set_item(
            &name,
            metadata_observation(&name, &metadata, &observer.observed_at)?,
        )?;
    }
    new_record_with(
        py,
        RecordId::AdapterSourceFreshnessBatch,
        &[
            ("observations", observations.into_any()),
            ("unknown", unknown_sources.into_any()),
        ],
    )
}

/// The state record of one observation, with its normalized version and hash.
pub(crate) fn observation_record<'py>(
    observation: &Bound<'py, PyAny>,
    source: &Bound<'py, PyAny>,
    run_id: &Bound<'py, PyAny>,
) -> PyResult<Bound<'py, PyAny>> {
    let py: Python<'py> = observation.py();
    let source_name: Bound<'py, PyAny> = native_field(observation, "source_name")?;
    let strategy: Bound<'py, PyAny> = native_field(observation, "strategy")?;
    let value_kind: Bound<'py, PyAny> = native_field(observation, "value_kind")?;
    let data_version: Bound<'py, PyAny> =
        normalize(&native_field(observation, "data_version")?, &value_kind)?;
    let data_version_hash: String = version_hash(
        &source_name.extract::<String>()?,
        &strategy.extract::<String>()?,
        &value_kind.extract::<String>()?,
        &data_version.extract::<String>()?,
    )?;
    new_record_with(
        py,
        RecordId::SourceFreshnessRecord,
        &[
            ("source_name", source_name),
            ("target_database", native_field(source, "database")?),
            ("target_schema", native_field(source, "schema")?),
            ("target_name", native_field(source, "table")?),
            ("run_id", run_id.clone()),
            ("strategy", native_field(&strategy, "value")?),
            ("value_kind", native_field(&value_kind, "value")?),
            ("data_version", data_version),
            ("data_version_hash", text(py, &data_version_hash)),
            ("observed_at", native_field(observation, "observed_at")?),
        ],
    )
}

/// Observe one source freshness config and return a comparable data version.
#[pyfunction]
#[pyo3(signature = (*, adapter, connection, source, observed_at))]
pub(crate) fn observe_configured_source_freshness<'py>(
    adapter: Bound<'py, PyAny>,
    connection: Bound<'py, PyAny>,
    source: &Bound<'py, PyAny>,
    observed_at: Bound<'py, PyAny>,
) -> PyResult<Bound<'py, PyAny>> {
    let observer: Observer<'py> = Observer {
        adapter,
        connection,
        observed_at,
    };
    observe_configured(&observer, source)
}

/// Observe adapter metadata freshness per source, batching the warehouse lookups.
#[pyfunction]
#[pyo3(signature = (*, adapter, connection, sources, observed_at))]
pub(crate) fn observe_adapter_sources_freshness<'py>(
    adapter: Bound<'py, PyAny>,
    connection: Bound<'py, PyAny>,
    sources: &Bound<'py, PyAny>,
    observed_at: Bound<'py, PyAny>,
) -> PyResult<Bound<'py, PyAny>> {
    let observer: Observer<'py> = Observer {
        adapter,
        connection,
        observed_at,
    };
    let sources: Vec<Bound<'py, PyAny>> = sources.try_iter()?.collect::<PyResult<_>>()?;
    observe_adapter_batch(&observer, &sources)
}
