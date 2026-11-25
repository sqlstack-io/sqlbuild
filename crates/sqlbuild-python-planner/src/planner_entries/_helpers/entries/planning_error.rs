//! One model that can't be planned becomes an error row, so the rest of the plan still plans.

use pyo3::prelude::{Bound, PyAny, PyAnyMethods, PyErr, PyResult, Python};
use sqlbuild_python_base::records::main::construct::enum_member::enum_member;
use sqlbuild_python_base::records::main::construct::error_class::error_class;
use sqlbuild_python_base::records::main::construct::new_record_with::new_record_with;
use sqlbuild_python_base::records::main::fields::field::field as native_field;
use sqlbuild_python_base::records::types::{ErrorId, RecordId};
use sqlbuild_python_project::compiled_views::main::compiled_models::destination::destination;
use sqlbuild_python_project::compiled_views::main::compiled_models::model_key::model_key;
use sqlbuild_python_project::compiled_views::main::compiled_models::name_object::name_object;

use crate::planner_entries::_helpers::entries::values::{planner_model, planner_type, string};
use crate::planner_entries::constants::SOURCE_VALIDATION_CODE_PREFIX;
use crate::planner_resolution::models::WindowFit;
use crate::planner_strategy::main::strategy::materialization_of::materialization_of;

/// The model's error row and its error-severity warning, or `error` itself when it isn't a planning error: a compile or validation error, such as a capped watermark edge (P001) or a source that breaks its contract (S4xx), still fails the plan.
pub(crate) fn planning_error<'py>(
    model: &Bound<'py, PyAny>,
    error: PyErr,
) -> PyResult<(Bound<'py, PyAny>, Bound<'py, PyAny>)> {
    let py: Python<'py> = model.py();
    let value: Bound<'py, PyAny> = error.value(py).clone().into_any();
    let attribute = |name: &str| -> PyResult<Bound<'py, PyAny>> {
        Ok(value
            .getattr_opt(name)?
            .unwrap_or_else(|| py.None().into_bound(py)))
    };
    let code: String = attribute("code")?.str()?.to_string();
    if !value.is_instance(&error_class(py, ErrorId::PlannerInput)?)?
        || code.starts_with(SOURCE_VALIDATION_CODE_PREFIX)
    {
        return Err(error);
    }
    let message: Bound<'py, PyAny> = match attribute("message")? {
        message if message.is_truthy()? => message,
        _ => value.str()?.into_any(),
    };
    let warning: Bound<'py, PyAny> = new_record_with(
        py,
        RecordId::PlanWarning,
        &[
            ("model_name", name_object(model)?),
            (
                "severity",
                enum_member(py, RecordId::WarningSeverity, "ERROR")?,
            ),
            ("message", message),
            ("code", attribute("code")?),
            ("help", attribute("help")?),
        ],
    )?;
    let entry = skipped_entry(model, ("NO_CHANGE", ("planning_error", warning.clone())))?;
    Ok((entry, warning))
}

/// The row of a model whose explicit cursor window ends at or before its `cursor_start` `floor`: skipped, with that as its reason.
pub(crate) fn window_skip<'py>(
    model: &Bound<'py, PyAny>,
    floor: &str,
) -> PyResult<Bound<'py, PyAny>> {
    let reason = string(
        model.py(),
        &format!("window ends before cursor_start {floor}"),
    );
    skipped_entry(
        model,
        ("WINDOW_BEFORE_CURSOR_START", ("skip_reason", reason)),
    )
}

/// A skipped model row with `reason` and one extra field, which runs nothing.
fn skipped_entry<'py>(
    model: &Bound<'py, PyAny>,
    (reason, extra): (&str, (&str, Bound<'py, PyAny>)),
) -> PyResult<Bound<'py, PyAny>> {
    let py: Python<'py> = model.py();
    planner_model(
        py,
        RecordId::ModelPlanEntry,
        &[
            ("key", model_key(model)?),
            ("name", name_object(model)?),
            ("relative_path", native_field(model, "relative_path")?),
            ("materialization_type", materialization_of(model)?),
            ("action", planner_type(py, RecordId::PlanAction, "SKIP")?),
            ("reason", planner_type(py, RecordId::PlanReason, reason)?),
            ("destination", destination(model)?),
            ("fingerprint_query_sql", string(py, "")),
            ("resolved_sql", string(py, "")),
            ("logical_ddl", string(py, "")),
            extra,
        ],
    )
}

/// The warning on a model whose explicit cursor window started before its `cursor_start`, which starts it there; `None` for any other fit.
pub(crate) fn window_clamp_warning<'py>(
    model: &Bound<'py, PyAny>,
    fit: &WindowFit,
) -> PyResult<Option<Bound<'py, PyAny>>> {
    let WindowFit::Clamped { requested, floor } = fit else {
        return Ok(None);
    };
    let py: Python<'py> = model.py();
    Ok(Some(new_record_with(
        py,
        RecordId::PlanWarning,
        &[
            ("model_name", name_object(model)?),
            (
                "severity",
                enum_member(py, RecordId::WarningSeverity, "WARNING")?,
            ),
            (
                "message",
                string(
                    py,
                    &format!(
                        "explicit start {requested} is before cursor_start {floor}; the window starts at {floor}"
                    ),
                ),
            ),
        ],
    )?))
}
