//! Model cursor orchestration over live warehouse snapshot facts.

use sqlbuild_host::entries::models::{AdapterHost, ExecutorHost};

use crate::bindings::_helpers::boundary::python_calls::call_with;
use crate::bindings::_helpers::boundary::python_calls::keywords;
use crate::bindings::_helpers::compile_attachment::cursor_roles::resolve_cursor_input_roles;
use crate::bindings::_helpers::compiled_views::model_reads;
use crate::bindings::_helpers::model_config::typed_config::{
    config_cursor_bound, config_text_object, model_values,
};
use crate::bindings::_helpers::native_host::host;
use crate::bindings::_helpers::plan_output::display::resolve_bounded_cursor_override;
use crate::bindings::_helpers::planner_entries::models::ModelPlanContext;
use crate::bindings::_helpers::planner_phases::contexts::class_field;
use crate::bindings::_helpers::planner_phases::objects::object;
use crate::bindings::_helpers::planner_resolution::constants::CURSOR_START;
use crate::bindings::_helpers::planner_resolution::cursor_bounds::compute_cursor_bounds;
use crate::bindings::_helpers::planner_resolution::cursor_policies::{
    future_cursor_config, start_cursor_config,
};
use crate::bindings::_helpers::planner_resolution::cursor_windows::cursor_inputs_without_rows;
use crate::bindings::_helpers::planner_resolution::cursor_windows::empty_cursor_window;
use crate::bindings::_helpers::planner_resolution::cursor_windows::empty_input_rebuild_refusal;
use crate::bindings::_helpers::planner_resolution::cursor_windows::unreadable_cursor_inputs_error;
use crate::bindings::_helpers::planner_resolution::cursor_windows::without_destination_cursor;
use crate::bindings::_helpers::planner_resolution::future_safety::resolve_future_cursor_safety;
use crate::bindings::_helpers::planner_resolution::models::ModelSqlRequest;
use crate::bindings::_helpers::planner_resolution::values::string;
use crate::bindings::_helpers::records::construct::error_class;
use crate::bindings::_helpers::records::construct::new_record_with;
use crate::bindings::_helpers::records::fields::field as native_field;
use crate::bindings::_helpers::records::ids::{ErrorId, RecordId};
use pyo3::prelude::{Bound, PyAny, PyAnyMethods, PyResult, Python};
use pyo3::{IntoPyObject, PyErr};

fn config<'py>(model: &Bound<'py, PyAny>, key: &str) -> PyResult<Bound<'py, PyAny>> {
    let py: Python<'py> = model.py();
    let values = model_values(model)?;
    let value: Option<Bound<'py, PyAny>> = if key == CURSOR_START {
        config_cursor_bound(py, &values, key)?
    } else {
        config_text_object(py, &values, key)?
    };
    Ok(value.unwrap_or_else(|| py.None().into_bound(py)))
}

/// Bounds the run resolves when it starts, as for inputs another node of the run produces.
pub(crate) fn unbounded(py: Python<'_>) -> PyResult<Bound<'_, PyAny>> {
    let sentinel = host::entry(py, ExecutorHost::BoundSentinel)?;
    new_record_with(
        py,
        RecordId::CursorBounds,
        &[
            ("start", native_field(&sentinel, "START")?),
            ("end", native_field(&sentinel, "END")?),
        ],
    )
}

fn destination_exists(model: &Bound<'_, PyAny>, snapshot: &Bound<'_, PyAny>) -> PyResult<bool> {
    let relation = native_field(snapshot, "existing_relations")?
        .call_method1("get", (model_reads::name_object(model)?,))?;
    if relation.is_none() {
        return Ok(false);
    }
    let destination = model_reads::destination(model)?;
    let key = call_with(
        &host::entry(model.py(), AdapterHost::RelationLookup)?.getattr("key")?,
        &[
            ("database", native_field(&destination, "database")?),
            ("schema", native_field(&destination, "schema")?),
            ("name", native_field(&destination, "name")?),
        ],
    )?;
    native_field(&relation, "identity")?.eq(key)
}

fn history_redirected(model: &Bound<'_, PyAny>, snapshot: &Bound<'_, PyAny>) -> PyResult<bool> {
    let target = native_field(snapshot, "target_relation")?;
    Ok(!target.is_none()
        && target.ne(native_field(
            &model_reads::destination(model)?,
            "qualified_name",
        )?)?)
}

/// One model's cursor bounds from its snapshot, overrides and cursor policies; `None` when unbounded by a full refresh.
pub(crate) fn compute_model_cursor_bounds<'py>(
    request: &ModelSqlRequest<'py>,
    context: &Bound<'py, PyAny>,
) -> PyResult<Bound<'py, PyAny>> {
    let model = request.model.clone();
    let py = model.py();
    if !config(&model, "materialized")?.eq("incremental")? || config(&model, "cursor")?.is_none() {
        return Ok(py.None().into_bound(py));
    }
    let incremental_mode = config(&model, "incremental_mode")?;
    let future_config = future_cursor_config(
        &model,
        &class_field(context, |c: &ModelPlanContext| &c.future_cursor_config)?,
    )?;
    let start_config = start_cursor_config(
        &model,
        &class_field(context, |c: &ModelPlanContext| &c.start_cursor_config)?,
    )?;
    if incremental_mode.eq("microbatch")? {
        return unbounded(py);
    }
    let cursor_type = config(&model, "cursor_type")?;
    let cursor_grain = config(&model, "cursor_grain")?;
    let (start_override, end_override) = request.cursor_overrides.clone();
    let bounded = object(
        py,
        resolve_bounded_cursor_override(
            &start_override,
            &end_override,
            &cursor_type,
            &cursor_grain,
        )?,
    )?;
    if !bounded.is_none() {
        return Ok(bounded);
    }
    let snapshot = &request.snapshot;
    let mut cursor_snapshot = native_field(snapshot, "cursor_snapshots")?
        .call_method1("get", (model_reads::name_object(&model)?,))?;
    let roles = resolve_cursor_input_roles(&model)?;
    let producers = class_field(context, |c: &ModelPlanContext| {
        &c.runtime_cursor_producer_names
    })?;
    let mut runtime_owned = false;
    for name in native_field(&roles, "watermark_inputs")?.try_iter()? {
        if producers.contains(name?)? {
            runtime_owned = true;
            break;
        }
    }
    let full_refresh = request.full_refresh;
    let replaces = request.replaces_relation;
    if !full_refresh && !runtime_owned && !cursor_snapshot.is_none() {
        let empty = native_field(
            &cursor_inputs_without_rows(&cursor_snapshot)?,
            "input_names",
        )?;
        if empty.is_truthy()?
            && replaces
            && (destination_exists(&model, snapshot)?
                || history_redirected(&model, &cursor_snapshot)?)
        {
            let refusal = object(
                py,
                empty_input_rebuild_refusal(
                    &model_reads::name_object(&model)?.extract::<String>()?,
                    empty.extract::<Vec<String>>()?,
                    cursor_type.extract::<Option<String>>()?.as_deref(),
                ),
            )?;
            return Err(PyErr::from_value(call_with(
                &error_class(py, ErrorId::PlannerInput)?,
                &[
                    ("message", refusal.get_item(0)?),
                    ("code", string(py, "S302")),
                    ("help", refusal.get_item(1)?),
                ],
            )?));
        }
        if empty.is_truthy()? {
            return empty_cursor_window(&cursor_type, &config(&model, "cursor_start")?);
        }
        if !native_field(&cursor_snapshot, "watermarks_available")?.is_truthy()? {
            let unreadable = native_field(&cursor_snapshot, "unreadable_input_names")?;
            let names = if unreadable.is_truthy()? {
                unreadable
            } else {
                native_field(&cursor_snapshot, "unavailable_watermark_tags")?
            };
            return Err(PyErr::from_value(unreadable_cursor_inputs_error(
                py,
                &model_reads::name_object(&model)?.extract::<String>()?,
                cursor_type.extract::<Option<String>>()?.as_deref(),
                names.extract::<Vec<String>>()?,
            )?));
        }
    }
    if full_refresh {
        return Ok(py.None().into_bound(py));
    }
    if runtime_owned {
        return unbounded(py);
    }
    if cursor_snapshot.is_none() {
        return Ok(py.None().into_bound(py));
    }
    if replaces {
        cursor_snapshot = without_destination_cursor(&cursor_snapshot)?;
    }
    let backfill = &request.backfill;
    let duration = if native_field(backfill, "action")?.eq("bounded")? {
        native_field(backfill, "duration")?
    } else {
        py.None().into_bound(py)
    };
    let policy = new_record_with(
        py,
        RecordId::MaximumStartPolicyInputs,
        &[
            ("config", start_config),
            (
                "invocation_time",
                class_field(context, |c: &ModelPlanContext| &c.invocation_time)?,
            ),
            (
                "incremental_strategy",
                config(&model, "incremental_strategy")?,
            ),
            ("incremental_mode", incremental_mode),
        ],
    )?;
    let bounds = compute_cursor_bounds(
        py,
        Some(keywords(
            py,
            &[
                ("cursor_snapshot", cursor_snapshot.clone()),
                ("cursor_type", cursor_type.clone()),
                ("cursor_start", config(&model, "cursor_start")?),
                ("lookback", config(&model, "lookback")?),
                ("backfill_duration", duration),
                ("start_cursor_override", start_override.clone()),
                ("end_cursor_override", end_override.clone()),
                (
                    "is_microbatch",
                    false.into_pyobject(py)?.to_owned().into_any(),
                ),
                ("cursor_grain", cursor_grain.clone()),
                ("maximum_start_policy", policy),
            ],
        )?),
    )?;
    if bounds.is_none() {
        return Ok(bounds);
    }
    resolve_future_cursor_safety(
        py,
        Some(keywords(
            py,
            &[
                ("bounds", bounds),
                ("cursor_type", cursor_type),
                ("cursor_grain", cursor_grain),
                ("config", future_config),
                (
                    "invocation_time",
                    class_field(context, |c: &ModelPlanContext| &c.invocation_time)?,
                ),
                (
                    "has_complete_override",
                    (!start_override.is_none() && !end_override.is_none())
                        .into_pyobject(py)?
                        .to_owned()
                        .into_any(),
                ),
                (
                    "input_evidence",
                    native_field(&cursor_snapshot, "input_evidence")?,
                ),
            ],
        )?),
    )
}
