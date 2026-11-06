//! The `assets` entries of execution-result documents.

use pyo3::prelude::{Bound, PyAny, PyAnyMethods, PyResult, Python};
use pyo3::types::{PyDict, PyDictMethods, PyList};
use sqlbuild_host::models::{
    ExecutionStatus, FunctionExecutionResult, LifeCycleEvent, LifeCycleEventKind,
    LoadExecutionResult, ModelExecutionResult, SeedExecutionResult,
};
use sqlbuild_host::types::ResultEnum;

use crate::bindings::_helpers::plan_output::safety::{future_cursor_safety, maximum_start_safety};
use crate::bindings::_helpers::plan_output::values::{any, dict, items};
use crate::bindings::_helpers::records::fields::field as native_field;
use crate::bindings::_helpers::records::fields::mapping_get;
use crate::bindings::_helpers::records::fields::mapping_items;
use crate::bindings::_helpers::result_output::projection::{Projection, rounded};

/// A dictionary of the entries whose value is not `None`, in order.
pub(crate) fn without_none<'py>(
    py: Python<'py>,
    entries: Vec<(&str, Bound<'py, PyAny>)>,
) -> PyResult<Bound<'py, PyDict>> {
    dict(
        py,
        entries
            .into_iter()
            .filter(|(_, value)| !value.is_none())
            .collect(),
    )
}

/// `value or None`.
pub(crate) fn or_none<'py>(value: Bound<'py, PyAny>) -> PyResult<Bound<'py, PyAny>> {
    if value.is_truthy()? {
        return Ok(value);
    }
    Ok(value.py().None().into_bound(value.py()))
}

/// The qualified names of a plan's locations, or nothing without a plan.
fn targets<'py>(
    plan: Option<&Bound<'py, PyAny>>,
    field: &str,
) -> PyResult<Option<Bound<'py, PyAny>>> {
    let Some(plan) = plan else {
        return Ok(None);
    };
    let py: Python<'py> = plan.py();
    let names: Bound<'py, PyDict> = PyDict::new(py);
    for item in mapping_items(&native_field(plan, field)?)?.try_iter()? {
        let (name, location): (Bound<'py, PyAny>, Bound<'py, PyAny>) = item?.extract()?;
        names.set_item(name, native_field(&location, "qualified_name")?)?;
    }
    Ok(Some(names.into_any()))
}

/// `targets.get(name)`, or `None` without a plan.
fn target<'py>(
    py: Python<'py>,
    targets: Option<&Bound<'py, PyAny>>,
    name: &Bound<'py, PyAny>,
) -> PyResult<Bound<'py, PyAny>> {
    match targets {
        Some(targets) => mapping_get(targets, name),
        None => Ok(py.None().into_bound(py)),
    }
}

fn last_recorded_sql(status: ExecutionStatus, events: &[LifeCycleEvent]) -> Option<&str> {
    if status != ExecutionStatus::Failed {
        return None;
    }
    events
        .iter()
        .rev()
        .find(|event| event.kind == LifeCycleEventKind::Sql)
        .map(|event| event.content.as_str())
}

/// Shared run type and bounded accounting summary, without per-interval evidence.
pub(crate) fn microbatch_summary<'py>(
    py: Python<'py>,
    result: &ModelExecutionResult<Bound<'py, PyAny>>,
) -> PyResult<Bound<'py, PyAny>> {
    let Some(run_type) = &result.microbatch_run_type else {
        return Ok(py.None().into_bound(py));
    };
    let state: Bound<'py, PyDict> = dict(py, vec![("run_type", any(py, run_type)?)])?;
    for (key, value) in [
        ("strategy", &result.microbatch_strategy),
        ("reason", &result.microbatch_plan_reason),
    ] {
        if let Some(value) = value {
            state.set_item(key, value)?;
        }
    }
    for (key, value) in [
        ("batch_count", any(py, result.batch_count)?),
        ("batch_size", any(py, &result.batch_size)?),
        (
            "recovery_batch_count",
            any(py, result.microbatch_recovery_batch_count)?,
        ),
        (
            "known_gap_count",
            any(py, result.microbatch_known_gap_count)?,
        ),
        (
            "unaccounted_interval_count",
            any(py, result.microbatch_unaccounted_interval_count)?,
        ),
        (
            "synthetic_completion_count",
            any(py, result.microbatch_synthetic_completion_count)?,
        ),
        (
            "unknown_fingerprint_count",
            any(py, result.microbatch_unknown_fingerprint_count)?,
        ),
        (
            "contiguous_frontier",
            any(py, &result.microbatch_contiguous_frontier)?,
        ),
        (
            "unaccounted_partition_policy",
            any(py, &result.microbatch_unaccounted_partition_policy)?,
        ),
        (
            "replay_requirement_id",
            any(py, &result.microbatch_replay_requirement_id)?,
        ),
        (
            "required_model_version_hash",
            any(py, &result.microbatch_required_model_version_hash)?,
        ),
        (
            "physical_generation_id",
            any(py, &result.microbatch_physical_generation_id)?,
        ),
        (
            "concurrent_enabled",
            any(py, result.microbatch_concurrent_enabled)?,
        ),
        (
            "batch_concurrency",
            any(py, result.microbatch_batch_concurrency)?,
        ),
        (
            "global_concurrency",
            any(py, result.microbatch_global_concurrency)?,
        ),
        (
            "replay_requirement_state",
            any(py, &result.microbatch_replay_requirement_state)?,
        ),
    ] {
        state.set_item(key, value)?;
    }
    Ok(state.into_any())
}

/// The run type and accounting of a microbatch model result, including interval evidence.
fn microbatch<'py>(
    py: Python<'py>,
    result: &ModelExecutionResult<Bound<'py, PyAny>>,
) -> PyResult<Bound<'py, PyAny>> {
    let summary: Bound<'py, PyAny> = microbatch_summary(py, result)?;
    if summary.is_none() {
        return Ok(summary);
    }
    let state: &Bound<'py, PyDict> = summary.downcast()?;
    let intervals: Bound<'py, PyList> = PyList::empty(py);
    for interval in &result.microbatch_accounting_intervals {
        pyo3::types::PyListMethods::append(
            &intervals,
            dict(
                py,
                vec![
                    ("partition_start", any(py, &interval.partition_start)?),
                    ("partition_end", any(py, &interval.partition_end)?),
                    ("accounting_status", any(py, &interval.accounting_status)?),
                    ("fingerprint_status", any(py, &interval.fingerprint_status)?),
                    ("model_version_hash", any(py, &interval.model_version_hash)?),
                    ("completion_type", any(py, &interval.completion_type)?),
                    ("event_id", any(py, &interval.event_id)?),
                ],
            )?,
        )?;
    }
    state.set_item("intervals", intervals)?;
    if let Some(limit) = result.microbatch_limit {
        state.set_item("limit", limit)?;
        state.set_item("count", result.microbatch_limit_count)?;
        state.set_item(
            "action",
            result.microbatch_limit_action.map(ResultEnum::value),
        )?;
        state.set_item("warning", &result.microbatch_limit_warning)?;
    }
    Ok(summary)
}

fn value<'py>(result: &Bound<'py, PyAny>, field: &str) -> PyResult<Bound<'py, PyAny>> {
    native_field(&native_field(result, field)?, "value")
}

/// `<field>.value` when it is set.
fn optional_value<'py>(result: &Bound<'py, PyAny>, field: &str) -> PyResult<Bound<'py, PyAny>> {
    let value: Bound<'py, PyAny> = native_field(result, field)?;
    if !value.is_truthy()? {
        return Ok(value.py().None().into_bound(value.py()));
    }
    native_field(&value, "value")
}

fn model_asset<'py>(
    projection: &Projection<'py>,
    result: &ModelExecutionResult<Bound<'py, PyAny>>,
    targets: Option<&Bound<'py, PyAny>>,
) -> PyResult<Bound<'py, PyDict>> {
    let py: Python<'py> = projection.py;
    let name: Bound<'py, PyAny> = any(py, &result.model_name)?;
    let planned: Bound<'py, PyAny> = target(py, targets, &name)?;
    let empty_inputs: Bound<'py, PyAny> = any(py, &result.empty_cursor_inputs)?;
    without_none(
        py,
        vec![
            ("kind", any(py, "model")?),
            ("name", name),
            ("status", any(py, result.status.value())?),
            (
                "duration_ms",
                projection.result_duration(result, &any(py, result.duration_ms)?)?,
            ),
            (
                "target",
                if planned.is_truthy()? {
                    planned
                } else {
                    any(py, &result.promoted_relation)?
                },
            ),
            ("staging_relation", any(py, &result.staging_relation)?),
            (
                "failed_phase",
                any(py, result.failed_phase.map(ResultEnum::value))?,
            ),
            (
                "skip_mode",
                any(py, result.skip_mode.map(ResultEnum::value))?,
            ),
            ("skip_reason", any(py, &result.skip_reason)?),
            ("error_code", any(py, &result.error_code)?),
            ("error_help", any(py, &result.error_help)?),
            ("error_message", any(py, &result.error_message)?),
            ("failed_sql", any(py, &result.failed_sql)?),
            (
                "last_recorded_sql",
                any(
                    py,
                    last_recorded_sql(result.status, &result.lifecycle_events),
                )?,
            ),
            ("warnings", any(py, &result.warning_messages)?),
            ("empty_cursor_inputs", or_none(empty_inputs)?),
            (
                "waiting_on_empty_inputs",
                or_none(any(py, result.waiting_on_empty_inputs)?)?,
            ),
            (
                "future_cursor_safety",
                future_cursor_safety(&any(py, &result.future_cursor_safety)?)?,
            ),
            (
                "maximum_start_safety",
                maximum_start_safety(&any(py, &result.maximum_start_safety)?)?,
            ),
            ("microbatch", microbatch(py, result)?),
        ],
    )
}

/// Model result assets, with plan targets when a plan is given.
pub(crate) fn model_assets<'py>(
    projection: &Projection<'py>,
    results: &[&ModelExecutionResult<Bound<'py, PyAny>>],
    plan: Option<&Bound<'py, PyAny>>,
) -> PyResult<Vec<Bound<'py, PyDict>>> {
    let targets: Option<Bound<'py, PyAny>> = targets(plan, "model_locations")?;
    let mut assets: Vec<Bound<'py, PyDict>> = Vec::new();
    for result in results {
        if projection.terminal_recorded(result)? {
            assets.push(model_asset(projection, result, targets.as_ref())?);
        }
    }
    Ok(assets)
}

/// Seed result assets, with plan targets and reasons when a plan is given.
pub(crate) fn seed_assets<'py>(
    projection: &Projection<'py>,
    results: &[&SeedExecutionResult],
    plan: Option<&Bound<'py, PyAny>>,
) -> PyResult<Vec<Bound<'py, PyDict>>> {
    let targets: Option<Bound<'py, PyAny>> = targets(plan, "seed_locations")?;
    let reasons: Option<Bound<'py, PyAny>> = match plan {
        Some(plan) => {
            let reasons: Bound<'py, PyDict> = PyDict::new(plan.py());
            for entry in items(&native_field(plan, "seed_entries")?)? {
                reasons.set_item(native_field(&entry, "name")?, value(&entry, "reason")?)?;
            }
            Some(reasons.into_any())
        }
        None => None,
    };
    let mut assets: Vec<Bound<'py, PyDict>> = Vec::new();
    for result in results {
        if !projection.terminal_recorded(result)? {
            continue;
        }
        let py: Python<'py> = projection.py;
        let name: Bound<'py, PyAny> = any(py, &result.seed_name)?;
        assets.push(without_none(
            py,
            vec![
                ("kind", any(py, "seed")?),
                ("name", name.clone()),
                ("status", any(py, result.status.value())?),
                (
                    "duration_ms",
                    projection.result_duration(result, &any(py, result.duration_ms)?)?,
                ),
                ("target", target(py, targets.as_ref(), &name)?),
                ("reason", target(py, reasons.as_ref(), &name)?),
                ("error_code", any(py, &result.error_code)?),
                ("error_help", any(py, &result.error_help)?),
                ("error_message", any(py, &result.error_message)?),
            ],
        )?);
    }
    Ok(assets)
}

/// Function result assets, with plan targets when a plan is given.
pub(crate) fn function_assets<'py>(
    projection: &Projection<'py>,
    results: &[&FunctionExecutionResult],
    plan: Option<&Bound<'py, PyAny>>,
) -> PyResult<Vec<Bound<'py, PyDict>>> {
    let targets: Option<Bound<'py, PyAny>> = targets(plan, "function_locations")?;
    let mut assets: Vec<Bound<'py, PyDict>> = Vec::new();
    for result in results {
        if !projection.terminal_recorded(result)? {
            continue;
        }
        let py: Python<'py> = projection.py;
        let name: Bound<'py, PyAny> = any(py, &result.function_name)?;
        assets.push(without_none(
            py,
            vec![
                ("kind", any(py, &result.function_kind)?),
                ("name", name.clone()),
                ("status", any(py, result.status.value())?),
                (
                    "duration_ms",
                    projection.result_duration(result, &any(py, result.duration_ms)?)?,
                ),
                ("target", target(py, targets.as_ref(), &name)?),
                ("error_code", any(py, &result.error_code)?),
                ("error_help", any(py, &result.error_help)?),
                ("error_message", any(py, &result.error_message)?),
                ("warnings", any(py, &result.warning_messages)?),
            ],
        )?);
    }
    Ok(assets)
}

/// Source and loader load result assets.
pub(crate) fn load_assets<'py>(
    projection: &Projection<'py>,
    results: &[&LoadExecutionResult<Bound<'py, PyAny>>],
) -> PyResult<Vec<Bound<'py, PyDict>>> {
    let mut assets: Vec<Bound<'py, PyDict>> = Vec::new();
    for result in results {
        if !projection.terminal_recorded(result)? {
            continue;
        }
        let py: Python<'py> = projection.py;
        assets.push(without_none(
            py,
            vec![
                ("kind", any(py, result.resource_kind.value())?),
                ("name", any(py, &result.source_name)?),
                ("status", any(py, result.status.value())?),
                (
                    "duration_ms",
                    projection.result_duration(result, &any(py, result.duration_ms)?)?,
                ),
                ("target", any(py, &result.target)?),
                ("staging_relation", any(py, &result.staging_relation)?),
                ("loader", any(py, &result.loader_name)?),
                ("rows_loaded", any(py, result.rows_loaded)?),
                (
                    "skip_mode",
                    any(py, result.skip_mode.map(ResultEnum::value))?,
                ),
                ("skip_reason", any(py, &result.skip_reason)?),
                ("error_message", any(py, &result.error_message)?),
                ("warnings", or_none(any(py, &result.warning_messages)?)?),
            ],
        )?);
    }
    Ok(assets)
}

/// Python node result assets.
pub(crate) fn python_node_assets<'py>(
    projection: &Projection<'py>,
    results: &[Bound<'py, PyAny>],
) -> PyResult<Vec<Bound<'py, PyDict>>> {
    let mut assets: Vec<Bound<'py, PyDict>> = Vec::new();
    for result in results {
        if !projection.terminal_recorded(result)? {
            continue;
        }
        let py: Python<'py> = result.py();
        assets.push(without_none(
            py,
            vec![
                ("kind", value(result, "kind")?),
                ("name", native_field(result, "node_name")?),
                ("status", value(result, "status")?),
                ("metadata", native_field(result, "metadata")?),
                ("materialized", native_field(result, "materialized")?),
                ("skip_mode", optional_value(result, "skip_mode")?),
                ("skip_reason", native_field(result, "skip_reason")?),
                ("error_message", native_field(result, "error_message")?),
                (
                    "warnings",
                    or_none(native_field(result, "warning_messages")?)?,
                ),
            ],
        )?);
    }
    Ok(assets)
}

/// One clone item asset under its resource type.
pub(crate) fn clone_asset<'py>(
    projection: &Projection<'py>,
    item: &Bound<'py, PyAny>,
    resource_type: &str,
) -> PyResult<Bound<'py, PyDict>> {
    let py: Python<'py> = item.py();
    let name: String = native_field(item, "name")?.extract()?;
    let seconds: Bound<'py, PyAny> = native_field(item, "duration_seconds")?;
    let fallback: Option<f64> = if seconds.is_none() {
        None
    } else {
        Some(seconds.extract::<f64>()? * 1000.0)
    };
    let duration: Option<f64> =
        projection.resource_duration((&name, &format!("{resource_type}:{name}")), fallback)?;
    without_none(
        py,
        vec![
            ("kind", any(py, resource_type)?),
            ("name", any(py, name)?),
            ("status", value(item, "status")?),
            ("action", value(item, "action")?),
            (
                "duration_ms",
                match duration {
                    Some(duration) => rounded(py, duration)?,
                    None => py.None().into_bound(py),
                },
            ),
            ("origin_relation", native_field(item, "origin_relation")?),
            ("target", native_field(item, "destination_relation")?),
            ("message", native_field(item, "message")?),
        ],
    )
}
