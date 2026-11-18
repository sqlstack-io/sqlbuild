//! Plain cost runs read from `CostRunRecord` objects, and their fields built back into Python values.

use pyo3::prelude::{Bound, PyAny, PyAnyMethods, PyResult, Python};
use pyo3::types::{PyDict, PyDictMethods, PyTuple};
use sqlbuild_core::cost_report::models::{CostResource, CostSummary};
use sqlbuild_host::entries::models::ExecutorHost;
use sqlbuild_python_base::boundary::main::imported::imported;
use sqlbuild_python_base::native_host::main::host_entry::host_entry;
use sqlbuild_python_base::records::main::construct::new_record::new_record;
use sqlbuild_python_base::records::main::fields::field::field as native_field;
use sqlbuild_python_base::records::types::RecordId;

pub(crate) fn text(value: &Bound<'_, PyAny>, field: &str) -> PyResult<String> {
    native_field(value, field)?.extract()
}

pub(crate) fn optional_text(value: &Bound<'_, PyAny>, field: &str) -> PyResult<Option<String>> {
    native_field(value, field)?.extract()
}

/// `str(value.<field>)`, the text a `Decimal` field is stored as.
pub(crate) fn decimal(value: &Bound<'_, PyAny>, field: &str) -> PyResult<String> {
    Ok(native_field(value, field)?.str()?.to_string())
}

pub(crate) fn integer(value: &Bound<'_, PyAny>, field: &str) -> PyResult<i64> {
    native_field(value, field)?.extract()
}

pub(crate) fn timestamp(value: &Bound<'_, PyAny>, field: &str) -> PyResult<String> {
    native_field(value, field)?
        .call_method0("isoformat")?
        .extract()
}

pub(crate) fn resource(value: &Bound<'_, PyAny>) -> PyResult<CostResource> {
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

/// Python constructors of the record fields.
pub(crate) struct Builders<'py> {
    pub(crate) decimal: Bound<'py, PyAny>,
    pub(crate) timestamp: Bound<'py, PyAny>,
}

impl<'py> Builders<'py> {
    pub(crate) fn new(py: Python<'py>) -> PyResult<Self> {
        Ok(Self {
            decimal: imported(py, "decimal", "Decimal")?,
            timestamp: imported(py, "datetime", "datetime")?.getattr("fromisoformat")?,
        })
    }

    pub(crate) fn decimal(&self, text: &str) -> PyResult<Bound<'py, PyAny>> {
        self.decimal.call1((text,))
    }
}

pub(crate) fn resource_object<'py>(
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

pub(crate) fn summary_object<'py>(
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
        host_entry(py, ExecutorHost::CostStatus)?.call1((cost.status.as_str(),))?,
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
    host_entry(py, ExecutorHost::RunCostSummary)?.call((), Some(&fields))
}
