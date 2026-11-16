//! Deferred target relation locations, and the warehouse relations they name.

use pyo3::prelude::{
    Bound, PyAny, PyAnyMethods, PyErr, PyModule, PyModuleMethods, PyResult, Python,
};
use pyo3::types::{PyDict, PyDictMethods, PyString, PyStringMethods, PyTuple};
use pyo3::{pyfunction, wrap_pyfunction};
use sqlbuild_python_base::records::main::construct::new_plain_error::new_plain_error;

use sqlbuild_model_config::templates::main::deferred_target_segments::deferred_target_segments;
use sqlbuild_model_config::templates::models::DeferredSegment;

use crate::bindings::_helpers::authored_values::project_vars::render_project_var_text;
use sqlbuild_python_base::boundary::main::call_with::call_with;
use sqlbuild_python_base::boundary::models::KeywordArguments;
use sqlbuild_python_base::records::main::construct::new_record::new_record;
use sqlbuild_python_base::records::main::construct::new_record_with::new_record_with;
use sqlbuild_python_base::records::main::fields::field::field as native_field;
use sqlbuild_python_base::records::main::fields::mapping_values::mapping_values;
use sqlbuild_python_base::records::types::ErrorId;
use sqlbuild_python_base::records::types::RecordId;

const PRESERVE_TARGET_VALUE: &str = "preserve";
/// `sorted(DEFERRED_TARGET_CONTEXT_KEYS)`.
const DEFERRED_TARGET_CONTEXT_KEYS: [&str; 2] = ["database", "schema"];

/// The deferred target's naming and the adapter defaults its locations fall back to.
struct DeferredNaming<'py> {
    target_config: Bound<'py, PyAny>,
    effective_vars: Bound<'py, PyAny>,
    default_schema: Bound<'py, PyAny>,
    default_database: Bound<'py, PyAny>,
    render_qualified_name: Bound<'py, PyAny>,
}

/// Build physical locations for all models and seeds under a deferred target.
#[pyfunction]
#[pyo3(signature = (**options))]
pub(crate) fn build_deferred_locations<'py>(
    py: Python<'py>,
    options: Option<Bound<'py, PyDict>>,
) -> PyResult<Bound<'py, PyDict>> {
    let arguments: KeywordArguments<'py> = KeywordArguments::read(
        py,
        "build_deferred_locations",
        options,
        (
            &[
                "project",
                "deferred_target_config",
                "effective_vars",
                "default_schema",
                "default_database",
                "render_qualified_name",
            ],
            &[],
        ),
    )?;
    deferred_locations(
        &arguments.value("project")?,
        &DeferredNaming {
            target_config: arguments.value("deferred_target_config")?,
            effective_vars: arguments.value("effective_vars")?,
            default_schema: arguments.value("default_schema")?,
            default_database: arguments.value("default_database")?,
            render_qualified_name: arguments.value("render_qualified_name")?,
        },
    )
}

/// `build_deferred_locations` with its arguments.
fn deferred_locations<'py>(
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
        if let Ok(name) = name?.downcast::<PyString>() {
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

/// Validate and resolve the deferred target config.
#[pyfunction]
#[pyo3(signature = (**options))]
pub(crate) fn resolve_deferred_target_config<'py>(
    py: Python<'py>,
    options: Option<Bound<'py, PyDict>>,
) -> PyResult<Bound<'py, PyAny>> {
    let arguments: KeywordArguments<'py> = KeywordArguments::read(
        py,
        "resolve_deferred_target_config",
        options,
        (
            &["discovered_inputs", "defer_to", "current_target_name"],
            &[],
        ),
    )?;
    deferred_target_config(
        &arguments.value("discovered_inputs")?,
        &arguments.value("defer_to")?,
        &arguments.value("current_target_name")?,
    )
}

/// `resolve_deferred_target_config` with its arguments.
fn deferred_target_config<'py>(
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

/// Gather existing relations from the deferred target's relation-location schemas.
#[pyfunction]
#[pyo3(signature = (**options))]
pub(crate) fn gather_deferred_relations<'py>(
    py: Python<'py>,
    options: Option<Bound<'py, PyDict>>,
) -> PyResult<Bound<'py, PyDict>> {
    let arguments: KeywordArguments<'py> = KeywordArguments::read(
        py,
        "gather_deferred_relations",
        options,
        (&["adapter", "connection", "deferred_locations"], &[]),
    )?;
    deferred_relations(
        &arguments.value("adapter")?,
        &arguments.value("connection")?,
        &arguments.value("deferred_locations")?,
    )
}

/// `gather_deferred_relations` with its arguments.
fn deferred_relations<'py>(
    adapter: &Bound<'py, PyAny>,
    connection: &Bound<'py, PyAny>,
    locations: &Bound<'py, PyAny>,
) -> PyResult<Bound<'py, PyDict>> {
    let py: Python<'py> = adapter.py();
    let mut schemas: Vec<String> = Vec::new();
    let mut database: Bound<'py, PyAny> = py.None().into_bound(py);
    for location in mapping_values(locations)?.try_iter()? {
        let location: Bound<'py, PyAny> = location?;
        let schema: Bound<'py, PyAny> = native_field(&location, "schema")?;
        if !schema.is_none() {
            let schema: String = schema.extract()?;
            if !schemas.contains(&schema) {
                schemas.push(schema);
            }
        }
        let location_database: Bound<'py, PyAny> = native_field(&location, "database")?;
        if !location_database.is_none() && database.is_none() {
            database = location_database;
        }
    }
    let relations_by_name: Bound<'py, PyDict> = PyDict::new(py);
    if schemas.is_empty() {
        return Ok(relations_by_name);
    }
    schemas.sort();
    let relations: Bound<'py, PyAny> = call_with(
        &native_field(adapter, "list_relations")?,
        &[
            ("connection", connection.clone()),
            ("database", database),
            ("schemas", PyTuple::new(py, schemas)?.into_any()),
        ],
    )?;
    for relation in relations.try_iter()? {
        let relation: Bound<'py, PyAny> = relation?;
        relations_by_name.set_item(native_field(&relation, "name")?, relation)?;
    }
    Ok(relations_by_name)
}

/// Resolve native target locations and warehouse relations for model deferral.
#[pyfunction]
#[pyo3(signature = (**options))]
pub(crate) fn resolve_model_deferral_inputs<'py>(
    py: Python<'py>,
    options: Option<Bound<'py, PyDict>>,
) -> PyResult<Bound<'py, PyAny>> {
    let arguments: KeywordArguments<'py> = KeywordArguments::read(
        py,
        "resolve_model_deferral_inputs",
        options,
        (
            &[
                "project",
                "discovered_inputs",
                "adapter",
                "connection",
                "defer_to",
            ],
            &[],
        ),
    )?;
    let defer_to: Bound<'py, PyAny> = arguments.value("defer_to")?;
    if defer_to.is_none() {
        return new_record(py, RecordId::DeferralInputs, (), None);
    }
    let project: Bound<'py, PyAny> = arguments.value("project")?;
    let adapter: Bound<'py, PyAny> = arguments.value("adapter")?;
    let target_config: Bound<'py, PyAny> = deferred_target_config(
        &arguments.value("discovered_inputs")?,
        &defer_to,
        &native_field(&project, "effective_target_name")?,
    )?;
    let locations: Bound<'py, PyDict> = deferred_locations(
        &project,
        &DeferredNaming {
            target_config,
            effective_vars: native_field(&project, "effective_vars")?,
            default_schema: adapter.call_method0("default_schema")?,
            default_database: adapter.call_method0("default_database")?,
            render_qualified_name: native_field(&adapter, "render_qualified_name")?,
        },
    )?;
    let relations: Bound<'py, PyDict> = deferred_relations(
        &adapter,
        &arguments.value("connection")?,
        locations.as_any(),
    )?;
    new_record_with(
        py,
        RecordId::DeferralInputs,
        &[
            ("deferred_locations", locations.into_any()),
            ("deferred_relations", relations.into_any()),
        ],
    )
}

pub(crate) fn register(module: &Bound<'_, PyModule>) -> PyResult<()> {
    module.add_function(wrap_pyfunction!(build_deferred_locations, module)?)?;
    module.add_function(wrap_pyfunction!(resolve_deferred_target_config, module)?)?;
    module.add_function(wrap_pyfunction!(gather_deferred_relations, module)?)?;
    module.add_function(wrap_pyfunction!(resolve_model_deferral_inputs, module)?)
}
