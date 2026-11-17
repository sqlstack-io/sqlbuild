//! Snapshot conversion and cursor window policy boundary.

use sqlbuild_host::entries::models::ExecutorHost;

use crate::planner_resolution::_helpers::references::values::string;
use crate::planner_resolution::_helpers::safety::maximum_safety::apply_maximum_start_policy;
use pyo3::prelude::{Bound, PyAny, PyAnyMethods, PyResult, Python};
use pyo3::types::PyDict;
use pyo3::{IntoPyObject, pyfunction};
use sqlbuild_cursor::algebra::main::inclusive_to_exclusive::inclusive_to_exclusive;
use sqlbuild_cursor::algebra::models::{DurationSpan, Grain, Scalar};
use sqlbuild_cursor::planner::main::raw_bounds::raw_bounds;
use sqlbuild_cursor::planner::main::replay_start::replay_start;
use sqlbuild_cursor::planner::models::{CursorKind, CursorSnapshot, ReplayPolicy};
use sqlbuild_python_base::boundary::main::keywords::keywords;
use sqlbuild_python_base::boundary::models::KeywordArguments;
use sqlbuild_python_base::native_host::main::host_entry::host_entry;
use sqlbuild_python_base::records::main::construct::new_record::new_record;
use sqlbuild_python_base::records::main::construct::new_record_with::new_record_with;
use sqlbuild_python_base::records::main::construct::record_class::record_class;
use sqlbuild_python_base::records::main::fields::field::field as native_field;
use sqlbuild_python_base::records::types::RecordId;
use sqlbuild_python_front::cursor::main::conversion::algebra_error::algebra_error;
use sqlbuild_python_front::cursor::main::scalars::try_parse::try_parse;
use sqlbuild_python_front::cursor::models::CursorSession;

pub(crate) fn optional_scalar<'py>(
    session: &CursorSession<'py>,
    value: &Bound<'py, PyAny>,
) -> PyResult<Option<Scalar>> {
    if value.is_none() {
        Ok(None)
    } else {
        session.scalar(value).map(Some)
    }
}

pub(crate) fn scalars<'py>(
    session: &CursorSession<'py>,
    object: &Bound<'py, PyAny>,
    field: &str,
) -> PyResult<Vec<Scalar>> {
    object
        .getattr(field)?
        .try_iter()?
        .map(|value| session.scalar(&value?))
        .collect()
}

fn read_snapshot<'py>(
    session: &CursorSession<'py>,
    object: &Bound<'py, PyAny>,
) -> PyResult<CursorSnapshot> {
    let mut end_inputs: Vec<(Option<Scalar>, Option<Scalar>)> = Vec::new();
    for pair in native_field(object, "upstream_end_inputs")?.try_iter()? {
        let pair = pair?;
        end_inputs.push((
            optional_scalar(session, &pair.get_item(0)?)?,
            optional_scalar(session, &pair.get_item(1)?)?,
        ));
    }
    Ok(CursorSnapshot {
        target_max: optional_scalar(session, &native_field(object, "target_max")?)?,
        upstream_mins: scalars(session, object, "upstream_mins")?,
        upstream_maxes: scalars(session, object, "upstream_maxes")?,
        terminal_starts: scalars(session, object, "upstream_terminal_starts")?,
        terminal_ends: scalars(session, object, "upstream_terminal_ends")?,
        availability_ends: scalars(session, object, "upstream_availability_ends")?,
        any_input: native_field(object, "cursor_watermark_mode")?.eq("any")?,
        end_inputs,
    })
}

pub(crate) fn duration(value: &Bound<'_, PyAny>) -> PyResult<Option<DurationSpan>> {
    if value.is_none() {
        return Ok(None);
    }
    let class = record_class(value.py(), RecordId::Duration)?;
    let parsed = if value.is_instance(&class)? {
        value.clone()
    } else {
        class.call_method1("parse", (value,))?
    };
    if parsed.is_none() {
        return Ok(None);
    }
    let field = |name: &str| -> PyResult<i64> { native_field(&parsed, name)?.extract() };
    Ok(Some(DurationSpan {
        months: field("years")? * 12 + field("months")?,
        days: field("days")?,
        hours: field("hours")?,
        minutes: field("minutes")?,
        seconds: field("seconds")?,
    }))
}

fn read_policy<'py>(
    session: &CursorSession<'py>,
    args: &KeywordArguments<'py>,
) -> PyResult<ReplayPolicy> {
    let floor = args.value("cursor_start")?;
    let cursor_type = args.value("cursor_type")?;
    let floor =
        if floor.is_none() || !(cursor_type.eq("timestamp")? || cursor_type.eq("integer")?) {
            None
        } else if session.scalar_typed(&floor)? {
            Some(session.scalar(&floor)?)
        } else {
            try_parse(&floor, &cursor_type)?
                .map(|value| session.scalar(&value))
                .transpose()?
        };
    Ok(ReplayPolicy {
        floor,
        lookback: duration(&args.value("lookback")?)?,
        backfill: duration(&args.value("backfill_duration")?)?,
        has_backfill: !args.value("backfill_duration")?.is_none(),
        has_start_override: args.flag("has_start_override")?,
    })
}

#[pyfunction]
#[pyo3(signature = (**kwargs))]
pub(crate) fn apply_typed_cursor_replay_policy<'py>(
    py: Python<'py>,
    kwargs: Option<Bound<'py, PyDict>>,
) -> PyResult<Bound<'py, PyAny>> {
    let args = KeywordArguments::read(
        py,
        "apply_typed_cursor_replay_policy",
        kwargs,
        (
            &[
                "start",
                "end",
                "cursor_start",
                "cursor_type",
                "lookback",
                "backfill_duration",
                "has_start_override",
            ],
            &[],
        ),
    )?;
    let session = CursorSession::new(py)?;
    let cursor_type: Option<String> = args.value("cursor_type")?.extract()?;
    let start = session.scalar(&args.value("start")?)?;
    let end = session.scalar(&args.value("end")?)?;
    let policy = read_policy(&session, &args)?;
    let result = session.checked(replay_start(
        (&start, &end),
        &policy,
        &CursorKind {
            cursor_type: cursor_type.as_deref(),
            grain: None,
        },
        &session,
    ))?;
    session.python_scalar(&result)
}

pub(crate) fn required_scalar<'py>(
    session: &CursorSession<'py>,
    raw: &Bound<'py, PyAny>,
    cursor_type: &Bound<'py, PyAny>,
) -> PyResult<Scalar> {
    let Some(value) = try_parse(raw, cursor_type)? else {
        return Err(algebra_error(
            raw.py(),
            &format!("invalid cursor value: {}", raw.str()?),
        ));
    };
    session.scalar(&value)
}

#[pyfunction]
#[pyo3(signature = (**kwargs))]
pub(crate) fn compute_cursor_bounds<'py>(
    py: Python<'py>,
    kwargs: Option<Bound<'py, PyDict>>,
) -> PyResult<Bound<'py, PyAny>> {
    let args = KeywordArguments::read(
        py,
        "compute_cursor_bounds",
        kwargs,
        (
            &[
                "cursor_snapshot",
                "cursor_type",
                "cursor_start",
                "lookback",
                "backfill_duration",
                "start_cursor_override",
                "end_cursor_override",
                "is_microbatch",
            ],
            &["cursor_grain", "maximum_start_policy"],
        ),
    )?;
    if args.flag("is_microbatch")? {
        let sentinel = host_entry(py, ExecutorHost::BoundSentinel)?;
        return new_record_with(
            py,
            RecordId::CursorBounds,
            &[
                ("start", native_field(&sentinel, "START")?),
                ("end", native_field(&sentinel, "END")?),
            ],
        );
    }
    let snapshot = args.value("cursor_snapshot")?;
    if !native_field(&snapshot, "watermarks_available")?.is_truthy()? {
        return Ok(py.None().into_bound(py));
    }
    let session = CursorSession::new(py)?;
    let cursor_type: Option<String> = args.value("cursor_type")?.extract()?;
    let grain = session.grain(&args.value("cursor_grain")?)?;
    let kind = CursorKind {
        cursor_type: cursor_type.as_deref(),
        grain,
    };
    let Some((mut start, mut end)) = session.checked(raw_bounds(
        &read_snapshot(&session, &snapshot)?,
        &kind,
        &session,
    ))?
    else {
        return Ok(py.None().into_bound(py));
    };
    let parse_type = string(py, cursor_type.as_deref().unwrap_or("timestamp"));
    let start_override = args.value("start_cursor_override")?;
    let end_override = args.value("end_cursor_override")?;
    if !start_override.is_none() {
        start = required_scalar(&session, &start_override, &parse_type)?;
    }
    if !end_override.is_none() {
        let value = required_scalar(&session, &end_override, &parse_type)?;
        end = session.checked(inclusive_to_exclusive(
            &value,
            if cursor_type.as_deref() == Some("integer") {
                None
            } else {
                Some(grain.unwrap_or(Grain::Second))
            },
        ))?;
    }
    let mut replay_args: Vec<(&str, Bound<'py, PyAny>)> = Vec::new();
    for key in [
        "cursor_start",
        "cursor_type",
        "lookback",
        "backfill_duration",
    ] {
        replay_args.push((key, args.value(key)?));
    }
    replay_args.extend([
        ("start", session.python_scalar(&start)?),
        ("end", session.python_scalar(&end)?),
        (
            "has_start_override",
            (!start_override.is_none())
                .into_pyobject(py)?
                .to_owned()
                .into_any(),
        ),
    ]);
    let replayed = apply_typed_cursor_replay_policy(py, Some(keywords(py, &replay_args)?))?;
    let bounds = new_record_with(
        py,
        RecordId::CursorBounds,
        &[("start", replayed), ("end", session.python_scalar(&end)?)],
    )?;
    let policy = args.value("maximum_start_policy")?;
    let policy = if policy.is_truthy()? {
        policy
    } else {
        new_record(py, RecordId::MaximumStartPolicyInputs, (), None)?
    };
    let mut policy_args = vec![
        ("bounds", bounds),
        ("snapshot", snapshot),
        ("policy", policy),
        (
            "has_start_override",
            (!start_override.is_none())
                .into_pyobject(py)?
                .to_owned()
                .into_any(),
        ),
    ];
    for key in [
        "cursor_type",
        "cursor_grain",
        "cursor_start",
        "lookback",
        "backfill_duration",
    ] {
        policy_args.push((key, args.value(key)?));
    }
    apply_maximum_start_policy(py, Some(keywords(py, &policy_args)?))
}
