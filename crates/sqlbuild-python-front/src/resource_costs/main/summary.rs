//! A `RunCostSummary` as a plain cost summary.

use crate::resource_costs::_helpers::cost_run_records::decimal;
use crate::resource_costs::_helpers::cost_run_records::integer;
use crate::resource_costs::_helpers::cost_run_records::optional_text;
use crate::resource_costs::_helpers::cost_run_records::resource;
use crate::resource_costs::_helpers::cost_run_records::text;
use pyo3::prelude::{Bound, PyAny, PyAnyMethods, PyResult};
use sqlbuild_core::cost_report::models::{CostResource, CostSummary};
use sqlbuild_python_base::records::main::fields::field::field as native_field;

/// A `RunCostSummary` as a plain cost summary.
pub fn summary(value: &Bound<'_, PyAny>) -> PyResult<CostSummary> {
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
