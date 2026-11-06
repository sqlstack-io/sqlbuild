//! Write compiled project output to `target/`: models, functions, audits, SQL tests, manifest.

use std::collections::{BTreeMap, HashMap, HashSet};
use std::path::{Path, PathBuf};

use pyo3::exceptions::PyValueError;
use pyo3::prelude::{
    Bound, Py, PyAny, PyAnyMethods, PyErr, PyModule, PyModuleMethods, PyResult, Python,
};
use pyo3::types::{PyDict, PyDictMethods, PyTuple, PyTupleMethods};
use pyo3::{IntoPyObjectExt, pyclass, pyfunction, wrap_pyfunction};
use sqlbuild_cache::artifacts::main::new_sql_test_record::new_sql_test_record;
use sqlbuild_cache::artifacts::main::reusable_sql_test_record::reusable_sql_test_record;
use sqlbuild_cache::artifacts::models::{SqlTestArtifactRecord, SqlTestRecordMiss};
use sqlbuild_cache::project_reuse::constants::FAILED_COMPILED_DIRECTORY_NAME;
use sqlbuild_host::compile_context::main::record_compile_timing::record_compile_timing;
use sqlbuild_host::entries::models::ExecutorHost;

use crate::bindings::_helpers::analysis::sql_test_comparisons::single_comparison_sql;
use crate::bindings::_helpers::boundary::python_calls::{
    KeywordArguments, call_with, imported, keywords, within,
};
use crate::bindings::_helpers::compile_command::artifact_batch::ArtifactBatch;
use crate::bindings::_helpers::compile_command::constants::RACY_WINDOW_NS;
use crate::bindings::_helpers::compile_command::reuse_recording::{
    ArtifactRecorder, artifact_kept, artifact_moved, artifact_tree_moved,
};
use crate::bindings::_helpers::compile_command::test_artifact_cache::{
    IdentityContext, artifact_identity_payload, artifact_record_key_payload, identity_context,
    payload_digests, read_cache, write_cache, yield_interpreter,
};
use crate::bindings::_helpers::compile_driver::stage_work::{
    SQL_TEST_ARTIFACT_PLANS, SQL_TEST_MODEL_CHAINS, count_stage,
};
use crate::bindings::_helpers::compile_outputs::artifacts::{
    publish_staged_artifacts, remove_stale_artifacts,
};
use crate::bindings::_helpers::compile_outputs::runtime_target::{
    compiled_sql_test_path, planned_sql_test_path,
};
use crate::bindings::_helpers::compiled_views::compiled_model_view::NativeCompiledModel;
use crate::bindings::_helpers::native_host::host;
use crate::bindings::_helpers::records::construct::new_record_with;
use crate::bindings::_helpers::records::construct::{enum_member, error_class};
use crate::bindings::_helpers::records::fields::field as native_field;
use crate::bindings::_helpers::records::fields::mapping_get_or;
use crate::bindings::_helpers::records::fields::path_posix_text;
use crate::bindings::_helpers::records::ids::{ErrorId, RecordId};
use crate::bindings::_helpers::sql_test_glue::models::SqlTestPlanOptions;
use crate::bindings::_helpers::sql_test_glue::plan_driver::planned_rows;
use crate::bindings::_helpers::sql_test_glue::plan_driver::sql_test_model_chain_names_by_key;
use crate::bindings::_helpers::sql_test_glue::planning::PlanRow;

const COMPILED_DIR: &str = "compiled";
const MODELS_DIR: &str = "models";
const FUNCTIONS_DIR: &str = "functions";
const AUDITS_DIR: &str = "audits";
const GENERIC_DIR: &str = "generic";
const SINGULAR_DIR: &str = "singular";
const TESTS_DIR: &str = "tests";
const MANIFEST_FILE: &str = "manifest.json";
/// The compile cache file stamping the compiled tree the last stale-file walk left.
const COMPILED_TREE_STAMP_FILE: &str = "compiled-tree.bin";

const SQL_FILE_SUFFIX: &str = ".sql";
const SQL_TEST_CACHE_LOGGER: &str = "sqlbuild.compile.reuse";
/// The miss reason of a SQL test the cache file holds no record for.
const NOT_RECORDED: &str = "not_recorded";
/// `functions/<language>` path components that already lead a function's path.
const LANGUAGE_DIR_DEPTH: usize = 2;
const PUBLISHED_STAGING_CHANGED: &str = "changed";
const PUBLISHED_TREE: &str = "tree";
const TARGET_OPTIONS: (&[&str], &[&str]) = (&["target_dir", "adapter", "project"], &["manifest"]);
const STAGING_CHANGED_HELP: &str =
    "Rerun the compile; avoid deleting target/ while a compile is running.";
/// The first line of a compiled model whose Rules failed.
const RULE_FAILED_HEADER: &str = "-- sqb: compile failed for this model; not buildable";

/// `base / part / ...`.
fn joined<'py>(base: &Bound<'py, PyAny>, parts: &[&str]) -> PyResult<Bound<'py, PyAny>> {
    let mut path: Bound<'py, PyAny> = base.clone();
    for part in parts {
        path = path.div(*part)?;
    }
    Ok(path)
}

fn path_class(py: Python<'_>) -> PyResult<Bound<'_, PyAny>> {
    imported(py, "pathlib", "Path")
}

/// `relative_path` under `models/`, unless it already starts there.
fn model_output_path<'py>(relative_path: &Bound<'py, PyAny>) -> PyResult<Bound<'py, PyAny>> {
    let py: Python<'py> = relative_path.py();
    let parts: Bound<'py, PyTuple> = native_field(relative_path, "parts")?.downcast_into()?;
    if !parts.is_empty() && parts.get_item(0)?.eq(MODELS_DIR)? {
        return path_class(py)?.call1(parts);
    }
    path_class(py)?.call1((MODELS_DIR,))?.div(relative_path)
}

/// `compiled / model_output_path(relative_path)` for a `/`-separated relative path.
fn native_model_output_path(compiled: &Path, relative_path: &str) -> PathBuf {
    let mut path: PathBuf = compiled.to_path_buf();
    let mut segments = relative_path
        .split('/')
        .filter(|segment| !segment.is_empty())
        .peekable();
    if segments.peek() != Some(&MODELS_DIR) {
        path.push(MODELS_DIR);
    }
    for segment in segments {
        path.push(segment);
    }
    path
}

/// Each path object as a native path.
fn extracted(paths: &[Bound<'_, PyAny>]) -> PyResult<Vec<PathBuf>> {
    paths.iter().map(|path| path.extract()).collect()
}

/// `relative_path` under `functions/<language>/`, as a `.sql` file.
fn function_output_path<'py>(
    relative_path: &Bound<'py, PyAny>,
    language: &Bound<'py, PyAny>,
) -> PyResult<Bound<'py, PyAny>> {
    let py: Python<'py> = relative_path.py();
    let parts: Bound<'py, PyTuple> = native_field(relative_path, "parts")?.downcast_into()?;
    let language_dir: Bound<'py, PyAny> = native_field(language, "value")?;
    let path: Bound<'py, PyAny> = if parts.len() >= LANGUAGE_DIR_DEPTH
        && parts.get_item(0)?.eq(FUNCTIONS_DIR)?
        && parts.get_item(1)?.eq(&language_dir)?
    {
        path_class(py)?.call1(parts)?
    } else {
        path_class(py)?
            .call1((FUNCTIONS_DIR,))?
            .div(language_dir)?
            .div(relative_path)?
    };
    path.call_method1("with_suffix", (SQL_FILE_SUFFIX,))
}

/// The audit's folder and file name: generic audits under their target, others singular.
fn audit_output_path<'py>(audit: &Bound<'py, PyAny>) -> PyResult<Bound<'py, PyAny>> {
    let py: Python<'py> = audit.py();
    let target: Bound<'py, PyAny> = native_field(audit, "attached_target_name")?;
    let column: Bound<'py, PyAny> = native_field(audit, "attached_column_name")?;
    let name: String = native_field(audit, "name")?.str()?.to_string();
    let folder: Bound<'py, PyAny> = if target.is_none() {
        path_class(py)?.call1((SINGULAR_DIR,))?
    } else {
        path_class(py)?.call1((GENERIC_DIR,))?.div(&target)?
    };
    let file_name: String = if !target.is_none() && !column.is_none() {
        format!("{name}__{}{SQL_FILE_SUFFIX}", column.str()?)
    } else {
        format!("{name}{SQL_FILE_SUFFIX}")
    };
    folder.div(file_name)
}

/// The function's `CREATE FUNCTION` statements joined into one artifact.
fn function_sql(adapter: &Bound<'_, PyAny>, function: &Bound<'_, PyAny>) -> PyResult<String> {
    let py: Python<'_> = adapter.py();
    let mut arguments: Vec<(&str, Bound<'_, PyAny>)> = vec![(
        "destination",
        native_field(function, "destination")?.getattr("qualified_name")?,
    )];
    for field in [
        "arguments",
        "returns",
        "body_sql",
        "return_columns",
        "language",
        "runtime_version",
        "entry_point",
        "packages",
    ] {
        arguments.push((field, native_field(function, field)?));
    }
    let statements: Vec<String> = native_field(adapter, "render_create_function")?
        .call((), Some(&keywords(py, &arguments)?))?
        .extract()?;
    Ok(statements.join(";\n\n"))
}

/// Delete compiled files no write group manages, skipping the walk while the tree is unchanged.
fn remove_stale(
    target_dir: &Bound<'_, PyAny>,
    managed: Vec<PathBuf>,
    cache_dir: Option<PathBuf>,
) -> PyResult<()> {
    let py: Python<'_> = target_dir.py();
    let compiled: PathBuf = joined(target_dir, &[COMPILED_DIR])?.extract()?;
    let stamp: Option<(PathBuf, i64)> =
        cache_dir.map(|directory| (directory.join(COMPILED_TREE_STAMP_FILE), RACY_WINDOW_NS));
    record_compile_timing("stale_traversal_ms", || {
        remove_stale_artifacts(py, compiled, managed, stamp).map(|_| ())
    })
}

fn write_manifest(
    target_dir: &Bound<'_, PyAny>,
    manifest: &Bound<'_, PyAny>,
    recorder: ArtifactRecorder<'_>,
) -> PyResult<()> {
    if manifest.is_none() {
        return Ok(());
    }
    let mut batch: ArtifactBatch = ArtifactBatch::new(true, recorder);
    batch.queue(
        &joined(target_dir, &[MANIFEST_FILE])?,
        manifest.call_method0("contents")?.extract()?,
    )?;
    batch.flush(target_dir.py())
}

/// Whether `target/compiled` exists, after making sure `target/` does.
fn prepared_target(target_dir: &Bound<'_, PyAny>) -> PyResult<bool> {
    let py: Python<'_> = target_dir.py();
    let existed: bool = joined(target_dir, &[COMPILED_DIR])?
        .call_method0("is_dir")?
        .is_truthy()?;
    let _ = target_dir.call_method(
        "mkdir",
        (),
        Some(&keywords(
            py,
            &[
                ("parents", true.into_bound_py_any(py)?),
                ("exist_ok", true.into_bound_py_any(py)?),
            ],
        )?),
    )?;
    Ok(existed)
}

fn written_target<'py>(
    target_dir: &Bound<'py, PyAny>,
    counts: [usize; 5],
    diagnostics: Bound<'py, PyAny>,
) -> PyResult<Bound<'py, PyAny>> {
    let py: Python<'py> = target_dir.py();
    let [models, seeds, functions, audits, tests] = counts;
    new_record_with(
        py,
        RecordId::WrittenTarget,
        &[
            ("model_count", models.into_bound_py_any(py)?),
            ("seed_count", seeds.into_bound_py_any(py)?),
            ("function_count", functions.into_bound_py_any(py)?),
            ("audit_count", audits.into_bound_py_any(py)?),
            ("test_count", tests.into_bound_py_any(py)?),
            ("target_dir", target_dir.clone()),
            ("diagnostics", diagnostics),
        ],
    )
}

fn length(value: &Bound<'_, PyAny>, field: &str) -> PyResult<usize> {
    native_field(value, field)?.len()
}

/// Write the planner's compiled output under `target_dir`.
fn write_planned_target<'py>(
    target_dir: &Bound<'py, PyAny>,
    adapter: &Bound<'py, PyAny>,
    plan_output: &Bound<'py, PyAny>,
    manifest: &Bound<'py, PyAny>,
) -> PyResult<Bound<'py, PyAny>> {
    let py: Python<'py> = target_dir.py();
    let remove_stale_files: bool = prepared_target(target_dir)?;
    let mut batch: ArtifactBatch = ArtifactBatch::new(remove_stale_files, None);
    let mut managed: Vec<Bound<'py, PyAny>> = Vec::new();
    let compiled: Bound<'py, PyAny> = joined(target_dir, &[COMPILED_DIR])?;
    for entry in native_field(plan_output, "model_entries")?.try_iter()? {
        let entry: Bound<'py, PyAny> = entry?;
        let path: Bound<'py, PyAny> =
            compiled.div(model_output_path(&native_field(&entry, "relative_path")?)?)?;
        batch.queue_sql(
            &path,
            &native_field(&entry, "resolved_sql")?.extract::<String>()?,
        )?;
        managed.push(path);
    }
    batch.flush(py)?;
    for entry in native_field(plan_output, "function_entries")?.try_iter()? {
        let entry: Bound<'py, PyAny> = entry?;
        if native_field(&entry, "destination")?
            .getattr("qualified_name")?
            .is_none()
        {
            continue;
        }
        let sql: String = function_sql(adapter, &entry)?;
        let path: Bound<'py, PyAny> = compiled.div(function_output_path(
            &native_field(&entry, "relative_path")?,
            &native_field(&entry, "language")?,
        )?)?;
        batch.queue_sql(&path, &sql)?;
        managed.push(path);
    }
    batch.flush(py)?;
    for entry in native_field(plan_output, "audit_entries")?.try_iter()? {
        let entry: Bound<'py, PyAny> = entry?;
        let path: Bound<'py, PyAny> = compiled.div(AUDITS_DIR)?.div(audit_output_path(&entry)?)?;
        batch.queue_sql(
            &path,
            &native_field(&entry, "resolved_sql")?.extract::<String>()?,
        )?;
        managed.push(path);
    }
    batch.flush(py)?;
    for entry in native_field(plan_output, "test_entries")?.try_iter()? {
        let entry: Bound<'py, PyAny> = entry?;
        let path: Bound<'py, PyAny> = compiled
            .div(TESTS_DIR)?
            .div(planned_sql_test_path(&entry)?)?;
        let comparison_sql: String = record_compile_timing("comparison_render_ms", || {
            single_comparison_sql(&entry, adapter)
        })?;
        batch.queue_sql(&path, &comparison_sql)?;
        managed.push(path);
    }
    batch.flush(py)?;
    if remove_stale_files {
        remove_stale(target_dir, extracted(&managed)?, None)?;
    }
    write_manifest(target_dir, manifest, None)?;
    written_target(
        target_dir,
        [
            length(plan_output, "model_entries")?,
            length(plan_output, "seed_entries")?,
            length(plan_output, "function_entries")?,
            length(plan_output, "audit_entries")?,
            length(plan_output, "test_entries")?,
        ],
        PyTuple::empty(py).into_any(),
    )
}

/// SQL tests planned ahead of the write phase, and where the compile notes its artifact writes.
#[derive(Clone, Copy, Default)]
pub(crate) struct Prepared<'a, 'py> {
    /// `planned_tests()` hands over the background plan, when one was started.
    pub(crate) planned_tests: Option<&'a Bound<'py, PyAny>>,
    pub(crate) recorder: ArtifactRecorder<'a>,
}

/// A test needing a fresh artifact, with its cache slot and artifact identity when caching.
type PendingTest = (Py<PyAny>, Option<String>, Option<String>);

/// The managed SQL test artifact paths and the planning diagnostics of one write.
type WrittenTests<'py> = (Vec<PathBuf>, Vec<Bound<'py, PyAny>>);

/// SQL tests whose cached artifacts cannot be reused, planned, beside the reusable ones.
#[pyclass(module = "sqlbuild._native", frozen)]
pub(crate) struct PlannedStaticSqlTests {
    tests_root: PathBuf,
    /// Each pending test with its cache slot and artifact identity, when caching.
    pending: Vec<PendingTest>,
    /// One native plan per pending test, with rendered SQL already validated.
    artifacts: Vec<PlanRow>,
    cached_paths: Vec<PathBuf>,
    cached_records: Vec<(String, SqlTestArtifactRecord)>,
}

/// `zip(pending, artifacts, strict=True)`.
fn zipped(
    planned: &PlannedStaticSqlTests,
) -> PyResult<impl Iterator<Item = (&PendingTest, &PlanRow)>> {
    if planned.pending.len() != planned.artifacts.len() {
        let relation: &str = if planned.artifacts.len() < planned.pending.len() {
            "shorter"
        } else {
            "longer"
        };
        return Err(PyValueError::new_err(format!(
            "zip() argument 2 is {relation} than argument 1"
        )));
    }
    Ok(planned.pending.iter().zip(planned.artifacts.iter()))
}

/// Plan the tests natively inside the SQL-test planning lifecycle.
fn planned_artifacts<'py>(
    adapter: &Bound<'py, PyAny>,
    project: &Bound<'py, PyAny>,
    tests: Vec<Bound<'py, PyAny>>,
) -> PyResult<Vec<PlanRow>> {
    let py: Python<'py> = project.py();
    if tests.is_empty() {
        return Ok(Vec::new());
    }
    let lifecycle: Bound<'py, PyAny> = call_with(
        &host::entry(py, ExecutorHost::OperationLifecycle)?,
        &[
            ("operation_kind", "project".into_bound_py_any(py)?),
            ("operation_name", "sql_test_planning".into_bound_py_any(py)?),
        ],
    )?;
    within(&lifecycle, |entered| {
        count_stage(py, SQL_TEST_ARTIFACT_PLANS, 1)?;
        let artifacts = planned_rows(
            (project, adapter),
            &PyTuple::new(py, tests)?.into_any(),
            &SqlTestPlanOptions {
                sql_analysis_enabled: native_field(
                    &native_field(project, "settings")?,
                    "sql_analysis",
                )?,
                render_sql: true,
                include_plan: false,
                artifact: true,
            },
        )?;
        let metadata: Bound<'py, PyDict> = PyDict::new(py);
        metadata.set_item("item_count", artifacts.len())?;
        let _ = call_with(
            &native_field(entered, "completed")?,
            &[("metadata", metadata.into_any())],
        )?;
        Ok(artifacts)
    })
}

/// Each test's cache slot and artifact identity, hashed together without holding the interpreter.
fn test_identities<'py>(
    tests: &[Bound<'py, PyAny>],
    context: &IdentityContext,
    chains: &Bound<'py, PyAny>,
) -> PyResult<Vec<(String, String)>> {
    let Some(first) = tests.first() else {
        return Ok(Vec::new());
    };
    let py: Python<'py> = first.py();
    let empty: Bound<'py, PyTuple> = PyTuple::empty(py);
    let mut payloads: Vec<String> = Vec::with_capacity(tests.len() * 2);
    for (index, test) in tests.iter().enumerate() {
        let chain: Bound<'py, PyAny> = mapping_get_or(chains, native_field(test, "key")?, &empty)?;
        payloads.push(artifact_record_key_payload(test)?);
        payloads.push(artifact_identity_payload(test, &chain, context)?);
        yield_interpreter(py, index);
    }
    let mut digests = payload_digests(py, &payloads).into_iter();
    let mut identities: Vec<(String, String)> = Vec::with_capacity(tests.len());
    while let (Some(record_key), Some(identity)) = (digests.next(), digests.next()) {
        identities.push((record_key, identity));
    }
    Ok(identities)
}

/// Log why a warm SQL test artifact cache could not keep some artifacts, by reason.
fn log_cache_misses(py: Python<'_>, misses: &BTreeMap<&'static str, usize>) -> PyResult<()> {
    let reasons: String = misses
        .iter()
        .map(|(reason, count)| format!("{reason}={count}"))
        .collect::<Vec<_>>()
        .join(",");
    host::log_debug_message(
        py,
        SQL_TEST_CACHE_LOGGER,
        &format!("SQL test artifact cache missed: {reasons}"),
    )
}

/// Plan every SQL test whose cached artifact under `target_dir` cannot be reused.
pub(crate) fn plan_tests<'py>(
    target_dir: &Bound<'py, PyAny>,
    (adapter, project): (&Bound<'py, PyAny>, &Bound<'py, PyAny>),
    recorder: ArtifactRecorder<'_>,
) -> PyResult<PlannedStaticSqlTests> {
    let py: Python<'py> = project.py();
    let tests_root: Bound<'py, PyAny> = joined(target_dir, &[COMPILED_DIR, TESTS_DIR])?;
    let tests_root_path: PathBuf = tests_root.extract()?;
    let cache_dir: Bound<'py, PyAny> = native_field(project, "compile_cache_dir")?;
    let cached: HashMap<String, SqlTestArtifactRecord> = read_cache(&cache_dir)?;
    let tests: Bound<'py, PyAny> = native_field(project, "sql_tests")?;
    let caching: Option<(IdentityContext, Bound<'py, PyAny>)> = if cache_dir.is_none() {
        None
    } else {
        let context: IdentityContext = identity_context(project, adapter)?;
        count_stage(py, SQL_TEST_MODEL_CHAINS, 1)?;
        let chains: Bound<'py, PyAny> =
            sql_test_model_chain_names_by_key(project, &tests)?.into_any();
        crate::bindings::_helpers::compile_delta::report_hooks::record_test_chains(
            &tests, &chains,
        )?;
        Some((context, chains))
    };
    let tests: Vec<Bound<'py, PyAny>> = tests.try_iter()?.collect::<PyResult<_>>()?;
    let identities: Vec<Option<(String, String)>> = match &caching {
        None => vec![None; tests.len()],
        Some((context, chains)) => test_identities(&tests, context, chains)?
            .into_iter()
            .map(Some)
            .collect(),
    };
    let mut pending: Vec<PendingTest> = Vec::new();
    let mut cached_paths: Vec<PathBuf> = Vec::new();
    let mut cached_records: Vec<(String, SqlTestArtifactRecord)> = Vec::new();
    let mut misses: BTreeMap<&'static str, usize> = BTreeMap::new();
    for (index, (test, identity)) in tests.into_iter().zip(identities).enumerate() {
        let Some((record_key, identity)) = identity else {
            pending.push((test.unbind(), None, None));
            continue;
        };
        let reusable: Result<SqlTestArtifactRecord, &'static str> = cached
            .get(&record_key)
            .ok_or(NOT_RECORDED)
            .and_then(|record| {
                reusable_sql_test_record(&tests_root_path, record, &identity)
                    .map_err(SqlTestRecordMiss::label)
            });
        if let Err(reason) = reusable {
            *misses.entry(reason).or_default() += 1;
        }
        if let Ok(record) = reusable {
            let path = tests_root_path.join(&record.relative_path);
            artifact_kept(py, recorder, &path, (record.size, record.mtime_ns))?;
            cached_paths.push(path);
            cached_records.push((record_key, record));
        } else {
            pending.push((test.unbind(), Some(record_key), Some(identity)));
        }
        yield_interpreter(py, index);
    }
    if !cached.is_empty() && !misses.is_empty() {
        log_cache_misses(py, &misses)?;
    }
    let pending_tests: Vec<Bound<'py, PyAny>> = pending
        .iter()
        .map(|(test, _, _)| test.bind(py).clone())
        .collect();
    let artifacts = planned_artifacts(adapter, project, pending_tests)?;
    Ok(PlannedStaticSqlTests {
        tests_root: tests_root_path,
        pending,
        artifacts,
        cached_paths,
        cached_records,
    })
}

/// The planned tests handed over by `planned_tests()`, when they target `target_dir`.
fn resolved_plan<'py>(
    target_dir: &Bound<'py, PyAny>,
    (adapter, project): (&Bound<'py, PyAny>, &Bound<'py, PyAny>),
    prepared: Prepared<'_, 'py>,
) -> PyResult<Bound<'py, PlannedStaticSqlTests>> {
    let py: Python<'py> = project.py();
    if let Some(planned_tests) = prepared.planned_tests {
        let planned: Bound<'py, PyAny> = planned_tests.call0()?;
        if !planned.is_none() {
            let planned: Bound<'py, PlannedStaticSqlTests> = planned.downcast_into()?;
            if planned
                .get()
                .tests_root
                .clone()
                .into_bound_py_any(py)?
                .eq(joined(target_dir, &[COMPILED_DIR, TESTS_DIR])?)?
            {
                return Ok(planned);
            }
        }
    }
    Bound::new(
        py,
        plan_tests(target_dir, (adapter, project), prepared.recorder)?,
    )
}

/// The planning errors of one test's artifact, as SQL test diagnostics.
fn artifact_diagnostics<'py>(
    test: &Bound<'py, PyAny>,
    artifact: &PlanRow,
) -> PyResult<Vec<Bound<'py, PyAny>>> {
    let py: Python<'py> = test.py();
    let mut diagnostics: Vec<Bound<'py, PyAny>> = Vec::new();
    for message in &artifact.error_messages {
        diagnostics.push(new_record_with(
            py,
            RecordId::CompilerDiagnostic,
            &[
                ("phase", "test".into_bound_py_any(py)?),
                ("severity", "error".into_bound_py_any(py)?),
                (
                    "code",
                    native_field(&error_class(py, ErrorId::PlannerInput)?, "code")?,
                ),
                ("message", message.into_bound_py_any(py)?),
                ("resource_type", "sql_test".into_bound_py_any(py)?),
                ("resource_name", native_field(test, "name")?),
                ("path", native_field(test, "source_path")?),
            ],
        )?);
    }
    Ok(diagnostics)
}

/// Write planned SQL test artifacts and the cache; returns managed paths and planning errors.
fn write_static_tests<'py>(
    target_dir: &Bound<'py, PyAny>,
    (adapter, project): (&Bound<'py, PyAny>, &Bound<'py, PyAny>),
    prepared: Prepared<'_, 'py>,
    batch: &mut ArtifactBatch,
) -> PyResult<WrittenTests<'py>> {
    let py: Python<'py> = project.py();
    let planned: Bound<'py, PlannedStaticSqlTests> =
        resolved_plan(target_dir, (adapter, project), prepared)?;
    let planned: &PlannedStaticSqlTests = planned.get();
    let tests_root = &planned.tests_root;
    let mut managed = planned.cached_paths.clone();
    let mut records: Vec<(String, SqlTestArtifactRecord)> = planned.cached_records.clone();
    let mut test_paths: Vec<PathBuf> = Vec::new();
    for ((test, _, _), artifact) in zipped(planned)? {
        let path = tests_root.join(compiled_sql_test_path(
            test.bind(py),
            artifact.plan.model_names.clone(),
        )?);
        batch.queue_sql_at(py, path.clone(), artifact.rendered_sql(py)?)?;
        managed.push(path.clone());
        test_paths.push(path);
    }
    batch.flush(py)?;
    let mut diagnostics: Vec<Bound<'py, PyAny>> = Vec::new();
    for (((test, record_key, identity), artifact), path) in zipped(planned)?.zip(&test_paths) {
        let found: Vec<Bound<'py, PyAny>> = artifact_diagnostics(test.bind(py), artifact)?;
        if !found.is_empty() {
            diagnostics.extend(found);
            continue;
        }
        if let (Some(record_key), Some(identity)) = (record_key, identity)
            && let Some(record) = new_sql_test_record(tests_root, path, identity)
        {
            records.retain(|(key, _)| key != record_key);
            records.push((record_key.clone(), record));
        }
    }
    let cache_dir: Bound<'py, PyAny> = native_field(project, "compile_cache_dir")?;
    if crate::bindings::_helpers::compile_context::delta::injection().is_some() {
        let mut kept: HashMap<String, SqlTestArtifactRecord> = read_cache(&cache_dir)?;
        for (key, record) in records.drain(..) {
            kept.insert(key, record);
        }
        records = kept.into_iter().collect();
        records.sort_by(|left, right| left.0.cmp(&right.0));
    }
    record_compile_timing("cache_publication_ms", || write_cache(&cache_dir, &records))?;
    Ok((managed, diagnostics))
}

/// Write offline compiled output under `target_dir`, reusing SQL tests planned ahead.
pub(crate) fn write_static_target<'py>(
    target_dir: &Bound<'py, PyAny>,
    (adapter, project): (&Bound<'py, PyAny>, &Bound<'py, PyAny>),
    manifest: &Bound<'py, PyAny>,
    prepared: Prepared<'_, 'py>,
) -> PyResult<Bound<'py, PyAny>> {
    let py: Python<'py> = target_dir.py();
    let remove_stale_files: bool = prepared_target(target_dir)?
        && crate::bindings::_helpers::compile_context::delta::injection().is_none();
    let mut batch: ArtifactBatch = ArtifactBatch::new(remove_stale_files, prepared.recorder);
    let mut managed: Vec<Bound<'py, PyAny>> = Vec::new();
    let mut managed_models: Vec<PathBuf> = Vec::new();
    let compiled: Bound<'py, PyAny> = joined(target_dir, &[COMPILED_DIR])?;
    let compiled_dir: PathBuf = compiled.extract()?;
    for model in native_field(project, "models")?.try_iter()? {
        let model: Bound<'py, PyAny> = model?;
        if let Ok(native) = model.cast::<NativeCompiledModel>() {
            let (relative_path, query_sql) = native.get().output_texts(py)?;
            let path: PathBuf = native_model_output_path(&compiled_dir, &relative_path);
            batch.queue_sql_at(py, path.clone(), &query_sql)?;
            managed_models.push(path);
            continue;
        }
        let path: Bound<'py, PyAny> =
            compiled.div(model_output_path(&native_field(&model, "relative_path")?)?)?;
        batch.queue_sql(
            &path,
            &native_field(&model, "query_sql")?.extract::<String>()?,
        )?;
        managed.push(path);
    }
    batch.flush_in_background(py)?;
    let queued: PyResult<WrittenTests<'py>> = (|| {
        for function in native_field(project, "functions")?.try_iter()? {
            let function: Bound<'py, PyAny> = function?;
            if native_field(&function, "destination")?
                .getattr("qualified_name")?
                .is_none()
            {
                continue;
            }
            let sql: String = function_sql(adapter, &function)?;
            let path: Bound<'py, PyAny> = compiled.div(function_output_path(
                &native_field(&function, "relative_path")?,
                &native_field(&function, "language")?,
            )?)?;
            batch.queue_sql(&path, &sql)?;
            managed.push(path);
        }
        batch.flush_in_background(py)?;
        for audit in native_field(project, "audits")?.try_iter()? {
            let audit: Bound<'py, PyAny> = audit?;
            let path: Bound<'py, PyAny> =
                compiled.div(AUDITS_DIR)?.div(audit_output_path(&audit)?)?;
            batch.queue_sql(
                &path,
                &native_field(&audit, "sql_body")?.extract::<String>()?,
            )?;
            managed.push(path);
        }
        batch.flush_in_background(py)?;
        write_static_tests(target_dir, (adapter, project), prepared, &mut batch)
    })();
    let (test_paths, diagnostics) = batch.settle(py, queued)?;
    managed_models.extend(test_paths);
    if remove_stale_files {
        let mut paths: Vec<PathBuf> = managed_models;
        paths.extend(extracted(&managed)?);
        let cache_dir: Bound<'py, PyAny> = native_field(project, "compile_cache_dir")?;
        let cache_dir: Option<PathBuf> = if cache_dir.is_none() {
            None
        } else {
            Some(cache_dir.extract()?)
        };
        remove_stale(target_dir, paths, cache_dir)?;
    }
    write_manifest(target_dir, manifest, prepared.recorder)?;
    written_target(
        target_dir,
        [
            length(project, "models")?,
            length(project, "seeds")?,
            length(project, "functions")?,
            length(project, "audits")?,
            length(project, "sql_tests")?,
        ],
        PyTuple::new(py, diagnostics)?.into_any(),
    )
}

/// `path` relative to `project_dir` with `/` separators, as model files name themselves.
fn project_relative(path: &Bound<'_, PyAny>, project_dir: &Bound<'_, PyAny>) -> PyResult<String> {
    let text: String = path_posix_text(path)?;
    if !Path::new(&text).is_absolute() {
        return Ok(text);
    }
    let root: PathBuf = std::fs::canonicalize(path_posix_text(project_dir)?)?;
    Ok(Path::new(&text)
        .strip_prefix(&root)
        .map_or(text.clone(), |relative| {
            relative
                .to_string_lossy()
                .replace(std::path::MAIN_SEPARATOR, "/")
        }))
}

/// Each source file's Rule errors as compiled-file header lines: code, location and message.
fn rule_error_lines(
    diagnostics: &Bound<'_, PyAny>,
    project_dir: &Bound<'_, PyAny>,
) -> PyResult<HashMap<String, Vec<String>>> {
    let py: Python<'_> = diagnostics.py();
    let rule_phase: Bound<'_, PyAny> = enum_member(py, RecordId::DiagnosticPhase, "RULE")?;
    let mut lines: HashMap<String, Vec<String>> = HashMap::new();
    for diagnostic in diagnostics.try_iter()? {
        let diagnostic: Bound<'_, PyAny> = diagnostic?;
        let path: Bound<'_, PyAny> = native_field(&diagnostic, "path")?;
        if path.is_none()
            || !native_field(&diagnostic, "is_error")?.is_truthy()?
            || !native_field(&diagnostic, "phase")?.is(&rule_phase)
        {
            continue;
        }
        let relative: String = project_relative(&path, project_dir)?;
        let position = |field: &str| -> PyResult<String> {
            let value: Bound<'_, PyAny> = native_field(&diagnostic, field)?;
            Ok(if value.is_none() {
                String::new()
            } else {
                format!(":{}", value.str()?)
            })
        };
        let message: String = native_field(&diagnostic, "message")?.str()?.to_string();
        let line: String = format!(
            "-- {} at {relative}{}{}: {}",
            native_field(&diagnostic, "code")?.str()?,
            position("line")?,
            position("column")?,
            message.split_whitespace().collect::<Vec<&str>>().join(" "),
        );
        lines.entry(relative).or_default().push(line);
    }
    Ok(lines)
}

/// After Rules failed, write every model's compiled SQL to `target/compiled-failed/` and remove every other file there, so it holds exactly the expansion these Rules checked. A model with Rule errors starts with a header marking it not buildable and listing them. `target/compiled/` keeps the last passing compile; only an earlier manifest is removed, since it no longer describes `target/`. Returns the failing models' files, relative to `target/compiled-failed/` and sorted.
pub(crate) fn write_rule_failed_models<'py>(
    target_dir: &Bound<'py, PyAny>,
    (project, project_dir): (&Bound<'py, PyAny>, &Bound<'py, PyAny>),
    diagnostics: &Bound<'py, PyAny>,
    recorder: ArtifactRecorder<'_>,
) -> PyResult<Vec<String>> {
    let py: Python<'py> = target_dir.py();
    let errors: HashMap<String, Vec<String>> = rule_error_lines(diagnostics, project_dir)?;
    let _ = prepared_target(target_dir)?;
    let target: PathBuf = target_dir.extract()?;
    match std::fs::remove_file(target.join(MANIFEST_FILE)) {
        Err(error) if error.kind() != std::io::ErrorKind::NotFound => return Err(error.into()),
        _ => {}
    }
    let failed_dir: PathBuf = target.join(FAILED_COMPILED_DIRECTORY_NAME);
    let mut models: Vec<(String, String)> = Vec::new();
    for model in native_field(project, "models")?.try_iter()? {
        let model: Bound<'py, PyAny> = model?;
        let (relative_path, query_sql): (String, String) =
            if let Ok(native) = model.cast::<NativeCompiledModel>() {
                let (relative_path, query_sql) = native.get().output_texts(py)?;
                (relative_path, query_sql.to_string())
            } else {
                (
                    path_posix_text(&native_field(&model, "relative_path")?)?,
                    native_field(&model, "query_sql")?.extract()?,
                )
            };
        models.push((relative_path, query_sql));
    }
    if let Some(injection) = crate::bindings::_helpers::compile_context::delta::injection() {
        models.extend(models_not_rerun(
            &target,
            &models,
            &injection.project_model_paths,
        )?);
    }
    let mut batch: ArtifactBatch = ArtifactBatch::new(true, recorder);
    let mut failing: Vec<String> = Vec::new();
    let mut written: Vec<PathBuf> = Vec::new();
    for (relative_path, query_sql) in models {
        let path: PathBuf = native_model_output_path(&failed_dir, &relative_path);
        let sql: String = match errors.get(&relative_path) {
            Some(lines) => {
                failing.push(failed_relative(&failed_dir, &path));
                format!("{RULE_FAILED_HEADER}\n{}\n{query_sql}", lines.join("\n"))
            }
            None => query_sql,
        };
        written.push(path.clone());
        batch.queue_sql_at(py, path, &sql)?;
    }
    batch.flush(py)?;
    let _ = remove_stale_artifacts(py, failed_dir, written, None)?;
    failing.sort_unstable();
    Ok(failing)
}

/// In a delta compile, which runs only after a passing compile, the compiled SQL of each project model it did not rerun, unchanged in `target/compiled/` since that compile.
fn models_not_rerun(
    target: &Path,
    rerun: &[(String, String)],
    project_model_paths: &[String],
) -> PyResult<Vec<(String, String)>> {
    let rerun: HashSet<&str> = rerun.iter().map(|(path, _)| path.as_str()).collect();
    let compiled_dir: PathBuf = target.join(COMPILED_DIR);
    let mut models: Vec<(String, String)> = Vec::new();
    for relative_path in project_model_paths {
        if rerun.contains(relative_path.as_str()) {
            continue;
        }
        let path: PathBuf = native_model_output_path(&compiled_dir, relative_path);
        let Ok(text) = std::fs::read_to_string(&path) else {
            return Err(
                crate::bindings::_helpers::compile_context::delta::delta_unsupported(
                    "the stored compile's SQL for a model it did not rerun is unreadable",
                ),
            );
        };
        let query_sql: String = if cfg!(windows) {
            text.replace("\r\n", "\n")
        } else {
            text
        };
        models.push((relative_path.clone(), query_sql));
    }
    Ok(models)
}

/// `path` relative to `failed_dir`, with `/` separators.
fn failed_relative(failed_dir: &Path, path: &Path) -> String {
    match path.strip_prefix(failed_dir) {
        Ok(relative) => relative
            .to_string_lossy()
            .replace(std::path::MAIN_SEPARATOR, "/"),
        Err(_outside) => path.to_string_lossy().into_owned(),
    }
}

/// Delete `target/compiled-failed/`, which only the latest compile whose Rules failed owns.
pub(crate) fn remove_failed_compile(target: &Path) -> PyResult<()> {
    match std::fs::remove_dir_all(target.join(FAILED_COMPILED_DIRECTORY_NAME)) {
        Err(error) if error.kind() != std::io::ErrorKind::NotFound => Err(error.into()),
        _ => Ok(()),
    }
}

/// Plan uncached SQL tests and report their errors without writing artifacts or cache.
pub(crate) fn planning_diagnostics<'py>(
    target_dir: &Bound<'py, PyAny>,
    (adapter, project): (&Bound<'py, PyAny>, &Bound<'py, PyAny>),
    prepared: Prepared<'_, 'py>,
) -> PyResult<Bound<'py, PyTuple>> {
    let py: Python<'py> = project.py();
    let planned: Bound<'py, PlannedStaticSqlTests> =
        resolved_plan(target_dir, (adapter, project), prepared)?;
    let mut diagnostics: Vec<Bound<'py, PyAny>> = Vec::new();
    for ((test, _, _), artifact) in zipped(planned.get())? {
        diagnostics.extend(artifact_diagnostics(test.bind(py), artifact)?);
    }
    PyTuple::new(py, diagnostics)
}

/// Publish staged files with the same unchanged-file and stale-file semantics as a write.
pub(crate) fn publish_staged_target<'py>(
    (prepared, expected_files): (&Bound<'py, PyAny>, &Bound<'py, PyAny>),
    target_dir: &Bound<'py, PyAny>,
    manifest: &Bound<'py, PyAny>,
    recorder: ArtifactRecorder<'_>,
) -> PyResult<Bound<'py, PyAny>> {
    let py: Python<'py> = target_dir.py();
    let staged_dir: Bound<'py, PyAny> =
        joined(&native_field(prepared, "target_dir")?, &[COMPILED_DIR])?;
    let compiled_dir: Bound<'py, PyAny> = joined(target_dir, &[COMPILED_DIR])?;
    let existed: bool = compiled_dir.call_method0("is_dir")?.is_truthy()?;
    let expected: Vec<PathBuf> = py
        .import("builtins")?
        .getattr("sorted")?
        .call1((expected_files,))?
        .extract()?;
    let (staged, compiled): (PathBuf, PathBuf) = (staged_dir.extract()?, compiled_dir.extract()?);
    let (kind, published) = record_compile_timing("physical_write_ms", || {
        publish_staged_artifacts(py, staged, compiled.clone(), expected.clone())
    })?;
    if kind == PUBLISHED_STAGING_CHANGED {
        return Err(staging_changed(
            py,
            "staged compile artifacts changed before publication; no artifact was published",
        )?);
    }
    if kind == PUBLISHED_TREE {
        artifact_tree_moved(recorder, &staged_dir, &compiled_dir)?;
    } else {
        let mut managed: Vec<Bound<'py, PyAny>> = Vec::new();
        for (source, path) in &published {
            artifact_moved(
                recorder,
                &source.into_bound_py_any(py)?,
                &path.into_bound_py_any(py)?,
            )?;
            managed.push(path_class(py)?.call1((path,))?);
        }
        if existed {
            remove_stale(target_dir, extracted(&managed)?, None)?;
        }
    }
    write_manifest(target_dir, manifest, recorder)?;
    written_target(
        target_dir,
        [
            native_field(prepared, "model_count")?.extract()?,
            native_field(prepared, "seed_count")?.extract()?,
            native_field(prepared, "function_count")?.extract()?,
            native_field(prepared, "audit_count")?.extract()?,
            native_field(prepared, "test_count")?.extract()?,
        ],
        native_field(prepared, "diagnostics")?,
    )
}

/// `StagedArtifactsChangedError(message, help=...)`.
pub(crate) fn staging_changed(py: Python<'_>, message: &str) -> PyResult<PyErr> {
    let error: Bound<'_, PyAny> = error_class(py, ErrorId::StagedArtifactsChanged)?.call(
        (message,),
        Some(&keywords(
            py,
            &[("help", STAGING_CHANGED_HELP.into_bound_py_any(py)?)],
        )?),
    )?;
    Ok(PyErr::from_value(error))
}

/// Write offline compiled output files under `target_dir`.
#[pyfunction]
#[pyo3(signature = (**options))]
fn write_static_compile_target<'py>(
    py: Python<'py>,
    options: Option<Bound<'py, PyDict>>,
) -> PyResult<Bound<'py, PyAny>> {
    let arguments: KeywordArguments<'py> =
        KeywordArguments::read(py, "write_static_compile_target", options, TARGET_OPTIONS)?;
    write_static_target(
        &arguments.value("target_dir")?,
        (&arguments.value("adapter")?, &arguments.value("project")?),
        &arguments.value("manifest")?,
        Prepared::default(),
    )
}

/// Write the planner's compiled output files under `target_dir`.
#[pyfunction]
#[pyo3(signature = (**options))]
pub(crate) fn write_compile_target<'py>(
    py: Python<'py>,
    options: Option<Bound<'py, PyDict>>,
) -> PyResult<Bound<'py, PyAny>> {
    let arguments: KeywordArguments<'py> = KeywordArguments::read(
        py,
        "write_compile_target",
        options,
        (&["target_dir", "adapter", "plan_output"], &["manifest"]),
    )?;
    write_planned_target(
        &arguments.value("target_dir")?,
        &arguments.value("adapter")?,
        &arguments.value("plan_output")?,
        &arguments.value("manifest")?,
    )
}

pub(crate) fn register(module: &Bound<'_, PyModule>) -> PyResult<()> {
    module.add_function(wrap_pyfunction!(write_static_compile_target, module)?)?;
    module.add_function(wrap_pyfunction!(write_compile_target, module)?)
}
