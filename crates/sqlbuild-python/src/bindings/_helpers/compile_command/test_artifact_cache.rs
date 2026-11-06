//! Persistent identities of compiled SQL test artifacts, so unchanged tests keep their files.

use std::collections::HashMap;
use std::path::PathBuf;

use pyo3::prelude::{Bound, PyAny, PyAnyMethods, PyResult, Python};
use rayon::prelude::{IntoParallelRefIterator, ParallelIterator};
use sqlbuild_cache::artifacts::main::read_sql_test_cache::read_sql_test_cache;
use sqlbuild_cache::artifacts::main::write_sql_test_cache::write_sql_test_cache;
use sqlbuild_cache::artifacts::models::SqlTestArtifactRecord;
use sqlbuild_cache::digest::main::bytes_digest::bytes_digest;
use sqlbuild_cache::digest::main::hex_digest::hex_digest;
use sqlbuild_core::json::models::JsonValue;

use crate::bindings::_helpers::boundary::python_calls::imported;
use crate::bindings::_helpers::compile_outputs::json_report::{orjson_sorted, orjson_value};
use crate::bindings::_helpers::records::construct::record_class;
use crate::bindings::_helpers::records::ids::RecordId;

const ALGORITHM_FINGERPRINT: &str = "sql-test-artifact-v5-orjson-blake3-identity";
/// Items a loop holding the interpreter handles before letting other threads run.
const INTERPRETER_YIELD_INTERVAL: usize = 64;
const METADATA_MODULE: &str = "importlib.metadata";

/// The digest shared by every test artifact identity, and each model's own identity.
pub(crate) struct IdentityContext {
    common_identity: String,
    model_identities: HashMap<String, String>,
}

/// The hexadecimal BLAKE3 digest of each payload text, hashed without holding the interpreter.
pub(crate) fn payload_digests(py: Python<'_>, payloads: &[String]) -> Vec<String> {
    py.detach(|| {
        payloads
            .par_iter()
            .map(|payload| hex_digest(&bytes_digest(payload.as_bytes())))
            .collect()
    })
}

/// Let other threads take the interpreter every few items of a long loop that holds it.
pub(crate) fn yield_interpreter(py: Python<'_>, index: usize) {
    if index % INTERPRETER_YIELD_INTERVAL == INTERPRETER_YIELD_INTERVAL - 1 {
        py.detach(|| ());
    }
}

/// One identity object, assembled without allocating temporary Python containers.
fn object(entries: Vec<(&str, JsonValue)>) -> JsonValue {
    JsonValue::Object(
        entries
            .into_iter()
            .map(|(key, value)| (key.to_owned(), value))
            .collect(),
    )
}

/// A compiled record's field converted at its boundary, retaining orjson's scalar checks.
fn json_field(value: &Bound<'_, PyAny>, name: &str) -> PyResult<JsonValue> {
    orjson_value(&value.getattr(name)?)
}

/// String pairs ordered lexicographically, as Python orders these cache inputs.
fn sorted_pairs(mut items: Vec<(String, String)>) -> JsonValue {
    items.sort();
    JsonValue::Array(
        items
            .into_iter()
            .map(|(first, second)| {
                JsonValue::Array(vec![JsonValue::String(first), JsonValue::String(second)])
            })
            .collect(),
    )
}

fn package_version(py: Python<'_>, package: &str) -> PyResult<JsonValue> {
    match imported(py, METADATA_MODULE, "version")?.call1((package,)) {
        Ok(version) => orjson_value(&version),
        Err(error)
            if error.is_instance(py, &imported(py, METADATA_MODULE, "PackageNotFoundError")?) =>
        {
            Ok(JsonValue::String("unknown".to_owned()))
        }
        Err(error) => Err(error),
    }
}

/// `sorted((str(dep.resource_type), dep.name) for dep in deps)`.
fn sorted_deps(deps: &Bound<'_, PyAny>) -> PyResult<JsonValue> {
    let mut pairs: Vec<(String, String)> = Vec::new();
    for dep in deps.try_iter()? {
        let dep = dep?;
        pairs.push((
            dep.getattr("resource_type")?.str()?.extract()?,
            dep.getattr("name")?.extract()?,
        ));
    }
    Ok(sorted_pairs(pairs))
}

fn model_identity_payload(model: &Bound<'_, PyAny>) -> PyResult<String> {
    let values: Bound<'_, PyAny> = model.getattr("config")?.getattr("values")?;
    orjson_sorted(&object(vec![
        ("name", json_field(model, "name")?),
        ("query_sql", json_field(model, "query_sql")?),
        ("deps", sorted_deps(&model.getattr("deps")?)?),
        (
            "cursor",
            JsonValue::Array(vec![
                orjson_value(&values.call_method1("get", ("cursor_type",))?)?,
                orjson_value(&values.call_method1("get", ("cursor_grain",))?)?,
            ]),
        ),
    ]))
}

/// Hash adapter and model inputs once for every test artifact identity.
pub(crate) fn identity_context<'py>(
    project: &Bound<'py, PyAny>,
    adapter: &Bound<'py, PyAny>,
) -> PyResult<IdentityContext> {
    let py: Python<'py> = project.py();
    let adapter_type: Bound<'py, PyAny> = adapter.get_type().into_any();
    let mut functions: Vec<(String, String)> = Vec::new();
    for function in project.getattr("functions")?.try_iter()? {
        let function: Bound<'py, PyAny> = function?;
        functions.push((
            function.getattr("name")?.extract()?,
            function
                .getattr("destination")?
                .getattr("qualified_name")?
                .extract()?,
        ));
    }
    let version_tuple: Bound<'py, PyAny> = py
        .import("platform")?
        .call_method0("python_version_tuple")?
        .get_item(pyo3::types::PySlice::new(py, 0, 2, 1))?;
    let common = object(vec![
        (
            "algorithm",
            JsonValue::String(ALGORITHM_FINGERPRINT.to_owned()),
        ),
        ("sqlbuild_version", package_version(py, "sqlbuild")?),
        (
            "polyglot_version",
            package_version(py, "polyglot-sql-chio")?,
        ),
        ("python_version", orjson_value(&version_tuple)?),
        (
            "adapter_class",
            JsonValue::String(format!(
                "{}.{}",
                adapter_type.getattr("__module__")?.str()?,
                adapter_type.getattr("__qualname__")?.str()?
            )),
        ),
        (
            "set_difference_operator",
            orjson_value(&adapter.call_method0("render_set_difference_operator")?)?,
        ),
        (
            "sql_analysis_dialect",
            orjson_value(&adapter.call_method0("sql_analysis_dialect")?)?,
        ),
        (
            "requires_derived_table_aliases",
            orjson_value(&adapter.call_method0("requires_derived_table_aliases")?)?,
        ),
        (
            "sql_analysis_enabled",
            json_field(&project.getattr("settings")?, "sql_analysis")?,
        ),
        (
            "effective_target_name",
            json_field(project, "effective_target_name")?,
        ),
        (
            "effective_target_database",
            json_field(project, "effective_target_database")?,
        ),
        (
            "effective_target_schema",
            json_field(project, "effective_target_schema")?,
        ),
        ("function_locations", sorted_pairs(functions)),
    ]);
    let mut names: Vec<String> = Vec::new();
    let mut payloads: Vec<String> = Vec::new();
    for (index, model) in project.getattr("models")?.try_iter()?.enumerate() {
        let model: Bound<'py, PyAny> = model?;
        names.push(model.getattr("name")?.extract()?);
        payloads.push(model_identity_payload(&model)?);
        yield_interpreter(py, index);
    }
    payloads.push(orjson_sorted(&common)?);
    let mut digests: Vec<String> = payload_digests(py, &payloads);
    let common_identity: String = digests.pop().unwrap_or_default();
    let model_identities: HashMap<String, String> = names.into_iter().zip(digests).collect();
    Ok(IdentityContext {
        common_identity,
        model_identities,
    })
}

/// `[(cte.name, cte.sql_body) for cte in ctes]`.
fn cte_payload(ctes: &Bound<'_, PyAny>) -> PyResult<JsonValue> {
    let mut rows: Vec<JsonValue> = Vec::new();
    for cte in ctes.try_iter()? {
        rows.push(cte_pair(&cte?)?);
    }
    Ok(JsonValue::Array(rows))
}

fn cte_pair(cte: &Bound<'_, PyAny>) -> PyResult<JsonValue> {
    Ok(JsonValue::Array(vec![
        json_field(cte, "name")?,
        json_field(cte, "sql_body")?,
    ]))
}

/// `sorted(mapping.items())`.
fn sorted_items(mapping: &Bound<'_, PyAny>) -> PyResult<JsonValue> {
    Ok(sorted_pairs(
        mapping
            .call_method0("items")?
            .try_iter()?
            .map(|item| item?.extract::<(String, String)>())
            .collect::<PyResult<Vec<_>>>()?,
    ))
}

fn test_payload(test: &Bound<'_, PyAny>) -> PyResult<JsonValue> {
    let py = test.py();
    let mut payload = vec![
        ("case_fingerprint", json_field(test, "case_fingerprint")?),
        ("mode", json_field(&test.getattr("mode")?, "value")?),
        ("sql_body", json_field(test, "sql_body")?),
        ("scope_deps", sorted_deps(&test.getattr("scope_deps")?)?),
    ];
    let test_payload = test.getattr("payload")?;
    if test_payload.is_instance(&record_class(py, RecordId::CompiledModelSqlTestPayload)?)? {
        let block = test.getattr("test_block")?;
        payload.push((
            "model",
            object(vec![
                (
                    "authored_ctes",
                    cte_payload(&test_payload.getattr("authored_ctes")?)?,
                ),
                (
                    "macro_mocks",
                    sorted_items(&test_payload.getattr("macro_mocks")?)?,
                ),
                (
                    "model_query_overrides",
                    sorted_items(&test_payload.getattr("model_query_overrides")?)?,
                ),
                (
                    "expected_model_names",
                    json_field(&test_payload, "expected_model_names")?,
                ),
                (
                    "assertion_ctes",
                    cte_payload(&test_payload.getattr("assertion_ctes")?)?,
                ),
                (
                    "cursor_window",
                    JsonValue::Array(vec![
                        json_field(&block, "cursor_start")?,
                        json_field(&block, "cursor_end")?,
                    ]),
                ),
            ]),
        ));
    } else if test_payload.is_instance(&record_class(
        py,
        RecordId::CompiledDirectLogicSqlTestPayload,
    )?)? {
        payload.push((
            "direct",
            object(vec![
                (
                    "helper_ctes",
                    cte_payload(&test_payload.getattr("helper_ctes")?)?,
                ),
                (
                    "actual_cte",
                    JsonValue::Array(vec![cte_pair(&test_payload.getattr("actual_cte")?)?]),
                ),
                (
                    "expected_cte",
                    JsonValue::Array(vec![cte_pair(&test_payload.getattr("expected_cte")?)?]),
                ),
                (
                    "tested_resource_names",
                    json_field(&test_payload, "tested_resource_names")?,
                ),
            ]),
        ));
    }
    Ok(object(payload))
}

/// The text whose digest is the complete identity of one rendered comparison artifact.
pub(crate) fn artifact_identity_payload(
    test: &Bound<'_, PyAny>,
    model_chain_names: &Bound<'_, PyAny>,
    context: &IdentityContext,
) -> PyResult<String> {
    let mut chain: Vec<JsonValue> = Vec::new();
    for name in model_chain_names.try_iter()? {
        let name: String = name?.extract()?;
        let identity: Option<&String> = context.model_identities.get(&name);
        chain.push(JsonValue::Array(vec![
            JsonValue::String(name),
            identity.cloned().map_or(JsonValue::Null, JsonValue::String),
        ]));
    }
    orjson_sorted(&object(vec![
        (
            "common_identity",
            JsonValue::String(context.common_identity.clone()),
        ),
        ("test", test_payload(test)?),
        ("model_chain", JsonValue::Array(chain)),
    ]))
}

/// The text whose digest is the stable cache slot of one source test case.
pub(crate) fn artifact_record_key_payload(test: &Bound<'_, PyAny>) -> PyResult<String> {
    let source_path: Bound<'_, PyAny> = test.getattr("source_path")?;
    orjson_sorted(&object(vec![
        (
            "source_path",
            if source_path.is_none() {
                JsonValue::Null
            } else {
                orjson_value(&source_path.call_method0("as_posix")?)?
            },
        ),
        ("block_index", json_field(test, "block_index")?),
        ("name", json_field(test, "name")?),
        ("parent_name", json_field(test, "parent_name")?),
        ("case_name", json_field(test, "case_name")?),
        ("case_index", json_field(test, "case_index")?),
    ]))
}

/// The valid cache records; a cache that cannot be read never fails compilation.
pub(crate) fn read_cache(
    cache_dir: &Bound<'_, PyAny>,
) -> PyResult<HashMap<String, SqlTestArtifactRecord>> {
    let py: Python<'_> = cache_dir.py();
    if cache_dir.is_none() {
        return Ok(HashMap::new());
    }
    let directory: PathBuf = cache_dir.extract()?;
    Ok(py.detach(|| read_sql_test_cache(&directory)))
}

/// Atomically replace the cache snapshot; a cache that cannot be written is ignored.
pub(crate) fn write_cache(
    cache_dir: &Bound<'_, PyAny>,
    records: &[(String, SqlTestArtifactRecord)],
) -> PyResult<()> {
    let py: Python<'_> = cache_dir.py();
    if cache_dir.is_none() {
        return Ok(());
    }
    let directory: PathBuf = cache_dir.extract()?;
    py.detach(|| write_sql_test_cache(&directory, records));
    Ok(())
}
