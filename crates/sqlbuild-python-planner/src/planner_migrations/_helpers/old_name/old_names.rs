//! Existing old-name planner models at the native decision boundary.

use sqlbuild_host::entries::models::AdapterHost;

use crate::planner_migrations::_helpers::old_name::old_name_objects::relation;
use pyo3::IntoPyObject;
use pyo3::prelude::{Bound, PyAny, PyAnyMethods, PyResult, Python};
use pyo3::types::{PyDict, PySet, PyTuple};
use sqlbuild_analysis::planner_migrations::main::{
    declared_column_aliases as aliases, old_name_claim_message as claim, old_name_schemas,
};
use sqlbuild_python_base::boundary::models::KeywordArguments;
use sqlbuild_python_base::native_host::main::host_entry::host_entry;
use sqlbuild_python_base::records::main::construct::enum_member::enum_member;
use sqlbuild_python_base::records::main::construct::new_record_with::new_record_with;
use sqlbuild_python_base::records::main::fields::field::field as native_field;
use sqlbuild_python_base::records::types::RecordId;
use sqlbuild_python_project::compiled_views::main::compiled_models::name_object::name_object;
use sqlbuild_python_project::compiled_views::main::compiled_models::schema_entry::schema_entry;

pub(crate) fn declared_column_aliases<'py>(
    py: Python<'py>,
    model: &Bound<'py, PyAny>,
) -> PyResult<Bound<'py, PyTuple>> {
    let schema = schema_entry(model)?;
    let mut columns: Vec<(String, Option<String>)> = Vec::new();
    if !schema.is_none() {
        for column in native_field(&schema, "columns")?.try_iter()? {
            let column = column?;
            columns.push((
                native_field(&column, "name")?.extract()?,
                native_field(&column, "migrate_from")?.extract()?,
            ));
        }
    }
    PyTuple::new(py, aliases::declared_column_aliases(&columns))
}

pub(crate) fn old_name_schemas_to_read<'py>(
    py: Python<'py>,
    snapshot: &Bound<'py, PyAny>,
    needed: &Bound<'py, PyAny>,
) -> PyResult<Bound<'py, PySet>> {
    let listed: Option<std::collections::BTreeSet<String>> =
        native_field(snapshot, "listed_state_schemas")?.extract()?;
    PySet::new(
        py,
        old_name_schemas::old_name_schemas(
            &needed.extract()?,
            &native_field(snapshot, "old_name_view_state_schemas")?.extract()?,
            listed.as_ref(),
        ),
    )
}
fn claim_message(model: &str, view: &Bound<'_, PyAny>, now: &Bound<'_, PyAny>) -> PyResult<String> {
    let old = native_field(view, "old")?;
    let qualified: Option<String> = native_field(&old, "qualified_name")?.extract()?;
    let name: String = qualified
        .filter(|name| !name.is_empty())
        .unwrap_or(native_field(&old, "name")?.extract()?);
    let expires = native_field(view, "expires_at")?;
    let date: Option<String> = if expires.is_none() {
        None
    } else {
        Some(expires.call_method1("strftime", ("%Y-%m-%d",))?.extract()?)
    };
    let expired = !expires.is_none() && expires.le(now)?;
    Ok(claim::old_name_claim_message(
        model,
        (
            &name,
            &native_field(view, "destination_model")?.extract::<String>()?,
        ),
        (date.as_deref(), expired),
    ))
}

pub(crate) fn old_name_claim_errors<'py>(
    py: Python<'py>,
    options: Option<Bound<'py, PyDict>>,
) -> PyResult<Bound<'py, PyTuple>> {
    let arguments = KeywordArguments::read(
        py,
        "old_name_claim_errors",
        options,
        (&["selected", "snapshot", "views", "now"], &[]),
    )?;
    let mut errors: Vec<Bound<'py, PyAny>> = Vec::new();
    for model in arguments.value("selected")?.try_iter()? {
        let model = model?;
        let name = name_object(&model)?;
        let existing = arguments
            .value("snapshot")?
            .getattr("existing_relations")?
            .call_method1("get", (&name,))?;
        if existing.is_none() {
            continue;
        }
        let kind = host_entry(py, AdapterHost::NormalizeRelationType)?
            .call1((native_field(&existing, "relation_type")?,))?;
        if !kind.eq("view")? {
            continue;
        }
        let destination = relation(&sqlbuild_python_project::compiled_views::main::compiled_models::destination::destination(&model)?)?;
        for view in arguments.value("views")?.try_iter()? {
            let view = view?;
            if native_field(&view, "destination_model")?.eq(&name)?
                || !relation(&native_field(&view, "old")?)?
                    .call_method1("matches", (&destination,))?
                    .is_truthy()?
            {
                continue;
            }
            let message =
                claim_message(&name.extract::<String>()?, &view, &arguments.value("now")?)?;
            errors.push(new_record_with(
                py,
                RecordId::PlanWarning,
                &[
                    ("model_name", name.clone()),
                    (
                        "severity",
                        enum_member(py, RecordId::WarningSeverity, "ERROR")?,
                    ),
                    ("message", message.into_pyobject(py)?.into_any()),
                    ("code", "M114".into_pyobject(py)?.into_any()),
                ],
            )?);
            break;
        }
    }
    PyTuple::new(py, errors)
}
