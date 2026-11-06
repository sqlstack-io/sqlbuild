//! Append observed source freshness to state once the models it affected have built.

use pyo3::prelude::{Bound, PyAny, PyAnyMethods, PyModule, PyModuleMethods, PyResult, Python};
use pyo3::types::{PyDict, PyDictMethods, PyFrozenSet, PyString, PyTuple};
use pyo3::{pyfunction, wrap_pyfunction};
use sqlbuild_host::entries::models::ExecutorHost;

use crate::bindings::_helpers::boundary::python_calls::keywords;
use crate::bindings::_helpers::boundary::python_calls::{
    KeywordArguments, call_with, imported, within,
};
use crate::bindings::_helpers::cost_telemetry::context::CostContext;
use crate::bindings::_helpers::freshness::state_io::write_source_freshness_records;
use crate::bindings::_helpers::native_host::host;
use crate::bindings::_helpers::records::construct::new_record_with;
use crate::bindings::_helpers::records::fields::field as native_field;
use crate::bindings::_helpers::records::fields::mapping_items;
use crate::bindings::_helpers::records::fields::replaced_keywords;
use crate::bindings::_helpers::records::ids::RecordId;

/// Observed records grouped by `(database, schema)` in first-seen order.
type RecordsByLocation<'py> = Vec<(
    (Bound<'py, PyAny>, Bound<'py, PyAny>),
    Vec<Bound<'py, PyAny>>,
)>;

/// The names of the selected models that built successfully.
fn successful_model_names(result: &Bound<'_, PyAny>) -> PyResult<Vec<String>> {
    let py = result.py();
    let success: Bound<'_, PyAny> =
        host::entry(py, ExecutorHost::ExecutionStatus)?.getattr("SUCCESS")?;
    let mut names: Vec<String> = Vec::new();
    for model_result in native_field(result, "model_results")?.try_iter()? {
        let model_result: Bound<'_, PyAny> = model_result?;
        let status: Bound<'_, PyAny> = native_field(&model_result, "status")?;
        let value: Bound<'_, PyAny> = status.getattr_opt("value")?.unwrap_or(status);
        if value.eq(&success)? {
            names.push(native_field(&model_result, "model_name")?.extract()?);
        }
    }
    Ok(names)
}

/// `record` added under each distinct schema location of `affected`, in sorted model order.
fn collect_record<'py>(
    mut grouped: RecordsByLocation<'py>,
    record: &Bound<'py, PyAny>,
    (affected, entries_by_name): (&Bound<'py, PyAny>, &Bound<'py, PyDict>),
) -> PyResult<RecordsByLocation<'py>> {
    let py: Python<'py> = record.py();
    let sorted: Bound<'py, PyAny> = imported(py, "builtins", "sorted")?.call1((affected,))?;
    let mut recorded: Vec<(Bound<'py, PyAny>, Bound<'py, PyAny>)> = Vec::new();
    for model_name in sorted.try_iter()? {
        let Some(entry) = entries_by_name.get_item(model_name?)? else {
            continue;
        };
        let destination: Bound<'py, PyAny> = native_field(&entry, "destination")?;
        let schema: Bound<'py, PyAny> = native_field(&destination, "schema")?;
        if schema.is_none() {
            continue;
        }
        let location: (Bound<'py, PyAny>, Bound<'py, PyAny>) =
            (native_field(&destination, "database")?, schema);
        if contains_location(&recorded, &location)? {
            continue;
        }
        recorded.push(location.clone());
        let mut position: Option<usize> = None;
        for (index, (known, _)) in grouped.iter().enumerate() {
            if same_location(known, &location)? {
                position = Some(index);
                break;
            }
        }
        match position {
            Some(index) => grouped[index].1.push(record.clone()),
            None => grouped.push((location, vec![record.clone()])),
        }
    }
    Ok(grouped)
}

/// Whether two `(database, schema)` locations are equal.
fn same_location(
    left: &(Bound<'_, PyAny>, Bound<'_, PyAny>),
    right: &(Bound<'_, PyAny>, Bound<'_, PyAny>),
) -> PyResult<bool> {
    Ok(left.0.eq(&right.0)? && left.1.eq(&right.1)?)
}

/// Whether `locations` holds `location`.
fn contains_location(
    locations: &[(Bound<'_, PyAny>, Bound<'_, PyAny>)],
    location: &(Bound<'_, PyAny>, Bound<'_, PyAny>),
) -> PyResult<bool> {
    for known in locations {
        if same_location(known, location)? {
            return Ok(true);
        }
    }
    Ok(false)
}

/// The observed records to publish, keyed by the schema of each affected model.
fn eligible_records<'py>(
    plan: &Bound<'py, PyAny>,
    successful_models: &[String],
    run_id: &Bound<'py, PyAny>,
) -> PyResult<RecordsByLocation<'py>> {
    let py: Python<'py> = plan.py();
    let freshness: Bound<'py, PyAny> = native_field(plan, "source_freshness")?;
    if freshness.is_none() || native_field(&freshness, "propagation")?.is_none() {
        return Ok(Vec::new());
    }
    let entries_by_name: Bound<'py, PyDict> = PyDict::new(py);
    for entry in native_field(plan, "model_entries")?.try_iter()? {
        let entry: Bound<'py, PyAny> = entry?;
        entries_by_name.set_item(native_field(&entry, "name")?, entry)?;
    }
    if entries_by_name.is_empty() {
        return Ok(Vec::new());
    }
    let selected: Bound<'py, PyFrozenSet> = PyFrozenSet::new(py, entries_by_name.keys())?;
    let successful: Bound<'py, PyFrozenSet> = PyFrozenSet::new(py, successful_models)?;
    let observed: Bound<'py, PyDict> = PyDict::new(py);
    for record in native_field(&freshness, "observed_records")?.try_iter()? {
        let record: Bound<'py, PyAny> = record?;
        observed.set_item(native_field(&record, "identity")?, record)?;
    }
    let changes: Bound<'py, PyDict> = PyDict::new(py);
    changes.set_item("run_id", run_id)?;
    let mut grouped: RecordsByLocation<'py> = Vec::new();
    let changed: Bound<'py, PyAny> =
        native_field(&freshness, "propagation")?.getattr("changed_source_model_names")?;
    for item in mapping_items(&changed)?.try_iter()? {
        let (identity, affected): (Bound<'py, PyAny>, Bound<'py, PyAny>) = item?.extract()?;
        let affected: Bound<'py, PyAny> = affected.call_method1("__and__", (&selected,))?;
        if !affected.is_truthy()?
            || !affected
                .call_method1("__le__", (&successful,))?
                .is_truthy()?
        {
            continue;
        }
        let Some(record) = observed.get_item(identity)? else {
            continue;
        };
        let replaced: Bound<'py, PyAny> = replaced_keywords(&record, &changes)?;
        grouped = collect_record(grouped, &replaced, (&affected, &entries_by_name))?;
    }
    Ok(grouped)
}

/// `SourceFreshnessRenderers` bound to `adapter`.
fn renderers<'py>(adapter: &Bound<'py, PyAny>) -> PyResult<Bound<'py, PyAny>> {
    new_record_with(
        adapter.py(),
        RecordId::SourceFreshnessRenderers,
        &[
            (
                "render_qualified_name",
                native_field(adapter, "render_qualified_name")?,
            ),
            (
                "render_framework_type",
                native_field(adapter, "render_framework_type")?,
            ),
            (
                "render_insert_records_sql",
                native_field(adapter, "render_insert_source_freshness_records_sql")?,
            ),
            (
                "render_create_table_sql",
                native_field(adapter, "render_create_source_freshness_table_sql")?,
            ),
            (
                "render_create_index_sqls",
                native_field(adapter, "render_create_source_freshness_index_sqls")?,
            ),
            ("read_columns", native_field(adapter, "get_columns")?),
            (
                "render_add_columns",
                native_field(adapter, "render_add_columns")?,
            ),
        ],
    )
}

/// Write each location's records, one cost scope per source.
fn write_records<'py>(
    adapter: &Bound<'py, PyAny>,
    connection: &Bound<'py, PyAny>,
    grouped: &RecordsByLocation<'py>,
) -> PyResult<()> {
    let py: Python<'py> = adapter.py();
    for ((database, schema), records) in grouped {
        let mut by_source: Vec<(Bound<'py, PyAny>, Vec<Bound<'py, PyAny>>)> = Vec::new();
        for record in records {
            let source_name: Bound<'py, PyAny> = native_field(record, "source_name")?;
            let mut position: Option<usize> = None;
            for (index, (known, _)) in by_source.iter().enumerate() {
                if known.eq(&source_name)? {
                    position = Some(index);
                    break;
                }
            }
            match position {
                Some(index) => by_source[index].1.push(record.clone()),
                None => by_source.push((source_name, vec![record.clone()])),
            }
        }
        for (source_name, source_records) in by_source {
            let scope: Bound<'py, PyAny> = Bound::new(
                py,
                CostContext::resource_scope(
                    py,
                    Some(keywords(
                        py,
                        &[
                            ("resource_type", PyString::new(py, "source").into_any()),
                            ("resource_name", source_name),
                            (
                                "phase",
                                PyString::new(py, "freshness_finalization").into_any(),
                            ),
                        ],
                    )?),
                )?,
            )?
            .into_any();
            within(&scope, |_| {
                write_source_freshness_records(
                    py,
                    Some(keywords(
                        py,
                        &[
                            ("connection", connection.clone()),
                            ("execute", native_field(adapter, "execute")?),
                            ("database", database.clone()),
                            ("schema", schema.clone()),
                            ("records", PyTuple::new(py, &source_records)?.into_any()),
                            ("renderers", renderers(adapter)?),
                        ],
                    )?),
                )
            })?;
        }
    }
    Ok(())
}

/// Append observed source freshness only after the affected selected models succeed.
pub(crate) fn append_freshness_records<'py>(
    (plan, successful_models): (&Bound<'py, PyAny>, &[String]),
    adapter: &Bound<'py, PyAny>,
    connection_config: &Bound<'py, PyAny>,
    run_id: &Bound<'py, PyAny>,
) -> PyResult<()> {
    let py: Python<'py> = plan.py();
    let grouped: RecordsByLocation<'py> = eligible_records(plan, successful_models, run_id)?;
    let record_count: usize = grouped.iter().map(|(_, records)| records.len()).sum();
    if record_count == 0 {
        return Ok(());
    }
    let run_scope: Bound<'py, PyAny> = host::entry(py, ExecutorHost::RunScope)?.call1((run_id,))?;
    within(&run_scope, |_| {
        let metadata: Bound<'py, PyDict> = PyDict::new(py);
        metadata.set_item("item_count", record_count)?;
        let attributes: Bound<'py, PyAny> = call_with(
            &host::entry(py, ExecutorHost::OperationAttributes)?,
            &[
                ("phase", PyString::new(py, "publish").into_any()),
                ("target_kind", PyString::new(py, "state_batch").into_any()),
            ],
        )?;
        let lifecycle: Bound<'py, PyAny> = call_with(
            &host::entry(py, ExecutorHost::OperationLifecycle)?,
            &[
                ("operation_kind", PyString::new(py, "freshness").into_any()),
                (
                    "operation_name",
                    PyString::new(py, "source_freshness_publication").into_any(),
                ),
                ("metadata", metadata.into_any()),
                ("attributes", attributes),
            ],
        )?;
        within(&lifecycle, |_| {
            let connection: Bound<'py, PyAny> =
                adapter.call_method1("connect", (connection_config,))?;
            let written: PyResult<()> = write_records(adapter, &connection, &grouped);
            let closed: PyResult<Bound<'py, PyAny>> = adapter.call_method1("close", (&connection,));
            written?;
            closed?;
            Ok(())
        })
    })
}

/// Append observed source freshness only after the affected selected models succeed.
#[pyfunction]
#[pyo3(signature = (**options))]
fn append_eligible_direct_source_freshness_records<'py>(
    py: Python<'py>,
    options: Option<Bound<'py, PyDict>>,
) -> PyResult<()> {
    let arguments: KeywordArguments<'py> = KeywordArguments::read(
        py,
        "append_eligible_direct_source_freshness_records",
        options,
        (
            &["plan", "result", "adapter", "connection_config", "run_id"],
            &[],
        ),
    )?;
    append_freshness_records(
        (
            &arguments.value("plan")?,
            &successful_model_names(&arguments.value("result")?)?,
        ),
        &arguments.value("adapter")?,
        &arguments.value("connection_config")?,
        &arguments.value("run_id")?,
    )
}

pub(crate) fn register(module: &Bound<'_, PyModule>) -> PyResult<()> {
    module.add_function(wrap_pyfunction!(
        append_eligible_direct_source_freshness_records,
        module
    )?)
}
