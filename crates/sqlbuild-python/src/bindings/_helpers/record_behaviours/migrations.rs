//! Behaviour of the natively hosted migration decisions and migration state records.

use pyo3::IntoPyObjectExt;
use pyo3::prelude::{Bound, Py, PyAny, PyAnyMethods, PyResult, Python};
use pyo3::types::{PyDict, PyTuple};

use sqlbuild_core::migration_values::main::column_decision_traits::column_decision_traits;
use sqlbuild_core::migration_values::main::migration_decision_traits::migration_decision_traits;
use sqlbuild_core::migration_values::main::plan_label::plan_label;
use sqlbuild_core::migration_values::models::{ColumnDecisionTraits, DecisionTraits};

use sqlbuild_python_base::boundary::main::call_argument::call_argument;
use sqlbuild_python_base::records::main::construct::enum_member::enum_member;
use sqlbuild_python_base::records::main::fields::field::field as native_field;
use sqlbuild_python_base::records::types::RecordId;

fn value(member: &Bound<'_, PyAny>) -> PyResult<String> {
    member.str()?.extract()
}

fn model_traits(member: &Bound<'_, PyAny>) -> PyResult<DecisionTraits> {
    Ok(migration_decision_traits(&value(member)?))
}

fn column_traits(member: &Bound<'_, PyAny>) -> PyResult<ColumnDecisionTraits> {
    Ok(column_decision_traits(&value(member)?))
}

/// `MigrationDecision.moves_data`.
pub(crate) fn decision_moves_data(member: &Bound<'_, PyAny>) -> PyResult<Py<PyAny>> {
    model_traits(member)?.moves_data.into_py_any(member.py())
}

/// `MigrationDecision.blocks_build`.
pub(crate) fn decision_blocks_build(member: &Bound<'_, PyAny>) -> PyResult<Py<PyAny>> {
    model_traits(member)?.blocks_build.into_py_any(member.py())
}

/// `MigrationDecision.checks_compatibility`.
pub(crate) fn decision_checks_compatibility(member: &Bound<'_, PyAny>) -> PyResult<Py<PyAny>> {
    model_traits(member)?
        .checks_compatibility
        .into_py_any(member.py())
}

/// `MigrationDecision.label` and `MigrationPromotion.label`.
pub(crate) fn value_plan_label(member: &Bound<'_, PyAny>) -> PyResult<Py<PyAny>> {
    plan_label(&value(member)?).into_py_any(member.py())
}

/// `ColumnMigrationDecision.renames`.
pub(crate) fn column_decision_renames(member: &Bound<'_, PyAny>) -> PyResult<Py<PyAny>> {
    column_traits(member)?.renames.into_py_any(member.py())
}

/// `ColumnMigrationDecision.records_event`.
pub(crate) fn column_decision_records_event(member: &Bound<'_, PyAny>) -> PyResult<Py<PyAny>> {
    column_traits(member)?
        .records_event
        .into_py_any(member.py())
}

/// `ColumnMigrationDecision.blocks_build`.
pub(crate) fn column_decision_blocks_build(member: &Bound<'_, PyAny>) -> PyResult<Py<PyAny>> {
    column_traits(member)?.blocks_build.into_py_any(member.py())
}

/// `ColumnMigrationDecision.label`.
pub(crate) fn column_decision_label(member: &Bound<'_, PyAny>) -> PyResult<Py<PyAny>> {
    match column_traits(member)?.label {
        Some(label) => label.into_py_any(member.py()),
        None => Err(pyo3::exceptions::PyKeyError::new_err(value(member)?)),
    }
}

/// `text.lower()`, keeping `None`.
fn lowered<'py>(text: &Bound<'py, PyAny>) -> PyResult<Bound<'py, PyAny>> {
    if text.is_none() {
        return Ok(text.clone());
    }
    text.call_method0("lower")
}

/// `(database.lower() or None, (schema or "").lower(), name.lower())`.
fn relation_identity<'py>(relation: &Bound<'py, PyAny>) -> PyResult<Bound<'py, PyTuple>> {
    let py: Python<'py> = relation.py();
    let schema: Bound<'py, PyAny> = native_field(relation, "schema")?;
    let schema: Bound<'py, PyAny> = if schema.is_none() {
        "".into_bound_py_any(py)?
    } else {
        schema.call_method0("lower")?
    };
    PyTuple::new(
        py,
        [
            lowered(&native_field(relation, "database")?)?,
            schema,
            native_field(relation, "name")?.call_method0("lower")?,
        ],
    )
}

/// Whether two relations match, an unknown database matching any database.
fn relations_match(left: &Bound<'_, PyAny>, right: &Bound<'_, PyAny>) -> PyResult<bool> {
    let left: Bound<'_, PyTuple> = relation_identity(left)?;
    let right: Bound<'_, PyTuple> = relation_identity(right)?;
    if !left.get_item(1)?.eq(right.get_item(1)?)? || !left.get_item(2)?.eq(right.get_item(2)?)? {
        return Ok(false);
    }
    let (left, right) = (left.get_item(0)?, right.get_item(0)?);
    Ok(left.is_none() || right.is_none() || left.eq(right)?)
}

/// `MigrationRelation.identity`: the case-insensitive relation identity.
pub(crate) fn relation_identity_property(relation: &Bound<'_, PyAny>) -> PyResult<Py<PyAny>> {
    Ok(relation_identity(relation)?.into_any().unbind())
}

/// `MigrationRelation.matches(other)`.
pub(crate) fn relation_matches(
    relation: &Bound<'_, PyAny>,
    args: &Bound<'_, PyTuple>,
    kwargs: Option<&Bound<'_, PyDict>>,
) -> PyResult<Py<PyAny>> {
    let other: Bound<'_, PyAny> = call_argument((args, kwargs), "other", 0)?;
    relations_match(relation, &other)?.into_py_any(relation.py())
}

/// `MigrationEvent.mentions(relation)`: the relation is the event's origin or destination.
pub(crate) fn event_mentions(
    event: &Bound<'_, PyAny>,
    args: &Bound<'_, PyTuple>,
    kwargs: Option<&Bound<'_, PyDict>>,
) -> PyResult<Py<PyAny>> {
    let relation: Bound<'_, PyAny> = call_argument((args, kwargs), "relation", 0)?;
    let mentions: bool = relations_match(&native_field(event, "origin")?, &relation)?
        || relations_match(&native_field(event, "destination")?, &relation)?;
    mentions.into_py_any(event.py())
}

/// `ColumnMigrationEvent.mentions(relation=..., columns=...)`.
pub(crate) fn column_event_mentions(
    event: &Bound<'_, PyAny>,
    args: &Bound<'_, PyTuple>,
    kwargs: Option<&Bound<'_, PyDict>>,
) -> PyResult<Py<PyAny>> {
    let relation: Bound<'_, PyAny> = call_argument((args, kwargs), "relation", 0)?;
    let columns: Bound<'_, PyAny> = call_argument((args, kwargs), "columns", 1)?;
    let mentions: bool = relations_match(&native_field(event, "relation")?, &relation)?
        && (columns.contains(native_field(event, "origin_column")?.call_method0("lower")?)?
            || columns
                .contains(native_field(event, "destination_column")?.call_method0("lower")?)?);
    mentions.into_py_any(event.py())
}

/// `ColumnMigrationEvent.renamed(origin_column=..., destination_column=...)`.
pub(crate) fn column_event_renamed(
    event: &Bound<'_, PyAny>,
    args: &Bound<'_, PyTuple>,
    kwargs: Option<&Bound<'_, PyDict>>,
) -> PyResult<Py<PyAny>> {
    let same = |field: &str, name: &str, position: usize| -> PyResult<bool> {
        let given: Bound<'_, PyAny> = call_argument((args, kwargs), name, position)?;
        event
            .getattr(field)?
            .call_method0("lower")?
            .eq(given.call_method0("lower")?)
    };
    let renamed: bool = same("origin_column", "origin_column", 0)?
        && same("destination_column", "destination_column", 1)?;
    renamed.into_py_any(event.py())
}

/// `OldNameViewHistory.status(now=...)`: the current state of the move's old-name steps.
pub(crate) fn history_status(
    history: &Bound<'_, PyAny>,
    args: &Bound<'_, PyTuple>,
    kwargs: Option<&Bound<'_, PyDict>>,
) -> PyResult<Py<PyAny>> {
    let py: Python<'_> = history.py();
    let now: Bound<'_, PyAny> = call_argument((args, kwargs), "now", 0)?;
    let created: Bound<'_, PyAny> = native_field(history, "created")?;
    let status: &str = if !native_field(history, "dropped")?.is_none() {
        "DROPPED"
    } else if !created.is_none() {
        let expires_at: Bound<'_, PyAny> = native_field(&created, "expires_at")?;
        if !expires_at.is_none() && expires_at.le(&now)? {
            "EXPIRED"
        } else {
            "LIVE"
        }
    } else if !native_field(history, "archived")?.is_none() {
        "PENDING_VIEW"
    } else {
        "PENDING_ARCHIVE"
    };
    Ok(enum_member(py, RecordId::OldNameViewStatus, status)?.unbind())
}

/// `OldNameViewHistory.old`: the relation name the compatibility view occupies.
pub(crate) fn history_old(history: &Bound<'_, PyAny>) -> PyResult<Py<PyAny>> {
    Ok(native_field(&native_field(history, "required")?, "old")?.unbind())
}

/// `OldNameViewHistory.new`: the relation the compatibility view reads.
pub(crate) fn history_new(history: &Bound<'_, PyAny>) -> PyResult<Py<PyAny>> {
    Ok(native_field(&native_field(history, "required")?, "new")?.unbind())
}
