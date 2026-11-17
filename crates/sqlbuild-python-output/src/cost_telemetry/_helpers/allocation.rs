//! `allocate_run_cost`: price one run's share of observed Snowflake warehouse busy time.

use pyo3::IntoPyObjectExt;
use pyo3::prelude::{Bound, PyAny, PyAnyMethods, PyResult, Python};
use pyo3::pyfunction;
use pyo3::types::{PyDict, PyTuple};
use sqlbuild_core::cost_allocation::main::allocate_run_cost::allocate_run_cost as allocate;
use sqlbuild_core::cost_allocation::models::{
    AllocationStatus, CostQuery, PyDecimal, ResourceAllocation, RunAllocation, WarehouseKey,
};
use sqlbuild_host::entries::models::ExecutorHost;

use sqlbuild_python_base::boundary::main::call_with::call_with;
use sqlbuild_python_base::boundary::models::KeywordArguments;
use sqlbuild_python_base::native_host::main::host_entry::host_entry;
use sqlbuild_python_base::records::main::construct::new_record_with::new_record_with;
use sqlbuild_python_base::records::main::fields::field::field as native_field;
use sqlbuild_python_base::records::types::RecordId;
use sqlbuild_python_front::cost_context::main::instant_micros::instant_micros;
use sqlbuild_python_front::cost_context::main::native_decimal::native_decimal;
use sqlbuild_python_front::cost_context::main::python_decimal::python_decimal;

const PENDING_MESSAGE: &str = "Snowflake query history is not complete yet.";

/// One `QueryCostObservation` as a native query, in the run when its run ID matches.
pub(crate) fn cost_query(observation: &Bound<'_, PyAny>, run_id: &str) -> PyResult<CostQuery> {
    let text =
        |name: &str| -> PyResult<Option<String>> { native_field(observation, name)?.extract() };
    Ok(CostQuery {
        query_id: native_field(observation, "query_id")?.extract()?,
        warehouse: WarehouseKey {
            name: native_field(observation, "warehouse_name")?.extract()?,
            size: native_field(observation, "warehouse_size")?.extract()?,
            kind: text("warehouse_type")?,
            cluster: native_field(observation, "cluster_number")?.extract()?,
        },
        started_micros: instant_micros(&native_field(observation, "started_at")?)?,
        completed_micros: instant_micros(&native_field(observation, "completed_at")?)?,
        in_run: text("run_id")?.as_deref() == Some(run_id),
        resource_type: text("resource_type")?,
        resource_name: text("resource_name")?,
        bytes_scanned: native_field(observation, "bytes_scanned")?.extract()?,
    })
}

/// The `CostStatus` member of a native allocation status.
pub(crate) fn status_member(status: AllocationStatus) -> &'static str {
    match status {
        AllocationStatus::Pending => "PENDING",
        AllocationStatus::Partial => "PARTIAL",
        AllocationStatus::Complete => "COMPLETE",
    }
}

/// Summary fields that collection derives on top of an allocation.
pub(crate) struct SummaryState {
    pub(crate) status: &'static str,
    pub(crate) message: Option<String>,
    pub(crate) limitations: Vec<String>,
    /// Expected, observed, missing and skipped statement counts.
    pub(crate) counts: [usize; 4],
}

fn resource_cost<'py>(
    py: Python<'py>,
    resource: &ResourceAllocation,
) -> PyResult<Bound<'py, PyAny>> {
    let decimal = |value: &PyDecimal| python_decimal(py, value);
    new_record_with(
        py,
        RecordId::ResourceCost,
        &[
            (
                "resource_type",
                resource.resource_type.clone().into_bound_py_any(py)?,
            ),
            (
                "resource_name",
                resource.resource_name.clone().into_bound_py_any(py)?,
            ),
            (
                "warehouse_name",
                resource.warehouse.name.clone().into_bound_py_any(py)?,
            ),
            (
                "warehouse_size",
                resource.warehouse.size.clone().into_bound_py_any(py)?,
            ),
            (
                "warehouse_type",
                resource.warehouse.kind.clone().into_bound_py_any(py)?,
            ),
            (
                "cluster_number",
                resource.warehouse.cluster.into_bound_py_any(py)?,
            ),
            ("query_count", resource.query_count.into_bound_py_any(py)?),
            ("attributed_seconds", decimal(&resource.attributed_seconds)?),
            (
                "bytes_scanned",
                resource.bytes_scanned.into_bound_py_any(py)?,
            ),
            (
                "estimated_compute_credits",
                decimal(&resource.estimated_compute_credits)?,
            ),
            ("estimated_usd", decimal(&resource.estimated_usd)?),
        ],
    )
}

/// A `RunCostSummary`; without an allocation its totals keep their defaults.
pub(crate) fn summary_object<'py>(
    allocation: Option<&RunAllocation>,
    state: &SummaryState,
    (usd_per_credit, rate_source): (&Bound<'py, PyAny>, &Bound<'py, PyAny>),
) -> PyResult<Bound<'py, PyAny>> {
    let py: Python<'py> = usd_per_credit.py();
    let [expected, observed, missing, skipped] = state.counts;
    let mut fields: Vec<(&str, Bound<'py, PyAny>)> = vec![
        (
            "status",
            host_entry(py, ExecutorHost::CostStatus)?.getattr(state.status)?,
        ),
        ("usd_per_credit", usd_per_credit.clone()),
        ("rate_source", rate_source.clone()),
        ("message", state.message.clone().into_bound_py_any(py)?),
        (
            "limitations",
            PyTuple::new(py, &state.limitations)?.into_any(),
        ),
        ("expected_statement_count", expected.into_bound_py_any(py)?),
        ("observed_statement_count", observed.into_bound_py_any(py)?),
        ("missing_statement_count", missing.into_bound_py_any(py)?),
        ("skipped_statement_count", skipped.into_bound_py_any(py)?),
    ];
    if let Some(allocation) = allocation {
        let resources: Vec<Bound<'py, PyAny>> = allocation
            .resources
            .iter()
            .map(|resource| resource_cost(py, resource))
            .collect::<PyResult<_>>()?;
        fields.extend([
            ("resources", PyTuple::new(py, resources)?.into_any()),
            ("query_count", allocation.query_count.into_bound_py_any(py)?),
            (
                "attributed_seconds",
                python_decimal(py, &allocation.attributed_seconds)?,
            ),
            (
                "bytes_scanned",
                allocation.bytes_scanned.into_bound_py_any(py)?,
            ),
            (
                "estimated_compute_credits",
                python_decimal(py, &allocation.estimated_compute_credits)?,
            ),
            (
                "estimated_usd",
                python_decimal(py, &allocation.estimated_usd)?,
            ),
        ]);
    }
    call_with(&host_entry(py, ExecutorHost::RunCostSummary)?, &fields)
}

/// The allocation that prices queries, or `None` while the run's history is pending.
pub(crate) fn priced(allocation: RunAllocation) -> (Option<RunAllocation>, SummaryState) {
    let pending: bool = allocation.status == AllocationStatus::Pending;
    let state: SummaryState = SummaryState {
        status: status_member(allocation.status),
        message: pending.then(|| PENDING_MESSAGE.to_owned()),
        limitations: allocation.limitations.clone(),
        counts: [0; 4],
    };
    ((!pending).then_some(allocation), state)
}

/// Native queries and the allocation of one run over observations.
pub(crate) fn allocate_observations(
    observations: &[Bound<'_, PyAny>],
    (run_id, usd_per_credit): (&str, &Bound<'_, PyAny>),
    result_limit_reached: bool,
) -> PyResult<RunAllocation> {
    let queries: Vec<CostQuery> = observations
        .iter()
        .map(|observation| cost_query(observation, run_id))
        .collect::<PyResult<_>>()?;
    Ok(allocate(
        &queries,
        &native_decimal(usd_per_credit)?,
        result_limit_reached,
    ))
}

/// Concurrency-safe Snowflake busy-compute attribution for one run.
#[pyfunction]
#[pyo3(signature = (**options))]
pub(crate) fn allocate_run_cost<'py>(
    py: Python<'py>,
    options: Option<Bound<'py, PyDict>>,
) -> PyResult<Bound<'py, PyAny>> {
    let arguments: KeywordArguments<'py> = KeywordArguments::read(
        py,
        "allocate_run_cost",
        options,
        (
            &["observations", "run_id", "usd_per_credit", "rate_source"],
            &["result_limit_reached"],
        ),
    )?;
    let observations: Vec<Bound<'py, PyAny>> = arguments
        .value("observations")?
        .try_iter()?
        .collect::<PyResult<_>>()?;
    let run_id: String = arguments.value("run_id")?.extract()?;
    let usd_per_credit: Bound<'py, PyAny> = arguments.value("usd_per_credit")?;
    let allocation: RunAllocation = allocate_observations(
        &observations,
        (&run_id, &usd_per_credit),
        arguments.value("result_limit_reached")?.is_truthy()?,
    )?;
    let (allocation, state) = priced(allocation);
    summary_object(
        allocation.as_ref(),
        &state,
        (&usd_per_credit, &arguments.value("rate_source")?),
    )
}
