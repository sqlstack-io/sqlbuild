//! Resolve authored migration origins at the adapter and inspection boundaries.

use crate::planner_migrations::constants::QUALIFIED_RELATION_MAX_PARTS;
use crate::planner_migrations::models::ModelMigrationRequest;
use crate::planner_phases::main::inputs::runtime_dialect::runtime_dialect;
use crate::planner_phases::main::inputs::runtime_field::runtime_field;
use pyo3::PyErr;
use pyo3::prelude::{Bound, PyAny, PyAnyMethods, PyResult, Python};
use pyo3::types::{PySet, PySetMethods};
use sqlbuild_python_base::boundary::main::call_with::call_with;
use sqlbuild_python_base::native_host::types::Syntax;
use sqlbuild_python_base::records::main::construct::error_class::error_class;
use sqlbuild_python_base::records::main::construct::new_record_with::new_record_with;
use sqlbuild_python_base::records::main::fields::field::field as native_field;
use sqlbuild_python_base::records::types::ErrorId;
use sqlbuild_python_base::records::types::RecordId;
use sqlbuild_python_project::compiled_views::main::compiled_models::name_object::name_object;
use sqlbuild_python_project::compiled_views::main::project::compiled_models::compiled_models;

pub(crate) fn location<'py>(
    runtime: &Bound<'py, PyAny>,
    fields: &[(&str, Bound<'py, PyAny>)],
) -> PyResult<Bound<'py, PyAny>> {
    let qualified = call_with(
        &runtime_dialect(runtime)?.syntax("render_qualified_name")?,
        fields,
    )?;
    let mut fields = fields.to_vec();
    fields.push(("qualified_name", qualified));
    new_record_with(runtime.py(), RecordId::CompiledRelationLocation, &fields)
}

fn named<'py>(
    name: &Bound<'py, PyAny>,
    destination: &Bound<'py, PyAny>,
    runtime: &Bound<'py, PyAny>,
    state: &Bound<'py, PyAny>,
) -> PyResult<(Bound<'py, PyAny>, Bound<'py, PyAny>)> {
    for item in compiled_models(&runtime_field(runtime, |runtime| &runtime.project)?)?.try_iter()? {
        let item = item?;
        if name_object(&item)?.eq(name)? {
            return Ok((native_field(&item, "destination")?, name.clone()));
        }
    }
    let fingerprint = call_with(
        &native_field(state, "named_fingerprint")?,
        &[("model_name", name.clone())],
    )?;
    let fields = if !fingerprint.is_none() && !native_field(&fingerprint, "target_name")?.is_none()
    {
        [
            ("database", native_field(&fingerprint, "target_database")?),
            ("schema", native_field(&fingerprint, "target_schema")?),
            ("name", native_field(&fingerprint, "target_name")?),
        ]
    } else {
        [
            ("database", native_field(destination, "database")?),
            ("schema", native_field(destination, "schema")?),
            ("name", name.clone()),
        ]
    };
    Ok((location(runtime, &fields)?, name.clone()))
}

fn input_error(py: Python<'_>, message: String) -> PyResult<PyErr> {
    Ok(PyErr::from_value(
        error_class(py, ErrorId::PlannerInput)?.call1((message,))?,
    ))
}

/// The database migration planning inspects: the first model destination's, else the target's.
pub(crate) fn planning_database<'py>(runtime: &Bound<'py, PyAny>) -> PyResult<Bound<'py, PyAny>> {
    let project = runtime_field(runtime, |runtime| &runtime.project)?;
    let fallback = native_field(&project, "effective_target_database")?;
    for item in compiled_models(&project)?.try_iter()? {
        let database = native_field(&sqlbuild_python_project::compiled_views::main::compiled_models::destination::destination(&item?)?, "database")?;
        if !database.is_none() {
            return Ok(database);
        }
    }
    Ok(fallback)
}

/// The schemas the project's models write to.
pub(crate) fn project_schemas<'py>(runtime: &Bound<'py, PyAny>) -> PyResult<Bound<'py, PyAny>> {
    let schemas = PySet::empty(runtime.py())?;
    for item in compiled_models(&runtime_field(runtime, |runtime| &runtime.project)?)?.try_iter()? {
        let schema = native_field(&sqlbuild_python_project::compiled_views::main::compiled_models::destination::destination(&item?)?, "schema")?;
        if !schema.is_none() {
            schemas.add(schema)?;
        }
    }
    Ok(schemas.into_any())
}

/// Reject a migration origin in a database other than the inspected one.
pub(crate) fn require_same_database(
    state: &Bound<'_, PyAny>,
    location: &Bound<'_, PyAny>,
) -> PyResult<()> {
    let database = native_field(location, "database")?;
    if database.is_none() {
        return Ok(());
    }
    let inspected = native_field(state, "database")?;
    if inspected.is_none()
        || database
            .call_method0("lower")?
            .eq(inspected.call_method0("lower")?)?
    {
        return Ok(());
    }
    let qualified = native_field(location, "qualified_name")?;
    let label = if qualified.is_truthy()? {
        qualified
    } else {
        native_field(location, "name")?
    };
    Err(input_error(
        state.py(),
        format!(
            "migration origin {label} is in database '{database}', but direct planning inspects '{inspected}'; cross-database migrations are not supported"
        ),
    )?)
}

/// The origin location and origin model name of one migration request.
pub(crate) fn resolve_origin<'py>(
    request: &ModelMigrationRequest<'py>,
    runtime: &Bound<'py, PyAny>,
    state: &Bound<'py, PyAny>,
) -> PyResult<(Bound<'py, PyAny>, Bound<'py, PyAny>)> {
    let py = runtime.py();
    if let Some(explicit) = &request.origin_location {
        return Ok((explicit.clone(), request.origin_model.clone()));
    }
    let raw = match &request.raw_origin {
        Some(raw) if raw.is_truthy()? => raw.clone(),
        _ => crate::planner_migrations::_helpers::column::column_objects::text(py, ""),
    };
    let item = request.model.clone();
    let destination =
        sqlbuild_python_project::compiled_views::main::compiled_models::destination::destination(
            &item,
        )?;
    let mut parts: Vec<Bound<'py, PyAny>> = Vec::new();
    for part in raw.call_method1("split", (".",))?.try_iter()? {
        parts.push(
            part?
                .call_method0("strip")?
                .call_method1("strip", ("\"`",))?,
        );
    }
    let maximum: usize = QUALIFIED_RELATION_MAX_PARTS;
    let mut invalid = parts.len() > maximum;
    for part in &parts {
        invalid |= !part.is_truthy()?;
    }
    if invalid {
        return Err(input_error(
            py,
            format!(
                "model '{}': migrate_from '{raw}' is not a model name or a qualified relation",
                native_field(&item, "name")?
            ),
        )?);
    }
    if parts.len() == 1 {
        return named(&parts[0], &destination, runtime, state);
    }
    let database = if parts.len() == maximum {
        parts[0].clone()
    } else {
        native_field(&destination, "database")?
    };
    let origin = location(
        runtime,
        &[
            ("database", database),
            ("schema", parts[parts.len() - 2].clone()),
            ("name", parts[parts.len() - 1].clone()),
        ],
    )?;
    Ok((origin, py.None().into_bound(py)))
}
