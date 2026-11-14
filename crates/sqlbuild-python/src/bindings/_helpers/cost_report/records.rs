//! `CostRunRecord` objects read into plain cost runs, and plain runs built back into records.

use pyo3::prelude::{Bound, PyAny, PyAnyMethods, PyErr, PyResult, Python};
use pyo3::types::{PyDict, PyDictMethods, PyTuple};
use sqlbuild_core::cost_report::errors::CostArtifactError;
use sqlbuild_core::cost_report::models::{CostResource, CostRun, CostSummary};
use sqlbuild_host::entries::models::ExecutorHost;

use crate::bindings::_helpers::boundary::python_calls::imported;
use crate::bindings::_helpers::native_host::host;
use crate::bindings::_helpers::records::construct::new_record;
use crate::bindings::_helpers::records::fields::field as native_field;
use crate::bindings::_helpers::records::ids::{NativeAttr, RecordId};
use crate::bindings::_helpers::records::registry;

/// The `CostArtifactError` Python raised for the same artifact.
pub(crate) fn artifact_error(py: Python<'_>, error: &CostArtifactError) -> PyErr {
    match registry::native(py, NativeAttr::CostArtifactError)
        .and_then(|class| class.call1((error.message.as_str(),)))
    {
        Ok(instance) => PyErr::from_value(instance),
        Err(failure) => failure,
    }
}

fn text(value: &Bound<'_, PyAny>, field: &str) -> PyResult<String> {
    native_field(value, field)?.extract()
}

fn optional_text(value: &Bound<'_, PyAny>, field: &str) -> PyResult<Option<String>> {
    native_field(value, field)?.extract()
}

/// `str(value.<field>)`, the text a `Decimal` field is stored as.
fn decimal(value: &Bound<'_, PyAny>, field: &str) -> PyResult<String> {
    Ok(native_field(value, field)?.str()?.to_string())
}

fn integer(value: &Bound<'_, PyAny>, field: &str) -> PyResult<i64> {
    native_field(value, field)?.extract()
}

fn timestamp(value: &Bound<'_, PyAny>, field: &str) -> PyResult<String> {
    native_field(value, field)?
        .call_method0("isoformat")?
        .extract()
}

fn resource(value: &Bound<'_, PyAny>) -> PyResult<CostResource> {
    Ok(CostResource {
        resource_type: text(value, "resource_type")?,
        resource_name: text(value, "resource_name")?,
        warehouse_name: text(value, "warehouse_name")?,
        warehouse_size: text(value, "warehouse_size")?,
        warehouse_type: optional_text(value, "warehouse_type")?,
        cluster_number: native_field(value, "cluster_number")?.extract()?,
        query_count: integer(value, "query_count")?,
        attributed_seconds: decimal(value, "attributed_seconds")?,
        bytes_scanned: integer(value, "bytes_scanned")?,
        estimated_compute_credits: decimal(value, "estimated_compute_credits")?,
        estimated_usd: decimal(value, "estimated_usd")?,
    })
}

pub(crate) fn summary(value: &Bound<'_, PyAny>) -> PyResult<CostSummary> {
    let mut resources: Vec<CostResource> = Vec::new();
    for item in native_field(value, "resources")?.try_iter()? {
        resources.push(resource(&item?)?);
    }
    let mut limitations: Vec<String> = Vec::new();
    for item in native_field(value, "limitations")?.try_iter()? {
        limitations.push(item?.extract()?);
    }
    Ok(CostSummary {
        status: native_field(value, "status")?.getattr("value")?.extract()?,
        usd_per_credit: decimal(value, "usd_per_credit")?,
        rate_source: text(value, "rate_source")?,
        resources,
        query_count: integer(value, "query_count")?,
        attributed_seconds: decimal(value, "attributed_seconds")?,
        bytes_scanned: integer(value, "bytes_scanned")?,
        estimated_compute_credits: decimal(value, "estimated_compute_credits")?,
        estimated_usd: decimal(value, "estimated_usd")?,
        expected_statement_count: integer(value, "expected_statement_count")?,
        observed_statement_count: integer(value, "observed_statement_count")?,
        missing_statement_count: integer(value, "missing_statement_count")?,
        skipped_statement_count: integer(value, "skipped_statement_count")?,
        source: text(value, "source")?,
        method: text(value, "method")?,
        limitations,
        message: optional_text(value, "message")?,
    })
}

/// A `CostRunRecord` as a plain run.
pub(crate) fn cost_run(record: &Bound<'_, PyAny>) -> PyResult<CostRun> {
    Ok(CostRun {
        run_id: text(record, "run_id")?,
        adapter_name: text(record, "adapter_name")?,
        target_name: optional_text(record, "target_name")?,
        build_status: text(record, "build_status")?,
        started_at: timestamp(record, "started_at")?,
        completed_at: timestamp(record, "completed_at")?,
        cost: summary(&native_field(record, "cost")?)?,
        version: integer(record, "version")?,
        had_executable_work: native_field(record, "had_executable_work")?.extract()?,
    })
}

/// Every record of an iterable as a plain run.
pub(crate) fn cost_runs(records: &Bound<'_, PyAny>) -> PyResult<Vec<CostRun>> {
    let mut runs: Vec<CostRun> = Vec::new();
    for record in records.try_iter()? {
        runs.push(cost_run(&record?)?);
    }
    Ok(runs)
}

/// Python constructors of the record fields.
struct Builders<'py> {
    decimal: Bound<'py, PyAny>,
    timestamp: Bound<'py, PyAny>,
}

impl<'py> Builders<'py> {
    fn new(py: Python<'py>) -> PyResult<Self> {
        Ok(Self {
            decimal: imported(py, "decimal", "Decimal")?,
            timestamp: imported(py, "datetime", "datetime")?.getattr("fromisoformat")?,
        })
    }

    fn decimal(&self, text: &str) -> PyResult<Bound<'py, PyAny>> {
        self.decimal.call1((text,))
    }
}

fn resource_object<'py>(
    builders: &Builders<'py>,
    resource: &CostResource,
) -> PyResult<Bound<'py, PyAny>> {
    let py: Python<'py> = builders.decimal.py();
    let fields: Bound<'py, PyDict> = PyDict::new(py);
    fields.set_item("resource_type", &resource.resource_type)?;
    fields.set_item("resource_name", &resource.resource_name)?;
    fields.set_item("warehouse_name", &resource.warehouse_name)?;
    fields.set_item("warehouse_size", &resource.warehouse_size)?;
    fields.set_item("warehouse_type", &resource.warehouse_type)?;
    fields.set_item("cluster_number", resource.cluster_number)?;
    fields.set_item("query_count", resource.query_count)?;
    fields.set_item(
        "attributed_seconds",
        builders.decimal(&resource.attributed_seconds)?,
    )?;
    fields.set_item("bytes_scanned", resource.bytes_scanned)?;
    fields.set_item(
        "estimated_compute_credits",
        builders.decimal(&resource.estimated_compute_credits)?,
    )?;
    fields.set_item("estimated_usd", builders.decimal(&resource.estimated_usd)?)?;
    new_record(py, RecordId::ResourceCost, (), Some(&fields))
}

fn summary_object<'py>(
    builders: &Builders<'py>,
    cost: &CostSummary,
) -> PyResult<Bound<'py, PyAny>> {
    let py: Python<'py> = builders.decimal.py();
    let mut resources: Vec<Bound<'py, PyAny>> = Vec::with_capacity(cost.resources.len());
    for resource in &cost.resources {
        resources.push(resource_object(builders, resource)?);
    }
    let fields: Bound<'py, PyDict> = PyDict::new(py);
    fields.set_item(
        "status",
        host::entry(py, ExecutorHost::CostStatus)?.call1((cost.status.as_str(),))?,
    )?;
    fields.set_item("usd_per_credit", builders.decimal(&cost.usd_per_credit)?)?;
    fields.set_item("rate_source", &cost.rate_source)?;
    fields.set_item("resources", PyTuple::new(py, resources)?)?;
    fields.set_item("query_count", cost.query_count)?;
    fields.set_item(
        "attributed_seconds",
        builders.decimal(&cost.attributed_seconds)?,
    )?;
    fields.set_item("bytes_scanned", cost.bytes_scanned)?;
    fields.set_item(
        "estimated_compute_credits",
        builders.decimal(&cost.estimated_compute_credits)?,
    )?;
    fields.set_item("estimated_usd", builders.decimal(&cost.estimated_usd)?)?;
    fields.set_item("expected_statement_count", cost.expected_statement_count)?;
    fields.set_item("observed_statement_count", cost.observed_statement_count)?;
    fields.set_item("missing_statement_count", cost.missing_statement_count)?;
    fields.set_item("skipped_statement_count", cost.skipped_statement_count)?;
    fields.set_item("source", &cost.source)?;
    fields.set_item("method", &cost.method)?;
    fields.set_item("limitations", PyTuple::new(py, &cost.limitations)?)?;
    fields.set_item("message", &cost.message)?;
    host::entry(py, ExecutorHost::RunCostSummary)?.call((), Some(&fields))
}

/// A plain run as the `CostRunRecord` it was read from.
pub(crate) fn record_object<'py>(py: Python<'py>, run: &CostRun) -> PyResult<Bound<'py, PyAny>> {
    let builders: Builders<'py> = Builders::new(py)?;
    let fields: Bound<'py, PyDict> = PyDict::new(py);
    fields.set_item("version", run.version)?;
    fields.set_item("run_id", &run.run_id)?;
    fields.set_item("adapter_name", &run.adapter_name)?;
    fields.set_item("target_name", &run.target_name)?;
    fields.set_item("build_status", &run.build_status)?;
    fields.set_item(
        "started_at",
        builders.timestamp.call1((run.started_at.as_str(),))?,
    )?;
    fields.set_item(
        "completed_at",
        builders.timestamp.call1((run.completed_at.as_str(),))?,
    )?;
    fields.set_item("cost", summary_object(&builders, &run.cost)?)?;
    fields.set_item("had_executable_work", run.had_executable_work)?;
    new_record(py, RecordId::CostRunRecord, (), Some(&fields))
}

/// Records of several plain runs, as a tuple.
pub(crate) fn record_tuple<'py>(
    py: Python<'py>,
    runs: &[CostRun],
) -> PyResult<Bound<'py, PyTuple>> {
    let mut records: Vec<Bound<'py, PyAny>> = Vec::with_capacity(runs.len());
    for run in runs {
        records.push(record_object(py, run)?);
    }
    PyTuple::new(py, records)
}
