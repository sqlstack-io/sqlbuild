//! Deferred target relation locations, and the warehouse relations they name.

use pyo3::IntoPyObject;
use pyo3::prelude::{Bound, PyAny, PyAnyMethods, PyErr, PyResult, Python};
use pyo3::types::{PyDict, PyDictMethods, PyString, PyStringMethods, PyTuple};
use sqlbuild_python_base::records::main::construct::new_plain_error::new_plain_error;

use sqlbuild_model_config::templates::main::deferred_target_segments::deferred_target_segments;
use sqlbuild_model_config::templates::models::DeferredSegment;

use sqlbuild_python_base::boundary::main::call_with::call_with;
use sqlbuild_python_base::records::main::construct::new_record_with::new_record_with;
use sqlbuild_python_base::records::main::fields::field::field as native_field;
use sqlbuild_python_base::records::main::fields::mapping_values::mapping_values;
use sqlbuild_python_base::records::types::ErrorId;
use sqlbuild_python_base::records::types::RecordId;
use sqlbuild_python_front::authored_values::main::render_project_var_text::render_project_var_text;

const PRESERVE_TARGET_VALUE: &str = "preserve";
/// `sorted(DEFERRED_TARGET_CONTEXT_KEYS)`.
const DEFERRED_TARGET_CONTEXT_KEYS: [&str; 2] = ["database", "schema"];

/// The deferred target's naming and the adapter defaults its locations fall back to.
pub(crate) struct DeferredNaming<'py> {
    pub(crate) target_config: Bound<'py, PyAny>,
    pub(crate) effective_vars: Bound<'py, PyAny>,
    pub(crate) default_schema: Bound<'py, PyAny>,
    pub(crate) default_database: Bound<'py, PyAny>,
    pub(crate) render_qualified_name: Bound<'py, PyAny>,
}

/// `build_deferred_locations` with its arguments.
pub(crate) fn deferred_locations<'py>(
    project: &Bound<'py, PyAny>,
    naming: &DeferredNaming<'py>,
) -> PyResult<Bound<'py, PyDict>> {
    let locations: Bound<'py, PyDict> = PyDict::new(project.py());
    for attribute in ["models", "seeds"] {
        for resource in native_field(project, attribute)?.try_iter()? {
            let resource: Bound<'py, PyAny> = resource?;
            let location: Bound<'py, PyAny> =
                deferred_location(&native_field(&resource, "destination")?, naming)?;
            locations.set_item(native_field(&resource, "name")?, location)?;
        }
    }
    Ok(locations)
}

/// One relation location under the deferred target's naming rules.
fn deferred_location<'py>(
    target: &Bound<'py, PyAny>,
    naming: &DeferredNaming<'py>,
) -> PyResult<Bound<'py, PyAny>> {
    let py: Python<'py> = target.py();
    let logical_schema: Bound<'py, PyAny> = native_field(target, "logical_schema")?;
    let logical_database: Bound<'py, PyAny> = native_field(target, "logical_database")?;
    let mut schema: Bound<'py, PyAny> = target_field(
        &native_field(&naming.target_config, "schema")?,
        &logical_schema,
        &naming.effective_vars,
    )?;
    let mut database: Bound<'py, PyAny> = target_field(
        &native_field(&naming.target_config, "database")?,
        &logical_database,
        &naming.effective_vars,
    )?;
    if schema.is_none() {
        schema = naming.default_schema.clone();
    }
    if database.is_none() {
        database = naming.default_database.clone();
    }
    let name: Bound<'py, PyAny> = native_field(target, "name")?;
    let qualified_name: Bound<'py, PyAny> = call_with(
        &naming.render_qualified_name,
        &[
            ("database", database.clone()),
            ("schema", schema.clone()),
            ("name", name.clone()),
        ],
    )?;
    new_record_with(
        py,
        RecordId::CompiledRelationLocation,
        &[
            ("database", database),
            ("schema", schema),
            ("name", name),
            ("qualified_name", qualified_name),
            ("logical_schema", logical_schema),
            ("logical_database", logical_database),
        ],
    )
}

/// One target schema or database field resolved against the logical value.
fn target_field<'py>(
    target_value: &Bound<'py, PyAny>,
    logical_value: &Bound<'py, PyAny>,
    effective_vars: &Bound<'py, PyAny>,
) -> PyResult<Bound<'py, PyAny>> {
    let py: Python<'py> = target_value.py();
    if target_value.is_none() || target_value.eq(PRESERVE_TARGET_VALUE)? {
        return Ok(logical_value.clone());
    }
    let template: String = target_value.extract()?;
    let logical: String = if logical_value.is_none() {
        String::new()
    } else {
        logical_value.extract()?
    };
    let mut variables: Vec<String> = Vec::new();
    for name in effective_vars.try_iter()? {
        if let Ok(name) = name?.cast::<PyString>() {
            variables.push(name.to_cow()?.into_owned());
        }
    }
    let context_keys: Vec<String> = DEFERRED_TARGET_CONTEXT_KEYS
        .iter()
        .map(|key| (*key).to_owned())
        .collect();
    let mut resolved: String = String::new();
    for segment in deferred_target_segments(&template, &logical, &context_keys, &variables) {
        match segment {
            DeferredSegment::Text(text) => resolved.push_str(&text),
            DeferredSegment::Variable(name) => {
                let text: String = render_project_var_text(
                    &effective_vars.get_item(name.as_str())?,
                    &format!("deferred target variable '${{{name}}}'"),
                )?;
                resolved.push_str(&text);
            }
        }
    }
    Ok(PyString::new(py, &resolved).into_any())
}

/// A `PlannerInputError` with `message` and the default code.
fn planner_error(py: Python<'_>, message: String) -> PyErr {
    new_plain_error(py, ErrorId::PlannerInput, &message)
}

/// `resolve_deferred_target_config` with its arguments.
pub(crate) fn deferred_target_config<'py>(
    discovered: &Bound<'py, PyAny>,
    defer_to: &Bound<'py, PyAny>,
    current_target_name: &Bound<'py, PyAny>,
) -> PyResult<Bound<'py, PyAny>> {
    let py: Python<'py> = discovered.py();
    let targets: Bound<'py, PyAny> =
        native_field(discovered, "project_config")?.getattr("targets")?;
    let name: String = defer_to.str()?.to_cow()?.into_owned();
    if !targets.contains(defer_to)? {
        return Err(planner_error(
            py,
            format!("Unknown deferred target '{name}'"),
        ));
    }
    if defer_to.eq(current_target_name)? {
        return Err(planner_error(
            py,
            format!("Cannot defer to the current target '{name}'"),
        ));
    }
    targets.get_item(defer_to)
}

/// `gather_deferred_relations` with its arguments: one listing per database the locations live in.
pub(crate) fn deferred_relations<'py>(
    adapter: &Bound<'py, PyAny>,
    connection: &Bound<'py, PyAny>,
    locations: &Bound<'py, PyAny>,
) -> PyResult<Bound<'py, PyDict>> {
    let py: Python<'py> = adapter.py();
    let mut databases: Vec<(Option<String>, Vec<String>)> = Vec::new();
    for location in mapping_values(locations)?.try_iter()? {
        let location: Bound<'py, PyAny> = location?;
        let schema: Option<String> = native_field(&location, "schema")?.extract()?;
        let database: Option<String> = native_field(&location, "database")?.extract()?;
        let Some(schema) = schema else {
            continue;
        };
        let index: usize = match databases.iter().position(|(listed, _)| *listed == database) {
            Some(index) => index,
            None => {
                databases.push((database, Vec::new()));
                databases.len() - 1
            }
        };
        if !databases[index].1.contains(&schema) {
            databases[index].1.push(schema);
        }
    }
    let relations_by_name: Bound<'py, PyDict> = PyDict::new(py);
    for (database, mut schemas) in databases {
        schemas.sort();
        let relations: Bound<'py, PyAny> = call_with(
            &native_field(adapter, "list_relations")?,
            &[
                ("connection", connection.clone()),
                ("database", database.into_pyobject(py)?.into_any()),
                ("schemas", PyTuple::new(py, schemas)?.into_any()),
            ],
        )?;
        for relation in relations.try_iter()? {
            let relation: Bound<'py, PyAny> = relation?;
            relations_by_name.set_item(native_field(&relation, "name")?, relation)?;
        }
    }
    Ok(relations_by_name)
}
