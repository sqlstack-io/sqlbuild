//! Diagnose literal references to compatibility-only old names.

use sqlbuild_host::entries::models::UserCodeHost;

use crate::bindings::_helpers::planner_migrations::column_objects::{native, text};
use crate::bindings::_helpers::planner_phases::objects::object;
use crate::bindings::_helpers::references::explicit_references::hard_coded_relation_remedy;
use crate::bindings::_helpers::references::explicit_references::old_name_reference_message;
use crate::bindings::_helpers::references::relations::match_project_relation;
use pyo3::prelude::{Bound, PyAny, PyAnyMethods, PyResult};
use pyo3::types::{PyDict, PyDictMethods, PyStringMethods, PyTuple};
use sqlbuild_python_base::boundary::main::call_with::call_with;
use sqlbuild_python_base::boundary::main::imported::imported;
use sqlbuild_python_base::native_host::main::host_entry::host_entry;
use sqlbuild_python_base::records::main::construct::enum_member::enum_member;
use sqlbuild_python_base::records::main::fields::field::field as native_field;
use sqlbuild_python_base::records::types::RecordId;

fn old_relation<'py>(
    old: &Bound<'py, PyAny>,
    name: Bound<'py, PyAny>,
) -> PyResult<Bound<'py, PyAny>> {
    let py = old.py();
    let reference = call_with(
        &host_entry(py, UserCodeHost::SqlResourceRef)?,
        &[
            (
                "kind",
                host_entry(py, UserCodeHost::SqlResourceRefKind)?.getattr("MODEL")?,
            ),
            ("name", name.clone()),
        ],
    )?;
    let relation = call_with(
        &imported(py, "sqlbuild.compiler.references.models", "RelationName")?,
        &[
            ("name", native_field(old, "name")?),
            ("schema", native_field(old, "schema")?),
            ("database", native_field(old, "database")?),
        ],
    )?;
    call_with(
        &imported(py, "sqlbuild.compiler.references.models", "ProjectRelation")?,
        &[
            ("ref", reference),
            ("relation", relation),
            ("compatibility_for", name),
        ],
    )
}

pub(super) fn errors<'py>(
    runtime: &Bound<'py, PyAny>,
    views: &Bound<'py, PyAny>,
    entries: &Bound<'py, PyAny>,
) -> PyResult<Vec<Bound<'py, PyAny>>> {
    let py = runtime.py();
    let literals = native_field(
        &native_field(runtime, "project")?,
        "unmatched_literal_sql_relations",
    )?;
    if !literals.is_truthy()? {
        return Ok(Vec::new());
    }
    let relations = PyDict::new(py);
    for view in views.try_iter()? {
        let view = view?;
        relations.set_item(
            old_relation(
                &native_field(&view, "old")?,
                native_field(&view, "destination_model")?,
            )?,
            py.None(),
        )?;
    }
    for entry in entries.try_iter()? {
        let entry = entry?;
        if native_field(&entry, "runs_steps")?.is_truthy()? {
            relations.set_item(
                old_relation(
                    &native_field(&entry, "origin")?,
                    native_field(&entry, "model_name")?,
                )?,
                py.None(),
            )?;
        }
    }
    let index = call_with(
        &imported(
            py,
            "sqlbuild.compiler.references.models",
            "ProjectRelationIndex",
        )?,
        &[("relations", PyTuple::new(py, relations.keys())?.into_any())],
    )?;
    let mut errors: Vec<Bound<'py, PyAny>> = Vec::new();
    for literal in literals.try_iter()? {
        let literal = literal?;
        let relation = native_field(&literal, "relation")?;
        let matched = object(py, match_project_relation(&index, &relation, None, None)?)?;
        if matched.is_none() || native_field(&matched, "compatibility_for")?.is_none() {
            continue;
        }
        let mut parts: Vec<String> = Vec::new();
        for field in ["database", "schema", "name"] {
            let part = native_field(&relation, field)?;
            if part.is_truthy()? {
                parts.push(part.extract()?);
            }
        }
        let message: String = object(
            py,
            old_name_reference_message(
                &native_field(&literal, "owner_label")?.extract::<String>()?,
                &parts.join("."),
                &native_field(&literal, "method")?.extract::<String>()?,
                &native_field(&matched, "compatibility_for")?.extract::<String>()?,
            ),
        )?
        .extract()?;
        let remedy: String = hard_coded_relation_remedy(
            &native_field(&literal, "owner_kind")?.extract::<String>()?,
            &native_field(&matched, "ref")?,
            None,
        )?;
        let path = native_field(&literal, "relative_path")?
            .str()?
            .to_str()?
            .to_owned();
        let line = native_field(&literal, "line")?.str()?.to_str()?.to_owned();
        errors.push(native(
            py,
            RecordId::PlanWarning,
            &[
                ("model_name", py.None().into_bound(py)),
                (
                    "severity",
                    enum_member(py, RecordId::WarningSeverity, "ERROR")?,
                ),
                (
                    "message",
                    text(py, &format!("{path}:{line}: {message}; {remedy}")),
                ),
                ("code", text(py, "P008")),
            ],
        )?);
    }
    Ok(errors)
}
