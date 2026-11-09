//! Version identity metadata JSON and hashes of models, functions and seeds, byte-identical to the recorded fingerprints.

use std::collections::HashMap;

use pyo3::prelude::{Bound, PyAny, PyAnyMethods, PyModule, PyModuleMethods, PyResult, Python};
use pyo3::types::{PyDict, PyDictMethods, PyList, PyListMethods, PyString, PyTuple};
use pyo3::{pyfunction, wrap_pyfunction};

use sqlbuild_analysis::planner_identity::constants::{
    CONTRACT_CONFIG_KEY, CURSOR_INPUTS_CONFIG_KEY, ENFORCED_CONTRACT_POLICY,
    MODEL_CUSTOM_CONFIG_KEY, MODEL_PLACEHOLDERS_CONFIG_KEY, MODEL_POST_HOOKS_CONFIG_KEY,
    MODEL_PRE_HOOKS_CONFIG_KEY, SQL_HOOK_IDENTITY_FIELDS, TABLE_FN_RESOURCE, UDF_RESOURCE,
    VERSION_IDENTITY_CONFIG_KEYS,
};
use sqlbuild_analysis::planner_identity::main::function_fingerprint_sql::function_fingerprint_sql;
use sqlbuild_analysis::planner_identity::models::FunctionFingerprintFields;
use sqlbuild_core::fingerprint_identity::main::stable_identity_hash::stable_identity_hash;

use crate::bindings::_helpers::boundary::python_calls::{imported, keywords, within};
use crate::bindings::_helpers::compiled_views::function_reads;
use crate::bindings::_helpers::compiled_views::model_reads;
use crate::bindings::_helpers::compiled_views::seed_reads;
use crate::bindings::_helpers::discovery::hook_entries::hook_entries;
use crate::bindings::_helpers::microbatches::conversion::timestamp;
use crate::bindings::_helpers::model_config::typed_config::{config_value, model_values};
use crate::bindings::_helpers::planner_identity::hashing::{
    baselined_function_definition_hash, function_definition_hash,
};
use crate::bindings::_helpers::planner_identity::json_values::{COMPACT, SPACED, sorted_dumps};
use crate::bindings::_helpers::records::fields::field as native_field;
use crate::bindings::_helpers::records::fields::mapping_get;
use crate::bindings::_helpers::records::fields::mapping_items;

const LOCAL_FUNCTION_HASHES_KEY: &str = "local_function_hashes";
const DATACLASSES_MODULE: &str = "dataclasses";
const CSV_MODULE: &str = "csv";
const BUILTINS_MODULE: &str = "builtins";
const DEFAULT_SEED_ENCODING: &str = "utf-8-sig";
/// CSV settings passed to `csv.reader` when set.
const SEED_DIALECT_OPTIONS: [&str; 5] = [
    "delimiter",
    "quotechar",
    "doublequote",
    "escapechar",
    "skipinitialspace",
];

/// The config entries that affect a model's produced version identity.
fn version_identity_config<'py>(config_values: &Bound<'py, PyAny>) -> PyResult<Bound<'py, PyDict>> {
    let identity: Bound<'py, PyDict> = PyDict::new(config_values.py());
    for key in std::iter::once(CURSOR_INPUTS_CONFIG_KEY).chain(VERSION_IDENTITY_CONFIG_KEYS) {
        if config_values.contains(key)? {
            identity.set_item(key, config_values.get_item(key)?)?;
        }
    }
    Ok(identity)
}

/// `value or {}`.
pub(crate) fn or_empty<'py>(
    value: Option<Bound<'py, PyAny>>,
    py: Python<'py>,
) -> PyResult<Bound<'py, PyAny>> {
    match value {
        Some(value) if value.is_truthy()? => Ok(value),
        _ => Ok(PyDict::new(py).into_any()),
    }
}

/// Deterministic metadata JSON for model version identity.
pub(crate) fn version_identity_metadata_json(
    model_name: &Bound<'_, PyAny>,
    config_values: &Bound<'_, PyAny>,
    (local_function_hashes, execution_signature): (
        Option<Bound<'_, PyAny>>,
        Option<Bound<'_, PyAny>>,
    ),
) -> PyResult<String> {
    let py: Python<'_> = model_name.py();
    let payload: Bound<'_, PyDict> = PyDict::new(py);
    payload.set_item("config", version_identity_config(config_values)?)?;
    payload.set_item("execution_signature", or_empty(execution_signature, py)?)?;
    payload.set_item(
        LOCAL_FUNCTION_HASHES_KEY,
        or_empty(local_function_hashes, py)?,
    )?;
    payload.set_item("model_name", model_name)?;
    sorted_dumps(payload.as_any(), COMPACT, true)
}

/// Deterministic non-query model fingerprint metadata JSON.
pub(crate) fn model_version_identity_metadata_json<'py>(
    model: &Bound<'py, PyAny>,
    function_local_hashes: Option<&Bound<'py, PyAny>>,
    hook_version_hashes: Option<&Bound<'py, PyAny>>,
) -> PyResult<String> {
    let py: Python<'py> = model.py();
    let local_function_hashes: Bound<'py, PyDict> = PyDict::new(py);
    if let Some(function_hashes) = function_local_hashes.filter(|hashes| !hashes.is_none()) {
        for upstream_key in model_reads::deps(model)?.try_iter()? {
            let upstream_key: Bound<'py, PyAny> = upstream_key?;
            let kind: String = native_field(&upstream_key, "resource_type")?
                .str()?
                .extract()?;
            if kind != UDF_RESOURCE && kind != TABLE_FN_RESOURCE {
                continue;
            }
            let name: Bound<'py, PyAny> = native_field(&upstream_key, "name")?;
            let upstream_hash: Bound<'py, PyAny> = mapping_get(function_hashes, &name)?;
            if !upstream_hash.is_none() {
                local_function_hashes.set_item(name, upstream_hash)?;
            }
        }
    }
    let config_values: Bound<'py, PyAny> = native_field(&model_reads::config(model)?, "values")?;
    let signature: Bound<'py, PyDict> =
        execution_signature(model, &config_values, hook_version_hashes)?;
    version_identity_metadata_json(
        &model_reads::name_object(model)?,
        &config_values,
        (
            Some(local_function_hashes.into_any()),
            Some(signature.into_any()),
        ),
    )
}

fn execution_signature<'py>(
    model: &Bound<'py, PyAny>,
    config_values: &Bound<'py, PyAny>,
    hook_version_hashes: Option<&Bound<'py, PyAny>>,
) -> PyResult<Bound<'py, PyDict>> {
    let py: Python<'py> = model.py();
    let signature: Bound<'py, PyDict> = PyDict::new(py);
    if let Some(contract) = contract_output_signature(model, config_values)? {
        signature.set_item("contract", contract)?;
    }
    for (config_key, signature_key) in [
        (MODEL_CUSTOM_CONFIG_KEY, "custom_config"),
        (MODEL_PLACEHOLDERS_CONFIG_KEY, "custom_placeholders"),
    ] {
        if config_values.contains(config_key)? {
            signature.set_item(signature_key, config_values.get_item(config_key)?)?;
        }
    }
    let python_hook_fields: Bound<'py, PyDict> = PyDict::new(py);
    if let Some(hashes) = hook_version_hashes.filter(|hashes| !hashes.is_none()) {
        for item in mapping_items(hashes)?.try_iter()? {
            let (name, version_hash): (Bound<'py, PyAny>, Bound<'py, PyAny>) = item?.extract()?;
            let fields: Bound<'py, PyDict> = PyDict::new(py);
            fields.set_item("version_hash", version_hash)?;
            python_hook_fields.set_item(name, fields)?;
        }
    }
    let sql_fields: Vec<String> = SQL_HOOK_IDENTITY_FIELDS
        .iter()
        .map(|field| (*field).to_owned())
        .collect();
    for config_key in [MODEL_PRE_HOOKS_CONFIG_KEY, MODEL_POST_HOOKS_CONFIG_KEY] {
        if config_values.contains(config_key)? {
            signature.set_item(
                config_key,
                hook_entries(
                    &config_values.get_item(config_key)?,
                    &sql_fields,
                    Some(&python_hook_fields),
                )?,
            )?;
        }
    }
    Ok(signature)
}

/// Required model output metadata that participates in execution identity.
fn contract_output_signature<'py>(
    model: &Bound<'py, PyAny>,
    config_values: &Bound<'py, PyAny>,
) -> PyResult<Option<Bound<'py, PyDict>>> {
    let py: Python<'py> = model.py();
    let schema_entry: Bound<'py, PyAny> = model_reads::schema_entry(model)?;
    if schema_entry.is_none() {
        return Ok(None);
    }
    let columns: Bound<'py, PyAny> = native_field(&schema_entry, "columns")?;
    let families: Bound<'py, PyAny> = native_field(&schema_entry, "dynamic_columns")?;
    if !(columns.is_truthy()? || families.is_truthy()?) {
        return Ok(None);
    }
    let enforced: bool =
        mapping_get(config_values, CONTRACT_CONFIG_KEY)?.eq(ENFORCED_CONTRACT_POLICY)?;
    if !enforced && native_field(&schema_entry, "model_schema")?.is_none() {
        return Ok(None);
    }
    let enum_columns: Bound<'py, PyAny> = model_reads::enum_columns(model)?;
    let rendered: Bound<'py, PyList> = PyList::empty(py);
    for column in columns.try_iter()? {
        let column: Bound<'py, PyAny> = column?;
        let entry: Bound<'py, PyDict> = column_signature(&column)?;
        let declaration: Bound<'py, PyAny> =
            mapping_get(&enum_columns, native_field(&column, "name")?)?;
        if !declaration.is_none() {
            let members: Bound<'py, PyList> = PyList::empty(py);
            for member in native_field(&declaration, "members")?.try_iter()? {
                let member: Bound<'py, PyAny> = member?;
                let rendered_member: Bound<'py, PyDict> = PyDict::new(py);
                rendered_member.set_item("name", native_field(&member, "name")?)?;
                rendered_member.set_item("value", native_field(&member, "value")?)?;
                members.append(rendered_member)?;
            }
            let enum_entry: Bound<'py, PyDict> = PyDict::new(py);
            enum_entry.set_item("name", native_field(&declaration, "name")?)?;
            enum_entry.set_item("members", members)?;
            entry.set_item("enum", enum_entry)?;
        }
        rendered.append(entry)?;
    }
    let signature: Bound<'py, PyDict> = PyDict::new(py);
    signature.set_item("enforced", enforced)?;
    signature.set_item("columns", rendered)?;
    if families.is_truthy()? {
        signature.set_item("dynamic_columns", family_signatures(&families)?)?;
    }
    Ok(Some(signature))
}

fn column_signature<'py>(column: &Bound<'py, PyAny>) -> PyResult<Bound<'py, PyDict>> {
    let entry: Bound<'py, PyDict> = PyDict::new(column.py());
    for field in ["name", "type", "nullable"] {
        entry.set_item(field, native_field(column, field)?)?;
    }
    Ok(entry)
}

fn family_signatures<'py>(families: &Bound<'py, PyAny>) -> PyResult<Bound<'py, PyList>> {
    let rendered: Bound<'py, PyList> = PyList::empty(families.py());
    for family in families.try_iter()? {
        let family: Bound<'py, PyAny> = family?;
        let entry: Bound<'py, PyDict> = PyDict::new(families.py());
        for field in [
            "name",
            "pivot_column",
            "value_column",
            "aggregate",
            "type",
            "name_pattern",
        ] {
            entry.set_item(field, native_field(&family, field)?)?;
        }
        rendered.append(entry)?;
    }
    Ok(rendered)
}

/// The hash of a model's own declared columns, column families and contract setting.
pub(crate) fn model_declared_columns_hash(model: &Bound<'_, PyAny>) -> PyResult<String> {
    let py: Python<'_> = model.py();
    let schema_entry: Bound<'_, PyAny> = model_reads::schema_entry(model)?;
    let payload: Bound<'_, PyDict> = PyDict::new(py);
    payload.set_item(
        "contract",
        config_value(py, &model_values(model)?, CONTRACT_CONFIG_KEY)?,
    )?;
    let columns: Bound<'_, PyList> = PyList::empty(py);
    let mut families: Bound<'_, PyList> = PyList::empty(py);
    let mut type_enforcement: Bound<'_, PyAny> = py.None().into_bound(py);
    if !schema_entry.is_none() {
        for column in native_field(&schema_entry, "columns")?.try_iter()? {
            columns.append(column_signature(&column?)?)?;
        }
        families = family_signatures(&native_field(&schema_entry, "dynamic_columns")?)?;
        type_enforcement = native_field(&schema_entry, "type_enforcement")?;
    }
    payload.set_item("columns", columns)?;
    payload.set_item("dynamic_columns", families)?;
    payload.set_item("type_enforcement", type_enforcement)?;
    Ok(stable_identity_hash(&sorted_dumps(
        payload.as_any(),
        COMPACT,
        true,
    )?))
}

/// Whether the function's recorded fingerprint came from an older scheme for the same definition.
fn recorded_function_unchanged(
    function: &Bound<'_, PyAny>,
    (name, dialect): (&str, Option<&str>),
    recorded: &Bound<'_, PyAny>,
) -> PyResult<bool> {
    if recorded.is_none() {
        return Ok(false);
    }
    let fingerprint: Bound<'_, PyAny> = mapping_get(&native_field(recorded, "functions")?, name)?;
    if fingerprint.is_none() {
        return Ok(false);
    }
    let py: Python<'_> = function.py();
    let language: String = function_reads::language(function)?.str()?.extract()?;
    let fingerprint_sql: String = compiled_function_fingerprint_sql(function)?;
    let current: String =
        function_definition_hash(py, name, (&fingerprint_sql, &language, dialect))?;
    Ok(baselined_function_definition_hash(
        py,
        name,
        (&fingerprint_sql, &language, dialect),
        &fingerprint,
    )? != current)
}

/// Each function's local hash in the most recently recorded model metadata that names it.
fn latest_recorded_function_hashes(
    recorded: &Bound<'_, PyAny>,
) -> PyResult<HashMap<String, String>> {
    let mut rows: Vec<(f64, String)> = Vec::new();
    for item in mapping_items(&native_field(recorded, "models")?)?.try_iter()? {
        let (_, fingerprint): (Bound<'_, PyAny>, Bound<'_, PyAny>) = item?.extract()?;
        rows.push((
            timestamp(&native_field(&fingerprint, "ts")?)?,
            native_field(&fingerprint, "metadata_json")?.extract()?,
        ));
    }
    rows.sort_by(|left, right| left.0.total_cmp(&right.0));
    let mut latest: HashMap<String, String> = HashMap::new();
    for (_, metadata) in &rows {
        let Ok(serde_json::Value::Object(object)) = serde_json::from_str(metadata) else {
            continue;
        };
        let Some(serde_json::Value::Object(hashes)) = object.get(LOCAL_FUNCTION_HASHES_KEY) else {
            continue;
        };
        for (name, hash) in hashes {
            if let Some(hash) = hash.as_str() {
                latest.insert(name.clone(), hash.to_owned());
            }
        }
    }
    Ok(latest)
}

/// Local hashes of functions by name; unchanged functions keep the latest local hash `recorded` models saw.
pub(crate) fn function_local_hashes<'py>(
    functions: &Bound<'py, PyAny>,
    dialect: Option<&str>,
    recorded: &Bound<'py, PyAny>,
) -> PyResult<Bound<'py, PyDict>> {
    let py: Python<'py> = functions.py();
    let hashes: Bound<'py, PyDict> = PyDict::new(py);
    let mut recorded_local: Option<HashMap<String, String>> = None;
    for function in functions.try_iter()? {
        let function: Bound<'py, PyAny> = function?;
        let name: String = function_reads::name(&function)?.extract()?;
        let language: Bound<'py, PyAny> =
            native_field(&function_reads::language(&function)?, "value")?;
        let body_hash: String = function_definition_hash(
            py,
            &name,
            (
                &function_reads::body_sql(&function)?.extract::<String>()?,
                &language.extract::<String>()?,
                dialect,
            ),
        )?;
        let payload: Bound<'py, PyDict> = PyDict::new(py);
        payload.set_item(
            "arguments",
            name_type_pairs(&function_reads::arguments(&function)?)?,
        )?;
        payload.set_item("returns", function_reads::returns(&function)?)?;
        payload.set_item(
            "return_columns",
            name_type_pairs(&function_reads::return_columns(&function)?)?,
        )?;
        payload.set_item("body_sql", body_hash)?;
        payload.set_item("language", language)?;
        payload.set_item(
            "runtime_version",
            function_reads::runtime_version(&function)?,
        )?;
        payload.set_item("entry_point", function_reads::entry_point(&function)?)?;
        payload.set_item("packages", function_reads::packages(&function)?)?;
        let mut local_hash: String =
            stable_identity_hash(&sorted_dumps(payload.as_any(), SPACED, true)?);
        if recorded_function_unchanged(&function, (&name, dialect), recorded)? {
            let latest = match &mut recorded_local {
                Some(latest) => latest,
                None => recorded_local.insert(latest_recorded_function_hashes(recorded)?),
            };
            if let Some(previous) = latest.get(&name) {
                local_hash.clone_from(previous);
            }
        }
        hashes.set_item(name, local_hash)?;
    }
    Ok(hashes)
}

fn name_type_pairs<'py>(items: &Bound<'py, PyAny>) -> PyResult<Bound<'py, PyList>> {
    let pairs: Bound<'py, PyList> = PyList::empty(items.py());
    for item in items.try_iter()? {
        let item: Bound<'py, PyAny> = item?;
        pairs.append(PyTuple::new(
            items.py(),
            [native_field(&item, "name")?, native_field(&item, "type")?],
        )?)?;
    }
    Ok(pairs)
}

/// `f"{value or ''}"` of an optional attribute.
fn text_or_empty(owner: &Bound<'_, PyAny>, name: &str) -> PyResult<String> {
    text_of(native_field(owner, name))
}

/// `f"{value or ''}"` of an attribute read, empty when the read failed.
fn text_of(read: PyResult<Bound<'_, PyAny>>) -> PyResult<String> {
    let value: Bound<'_, PyAny> = match read {
        Ok(value) => value,
        Err(_) => return Ok(String::new()),
    };
    if value.is_truthy()? {
        value.str()?.extract()
    } else {
        Ok(String::new())
    }
}

fn rendered_pairs(items: &Bound<'_, PyAny>) -> PyResult<Vec<(String, String)>> {
    items
        .try_iter()?
        .map(|item| {
            let item: Bound<'_, PyAny> = item?;
            Ok((text_or_empty(&item, "name")?, text_or_empty(&item, "type")?))
        })
        .collect()
}

/// Stable function definition text used for change fingerprints.
pub(crate) fn compiled_function_fingerprint_sql(function: &Bound<'_, PyAny>) -> PyResult<String> {
    Ok(function_fingerprint_sql(&FunctionFingerprintFields {
        language: function_reads::language(function)?.str()?.extract()?,
        arguments: rendered_pairs(&function_reads::arguments(function)?)?,
        returns: function_reads::returns(function)?.str()?.extract()?,
        return_columns: rendered_pairs(&function_reads::return_columns(function)?)?,
        runtime_version: text_of(function_reads::runtime_version(function))?,
        entry_point: text_of(function_reads::entry_point(function))?,
        packages: function_reads::packages(function)?
            .try_iter()?
            .map(|package| package?.extract::<String>())
            .collect::<PyResult<_>>()?,
        body_sql: function_reads::body_sql(function)?.extract()?,
    }))
}

/// `(identity_hash, metadata_json)` of one compiled seed from its content and load-affecting config.
pub(crate) fn seed_identity(seed: &Bound<'_, PyAny>) -> PyResult<(String, String)> {
    let py: Python<'_> = seed.py();
    let schema_entry: Bound<'_, PyAny> = seed_reads::schema_entry(seed)?;
    let csv_settings: Bound<'_, PyAny> = native_field(&schema_entry, "csv_settings")?;
    let columns: Bound<'_, PyList> = PyList::empty(py);
    for column in native_field(&schema_entry, "columns")?.try_iter()? {
        columns.append(column_signature(&column?)?)?;
    }
    let rows: Bound<'_, PyAny> = if seed_reads::external(seed)?.is_truthy()? {
        PyList::empty(py).into_any()
    } else {
        seed_rows(
            &native_field(&seed_reads::seed_file(seed)?, "file_path")?,
            &csv_settings,
        )?
    };
    let payload: Bound<'_, PyDict> = PyDict::new(py);
    payload.set_item("name", seed_reads::name(seed)?)?;
    payload.set_item(
        "csv_settings",
        imported(py, DATACLASSES_MODULE, "asdict")?.call1((&csv_settings,))?,
    )?;
    payload.set_item("columns", columns)?;
    payload.set_item("rows", rows)?;
    let metadata_json: String = sorted_dumps(payload.as_any(), COMPACT, false)?;
    Ok((stable_identity_hash(&metadata_json), metadata_json))
}

/// Every row of the seed file as the standard library CSV reader yields it.
fn seed_rows<'py>(
    file_path: &Bound<'py, PyAny>,
    csv_settings: &Bound<'py, PyAny>,
) -> PyResult<Bound<'py, PyAny>> {
    let py: Python<'py> = file_path.py();
    let encoding: Bound<'py, PyAny> = match native_field(csv_settings, "encoding")? {
        value if value.is_truthy()? => value,
        _ => PyString::new(py, DEFAULT_SEED_ENCODING).into_any(),
    };
    let options: Bound<'py, PyDict> = PyDict::new(py);
    for option in SEED_DIALECT_OPTIONS {
        let value: Bound<'py, PyAny> = native_field(csv_settings, option)?;
        if !value.is_none() {
            options.set_item(option, value)?;
        }
    }
    let handle: Bound<'py, PyAny> = imported(py, BUILTINS_MODULE, "open")?.call(
        (file_path, "r"),
        Some(&keywords(
            py,
            &[
                ("encoding", encoding),
                ("newline", PyString::new(py, "").into_any()),
            ],
        )?),
    )?;
    within(&handle, |entered| {
        let rows: Bound<'py, PyList> = PyList::empty(py);
        let reader: Bound<'py, PyAny> =
            imported(py, CSV_MODULE, "reader")?.call((entered,), Some(&options))?;
        for row in reader.try_iter()? {
            rows.append(PyList::new(
                py,
                row?.try_iter()?.collect::<PyResult<Vec<_>>>()?,
            )?)?;
        }
        Ok(rows.into_any())
    })
}

/// Deterministic metadata JSON for model version identity.
#[pyfunction]
#[pyo3(signature = (*, model_name, config_values, local_function_hashes = None, execution_signature = None))]
fn build_version_identity_metadata_json<'py>(
    model_name: &Bound<'py, PyAny>,
    config_values: &Bound<'py, PyAny>,
    local_function_hashes: Option<Bound<'py, PyAny>>,
    execution_signature: Option<Bound<'py, PyAny>>,
) -> PyResult<String> {
    version_identity_metadata_json(
        model_name,
        config_values,
        (local_function_hashes, execution_signature),
    )
}

/// Deterministic non-query model fingerprint metadata JSON.
#[pyfunction]
#[pyo3(signature = (*, model, function_local_hashes = None, hook_version_hashes = None))]
pub(crate) fn build_model_version_identity_metadata_json<'py>(
    model: &Bound<'py, PyAny>,
    function_local_hashes: Option<Bound<'py, PyAny>>,
    hook_version_hashes: Option<Bound<'py, PyAny>>,
) -> PyResult<String> {
    model_version_identity_metadata_json(
        model,
        function_local_hashes.as_ref(),
        hook_version_hashes.as_ref(),
    )
}

/// Hash the model's own declared columns, column families and contract setting.
#[pyfunction]
#[pyo3(signature = (*, model))]
fn declared_columns_hash(model: &Bound<'_, PyAny>) -> PyResult<String> {
    model_declared_columns_hash(model)
}

/// Local-only semantic hashes of functions by name.
#[pyfunction]
#[pyo3(signature = (*, functions, dialect))]
pub(crate) fn build_function_local_hashes<'py>(
    functions: &Bound<'py, PyAny>,
    dialect: Option<&str>,
) -> PyResult<Bound<'py, PyDict>> {
    function_local_hashes(
        functions,
        dialect,
        &functions.py().None().into_bound(functions.py()),
    )
}

/// Stable function definition text used for change fingerprints.
#[pyfunction]
pub(crate) fn build_compiled_function_fingerprint_sql(
    function: &Bound<'_, PyAny>,
) -> PyResult<String> {
    compiled_function_fingerprint_sql(function)
}

/// `(identity_hash, metadata_json)` for one compiled seed.
#[pyfunction]
pub(crate) fn build_seed_identity(seed: &Bound<'_, PyAny>) -> PyResult<(String, String)> {
    seed_identity(seed)
}

pub(crate) fn register(module: &Bound<'_, PyModule>) -> PyResult<()> {
    module.add_function(wrap_pyfunction!(
        build_version_identity_metadata_json,
        module
    )?)?;
    module.add_function(wrap_pyfunction!(
        build_model_version_identity_metadata_json,
        module
    )?)?;
    module.add_function(wrap_pyfunction!(declared_columns_hash, module)?)?;
    module.add_function(wrap_pyfunction!(build_function_local_hashes, module)?)?;
    module.add_function(wrap_pyfunction!(
        build_compiled_function_fingerprint_sql,
        module
    )?)?;
    module.add_function(wrap_pyfunction!(build_seed_identity, module)?)
}
