//! Plan-entry cursor windows: overrides, planner-owned bounds and microbatch ranges.

use crate::planner_resolution::main::cursor::config_str::config_str;
use crate::planner_resolution::main::cursor::model_cursor_overrides::model_cursor_overrides;
use crate::planner_resolution::main::cursor::plan_cursor_bounds::plan_cursor_bounds;
use crate::planner_resolution::main::cursor::resolve_effective_timestamp_grain::resolve_effective_timestamp_grain;
use crate::planner_resolution::models::CursorWindowInputs;
use sqlbuild_host::entries::models::ExecutorHost;
use sqlbuild_python_base::records::main::construct::new_plain_error::new_plain_error;

use crate::plan_output::main::resolve_bounded_cursor_override::resolve_bounded_cursor_override;
use crate::planner_resolution::_helpers::cursor::cursor_bounds::compute_cursor_bounds;
use crate::planner_resolution::_helpers::cursor::cursor_bounds::duration;
use crate::planner_resolution::_helpers::cursor::cursor_safety::replace;
use crate::planner_resolution::_helpers::cursor::cursor_safety::utc_clock;
use crate::planner_resolution::_helpers::cursor::cursor_windows::normalize_cursor_snapshot_grain;
use crate::planner_resolution::_helpers::cursor::cursor_windows::without_destination_cursor;
use crate::planner_resolution::_helpers::references::values::string;
use crate::planner_resolution::_helpers::safety::future_safety::resolve_future_cursor_safety;
use crate::planner_strategy::main::model_entries::resolve_microbatch_lookback::resolve_microbatch_lookback;
use pyo3::prelude::Bound;
use pyo3::prelude::PyAny;
use pyo3::prelude::PyAnyMethods;
use pyo3::prelude::PyResult;
use pyo3::prelude::Python;
use pyo3::types::{PyDict, PyString, PyTuple};
use pyo3::{IntoPyObject, PyErr, pyfunction};
use sqlbuild_cursor::algebra::main::compare::compare;
use sqlbuild_cursor::algebra::models::{Scalar, ScalarValue, Timestamp, ZoneOffset};
use sqlbuild_cursor::planner::main::cursor_end_clamp::cursor_end_clamp;
use sqlbuild_cursor::planner::main::rolling_window::rolling_window;
use sqlbuild_cursor::planner::models::CursorEndClamp;
use sqlbuild_python_base::boundary::main::call_with::call_with;
use sqlbuild_python_base::boundary::main::imported::imported;
use sqlbuild_python_base::boundary::main::keywords::keywords;
use sqlbuild_python_base::boundary::models::KeywordArguments;
use sqlbuild_python_base::native_host::main::host_entry::host_entry;
use sqlbuild_python_base::records::main::construct::new_record_with::new_record_with;
use sqlbuild_python_base::records::main::fields::field::field as native_field;
use sqlbuild_python_base::records::types::ErrorId;
use sqlbuild_python_base::records::types::RecordId;
use sqlbuild_python_front::cursor::main::scalars::parse_value::parse_value;
use sqlbuild_python_front::cursor::main::scalars::sentinel_to_token::sentinel_to_token;
use sqlbuild_python_front::cursor::main::values::local_date_time::local_date_time;
use sqlbuild_python_front::cursor::main::values::replaced::replaced;
use sqlbuild_python_front::cursor::models::CursorSession;
use sqlbuild_python_front::model_config::main::config::config_cursor_bound::config_cursor_bound as typed_cursor_bound;
use sqlbuild_python_front::model_config::main::config::model_values::model_values;
use sqlbuild_python_project::compiled_views::main::compiled_models::name_object::name_object;
use sqlbuild_python_project::planner_phases::main::object::object;
use std::cmp::Ordering;

pub(crate) const INCREMENTAL: &str = "incremental";
pub(crate) const MICROBATCH: &str = "microbatch";
pub(crate) const ROLLING_WINDOW: &str = "rolling_window";
pub(crate) const TIMESTAMP: &str = "timestamp";
pub(crate) const INTEGER: &str = "integer";
const BOUNDED_BACKFILL: &str = "bounded";
pub(crate) const FULL_BACKFILL: &str = "full";
const WINDOW_REQUIRED: [&str; 7] = [
    "model",
    "snapshot",
    "backfill",
    "full_refresh",
    "start_cursor_override",
    "end_cursor_override",
    "runtime_owned_cursor_bounds",
];
const WINDOW_OPTIONAL: [&str; 5] = [
    "invocation_time",
    "future_cursor_config",
    "start_cursor_config",
    "replaces_relation",
    "cursor_input_relations",
];

/// One cursor-bound model configuration entry as its token text.
pub(crate) fn config_bound<'py>(
    model: &Bound<'py, PyAny>,
    key: &str,
) -> PyResult<Bound<'py, PyAny>> {
    let py: Python<'py> = model.py();
    object(py, typed_cursor_bound(py, &model_values(model)?, key)?)
}

fn boolean(py: Python<'_>, value: bool) -> PyResult<Bound<'_, PyAny>> {
    Ok(value.into_pyobject(py)?.to_owned().into_any())
}

/// A cursor token as the scalar it orders by: integers for integer cursors, else instants.
fn order_scalar<'py>(
    session: &CursorSession<'py>,
    token: &Bound<'py, PyAny>,
    cursor_type: &Bound<'py, PyAny>,
) -> PyResult<Scalar> {
    let py = session.py();
    let kind = if cursor_type.eq(INTEGER)? {
        INTEGER
    } else {
        TIMESTAMP
    };
    session.scalar(&parse_value(token, &string(py, kind))?)
}

pub(crate) fn order<'py>(
    session: &CursorSession<'py>,
    tokens: (&Bound<'py, PyAny>, &Bound<'py, PyAny>),
    cursor_type: &Bound<'py, PyAny>,
) -> PyResult<Ordering> {
    let left = order_scalar(session, tokens.0, cursor_type)?;
    let right = order_scalar(session, tokens.1, cursor_type)?;
    session.checked(compare(&left, &right, session))
}

pub(crate) fn token<'py>(
    py: Python<'py>,
    bound: &Bound<'py, PyAny>,
) -> PyResult<Bound<'py, PyAny>> {
    Ok(PyString::new(py, &sentinel_to_token(bound)?).into_any())
}

fn sentinel<'py>(py: Python<'py>, name: &str) -> PyResult<Bound<'py, PyAny>> {
    host_entry(py, ExecutorHost::BoundSentinel)?.getattr(name)
}

/// `bounds` clamped so a concrete window never reaches the exclusive configured `cursor_end`.
pub(crate) fn clamp_cursor_end<'py>(
    bounds: Bound<'py, PyAny>,
    model: &Bound<'py, PyAny>,
) -> PyResult<Bound<'py, PyAny>> {
    let py = model.py();
    let cursor_end = config_bound(model, "cursor_end")?;
    let cursor_type = config_str(model, "cursor_type")?;
    if bounds.is_none() || cursor_end.is_none() {
        return Ok(bounds);
    }
    let start = native_field(&bounds, "start")?;
    let end = native_field(&bounds, "end")?;
    if start.eq(sentinel(py, "START")?)? || end.eq(sentinel(py, "END")?)? {
        return Ok(bounds);
    }
    let session = CursorSession::new(py)?;
    let window = (
        order_scalar(&session, &token(py, &start)?, &cursor_type)?,
        order_scalar(&session, &token(py, &end)?, &cursor_type)?,
    );
    let limit = order_scalar(&session, &cursor_end, &cursor_type)?;
    match session.checked(cursor_end_clamp((&window.0, &window.1), &limit, &session))? {
        CursorEndClamp::Unchanged => Ok(bounds),
        CursorEndClamp::Empty => replace(
            &bounds,
            &[("start", cursor_end.clone()), ("end", cursor_end)],
        ),
        CursorEndClamp::Capped => replace(&bounds, &[("end", cursor_end)]),
    }
}

fn timestamp_value<'py>(clock: &Bound<'py, PyAny>, scalar: &Scalar) -> PyResult<Bound<'py, PyAny>> {
    let ScalarValue::Timestamp(timestamp) = &scalar.value else {
        return Ok(clock.py().None().into_bound(clock.py()));
    };
    new_record_with(
        clock.py(),
        RecordId::TimestampValue,
        &[("value", replaced(clock, &timestamp.local, timestamp.fold)?)],
    )
}

/// The invocation-anchored rolling microbatch window, or `None` without a grain or lookback.
fn rolling_window_bounds<'py>(
    model: &Bound<'py, PyAny>,
    invocation_time: &Bound<'py, PyAny>,
) -> PyResult<Bound<'py, PyAny>> {
    let py = model.py();
    let grain = config_str(model, "cursor_grain")?;
    let lookback = object(
        py,
        resolve_microbatch_lookback(model, grain.extract::<Option<String>>()?)?,
    )?;
    let lookback = if lookback.is_truthy()? {
        lookback
    } else {
        string(py, "")
    };
    let span = duration(&lookback)?;
    let (Some(span), false) = (span, grain.is_none()) else {
        return Ok(py.None().into_bound(py));
    };
    let utc = imported(py, "datetime", "UTC")?;
    let moment = if invocation_time.is_none() {
        imported(py, "datetime", "datetime")?.call_method1("now", (&utc,))?
    } else {
        invocation_time.clone()
    };
    let moment = if native_field(&moment, "tzinfo")?.is_none() {
        call_with(&native_field(&moment, "replace")?, &[("tzinfo", utc)])?
    } else {
        utc_clock(&moment)?
    };
    let session = CursorSession::new(py)?;
    let Some(grain) = session.grain(&grain)? else {
        return Ok(py.None().into_bound(py));
    };
    let clock = Scalar {
        value: ScalarValue::Timestamp(Timestamp {
            local: local_date_time(&moment)?,
            fold: native_field(&moment, "fold")?.extract::<u8>()? != 0,
            offset: ZoneOffset::Fixed(0),
            template: None,
            source_text: None,
        }),
        handle: None,
    };
    let (start, end) = session.checked(rolling_window(&clock, &span, grain))?;
    let mut bounds = new_record_with(
        py,
        RecordId::CursorBounds,
        &[
            ("start", timestamp_value(&moment, &start)?),
            ("end", timestamp_value(&moment, &end)?),
        ],
    )?;
    let cursor_start = config_bound(model, "cursor_start")?;
    let timestamp = string(py, TIMESTAMP);
    if !cursor_start.is_none()
        && order(
            &session,
            (&token(py, &native_field(&bounds, "start")?)?, &cursor_start),
            &timestamp,
        )? == Ordering::Less
    {
        bounds = replace(&bounds, &[("start", cursor_start)])?;
    }
    clamp_cursor_end(bounds, model)
}

fn apply_future_safety<'py>(
    bounds: Bound<'py, PyAny>,
    typing: (&Bound<'py, PyAny>, &Bound<'py, PyAny>),
    inputs: &CursorWindowInputs<'py>,
    input_evidence: Bound<'py, PyAny>,
) -> PyResult<Bound<'py, PyAny>> {
    let py = bounds.py();
    if bounds.is_none() || native_field(&bounds, "end")?.eq(sentinel(py, "END")?)? {
        return Ok(bounds);
    }
    resolve_future_cursor_safety(
        py,
        Some(keywords(
            py,
            &[
                ("bounds", bounds),
                ("cursor_type", typing.0.clone()),
                ("cursor_grain", typing.1.clone()),
                ("config", inputs.policies.0.clone()),
                ("invocation_time", inputs.invocation_time.clone()),
                (
                    "has_complete_override",
                    boolean(
                        py,
                        !inputs.overrides.0.is_none() && !inputs.overrides.1.is_none(),
                    )?,
                ),
                ("input_evidence", input_evidence),
            ],
        )?),
    )
}

pub(crate) fn bounded_override<'py>(
    model: &Bound<'py, PyAny>,
    inputs: &CursorWindowInputs<'py>,
) -> PyResult<Bound<'py, PyAny>> {
    object(
        model.py(),
        resolve_bounded_cursor_override(
            &inputs.overrides.0.clone(),
            &inputs.overrides.1.clone(),
            &config_str(model, "cursor_type")?,
            &config_str(model, "cursor_grain")?,
        )?,
    )
}

fn backfill_duration<'py>(backfill: &Bound<'py, PyAny>) -> PyResult<Bound<'py, PyAny>> {
    if native_field(backfill, "action")?.eq(BOUNDED_BACKFILL)? {
        return native_field(backfill, "duration");
    }
    Ok(backfill.py().None().into_bound(backfill.py()))
}

/// Cursor bounds and the maximum-start policy inputs, shared by both window computations.
pub(crate) struct BoundsRequest<'py> {
    pub(crate) cursor_snapshot: Bound<'py, PyAny>,
    pub(crate) typing: (Bound<'py, PyAny>, Bound<'py, PyAny>),
    pub(crate) lookback: Bound<'py, PyAny>,
    pub(crate) is_microbatch: bool,
}

pub(crate) fn snapshot_bounds<'py>(
    model: &Bound<'py, PyAny>,
    inputs: &CursorWindowInputs<'py>,
    request: BoundsRequest<'py>,
) -> PyResult<Bound<'py, PyAny>> {
    let py = model.py();
    let policy = new_record_with(
        py,
        RecordId::MaximumStartPolicyInputs,
        &[
            ("config", inputs.policies.1.clone()),
            ("invocation_time", inputs.invocation_time.clone()),
            (
                "incremental_strategy",
                config_str(model, "incremental_strategy")?,
            ),
            ("incremental_mode", config_str(model, "incremental_mode")?),
        ],
    )?;
    let bounds = compute_cursor_bounds(
        py,
        Some(keywords(
            py,
            &[
                ("cursor_snapshot", request.cursor_snapshot.clone()),
                ("cursor_type", request.typing.0.clone()),
                ("cursor_start", config_bound(model, "cursor_start")?),
                ("lookback", request.lookback),
                ("backfill_duration", backfill_duration(&inputs.backfill)?),
                ("start_cursor_override", inputs.overrides.0.clone()),
                ("end_cursor_override", inputs.overrides.1.clone()),
                ("is_microbatch", boolean(py, request.is_microbatch)?),
                ("cursor_grain", request.typing.1.clone()),
                ("maximum_start_policy", policy),
            ],
        )?),
    )?;
    let bounds = apply_future_safety(
        bounds,
        (&request.typing.0, &request.typing.1),
        inputs,
        native_field(&request.cursor_snapshot, "input_evidence")?,
    )?;
    clamp_cursor_end(bounds, model)
}

pub(crate) fn model_cursor_snapshot<'py>(
    model: &Bound<'py, PyAny>,
    snapshot: &Bound<'py, PyAny>,
) -> PyResult<Bound<'py, PyAny>> {
    native_field(snapshot, "cursor_snapshots")?.call_method1("get", (name_object(model)?,))
}

pub(crate) fn without_destination<'py>(
    cursor_snapshot: Bound<'py, PyAny>,
) -> PyResult<Bound<'py, PyAny>> {
    without_destination_cursor(&cursor_snapshot)
}

/// Compute the real overall cursor range for microbatch batch splitting.
pub(crate) fn microbatch_range<'py>(
    model: &Bound<'py, PyAny>,
    inputs: &CursorWindowInputs<'py>,
) -> PyResult<Bound<'py, PyAny>> {
    let py = model.py();
    if !config_str(model, "materialized")?.eq(INCREMENTAL)?
        || !config_str(model, "incremental_mode")?.eq(MICROBATCH)?
        || config_str(model, "cursor")?.is_none()
    {
        return Ok(py.None().into_bound(py));
    }
    let bounded = bounded_override(model, inputs)?;
    if !bounded.is_none() {
        return clamp_cursor_end(bounded, model);
    }
    let strategy = config_str(model, "microbatch_strategy")?;
    if strategy.eq(ROLLING_WINDOW)? {
        let rolling = rolling_window_bounds(model, &inputs.invocation_time)?;
        if inputs.full_refresh && !rolling.is_none() {
            let cursor_start = config_bound(model, "cursor_start")?;
            if !cursor_start.is_none() {
                return replace(&rolling, &[("start", cursor_start)]);
            }
        }
        return Ok(rolling);
    }
    if inputs.runtime_owned {
        return Ok(py.None().into_bound(py));
    }
    let mut cursor_snapshot = model_cursor_snapshot(model, &inputs.snapshot)?;
    if cursor_snapshot.is_none() {
        return Ok(cursor_snapshot);
    }
    if inputs.replaces_relation && !inputs.full_refresh {
        cursor_snapshot = without_destination(cursor_snapshot)?;
    }
    let cursor_type = config_str(model, "cursor_type")?;
    let mut grains: Vec<Bound<'py, PyAny>> = Vec::new();
    for relation in inputs.cursor_input_relations.try_iter()? {
        grains.push(native_field(&relation?, "cursor_grain")?);
    }
    let effective_grain = resolve_effective_timestamp_grain(
        &cursor_type,
        &config_str(model, "cursor_grain")?,
        &PyTuple::new(py, grains)?.into_any(),
        strategy.extract::<Option<String>>()?.as_deref(),
    )?;
    if cursor_type.eq(TIMESTAMP)? && !effective_grain.is_none() {
        cursor_snapshot =
            normalize_cursor_snapshot_grain(&cursor_snapshot, &cursor_type, &effective_grain)?;
    }
    let lookback = object(
        py,
        resolve_microbatch_lookback(model, effective_grain.extract::<Option<String>>()?)?,
    )?;
    snapshot_bounds(
        model,
        inputs,
        BoundsRequest {
            cursor_snapshot,
            typing: (cursor_type, effective_grain),
            lookback,
            is_microbatch: false,
        },
    )
}

pub(crate) fn compile_input_error(py: Python<'_>, message: String) -> PyErr {
    new_plain_error(py, ErrorId::CompileInput, &message)
}

fn window_inputs<'py>(args: &KeywordArguments<'py>) -> PyResult<CursorWindowInputs<'py>> {
    Ok(CursorWindowInputs {
        snapshot: args.value("snapshot")?,
        backfill: args.value("backfill")?,
        overrides: (
            args.value("start_cursor_override")?,
            args.value("end_cursor_override")?,
        ),
        runtime_owned: args.flag("runtime_owned_cursor_bounds")?,
        policies: (
            args.value("future_cursor_config")?,
            args.value("start_cursor_config")?,
        ),
        invocation_time: args.value("invocation_time")?,
        full_refresh: args.flag("full_refresh")?,
        replaces_relation: args.flag("replaces_relation")?,
        cursor_input_relations: if args.passed("cursor_input_relations")? {
            args.value("cursor_input_relations")?
        } else {
            PyTuple::empty(args.value("snapshot")?.py()).into_any()
        },
    })
}

/// Compute cursor bounds for inclusion on the plan entry.
#[pyfunction]
#[pyo3(signature = (**kwargs))]
pub(crate) fn compute_plan_cursor_bounds<'py>(
    py: Python<'py>,
    kwargs: Option<Bound<'py, PyDict>>,
) -> PyResult<Bound<'py, PyAny>> {
    let args = KeywordArguments::read(
        py,
        "compute_plan_cursor_bounds",
        kwargs,
        (&WINDOW_REQUIRED, &WINDOW_OPTIONAL),
    )?;
    plan_cursor_bounds(&args.value("model")?, &window_inputs(&args)?)
}

/// Compute the real overall cursor range for microbatch batch splitting.
#[pyfunction]
#[pyo3(signature = (**kwargs))]
pub(crate) fn compute_microbatch_range<'py>(
    py: Python<'py>,
    kwargs: Option<Bound<'py, PyDict>>,
) -> PyResult<Bound<'py, PyAny>> {
    let args = KeywordArguments::read(
        py,
        "compute_microbatch_range",
        kwargs,
        (&WINDOW_REQUIRED, &WINDOW_OPTIONAL),
    )?;
    microbatch_range(&args.value("model")?, &window_inputs(&args)?)
}

/// Resolve typed cursor overrides to generic start/end for one model.
#[pyfunction]
#[pyo3(signature = (*, model, cursor_overrides, start_cursor_override, end_cursor_override))]
pub(crate) fn resolve_cursor_overrides<'py>(
    model: &Bound<'py, PyAny>,
    cursor_overrides: &Bound<'py, PyAny>,
    start_cursor_override: Bound<'py, PyAny>,
    end_cursor_override: Bound<'py, PyAny>,
) -> PyResult<(Bound<'py, PyAny>, Bound<'py, PyAny>)> {
    model_cursor_overrides(
        model,
        cursor_overrides,
        (start_cursor_override, end_cursor_override),
    )
}
