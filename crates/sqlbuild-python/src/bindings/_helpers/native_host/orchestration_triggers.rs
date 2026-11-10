//! Authored trigger controls and bounded calendars evaluated only in the location interpreter.

use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;

use chrono::{DateTime, TimeDelta, Timelike, Utc};
use chrono_tz::Tz;
use pyo3::FromPyObject;
use pyo3::exceptions::PyValueError;
use pyo3::prelude::{Bound, Py, PyAny, PyAnyMethods, PyResult, Python};
use pyo3::types::{PyBool, PyDict, PyDictMethods};
use serde_json::{Value, json};
use sqlbuild_analysis::graph::main::resolve_selectors::resolve_selectors;
use sqlbuild_analysis::graph::models::ProjectGraph;
use sqlbuild_cursor::algebra::main::parse_duration::parse_duration;
use sqlbuild_host::entries::models::UserCodeHost;
use sqlbuild_orchestration::declarations::models::{Definitions, Schedule, TriggerDefinition};
use sqlbuild_orchestration::scheduler::constants::MAX_TICKS_PER_BATCH;
use sqlbuild_orchestration::sensors::models::SensorDefinition;
use sqlbuild_orchestration::snapshots::models::{SnapshotInput, SnapshotNode};

use crate::bindings::_helpers::boundary::python_calls::{call_with, imported, within};
use crate::bindings::_helpers::discovery::imported_modules::{provider_by_name, provider_usages};
use crate::bindings::_helpers::discovery::imported_providers::discover_provider_classes;
use crate::bindings::_helpers::native_host::constants::{
    CALENDAR_WINDOW_PARAMETER, CHECK_KIND, NODE_EVENT_CONDITION, SOURCE_NEW_DATA_CONDITION,
    SOURCE_STALE_CONDITION,
};
use crate::bindings::_helpers::native_host::host::entry;
use crate::bindings::_helpers::native_host::orchestration_declarations::{
    AuthoredSensor, authored_setting, declared_at,
};
use crate::bindings::_helpers::native_host::orchestration_environment::{
    callback_code_hash, orchestration_code_hash,
};

#[derive(FromPyObject)]
struct AuthoredControls {
    #[pyo3(from_py_with = authored_setting)]
    starts_at: Value,
    default_status: String,
}

fn compile_trigger(
    authored: &Bound<'_, PyAny>,
    job_key: &str,
    description: &str,
    schedule: Schedule,
) -> PyResult<TriggerDefinition> {
    let controls = authored.extract::<AuthoredControls>()?;
    Ok(TriggerDefinition {
        description: description.into(),
        job_key: job_key.into(),
        schedule,
        starts_at: serde_json::from_value(controls.starts_at)
            .map_err(|error| PyValueError::new_err(format!("starts_at: {error}")))?,
        default_status: serde_json::from_value(Value::String(controls.default_status))
            .map_err(|error| PyValueError::new_err(format!("default_status: {error}")))?,
        targets: BTreeSet::new(),
        declared_at: None,
    })
}

pub(crate) fn compile_cron(
    authored: &Bound<'_, PyAny>,
    name: &str,
    description: &str,
) -> PyResult<TriggerDefinition> {
    compile_trigger(
        authored,
        name,
        description,
        Schedule::Cron {
            expression: authored.getattr("cron")?.extract()?,
            timezone: authored.getattr("timezone")?.extract()?,
        },
    )
}

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

fn calendar_times(
    authored: &Bound<'_, PyAny>,
    session: &Bound<'_, PyAny>,
    providers: &Bound<'_, PyDict>,
    start: DateTime<Utc>,
) -> PyResult<Schedule> {
    let py = authored.py();
    let timezone: String = authored.getattr("timezone")?.extract()?;
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

pub(crate) fn compile_calendars(
    py: Python<'_>,
    (project_dir, target): (&Path, &str),
    authored: Vec<Py<PyAny>>,
    mut definitions: Definitions,
) -> PyResult<Definitions> {
    let mut calendars: Vec<Py<PyAny>> = Vec::new();
    for calendar in authored {
        let targets = calendar
            .bind(py)
            .getattr("targets")?
            .extract::<Vec<String>>()?;
        if targets.is_empty() || targets.iter().any(|value| value == target) {
            calendars.push(calendar);
        }
    }
    if calendars.is_empty() {
        return Ok(definitions);
    }
    let providers =
        discover_provider_classes(imported(py, "pathlib", "Path")?.call1((project_dir,))?)?;
    let by_name = provider_by_name(&providers)?;
    let session = call_with(
        &entry(py, UserCodeHost::BuildProviderSession)?,
        &[
            ("discovered_providers", providers),
            ("allow_shared", PyBool::new(py, false).to_owned().into_any()),
        ],
    )?;
    let now = Utc::now();
    let start = now
        .with_nanosecond(now.timestamp_subsec_micros() * 1_000)
        .ok_or_else(|| PyValueError::new_err("calendar clock is out of range"))?;
    within(&session, |_| {
        for calendar in calendars {
            let calendar = calendar.bind(py);
            let name: String = calendar
                .getattr("function")?
                .getattr("__name__")?
                .extract()?;
            let job_key: String = calendar.getattr("job")?.getattr("name")?.extract()?;
            let description: String = calendar.getattr("description")?.extract()?;
            if description.trim().is_empty()
                || (!definitions.jobs.contains_key(&job_key)
                    && !definitions.groups.contains_key(&job_key))
            {
                return Err(PyValueError::new_err(format!(
                    "calendar {name} needs a description and a published job"
                )));
            }
            let schedule = calendar_times(calendar, &session, &by_name, start)?;
            let mut trigger = compile_trigger(calendar, &job_key, &description, schedule)?;
            trigger.starts_at.get_or_insert(start);
            trigger.targets = calendar
                .getattr("targets")?
                .extract::<Vec<String>>()?
                .into_iter()
                .collect();
            trigger.declared_at =
                declared_at(project_dir, calendar.getattr("_declared_at")?.extract()?)?;
            definitions.triggers.insert(name, trigger);
        }
        Ok(definitions)
    })
}

pub(crate) fn compile_sensors(
    py: Python<'_>,
    (project_dir, input, graph): (&Path, &SnapshotInput, &ProjectGraph),
    authored: Vec<AuthoredSensor>,
    mut definitions: Definitions,
) -> PyResult<Definitions> {
    let sources = if authored.iter().any(|sensor| sensor.function.is_some()) {
        orchestration_code_hash(project_dir)?
    } else {
        String::new()
    };
    for sensor in authored {
        if !sensor.targets.is_empty() && !sensor.targets.iter().any(|value| value == &input.target)
        {
            continue;
        }
        let condition = match (sensor.when, sensor.function) {
            (Some(when), None) => {
                let mut condition = json!({"kind": when.kind});
                if when.kind == SOURCE_STALE_CONDITION {
                    condition["after_seconds"] = when.after_seconds;
                } else if when.kind != SOURCE_NEW_DATA_CONDITION {
                    condition["kinds"] = json!(when.kinds);
                }
                if let Some(job) = when.job {
                    condition["job_key"] = json!(job.name);
                }
                if let Some(select) = when.select {
                    let selectors: Vec<String> =
                        select.split_whitespace().map(str::to_owned).collect();
                    condition["selection"] =
                        sensor_nodes(graph, &input.nodes, &selectors, &when.kind)?;
                }
                if let Some(selectors) = when.executed_successfully {
                    condition["executed_successfully"] =
                        sensor_nodes(graph, &input.nodes, &selectors, "executed_successfully")?;
                }
                condition
            }
            (None, Some(function)) => json!({"kind": "custom", "function": sensor.name,
                "code_hash": callback_code_hash(function.bind(py), &sources)?}),
            _ => {
                return Err(PyValueError::new_err(
                    "sensor needs either a built-in condition or a decorated polling callback; callbacks with when are not supported yet",
                ));
            }
        };
        let definition: SensorDefinition = serde_json::from_value(json!({
            "description": sensor.description,
            "job_key": sensor.run.name,
            "when": condition,
            "minimum_interval_seconds": sensor.minimum_interval_seconds,
            "default_status": sensor.default_status,
            "targets": sensor.targets,
            "declared_at": declared_at(project_dir, sensor.declared_at)?,
        }))
        .map_err(|error| PyValueError::new_err(format!("sensor {}: {error}", sensor.name)))?;
        definitions.sensors.insert(sensor.name, definition);
    }
    Ok(definitions)
}

fn sensor_nodes(
    graph: &ProjectGraph,
    available: &BTreeMap<String, SnapshotNode>,
    selectors: &[String],
    field: &str,
) -> PyResult<Value> {
    if selectors.is_empty() || selectors.iter().any(|value| value.trim().is_empty()) {
        return Err(PyValueError::new_err(format!(
            "{field} needs nonempty selectors"
        )));
    }
    let mut nodes: BTreeSet<String> = BTreeSet::new();
    let mut sql_selectors: Vec<String> = Vec::new();
    for selector in selectors {
        if field == NODE_EVENT_CONDITION && selector.starts_with("check:") {
            if !available
                .get(selector)
                .is_some_and(|node| node.kind == CHECK_KIND)
            {
                return Err(PyValueError::new_err(format!(
                    "unknown check node {selector}"
                )));
            }
            nodes.insert(selector.clone());
        } else {
            sql_selectors.push(selector.clone());
        }
    }
    if !sql_selectors.is_empty() {
        nodes.extend(
            resolve_selectors(graph, &sql_selectors, &[])
                .map_err(|error| PyValueError::new_err(error.message))?
                .into_iter()
                .map(|(kind, name)| format!("{kind}:{name}")),
        );
    }
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
