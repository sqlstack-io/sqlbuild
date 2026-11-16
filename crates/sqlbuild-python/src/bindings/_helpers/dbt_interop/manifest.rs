//! dbt manifest loading, indexing and `__dbt_ref` resolution.

use sqlbuild_python_base::records::main::construct::new_error::new_error;
use std::collections::HashSet;

use pyo3::prelude::{
    Bound, PyAny, PyAnyMethods, PyErr, PyModule, PyModuleMethods, PyResult, Python,
};
use pyo3::types::{PyDict, PyDictMethods, PyList, PyString, PyTuple};
use pyo3::{pyfunction, wrap_pyfunction};
use sqlbuild_config::dbt_manifest::constants::{
    MODEL_RESOURCE_TYPE, NODE_TEXT_FIELDS, SEED_RESOURCE_TYPE, SNAPSHOT_RESOURCE_TYPE,
    SOURCE_RESOURCE_TYPE, TEST_RESOURCE_TYPE, UNIT_TEST_RESOURCE_TYPE,
};
use sqlbuild_config::dbt_manifest::errors::ManifestError;
use sqlbuild_config::dbt_manifest::main::parse_manifest_model::parse_manifest_model;
use sqlbuild_config::dbt_manifest::main::parse_manifest_seed::parse_manifest_seed;
use sqlbuild_config::dbt_manifest::main::parse_manifest_source::parse_manifest_source;
use sqlbuild_config::dbt_manifest::models::{
    ManifestNode, NodeChecksum, ParsedModel, ParsedSeed, ParsedSource,
};

use sqlbuild_python_base::boundary::main::imported::imported;
use sqlbuild_python_base::boundary::models::KeywordArguments;
use sqlbuild_python_base::records::main::construct::new_record_with::new_record_with;
use sqlbuild_python_base::records::main::fields::field::field as native_field;
use sqlbuild_python_base::records::types::ErrorId;
use sqlbuild_python_base::records::types::RecordId;

const DBT_REF_KIND: &str = "dbt_ref";
const RELATION_OPTIONS: (&[&str], &[&str]) =
    (&["manifest", "ref_kind", "ref_name", "ref_package"], &[]);

fn raised(py: Python<'_>, error: ManifestError) -> PyErr {
    new_error(
        py,
        ErrorId::CompileInput,
        &error.message,
        (Some(error.code), error.help.as_deref()),
    )
}

fn text_value(value: Option<Bound<'_, PyAny>>) -> PyResult<Option<String>> {
    match value {
        Some(value) if value.is_instance_of::<PyString>() => Ok(Some(value.extract()?)),
        _ => Ok(None),
    }
}

fn checksum(value: Option<Bound<'_, PyAny>>) -> PyResult<NodeChecksum> {
    let Some(value) = value else {
        return Ok(NodeChecksum::Other);
    };
    if value.is_instance_of::<PyString>() {
        return Ok(NodeChecksum::Text(value.extract()?));
    }
    match value.cast::<PyDict>() {
        Ok(nested) => Ok(NodeChecksum::Nested(text_value(
            nested.get_item("checksum")?,
        )?)),
        Err(_) => Ok(NodeChecksum::Other),
    }
}

fn fqn(value: Option<Bound<'_, PyAny>>) -> PyResult<Option<Vec<Option<String>>>> {
    let Some(value) = value.filter(|value| value.is_instance_of::<PyList>()) else {
        return Ok(None);
    };
    let mut parts: Vec<Option<String>> = Vec::new();
    for part in value.try_iter()? {
        parts.push(text_value(Some(part?))?);
    }
    Ok(Some(parts))
}

fn depends_on(value: Option<Bound<'_, PyAny>>) -> PyResult<Vec<String>> {
    let Some(Ok(depends_on)) = value.map(|value| value.cast_into::<PyDict>()) else {
        return Ok(Vec::new());
    };
    let Some(nodes) = depends_on
        .get_item("nodes")?
        .filter(|nodes| nodes.is_instance_of::<PyList>())
    else {
        return Ok(Vec::new());
    };
    let mut unique_ids: Vec<String> = Vec::new();
    for node in nodes.try_iter()? {
        if let Some(unique_id) = text_value(Some(node?))? {
            unique_ids.push(unique_id);
        }
    }
    Ok(unique_ids)
}

fn manifest_node(raw: &Bound<'_, PyDict>) -> PyResult<ManifestNode> {
    let mut text: Vec<(String, String)> = Vec::new();
    for field in NODE_TEXT_FIELDS {
        if let Some(value) = text_value(raw.get_item(field)?)? {
            text.push((field.to_owned(), value));
        }
    }
    Ok(ManifestNode {
        text,
        checksum: checksum(raw.get_item("checksum")?)?,
        fqn: fqn(raw.get_item("fqn")?)?,
        depends_on: depends_on(raw.get_item("depends_on")?)?,
    })
}

fn resource_type(raw: &Bound<'_, PyDict>) -> PyResult<Option<String>> {
    text_value(raw.get_item("resource_type")?)
}

/// `{str(key): value for key, value in raw_node.items()}`.
fn payload<'py>(raw: &Bound<'py, PyDict>) -> PyResult<Bound<'py, PyAny>> {
    let payload: Bound<'py, PyDict> = PyDict::new(raw.py());
    for (key, value) in raw.iter() {
        payload.set_item(key.str()?, value)?;
    }
    Ok(payload.into_any())
}

fn optional<'py>(py: Python<'py>, value: Option<&str>) -> Bound<'py, PyAny> {
    match value {
        Some(text) => PyString::new(py, text).into_any(),
        None => py.None().into_bound(py),
    }
}

fn text<'py>(py: Python<'py>, value: &str) -> Bound<'py, PyAny> {
    PyString::new(py, value).into_any()
}

fn model_object<'py>(
    unique_id: &Bound<'py, PyAny>,
    model: &ParsedModel,
    raw: &Bound<'py, PyDict>,
) -> PyResult<Bound<'py, PyAny>> {
    let py: Python<'py> = raw.py();
    new_record_with(
        py,
        RecordId::DbtManifestModel,
        &[
            ("unique_id", unique_id.clone()),
            ("package_name", text(py, &model.package_name)),
            ("name", text(py, &model.name)),
            ("database", optional(py, model.database.as_deref())),
            ("schema", optional(py, model.schema.as_deref())),
            ("alias", optional(py, model.alias.as_deref())),
            (
                "node_checksum",
                optional(py, model.node_checksum.as_deref()),
            ),
            ("relation_name", text(py, &model.relation_name)),
            ("fqn", PyTuple::new(py, &model.fqn)?.into_any()),
            ("query_sql", text(py, &model.query_sql)),
            (
                "depends_on_nodes",
                PyTuple::new(py, &model.depends_on)?.into_any(),
            ),
            ("payload", payload(raw)?),
        ],
    )
}

fn seed_object<'py>(
    unique_id: &Bound<'py, PyAny>,
    seed: &ParsedSeed,
    raw: &Bound<'py, PyDict>,
) -> PyResult<Bound<'py, PyAny>> {
    let py: Python<'py> = raw.py();
    new_record_with(
        py,
        RecordId::DbtManifestSeed,
        &[
            ("unique_id", unique_id.clone()),
            ("package_name", text(py, &seed.package_name)),
            ("name", text(py, &seed.name)),
            ("database", optional(py, seed.database.as_deref())),
            ("schema", optional(py, seed.schema.as_deref())),
            ("alias", optional(py, seed.alias.as_deref())),
            ("relation_name", text(py, &seed.relation_name)),
            ("payload", payload(raw)?),
        ],
    )
}

fn source_object<'py>(
    unique_id: &Bound<'py, PyAny>,
    source: &ParsedSource,
    raw: &Bound<'py, PyDict>,
) -> PyResult<Bound<'py, PyAny>> {
    let py: Python<'py> = raw.py();
    new_record_with(
        py,
        RecordId::DbtManifestSource,
        &[
            ("unique_id", unique_id.clone()),
            ("package_name", text(py, &source.package_name)),
            ("source_name", text(py, &source.source_name)),
            ("name", text(py, &source.name)),
            ("database", optional(py, source.database.as_deref())),
            ("schema", optional(py, source.schema.as_deref())),
            ("identifier", optional(py, source.identifier.as_deref())),
            ("relation_name", text(py, &source.relation_name)),
            ("payload", payload(raw)?),
        ],
    )
}

/// The manifest section `key`: an object, or empty when missing and `optional`.
fn section<'py>(
    manifest: &Bound<'py, PyDict>,
    (key, optional_section): (&str, bool),
) -> PyResult<Bound<'py, PyDict>> {
    let py: Python<'py> = manifest.py();
    match manifest.get_item(key)? {
        None if optional_section => Ok(PyDict::new(py)),
        Some(value) if value.is_none() && optional_section => Ok(PyDict::new(py)),
        Some(value) => value.cast_into::<PyDict>().map_err(|_| {
            raised(
                py,
                ManifestError::invalid(&format!(": {key} must be an object")),
            )
        }),
        None => Err(raised(
            py,
            ManifestError::invalid(&format!(": {key} must be an object")),
        )),
    }
}

/// `(unique_id, node)` pairs whose key is a string and value an object.
fn node_items<'py>(nodes: &Bound<'py, PyDict>) -> Vec<(Bound<'py, PyAny>, Bound<'py, PyDict>)> {
    let mut items: Vec<(Bound<'py, PyAny>, Bound<'py, PyDict>)> = Vec::new();
    for (key, value) in nodes.iter() {
        if !key.is_instance_of::<PyString>() {
            continue;
        }
        if let Ok(node) = value.cast_into::<PyDict>() {
            items.push((key, node));
        }
    }
    items
}

/// The model indexes built from `nodes`, in manifest order.
struct ModelIndexes<'py> {
    by_unique_id: Bound<'py, PyDict>,
    by_name: Bound<'py, PyDict>,
    by_package_and_name: Bound<'py, PyDict>,
    seeds: Bound<'py, PyDict>,
}

fn index_nodes<'py>(nodes: &Bound<'py, PyDict>) -> PyResult<ModelIndexes<'py>> {
    let py: Python<'py> = nodes.py();
    let indexes: ModelIndexes<'py> = ModelIndexes {
        by_unique_id: PyDict::new(py),
        by_name: PyDict::new(py),
        by_package_and_name: PyDict::new(py),
        seeds: PyDict::new(py),
    };
    for (unique_id, raw) in node_items(nodes) {
        let kind: Option<String> = resource_type(&raw)?;
        let kind: &str = kind.as_deref().unwrap_or_default();
        if kind == MODEL_RESOURCE_TYPE || kind == SNAPSHOT_RESOURCE_TYPE {
            let model: ParsedModel =
                parse_manifest_model(&manifest_node(&raw)?).map_err(|error| raised(py, error))?;
            let object: Bound<'py, PyAny> = model_object(&unique_id, &model, &raw)?;
            indexes.by_unique_id.set_item(&unique_id, &object)?;
            let named: Bound<'py, PyAny> = match indexes.by_name.get_item(&model.name)? {
                Some(named) => named,
                None => {
                    let named: Bound<'py, PyList> = PyList::empty(py);
                    indexes.by_name.set_item(&model.name, &named)?;
                    named.into_any()
                }
            };
            named.call_method1("append", (&object,))?;
            indexes
                .by_package_and_name
                .set_item((&model.package_name, &model.name), &object)?;
        } else if kind == SEED_RESOURCE_TYPE {
            let seed: ParsedSeed =
                parse_manifest_seed(&manifest_node(&raw)?).map_err(|error| raised(py, error))?;
            indexes
                .seeds
                .set_item(&unique_id, seed_object(&unique_id, &seed, &raw)?)?;
        }
    }
    let named: Vec<(Bound<'py, PyAny>, Bound<'py, PyAny>)> = indexes.by_name.iter().collect();
    for (name, models) in named {
        let models: Vec<Bound<'py, PyAny>> = models.try_iter()?.collect::<PyResult<_>>()?;
        indexes.by_name.set_item(name, PyTuple::new(py, models)?)?;
    }
    Ok(indexes)
}

fn validation_dependencies<'py>(
    sections: [&Bound<'py, PyDict>; 2],
) -> PyResult<Bound<'py, PyDict>> {
    let py: Python<'py> = sections[0].py();
    let dependencies: Bound<'py, PyDict> = PyDict::new(py);
    for nodes in sections {
        for (unique_id, raw) in node_items(nodes) {
            let kind: Option<String> = resource_type(&raw)?;
            if kind.as_deref() != Some(TEST_RESOURCE_TYPE)
                && kind.as_deref() != Some(UNIT_TEST_RESOURCE_TYPE)
            {
                continue;
            }
            let unique_ids: Vec<String> = depends_on(raw.get_item("depends_on")?)?
                .into_iter()
                .filter(|node| !node.is_empty())
                .collect();
            dependencies.set_item(unique_id, PyTuple::new(py, unique_ids)?)?;
        }
    }
    Ok(dependencies)
}

fn manifest_index<'py>(raw_data: &Bound<'py, PyAny>) -> PyResult<Bound<'py, PyAny>> {
    let py: Python<'py> = raw_data.py();
    let manifest: &Bound<'py, PyDict> = raw_data
        .cast::<PyDict>()
        .map_err(|_| raised(py, ManifestError::invalid(": root must be an object")))?;
    let nodes: Bound<'py, PyDict> = section(manifest, ("nodes", false))?;
    let sources: Bound<'py, PyDict> = section(manifest, ("sources", true))?;
    let unit_tests: Bound<'py, PyDict> = section(manifest, ("unit_tests", true))?;
    let indexes: ModelIndexes<'py> = index_nodes(&nodes)?;
    let validation: Bound<'py, PyDict> = validation_dependencies([&nodes, &unit_tests])?;
    let sources_by_unique_id: Bound<'py, PyDict> = PyDict::new(py);
    for (unique_id, raw) in node_items(&sources) {
        if resource_type(&raw)?.as_deref() != Some(SOURCE_RESOURCE_TYPE) {
            continue;
        }
        let source: ParsedSource =
            parse_manifest_source(&manifest_node(&raw)?).map_err(|error| raised(py, error))?;
        sources_by_unique_id.set_item(&unique_id, source_object(&unique_id, &source, &raw)?)?;
    }
    new_record_with(
        py,
        RecordId::DbtManifestIndex,
        &[
            ("models_by_unique_id", indexes.by_unique_id.into_any()),
            ("models_by_name", indexes.by_name.into_any()),
            (
                "models_by_package_and_name",
                indexes.by_package_and_name.into_any(),
            ),
            ("sources_by_unique_id", sources_by_unique_id.into_any()),
            ("seeds_by_unique_id", indexes.seeds.into_any()),
            (
                "validation_depends_on_nodes_by_unique_id",
                validation.into_any(),
            ),
        ],
    )
}

/// `json.loads(contents)`, with a decode failure raised as an invalid manifest.
fn decoded<'py>(contents: &Bound<'py, PyAny>) -> PyResult<Bound<'py, PyAny>> {
    let py: Python<'py> = contents.py();
    match imported(py, "json", "loads")?.call1((contents,)) {
        Ok(value) => Ok(value),
        Err(error) if error.is_instance(py, &imported(py, "json", "JSONDecodeError")?) => {
            let detail: String = native_field(error.value(py), "msg")?.extract()?;
            Err(raised(
                py,
                ManifestError::invalid(&format!(" JSON: {detail}")),
            ))
        }
        Err(error) => Err(error),
    }
}

/// Load and index dbt model nodes from a manifest.json file.
#[pyfunction]
#[pyo3(signature = (*, manifest_path))]
pub(crate) fn load_dbt_manifest_index<'py>(
    py: Python<'py>,
    manifest_path: Bound<'py, PyAny>,
) -> PyResult<Bound<'py, PyAny>> {
    if !manifest_path.call_method0("is_file")?.is_truthy()? {
        let path: String = manifest_path.str()?.extract()?;
        return Err(raised(py, ManifestError::missing_manifest_file(&path)));
    }
    let keywords: Bound<'py, PyDict> = PyDict::new(py);
    keywords.set_item("encoding", "utf-8")?;
    let contents: Bound<'py, PyAny> =
        manifest_path.call_method("read_text", (), Some(&keywords))?;
    manifest_index(&decoded(&contents)?)
}

/// Build a dbt manifest index from discovered manifest contents.
#[pyfunction]
#[pyo3(signature = (*, manifest_contents))]
pub(crate) fn build_compile_dbt_manifest_index<'py>(
    py: Python<'py>,
    manifest_contents: Bound<'py, PyAny>,
) -> PyResult<Bound<'py, PyAny>> {
    if manifest_contents.is_none() {
        return Ok(py.None().into_bound(py));
    }
    manifest_index(&decoded(&manifest_contents)?)
}

/// Build dbt model lookup indexes from decoded manifest JSON.
#[pyfunction]
#[pyo3(signature = (*, raw_data))]
fn build_dbt_manifest_index<'py>(raw_data: Bound<'py, PyAny>) -> PyResult<Bound<'py, PyAny>> {
    manifest_index(&raw_data)
}

/// The manifest model a one-arg or package-qualified `__dbt_ref` names.
fn resolved_model<'py>(
    manifest: &Bound<'py, PyAny>,
    name: &str,
    package_name: Option<&str>,
) -> PyResult<Bound<'py, PyAny>> {
    let py: Python<'py> = manifest.py();
    if let Some(package) = package_name {
        let model: Bound<'py, PyAny> = native_field(manifest, "models_by_package_and_name")?
            .call_method1("get", ((package, name),))?;
        if model.is_none() {
            return Err(raised(
                py,
                ManifestError::unknown_model(name, Some(package)),
            ));
        }
        return Ok(model);
    }
    let matches: Bound<'py, PyAny> = native_field(manifest, "models_by_name")?
        .call_method1("get", (name, PyTuple::empty(py)))?;
    let matches: Vec<Bound<'py, PyAny>> = matches.try_iter()?.collect::<PyResult<_>>()?;
    match matches.as_slice() {
        [] => Err(raised(py, ManifestError::unknown_model(name, None))),
        [model] => Ok(model.clone()),
        _ => {
            let packages: Vec<String> = matches
                .iter()
                .map(|model| native_field(model, "package_name")?.extract())
                .collect::<PyResult<_>>()?;
            Err(raised(py, ManifestError::ambiguous_model(name, packages)))
        }
    }
}

/// Resolve a dbt model by one-arg or package-qualified ref semantics.
#[pyfunction]
#[pyo3(signature = (*, manifest, name, package_name = None))]
fn resolve_dbt_manifest_model<'py>(
    manifest: Bound<'py, PyAny>,
    name: String,
    package_name: Option<String>,
) -> PyResult<Bound<'py, PyAny>> {
    resolved_model(&manifest, &name, package_name.as_deref())
}

/// Reject ambiguous ownership between dbt and SQLBuild models.
#[pyfunction]
#[pyo3(signature = (*, known_model_names, dbt_manifest))]
pub(crate) fn validate_compile_dbt_model_names(
    py: Python<'_>,
    known_model_names: Bound<'_, PyAny>,
    dbt_manifest: Bound<'_, PyAny>,
) -> Result<(), PyErr> {
    if dbt_manifest.is_none() {
        return Ok(());
    }
    let dbt_names: Bound<'_, PyAny> = native_field(&dbt_manifest, "models_by_name")?;
    let mut shared: Vec<String> = Vec::new();
    for name in known_model_names.try_iter()? {
        let name: Bound<'_, PyAny> = name?;
        if dbt_names.contains(&name)? {
            shared.push(name.extract()?);
        }
    }
    match ManifestError::shared_model_names(shared) {
        Some(error) => Err(raised(py, error)),
        None => Ok(()),
    }
}

fn dbt_ref_kind(ref_kind: &Bound<'_, PyAny>) -> PyResult<bool> {
    ref_kind.eq(DBT_REF_KIND)
}

fn optional_text(value: &Bound<'_, PyAny>) -> PyResult<Option<String>> {
    if value.is_none() {
        return Ok(None);
    }
    Ok(Some(value.extract()?))
}

/// Validate one compile-time dbt ref against a dbt manifest.
#[pyfunction]
#[pyo3(signature = (*, reference, model_relative_path, dbt_manifest))]
pub(crate) fn validate_compile_dbt_model_reference(
    py: Python<'_>,
    reference: Bound<'_, PyAny>,
    model_relative_path: Bound<'_, PyAny>,
    dbt_manifest: Bound<'_, PyAny>,
) -> Result<(), PyErr> {
    if !dbt_ref_kind(&native_field(&reference, "ref_kind")?)? {
        return Ok(());
    }
    let name: String = native_field(&reference, "ref_name")?.extract()?;
    if dbt_manifest.is_none() {
        let path: String = model_relative_path.str()?.extract()?;
        return Err(raised(py, ManifestError::missing_manifest(&path, &name)));
    }
    let package: Option<String> = optional_text(&native_field(&reference, "ref_package")?)?;
    resolved_model(&dbt_manifest, &name, package.as_deref())?;
    Ok(())
}

fn name_set(names: &Bound<'_, PyAny>) -> PyResult<HashSet<String>> {
    let mut set: HashSet<String> = HashSet::new();
    for name in names.try_iter()? {
        set.insert(name?.extract()?);
    }
    Ok(set)
}

/// Return resolved dbt refs used directly by SQLBuild models.
#[pyfunction]
#[pyo3(signature = (*, project, manifest, selected_model_names = None))]
pub(crate) fn resolve_sqlbuild_model_dbt_refs<'py>(
    py: Python<'py>,
    project: Bound<'py, PyAny>,
    manifest: Bound<'py, PyAny>,
    selected_model_names: Option<Bound<'py, PyAny>>,
) -> PyResult<Bound<'py, PyTuple>> {
    let selected_model_names: Option<HashSet<String>> = selected_model_names
        .map(|names| name_set(&names))
        .transpose()?;
    let mut refs: Vec<Bound<'py, PyTuple>> = Vec::new();
    for model in native_field(&project, "models")?.try_iter()? {
        let model: Bound<'py, PyAny> = model?;
        if let Some(selected) = &selected_model_names {
            let name: String = native_field(&model, "name")?.extract()?;
            if !selected.contains(&name) {
                continue;
            }
        }
        for reference in native_field(&model, "references")?.try_iter()? {
            let reference: Bound<'py, PyAny> = reference?;
            if !dbt_ref_kind(&native_field(&reference, "ref_kind")?)? {
                continue;
            }
            let name: String = native_field(&reference, "ref_name")?.extract()?;
            let package: Option<String> = optional_text(&native_field(&reference, "ref_package")?)?;
            let resolved: Bound<'py, PyAny> = resolved_model(&manifest, &name, package.as_deref())?;
            refs.push(PyTuple::new(py, [model.clone(), resolved])?);
        }
    }
    PyTuple::new(py, refs)
}

/// Resolve one external dbt reference to its manifest relation.
#[pyfunction]
#[pyo3(signature = (**options))]
pub(crate) fn resolve_dbt_reference_relation<'py>(
    py: Python<'py>,
    options: Option<Bound<'py, PyDict>>,
) -> PyResult<Bound<'py, PyAny>> {
    let arguments: KeywordArguments<'py> = KeywordArguments::read(
        py,
        "resolve_dbt_reference_relation",
        options,
        RELATION_OPTIONS,
    )?;
    let manifest: Bound<'py, PyAny> = arguments.value("manifest")?;
    if !dbt_ref_kind(&arguments.value("ref_kind")?)? || manifest.is_none() {
        return Ok(py.None().into_bound(py));
    }
    let name: String = arguments.value("ref_name")?.extract()?;
    let package: Option<String> = optional_text(&arguments.value("ref_package")?)?;
    native_field(
        &resolved_model(&manifest, &name, package.as_deref())?,
        "relation_name",
    )
}

pub(crate) fn register(module: &Bound<'_, PyModule>) -> PyResult<()> {
    module.add_function(wrap_pyfunction!(load_dbt_manifest_index, module)?)?;
    module.add_function(wrap_pyfunction!(build_compile_dbt_manifest_index, module)?)?;
    module.add_function(wrap_pyfunction!(build_dbt_manifest_index, module)?)?;
    module.add_function(wrap_pyfunction!(resolve_dbt_manifest_model, module)?)?;
    module.add_function(wrap_pyfunction!(validate_compile_dbt_model_names, module)?)?;
    module.add_function(wrap_pyfunction!(
        validate_compile_dbt_model_reference,
        module
    )?)?;
    module.add_function(wrap_pyfunction!(resolve_sqlbuild_model_dbt_refs, module)?)?;
    module.add_function(wrap_pyfunction!(resolve_dbt_reference_relation, module)?)
}
