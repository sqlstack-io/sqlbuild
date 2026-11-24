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
use crate::planner_strategy::main::strategy::materialization_of::materialization_of;

/// The model's error row and its error-severity warning, or `error` itself when it isn't a planning error a person can fix.
pub(crate) fn planning_error<'py>(
    model: &Bound<'py, PyAny>,
    error: PyErr,
) -> PyResult<(Bound<'py, PyAny>, Bound<'py, PyAny>)> {
    let py: Python<'py> = model.py();
    let value: Bound<'py, PyAny> = error.value(py).clone().into_any();
    if !value.is_instance(&error_class(py, ErrorId::PlannerInput)?)?
        && !value.is_instance(&error_class(py, ErrorId::CompileInput)?)?
    {
        return Err(error);
    }
    let attribute = |name: &str| -> PyResult<Bound<'py, PyAny>> {
        Ok(value
            .getattr_opt(name)?
            .unwrap_or_else(|| py.None().into_bound(py)))
    };
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
    let entry: Bound<'py, PyAny> = planner_model(
        py,
        RecordId::ModelPlanEntry,
        &[
            ("key", model_key(model)?),
            ("name", name_object(model)?),
            ("relative_path", native_field(model, "relative_path")?),
            ("materialization_type", materialization_of(model)?),
            ("action", planner_type(py, RecordId::PlanAction, "SKIP")?),
            (
                "reason",
                planner_type(py, RecordId::PlanReason, "NO_CHANGE")?,
            ),
            ("destination", destination(model)?),
            ("fingerprint_query_sql", string(py, "")),
            ("resolved_sql", string(py, "")),
            ("logical_ddl", string(py, "")),
            ("planning_error", warning.clone()),
        ],
    )?;
    Ok((entry, warning))
}
