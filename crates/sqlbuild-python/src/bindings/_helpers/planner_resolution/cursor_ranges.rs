//! Plan-entry cursor windows: overrides, planner-owned bounds and microbatch ranges.

use sqlbuild_host::entries::models::ExecutorHost;
use sqlbuild_python_base::records::main::construct::new_plain_error::new_plain_error;

use crate::bindings::_helpers::compiled_views::model_reads;
use crate::bindings::_helpers::cursor::conversion::CursorSession;
use crate::bindings::_helpers::cursor::scalars::{parse_value, sentinel_to_token};
use crate::bindings::_helpers::cursor::values::{local_date_time, replaced};
use crate::bindings::_helpers::model_config::typed_config::{
    config_cursor_bound as typed_cursor_bound, config_text_object, model_values,
};
use crate::bindings::_helpers::plan_output::display::resolve_bounded_cursor_override;
use crate::bindings::_helpers::planner_entries::models::ModelPlanContext;
use crate::bindings::_helpers::planner_phases::contexts::class_field;
use crate::bindings::_helpers::planner_phases::objects::object;
use crate::bindings::_helpers::planner_resolution::cursor_bounds::compute_cursor_bounds;
use crate::bindings::_helpers::planner_resolution::cursor_bounds::duration;
use crate::bindings::_helpers::planner_resolution::cursor_policies::{
    future_cursor_config, start_cursor_config,
};
use crate::bindings::_helpers::planner_resolution::cursor_safety::{replace, utc_clock};
use crate::bindings::_helpers::planner_resolution::cursor_windows::cursor_inputs_without_rows;
use crate::bindings::_helpers::planner_resolution::cursor_windows::normalize_cursor_snapshot_grain;
use crate::bindings::_helpers::planner_resolution::cursor_windows::resolve_effective_timestamp_grain;
use crate::bindings::_helpers::planner_resolution::cursor_windows::without_destination_cursor;
use crate::bindings::_helpers::planner_resolution::future_safety::resolve_future_cursor_safety;
use crate::bindings::_helpers::planner_resolution::values::string;
use crate::bindings::_helpers::planner_strategy::model_entries::resolve_microbatch_lookback;
use pyo3::prelude::{Bound, PyAny, PyAnyMethods, PyModule, PyModuleMethods, PyResult, Python};
use pyo3::types::{PyDict, PyString, PyTuple};
use pyo3::{IntoPyObject, PyErr, pyfunction, wrap_pyfunction};
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
use std::cmp::Ordering;

const INCREMENTAL: &str = "incremental";
const MICROBATCH: &str = "microbatch";
const ROLLING_WINDOW: &str = "rolling_window";
const TIMESTAMP: &str = "timestamp";
const INTEGER: &str = "integer";
const BOUNDED_BACKFILL: &str = "bounded";
const FULL_BACKFILL: &str = "full";
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

/// The planner inputs that decide one model's plan-time cursor window.
pub(crate) struct CursorWindowInputs<'py> {
    pub(crate) snapshot: Bound<'py, PyAny>,
    pub(crate) backfill: Bound<'py, PyAny>,
    pub(crate) overrides: (Bound<'py, PyAny>, Bound<'py, PyAny>),
    pub(crate) runtime_owned: bool,
    pub(crate) policies: (Bound<'py, PyAny>, Bound<'py, PyAny>),
    pub(crate) invocation_time: Bound<'py, PyAny>,
    pub(crate) full_refresh: bool,
    pub(crate) replaces_relation: bool,
    pub(crate) cursor_input_relations: Bound<'py, PyAny>,
}

/// One string-valued model configuration entry.
pub(crate) fn config_str<'py>(model: &Bound<'py, PyAny>, key: &str) -> PyResult<Bound<'py, PyAny>> {
    let py: Python<'py> = model.py();
    Ok(config_text_object(py, &model_values(model)?, key)?
        .unwrap_or_else(|| py.None().into_bound(py)))
}

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

fn order<'py>(
    session: &CursorSession<'py>,
    tokens: (&Bound<'py, PyAny>, &Bound<'py, PyAny>),
    cursor_type: &Bound<'py, PyAny>,
) -> PyResult<Ordering> {
    let left = order_scalar(session, tokens.0, cursor_type)?;
    let right = order_scalar(session, tokens.1, cursor_type)?;
    session.checked(compare(&left, &right, session))
}

fn token<'py>(py: Python<'py>, bound: &Bound<'py, PyAny>) -> PyResult<Bound<'py, PyAny>> {
    Ok(PyString::new(py, &sentinel_to_token(bound)?).into_any())
}

fn sentinel<'py>(py: Python<'py>, name: &str) -> PyResult<Bound<'py, PyAny>> {
    host_entry(py, ExecutorHost::BoundSentinel)?.getattr(name)
}

/// `bounds` clamped so a concrete window never reaches the exclusive configured `cursor_end`.
fn clamp_cursor_end<'py>(
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

fn bounded_override<'py>(
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
struct BoundsRequest<'py> {
    cursor_snapshot: Bound<'py, PyAny>,
    typing: (Bound<'py, PyAny>, Bound<'py, PyAny>),
    lookback: Bound<'py, PyAny>,
    is_microbatch: bool,
}

fn snapshot_bounds<'py>(
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

fn model_cursor_snapshot<'py>(
    model: &Bound<'py, PyAny>,
    snapshot: &Bound<'py, PyAny>,
) -> PyResult<Bound<'py, PyAny>> {
    native_field(snapshot, "cursor_snapshots")?
        .call_method1("get", (model_reads::name_object(model)?,))
}

fn without_destination<'py>(cursor_snapshot: Bound<'py, PyAny>) -> PyResult<Bound<'py, PyAny>> {
    without_destination_cursor(&cursor_snapshot)
}

/// Compute cursor bounds for inclusion on the plan entry.
pub(crate) fn plan_cursor_bounds<'py>(
    model: &Bound<'py, PyAny>,
    inputs: &CursorWindowInputs<'py>,
) -> PyResult<Bound<'py, PyAny>> {
    let py = model.py();
    if !config_str(model, "materialized")?.eq(INCREMENTAL)?
        || config_str(model, "cursor")?.is_none()
    {
        return Ok(py.None().into_bound(py));
    }
    let bounded = bounded_override(model, inputs)?;
    if !bounded.is_none() {
        return clamp_cursor_end(bounded, model);
    }
    if inputs.runtime_owned || inputs.full_refresh {
        return Ok(py.None().into_bound(py));
    }
    let mut cursor_snapshot = model_cursor_snapshot(model, &inputs.snapshot)?;
    if cursor_snapshot.is_none() {
        return Ok(cursor_snapshot);
    }
    if native_field(&inputs.backfill, "action")?.eq(FULL_BACKFILL)? {
        cursor_snapshot = without_destination(cursor_snapshot)?;
    }
    snapshot_bounds(
        model,
        inputs,
        BoundsRequest {
            cursor_snapshot,
            typing: (
                config_str(model, "cursor_type")?,
                config_str(model, "cursor_grain")?,
            ),
            lookback: config_str(model, "lookback")?,
            is_microbatch: config_str(model, "incremental_mode")?.eq(MICROBATCH)?,
        },
    )
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

/// Empty inputs that leave a planner-owned window empty, and whether others have rows.
pub(crate) fn empty_cursor_inputs<'py>(
    model: &Bound<'py, PyAny>,
    inputs: &CursorWindowInputs<'py>,
) -> PyResult<(Bound<'py, PyAny>, bool)> {
    let py = model.py();
    let nothing =
        || -> PyResult<(Bound<'py, PyAny>, bool)> { Ok((PyTuple::empty(py).into_any(), false)) };
    if inputs.full_refresh
        || inputs.runtime_owned
        || !config_str(model, "materialized")?.eq(INCREMENTAL)?
        || config_str(model, "cursor")?.is_none()
        || config_str(model, "microbatch_strategy")?.eq(ROLLING_WINDOW)?
        || (!inputs.overrides.0.is_none() && !inputs.overrides.1.is_none())
    {
        return nothing();
    }
    let cursor_snapshot = model_cursor_snapshot(model, &inputs.snapshot)?;
    if cursor_snapshot.is_none() {
        return nothing();
    }
    let decision = cursor_inputs_without_rows(&cursor_snapshot)?;
    Ok((
        native_field(&decision, "input_names")?,
        native_field(&decision, "waiting_on_empty_inputs")?.is_truthy()?,
    ))
}

/// The future-end and automatic-start policies effective for one model.
pub(crate) fn cursor_safety_configs<'py>(
    model: &Bound<'py, PyAny>,
    context: &Bound<'py, PyAny>,
) -> PyResult<(Bound<'py, PyAny>, Bound<'py, PyAny>)> {
    Ok((
        future_cursor_config(
            model,
            &class_field(context, |c: &ModelPlanContext| &c.future_cursor_config)?,
        )?,
        start_cursor_config(
            model,
            &class_field(context, |c: &ModelPlanContext| &c.start_cursor_config)?,
        )?,
    ))
}

fn compile_input_error(py: Python<'_>, message: String) -> PyErr {
    new_plain_error(py, ErrorId::CompileInput, &message)
}

/// Resolve typed cursor overrides to generic start/end for one model.
pub(crate) fn model_cursor_overrides<'py>(
    model: &Bound<'py, PyAny>,
    cursor_overrides: &Bound<'py, PyAny>,
    (start, end): (Bound<'py, PyAny>, Bound<'py, PyAny>),
) -> PyResult<(Bound<'py, PyAny>, Bound<'py, PyAny>)> {
    let py = model.py();
    let cursor_type = config_str(model, "cursor_type")?;
    let (mut resolved_start, mut resolved_end) = (start, end);
    let typed_fields = if cursor_overrides.is_none() {
        None
    } else if cursor_type.eq(TIMESTAMP)? {
        Some(("start_ts", "end_ts"))
    } else if cursor_type.eq(INTEGER)? {
        Some(("start_int", "end_int"))
    } else {
        None
    };
    if let Some((start_field, end_field)) = typed_fields {
        let typed_start = native_field(cursor_overrides, start_field)?;
        if !typed_start.is_none() {
            resolved_start = token(py, &typed_start)?;
        }
        let typed_end = native_field(cursor_overrides, end_field)?;
        if !typed_end.is_none() {
            resolved_end = token(py, &typed_end)?;
        }
    }
    let session = CursorSession::new(py)?;
    let name = model_reads::name_object(model)?.str()?;
    let cursor_start = config_bound(model, "cursor_start")?;
    let cursor_end = config_bound(model, "cursor_end")?;
    if !resolved_start.is_none()
        && !cursor_start.is_none()
        && order(&session, (&resolved_start, &cursor_start), &cursor_type)? == Ordering::Less
    {
        return Err(compile_input_error(
            py,
            format!(
                "model '{name}': explicit start {} is before cursor_start {}",
                resolved_start.str()?,
                cursor_start.str()?
            ),
        ));
    }
    if !cursor_end.is_none() {
        for (value, label) in [(&resolved_start, "start"), (&resolved_end, "end")] {
            if !value.is_none()
                && order(&session, (value, &cursor_end), &cursor_type)? != Ordering::Less
            {
                return Err(compile_input_error(
                    py,
                    format!(
                        "model '{name}': explicit {label} {} is at or beyond exclusive cursor_end {}",
                        value.str()?,
                        cursor_end.str()?
                    ),
                ));
            }
        }
    }
    Ok((resolved_start, resolved_end))
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
fn compute_plan_cursor_bounds<'py>(
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
fn compute_microbatch_range<'py>(
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
fn resolve_cursor_overrides<'py>(
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

pub(super) fn register(module: &Bound<'_, PyModule>) -> PyResult<()> {
    module.add_function(wrap_pyfunction!(compute_plan_cursor_bounds, module)?)?;
    module.add_function(wrap_pyfunction!(compute_microbatch_range, module)?)?;
    module.add_function(wrap_pyfunction!(resolve_cursor_overrides, module)?)
}
