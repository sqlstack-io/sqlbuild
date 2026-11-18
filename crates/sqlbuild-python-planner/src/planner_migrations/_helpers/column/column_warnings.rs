//! Preserve column-migration diagnostic codes and public messages.

use crate::planner_migrations::_helpers::column::column_objects::native;
use crate::planner_migrations::_helpers::column::column_objects::text;
use crate::planner_migrations::constants::DONE;
use crate::planner_migrations::constants::MISSING_ORIGIN_ALLOW_OUTCOME;
use crate::planner_migrations::constants::MISSING_ORIGIN_CONFIRMATION_OUTCOME;
use crate::planner_migrations::constants::MISSING_ORIGIN_DENY_OUTCOME;
use pyo3::exceptions::PyKeyError;
use pyo3::prelude::{Bound, PyAny, PyAnyMethods, PyResult};
use sqlbuild_analysis::planner_migrations::main::column_warning::column_warning;
use sqlbuild_analysis::planner_migrations::models::ColumnWarning;
use sqlbuild_python_base::records::main::construct::enum_member::enum_member;
use sqlbuild_python_base::records::main::fields::field::field as native_field;
use sqlbuild_python_base::records::types::RecordId;

pub(crate) fn warning<'py>(
    entry: &Bound<'py, PyAny>,
    code: &str,
    message: &str,
    error: bool,
) -> PyResult<Bound<'py, PyAny>> {
    let py = entry.py();
    native(
        py,
        RecordId::PlanWarning,
        &[
            ("model_name", native_field(entry, "model_name")?),
            (
                "severity",
                enum_member(
                    py,
                    RecordId::WarningSeverity,
                    if error { "ERROR" } else { "WARNING" },
                )?,
            ),
            ("message", text(py, message)),
            ("code", text(py, code)),
        ],
    )
}

pub(crate) fn entry_warning<'py>(entry: &Bound<'py, PyAny>) -> PyResult<Option<Bound<'py, PyAny>>> {
    let destination = native_field(entry, "destination")?;
    let qualified = native_field(&destination, "qualified_name")?;
    let relation: String = if qualified.is_truthy()? {
        qualified
    } else {
        native_field(&destination, "name")?
    }
    .extract()?;
    let origin: String = native_field(entry, "origin_column")?.extract()?;
    let column: String = native_field(entry, "destination_column")?.extract()?;
    let name: String = native_field(entry, "model_name")?.extract()?;
    let decision: String = native_field(entry, "decision")?.extract()?;
    if decision == DONE && !native_field(entry, "discovery")?.eq("manual")? {
        return Ok(None);
    }
    let completed = native_field(entry, "completed_at")?;
    let completed: String = if completed.is_truthy()? {
        completed.call_method0("isoformat")?.extract()?
    } else {
        "unknown time".into()
    };
    let target = native_field(entry, "target_name")?;
    let target: String = if target.is_truthy()? {
        target.extract()?
    } else {
        "default".into()
    };
    let outcome: String =
        missing_origin_outcome(&native_field(entry, "missing_origin_policy")?)?.to_owned();
    let message: String = native_field(entry, "message")?.str()?.extract()?;
    let Some((code, message)) = column_warning(
        &decision,
        &ColumnWarning {
            model_name: &name,
            relation: &relation,
            origin: &origin,
            destination: &column,
            completed: &completed,
            target: &target,
            missing_outcome: &outcome,
            message: &message,
        },
    ) else {
        return Ok(None);
    };
    Ok(Some(warning(
        entry,
        code,
        &message,
        decision != DONE && native_field(entry, "blocks_build")?.is_truthy()?,
    )?))
}

pub(crate) fn unavailable<'py>(
    entry: &Bound<'py, PyAny>,
    reason: &str,
) -> PyResult<Bound<'py, PyAny>> {
    let origin: String = native_field(entry, "origin_column")?.extract()?;
    let destination: String = native_field(entry, "destination_column")?.extract()?;
    let facts = ColumnWarning {
        model_name: "",
        relation: "",
        origin: &origin,
        destination: &destination,
        completed: "",
        target: "",
        missing_outcome: "",
        message: reason,
    };
    let (code, message) = column_warning("automatic_unavailable", &facts).ok_or_else(|| {
        pyo3::exceptions::PyValueError::new_err("missing column migration diagnostic")
    })?;
    warning(entry, code, &message, false)
}

/// The build outcome a missing migration origin has under its policy.
pub(crate) fn missing_origin_outcome(policy: &Bound<'_, PyAny>) -> PyResult<&'static str> {
    match policy.extract::<String>()?.as_str() {
        "allow" => Ok(MISSING_ORIGIN_ALLOW_OUTCOME),
        "require_confirmation" => Ok(MISSING_ORIGIN_CONFIRMATION_OUTCOME),
        "deny" => Ok(MISSING_ORIGIN_DENY_OUTCOME),
        _ => Err(PyKeyError::new_err(policy.clone().unbind())),
    }
}
