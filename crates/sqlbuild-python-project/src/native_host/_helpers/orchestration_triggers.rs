//! Authored trigger controls and bounded calendars evaluated only in the location interpreter.

use std::collections::BTreeSet;

use chrono::{DateTime, TimeDelta, Utc};
use chrono_tz::Tz;
use pyo3::exceptions::PyValueError;
use pyo3::prelude::{Bound, PyAny, PyAnyMethods, PyResult};
use pyo3::types::{PyDict, PyDictMethods};
use serde_json::{Value, json};
use sqlbuild_analysis::graph::models::ProjectGraph;
use sqlbuild_cursor::algebra::main::parse_duration::parse_duration;
use sqlbuild_orchestration::declarations::models::Schedule;
use sqlbuild_orchestration::scheduler::constants::MAX_TICKS_PER_BATCH;

use crate::graph::models::ProjectSelection;
use sqlbuild_python_base::boundary::main::imported::imported;
use sqlbuild_python_base::native_host::constants::CALENDAR_WINDOW_PARAMETER;
use sqlbuild_python_base::native_host::constants::SOURCE_NEW_DATA_CONDITION;
use sqlbuild_python_base::native_host::constants::SOURCE_STALE_CONDITION;
use sqlbuild_python_front::discovery::main::python_files::provider_usages::provider_usages;

fn horizon(value: &str) -> PyResult<TimeDelta> {
    let invalid = || {
        PyValueError::new_err(
            "calendar horizon must be a positive fixed duration using d, h, m or s, within datetime range",
        )
    };
    let digits = parse_duration(value).ok_or_else(invalid)?;
    if [&digits.years, &digits.months]
        .iter()
        .any(|digits| !digits.trim_start_matches('0').is_empty())
    {
        return Err(invalid());
    }
    let seconds = [
        (digits.days, 86_400_i64),
        (digits.hours, 3_600),
        (digits.minutes, 60),
        (digits.seconds, 1),
    ]
    .into_iter()
    .try_fold(0_i64, |total, (digits, factor)| {
        let digits = digits.parse::<i64>().map_err(|_| invalid())?;
        let seconds = digits.checked_mul(factor).ok_or_else(invalid)?;
        total.checked_add(seconds).ok_or_else(invalid)
    })?;
    TimeDelta::try_seconds(seconds).ok_or_else(invalid)
}

pub(crate) fn calendar_times(
    (authored, timezone): (&Bound<'_, PyAny>, &str),
    session: &Bound<'_, PyAny>,
    providers: &Bound<'_, PyDict>,
    start: DateTime<Utc>,
) -> PyResult<Schedule> {
    let py = authored.py();
    timezone
        .parse::<Tz>()
        .map_err(|_| PyValueError::new_err("calendar timezone must be an IANA timezone"))?;
    let duration = horizon(&authored.getattr("horizon")?.extract::<String>()?)?;
    let end = start
        .checked_add_signed(duration)
        .ok_or_else(|| PyValueError::new_err("calendar horizon is out of datetime range"))?;
    let datetime = imported(py, "datetime", "datetime")?;
    let zone = imported(py, "zoneinfo", "ZoneInfo")?.call1((timezone,))?;
    let window = PyDict::new(py);
    for (name, instant) in [("start", start), ("end", end)] {
        window.set_item(
            name,
            datetime
                .call_method1("fromisoformat", (instant.to_rfc3339(),))?
                .call_method1("astimezone", (&zone,))?,
        )?;
    }
    let window = py
        .import("sqlbuild.orchestration")?
        .getattr("CalendarWindow")?
        .call((), Some(&window))?;
    let function = authored.getattr("function")?;
    let arguments = PyDict::new(py);
    arguments.set_item(CALENDAR_WINDOW_PARAMETER, window)?;
    for usage in provider_usages(&function, providers)?.try_iter()? {
        let usage = usage?;
        if usage.getattr("parameter_name")?.extract::<String>()? == CALENDAR_WINDOW_PARAMETER {
            continue;
        }
        arguments.set_item(
            usage.getattr("parameter_name")?,
            session.call_method1("get", (usage.getattr("provider_name")?,))?,
        )?;
    }
    let mut times: BTreeSet<DateTime<Utc>> = BTreeSet::new();
    for (index, time) in function
        .call((), Some(&arguments))?
        .try_iter()?
        .take(MAX_TICKS_PER_BATCH + 1)
        .enumerate()
    {
        if index == MAX_TICKS_PER_BATCH {
            return Err(PyValueError::new_err(format!(
                "calendar exceeds {MAX_TICKS_PER_BATCH} upcoming times"
            )));
        }
        let time = time?;
        if !time.is_instance(&datetime)? {
            return Err(PyValueError::new_err(
                "calendar times must be timezone-aware datetimes",
            ));
        }
        let time =
            DateTime::parse_from_rfc3339(&time.call_method0("isoformat")?.extract::<String>()?)
                .map_err(|_| {
                    PyValueError::new_err("calendar times must be timezone-aware datetimes")
                })?
                .with_timezone(&Utc);
        if time < start || time >= end {
            return Err(PyValueError::new_err(
                "calendar time lies outside its half-open evaluation window",
            ));
        }
        times.insert(time);
    }
    Ok(Schedule::Calendar {
        times,
        refresh_at: start + duration / 2,
    })
}

pub(crate) fn sensor_nodes(
    (graph, selected): (&ProjectGraph, &ProjectSelection),
    selectors: &[String],
    field: &str,
) -> PyResult<Value> {
    if selectors.is_empty() || selectors.iter().any(|value| value.trim().is_empty()) {
        return Err(PyValueError::new_err(format!(
            "{field} needs nonempty selectors"
        )));
    }
    let nodes: BTreeSet<String> = selected
        .node_keys(graph, selectors, &[])
        .map_err(|error| PyValueError::new_err(format!("{field}: {error}")))?;
    if nodes.is_empty() {
        return Err(PyValueError::new_err(format!(
            "{field} needs nonempty selectors"
        )));
    }
    if matches!(field, SOURCE_NEW_DATA_CONDITION | SOURCE_STALE_CONDITION)
        && nodes.iter().any(|key| !key.starts_with("source:"))
    {
        return Err(PyValueError::new_err(format!(
            "{field} must select only source nodes"
        )));
    }
    Ok(json!(nodes))
}
