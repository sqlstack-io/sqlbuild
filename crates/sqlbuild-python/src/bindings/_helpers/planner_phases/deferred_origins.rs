//! Deferred locations of renamed models the deferred target holds only under their old name.

use pyo3::exceptions::PyException;
use pyo3::prelude::{Bound, IntoPyObject, PyAny, PyAnyMethods, PyResult, Python};
use pyo3::types::{PyDict, PyDictMethods, PyString, PyTuple};

use sqlbuild_python_base::boundary::main::call_with::call_with;
use sqlbuild_python_base::records::main::construct::new_record_with::new_record_with;
use sqlbuild_python_base::records::main::fields::field::field as native_field;
use sqlbuild_python_base::records::types::RecordId;
use sqlbuild_python_front::model_config::main::config::config_value::config_value;
use sqlbuild_python_front::model_config::main::config::model_values::model_values;

const MIGRATE_FROM_KEY: &str = "migrate_from";
/// `database.schema.name`.
const QUALIFIED_RELATION_MAX_PARTS: usize = 3;

/// The relation in `relations` named `name`, ignoring case.
fn relation_named<'py>(
    relations: &Bound<'py, PyDict>,
    name: &str,
) -> PyResult<Option<Bound<'py, PyAny>>> {
    if let Some(relation) = relations.get_item(name)? {
        return Ok(Some(relation));
    }
    let wanted: String = name.to_lowercase();
    for (key, relation) in relations.iter() {
        if key.extract::<String>()?.to_lowercase() == wanted {
            return Ok(Some(relation));
        }
    }
    Ok(None)
}

/// A model's declared `migrate_from`, split into its dotted parts.
fn migration_origin_parts(model: &Bound<'_, PyAny>) -> PyResult<Option<Vec<String>>> {
    let py: Python<'_> = model.py();
    let origin: Bound<'_, PyAny> = config_value(py, &model_values(model)?, MIGRATE_FROM_KEY)?;
    if !origin.is_instance_of::<PyString>() {
        return Ok(None);
    }
    let raw: String = origin.extract()?;
    let parts: Vec<String> = raw
        .split('.')
        .map(|part| part.trim().trim_matches(['"', '`']).to_owned())
        .collect();
    if parts.is_empty()
        || parts.len() > QUALIFIED_RELATION_MAX_PARTS
        || parts.iter().any(String::is_empty)
    {
        return Ok(None);
    }
    Ok(Some(parts))
}

/// Relations already listed in the deferred target, by `(database, schema)`.
type ListedSchemas<'py> = Vec<(Option<String>, String, Bound<'py, PyDict>)>;

/// Relations of one deferred `database.schema`, listing it on first use, with the cache.
fn schema_relations<'py>(
    (adapter, connection): (&Bound<'py, PyAny>, &Bound<'py, PyAny>),
    (database, schema): (&Option<String>, &str),
    mut listed: ListedSchemas<'py>,
) -> (PyResult<Bound<'py, PyDict>>, ListedSchemas<'py>) {
    if let Some((_, _, found)) = listed.iter().find(|(listed_database, listed_schema, _)| {
        listed_database == database && listed_schema.eq_ignore_ascii_case(schema)
    }) {
        return (Ok(found.clone()), listed);
    }
    let py: Python<'py> = adapter.py();
    let found = (|| -> PyResult<Bound<'py, PyDict>> {
        let found: Bound<'py, PyDict> = PyDict::new(py);
        for relation in call_with(
            &native_field(adapter, "list_relations")?,
            &[
                ("connection", connection.clone()),
                ("database", database.clone().into_pyobject(py)?.into_any()),
                ("schemas", PyTuple::new(py, [schema])?.into_any()),
            ],
        )?
        .try_iter()?
        {
            let relation: Bound<'py, PyAny> = relation?;
            found.set_item(native_field(&relation, "name")?, relation)?;
        }
        Ok(found)
    })();
    if let Ok(found) = &found {
        listed.push((database.clone(), schema.to_owned(), found.clone()));
    }
    (found, listed)
}

/// The `(database, schema, name)` a `migrate_from` names, resolving omitted parts in `location`.
fn migration_origin_relation(
    location: &Bound<'_, PyAny>,
    parts: &[String],
) -> PyResult<Option<(Option<String>, String, String)>> {
    let location_database: Option<String> = native_field(location, "database")?.extract()?;
    let location_schema: Option<String> = native_field(location, "schema")?.extract()?;
    let name: String = parts[parts.len() - 1].clone();
    Ok(match parts.len() {
        1 => location_schema.map(|schema| (location_database, schema, name)),
        QUALIFIED_RELATION_MAX_PARTS => Some((Some(parts[0].clone()), parts[1].clone(), name)),
        _ => Some((location_database, parts[0].clone(), name)),
    })
}

/// Redirect unselected `models` unbuilt in the deferred target to their `migrate_from` relation.
pub(crate) fn defer_to_migration_origins<'py>(
    (project, adapter, connection): (&Bound<'py, PyAny>, &Bound<'py, PyAny>, &Bound<'py, PyAny>),
    (locations, relations): (&Bound<'py, PyDict>, &Bound<'py, PyDict>),
    models: &[String],
) -> PyResult<Vec<(String, String)>> {
    let py: Python<'py> = project.py();
    let mut failures: Vec<(String, String)> = Vec::new();
    let mut listed: ListedSchemas<'py> = Vec::new();
    for model in native_field(project, "models")?.try_iter()? {
        let model: Bound<'py, PyAny> = model?;
        let name: String = native_field(&model, "name")?.extract()?;
        if !models.contains(&name) {
            continue;
        }
        let Some(location) = locations.get_item(&name)? else {
            continue;
        };
        let Some(parts) = migration_origin_parts(&model)? else {
            continue;
        };
        let Some((origin_database, origin_schema, origin_name)) =
            migration_origin_relation(&location, &parts)?
        else {
            continue;
        };
        let current: Option<(Option<String>, String, String)> =
            migration_origin_relation(&location, &[native_field(&location, "name")?.extract()?])?;
        let mut lookups: Vec<(Option<String>, String, String)> = current.into_iter().collect();
        lookups.push((origin_database, origin_schema, origin_name));
        let mut origin: Option<Bound<'py, PyAny>> = None;
        for (index, (database, schema, relation_name)) in lookups.iter().enumerate() {
            let (schema_found, updated) =
                schema_relations((adapter, connection), (database, schema), listed);
            listed = updated;
            let hit: Option<Bound<'py, PyAny>> = match schema_found {
                Ok(schema_found) => relation_named(&schema_found, relation_name)?,
                Err(error) if error.is_instance_of::<PyException>(py) => {
                    let qualified: String = [
                        database.as_deref(),
                        Some(schema.as_str()),
                        Some(relation_name.as_str()),
                    ]
                    .into_iter()
                    .flatten()
                    .collect::<Vec<&str>>()
                    .join(".");
                    failures.push((
                        name.clone(),
                        format!(
                            "could not list {qualified} in the deferred target to look for a pre-migration relation, so model '{name}' is deferred under its current name: {}",
                            error.value(py)
                        ),
                    ));
                    break;
                }
                Err(error) => return Err(error),
            };
            if index + 1 < lookups.len() {
                if hit.is_some() {
                    break;
                }
            } else {
                origin = hit;
            }
        }
        let Some(origin) = origin else {
            continue;
        };
        let mut database: Bound<'py, PyAny> = native_field(&origin, "database")?;
        if database.is_none() {
            database = native_field(&location, "database")?;
        }
        let schema: Bound<'py, PyAny> = native_field(&origin, "schema")?;
        let relation_name: Bound<'py, PyAny> = native_field(&origin, "name")?;
        let qualified_name: Bound<'py, PyAny> = call_with(
            &native_field(adapter, "render_qualified_name")?,
            &[
                ("database", database.clone()),
                ("schema", schema.clone()),
                ("name", relation_name.clone()),
            ],
        )?;
        locations.set_item(
            &name,
            new_record_with(
                py,
                RecordId::CompiledRelationLocation,
                &[
                    ("database", database),
                    ("schema", schema),
                    ("name", relation_name),
                    ("qualified_name", qualified_name),
                    ("logical_schema", native_field(&location, "logical_schema")?),
                    (
                        "logical_database",
                        native_field(&location, "logical_database")?,
                    ),
                ],
            )?,
        )?;
        relations.set_item(&name, origin)?;
    }
    Ok(failures)
}
