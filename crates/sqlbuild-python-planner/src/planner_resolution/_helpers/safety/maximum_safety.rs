//! Maximum automatic-start recovery and evidence boundary.

use crate::planner_resolution::_helpers::cursor::cursor_bounds::apply_typed_cursor_replay_policy;
use crate::planner_resolution::_helpers::cursor::cursor_bounds::optional_scalar;
use crate::planner_resolution::_helpers::cursor::cursor_bounds::required_scalar;
use crate::planner_resolution::_helpers::cursor::cursor_bounds::scalars;
use crate::planner_resolution::_helpers::cursor::cursor_safety::horizon;
use crate::planner_resolution::_helpers::cursor::cursor_safety::replace;
use crate::planner_resolution::_helpers::cursor::cursor_safety::safety_duration;
use crate::planner_resolution::_helpers::cursor::cursor_safety::safety_error;
use crate::planner_resolution::_helpers::cursor::cursor_safety::timestamp_bound;
use crate::planner_resolution::_helpers::cursor::cursor_safety::utc_clock;
use crate::planner_resolution::_helpers::references::values::string;
use pyo3::prelude::Bound;
use pyo3::prelude::PyAny;
use pyo3::prelude::PyAnyMethods;
use pyo3::prelude::PyErr;
use pyo3::prelude::PyResult;
use pyo3::prelude::Python;
use pyo3::types::PyDict;
use pyo3::{IntoPyObject, pyfunction};
use sqlbuild_cursor::algebra::main::compare::compare;
use sqlbuild_cursor::planner::main::eligible_start::eligible_start;
use sqlbuild_python_base::boundary::main::call_with::call_with;
use sqlbuild_python_base::boundary::main::keywords::keywords;
use sqlbuild_python_base::boundary::models::KeywordArguments;
use sqlbuild_python_base::records::main::construct::error_class::error_class;
use sqlbuild_python_base::records::main::construct::new_record_with::new_record_with;
use sqlbuild_python_base::records::main::construct::record_class::record_class;
use sqlbuild_python_base::records::main::fields::field::field as native_field;
use sqlbuild_python_base::records::types::ErrorId;
use sqlbuild_python_base::records::types::RecordId;
use sqlbuild_python_front::cursor::main::scalars::render_value::render_value;
use sqlbuild_python_front::cursor::models::CursorSession;
use sqlbuild_python_project::planner_phases::main::object::object;
use std::cmp::Ordering;

#[pyfunction]
#[pyo3(signature = (**kwargs))]
pub(crate) fn apply_maximum_start_policy<'py>(
    py: Python<'py>,
    kwargs: Option<Bound<'py, PyDict>>,
) -> PyResult<Bound<'py, PyAny>> {
    let args = KeywordArguments::read(
        py,
        "apply_maximum_start_policy",
        kwargs,
        (
            &[
                "bounds",
                "snapshot",
                "cursor_type",
                "cursor_grain",
                "cursor_start",
                "lookback",
                "backfill_duration",
                "policy",
                "has_start_override",
            ],
            &[],
        ),
    )?;
    let bounds = args.value("bounds")?;
    let snapshot = args.value("snapshot")?;
    let physical = native_field(&snapshot, "physical_target_max")?;
    let physical = if physical.is_truthy()? {
        physical
    } else {
        native_field(&snapshot, "target_max")?
    };
    let policy = args.value("policy")?;
    let config = native_field(&policy, "config")?;
    let clock = native_field(&policy, "invocation_time")?;
    if config.is_none()
        || native_field(&config, "max_ahead")?.is_none()
        || !args.value("cursor_type")?.eq("timestamp")?
        || clock.is_none()
        || args.flag("has_start_override")?
        || physical.is_none()
    {
        return Ok(bounds);
    }
    if !timestamp_bound(&native_field(&bounds, "start")?)?
        || !timestamp_bound(&native_field(&bounds, "end")?)?
    {
        return Ok(bounds);
    }
    let duration = safety_duration(&native_field(&config, "max_ahead")?, false)?;
    if !timestamp_bound(&physical)? {
        return Err(safety_error(
            py,
            ErrorId::MaximumAutomaticStart,
            "automatic start requires a timestamp cursor",
        ));
    }
    let session = CursorSession::new(py)?;
    let horizon = horizon(
        &session,
        &physical,
        (&clock, &args.value("cursor_grain")?),
        &duration,
    )?;
    let physical_typed = session.scalar(&physical)?;
    if session.checked(compare(&physical_typed, &horizon, &session))? != Ordering::Greater {
        return Ok(bounds);
    }
    let target = optional_scalar(&session, &native_field(&snapshot, "target_eligible_max")?)?;
    let upstream = if target.is_none() {
        scalars(&session, &snapshot, "upstream_mins")?
    } else {
        Vec::new()
    };
    let mut eligible = session.checked(eligible_start(
        target.as_ref(),
        &upstream,
        &horizon,
        &session,
    ))?;
    if eligible.is_none() {
        let floor = args.value("cursor_start")?;
        if !floor.is_none() {
            let floor = if timestamp_bound(&floor)? {
                session.scalar(&floor)?
            } else {
                required_scalar(&session, &floor, &string(py, "timestamp"))?
            };
            if session.checked(compare(&floor, &horizon, &session))? != Ordering::Greater {
                eligible = Some(floor);
            }
        }
    }
    let effective = if let Some(eligible) = &eligible {
        let mut replay = vec![
            ("start", session.python_scalar(eligible)?),
            ("end", native_field(&bounds, "end")?),
            (
                "has_start_override",
                false.into_pyobject(py)?.to_owned().into_any(),
            ),
        ];
        for key in [
            "cursor_start",
            "cursor_type",
            "lookback",
            "backfill_duration",
        ] {
            replay.push((key, args.value(key)?));
        }
        apply_typed_cursor_replay_policy(py, Some(keywords(py, &replay)?))?
    } else {
        native_field(&bounds, "start")?
    };
    let relation = native_field(&snapshot, "target_relation")?;
    let column = native_field(&snapshot, "destination_cursor_column")?;
    let evidence = new_record_with(
        py,
        RecordId::MaximumStartSafetyEvidence,
        &[
            ("action", native_field(&config, "action")?),
            ("max_ahead", native_field(&config, "max_ahead")?),
            (
                "invocation_time",
                call_with(
                    &record_class(py, RecordId::TimestampValue)?,
                    &[("value", utc_clock(&clock)?)],
                )?,
            ),
            ("physical_target_max", physical),
            (
                "highest_eligible_target_max",
                native_field(&snapshot, "target_eligible_max")?,
            ),
            ("effective_start", effective.clone()),
            ("maximum_allowed_start", session.python_scalar(&horizon)?),
            (
                "target_relation",
                if relation.is_truthy()? {
                    relation
                } else {
                    string(py, "")
                },
            ),
            (
                "cursor_column",
                if column.is_truthy()? {
                    column
                } else {
                    string(py, "")
                },
            ),
        ],
    )?;
    let error = |non_idempotent: bool| -> PyResult<Bound<'py, PyAny>> {
        Err(PyErr::from_value(call_with(
            &error_class(py, ErrorId::MaximumAutomaticStart)?,
            &[
                (
                    "message",
                    string(py, &exceeded_message(&evidence, non_idempotent)?),
                ),
                ("evidence", evidence.clone()),
            ],
        )?))
    };
    if native_field(&config, "action")?.eq("error")?
        || eligible.is_none()
        || session.checked(compare(&session.scalar(&effective)?, &horizon, &session))?
            == Ordering::Greater
    {
        return error(false);
    }
    let strategy = native_field(&policy, "incremental_strategy")?;
    if !strategy.eq("delete_insert")? && !strategy.eq("merge")? {
        return error(true);
    }
    replace(
        &bounds,
        &[("start", effective), ("maximum_start_safety", evidence)],
    )
}

fn rendered(value: &Bound<'_, PyAny>) -> PyResult<String> {
    object(value.py(), render_value(value)?)?.extract()
}

/// The message of an automatic start that exceeds its horizon.
fn exceeded_message(evidence: &Bound<'_, PyAny>, non_idempotent: bool) -> PyResult<String> {
    let detail: &str = if non_idempotent {
        " on a non-idempotent materialization"
    } else {
        ""
    };
    let highest = native_field(evidence, "highest_eligible_target_max")?;
    let highest: String = if highest.is_none() {
        "None".to_owned()
    } else {
        string(evidence.py(), &rendered(&highest)?)
            .repr()?
            .to_string()
    };
    Ok(format!(
        "maximum automatic start policy exceeded{detail}: physical target MAX='{}', highest eligible MAX={highest}, effective post-lookback start='{}', horizon='{}', action={}, input={}.{}",
        rendered(&native_field(evidence, "physical_target_max")?)?,
        rendered(&native_field(evidence, "effective_start")?)?,
        rendered(&native_field(evidence, "maximum_allowed_start")?)?,
        native_field(&native_field(evidence, "action")?, "value")?.str()?,
        native_field(evidence, "target_relation")?.str()?,
        native_field(evidence, "cursor_column")?.str()?,
    ))
}
