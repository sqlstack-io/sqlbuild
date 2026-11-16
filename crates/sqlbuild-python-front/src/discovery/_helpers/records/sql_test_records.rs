//! SQL unit-test and scenario files discovered natively and built as Python discovery records.

use pyo3::prelude::{Bound, PyAny, PyAnyMethods, PyResult, Python};
use pyo3::types::{
    PyDict, PyDictMethods, PyList, PyListMethods, PyString, PyStringMethods, PyTuple,
};
use pyo3::{IntoPyObjectExt, pyfunction};
use sqlbuild_discovery::models::{DiscoveredFile, DiscoveryFailure, FileOutcome, StageFailure};
use sqlbuild_discovery::sql_tests::main::parse_scenario_text::parse_scenario_text;
use sqlbuild_discovery::sql_tests::models::{
    DiscoveredScenarioFile, DiscoveredSqlTestFile, SqlTestBlock, SqlTestFileOptions,
};
use sqlbuild_discovery::tree::models::ProjectTree;
use sqlbuild_model_config::execution_limits::main::parse_execution_limit::parse_execution_limit;

use crate::discovery::_helpers::failures::faults::{FileFaults, record_tuple};
use crate::discovery::_helpers::failures::raised::{
    discovery_error, failure_error, read_error, stage_error,
};
use crate::discovery::_helpers::snapshots::native_classes::NativeProjectTree;
use crate::discovery::_helpers::snapshots::snapshot::{in_snapshot, project_root, project_tree};
use crate::discovery::_helpers::sources::sql_test_blocks::{
    sql_test_block_records, validate_block_names,
};
use crate::sqltext::main::map_to_python::map_to_python;
use sqlbuild_discovery::constants;
use sqlbuild_discovery::sql_tests::main::sql_test_file_options::sql_test_file_options;
use sqlbuild_python_base::boundary::main::imported::imported;
use sqlbuild_python_base::records::main::construct::new_record_with::new_record_with;
use sqlbuild_python_base::records::main::fields::field::field as native_field;
use sqlbuild_python_base::records::types::RecordId;

const SQL_TESTS_OWNERSHIP_ROOT: &str = "tests/unit";
const SQL_SCENARIOS_OWNERSHIP_ROOT: &str = "tests/scenarios";
const PATHLIB_MODULE: &str = "pathlib";
const SCENARIO_KIND: &str = "sql_scenario";
const DESCRIPTION_KEY: &str = "description";
const TAGS_KEY: &str = "tags";
const TIMEOUT_KEY: &str = "timeout";

/// A native discoverer of one kind of SQL test file.
type Discoverer<T> = fn(
    &sqlbuild_discovery::models::ProjectRoot,
    &ProjectTree,
    &SqlTestFileOptions,
) -> Result<Vec<DiscoveredFile<T>>, StageFailure>;

/// Build one parsed file's record from its `(file_path, relative_path)`.
type RecordBuilder<'py, T> =
    fn((&Bound<'py, PyAny>, &Bound<'py, PyAny>), T) -> PyResult<Bound<'py, PyAny>>;

pub(crate) fn options() -> PyResult<SqlTestFileOptions> {
    Ok(sql_test_file_options(
        sqlbuild_python_base::compile_context::main::delta::injection::injection()
            .map(|injection| std::sync::Arc::new(injection.test_paths.clone())),
    ))
}

pub(crate) fn path_class(py: Python<'_>) -> PyResult<Bound<'_, PyAny>> {
    imported(py, PATHLIB_MODULE, "Path")
}

/// `Path(constants.<name>)`.
fn ownership_root<'py>(py: Python<'py>, root: &str) -> PyResult<Bound<'py, PyAny>> {
    path_class(py)?.call1((root,))
}

/// Discover the files natively in the pass's snapshot: each relative path text and outcome.
pub(crate) fn discovered_files<'py, T: Send>(
    project_dir: &Bound<'py, PyAny>,
    discover: Discoverer<T>,
) -> PyResult<(Bound<'py, NativeProjectTree>, Vec<DiscoveredFile<T>>)> {
    let py: Python<'py> = project_dir.py();
    in_snapshot(project_dir, || {
        let tree: Bound<'py, NativeProjectTree> = project_tree(project_dir)?;
        let root = project_root(project_dir)?;
        let options: SqlTestFileOptions = options()?;
        let discovered: Result<Vec<DiscoveredFile<T>>, StageFailure> = {
            let inner = &tree.get().inner;
            py.detach(|| discover(&root, inner, &options))
        };
        let files =
            discovered.map_err(|failure| stage_error(&tree.get().inner, project_dir, failure))?;
        Ok((tree, files))
    })
}

/// Each block as a `DiscoveredSqlTestBlock`, then the split failure, then the name check.
pub(crate) fn test_blocks<'py>(
    file_path: &Bound<'py, PyAny>,
    blocks: Vec<SqlTestBlock>,
    failure: Option<DiscoveryFailure>,
) -> PyResult<Bound<'py, PyTuple>> {
    let py: Python<'py> = file_path.py();
    let path: String = file_path.str()?.to_str()?.to_owned();
    let mut native = Vec::with_capacity(blocks.len());
    for block in blocks {
        native.push((
            map_to_python(py, &block.header_values)?
                .into_bound(py)
                .downcast_into::<PyDict>()?,
            block.sql_body,
            block.body_span.into_bound_py_any(py)?,
        ));
    }
    let records: Vec<Bound<'py, PyAny>> = sql_test_block_records(py, &path, native)?;
    if let Some(failure) = failure {
        return Err(failure_error(py, failure));
    }
    let names: Vec<(usize, Option<String>)> = records
        .iter()
        .map(|block| {
            Ok((
                native_field(block, "test_index")?.extract()?,
                native_field(block, "name")?.extract()?,
            ))
        })
        .collect::<PyResult<_>>()?;
    validate_block_names(py, &path, &names)?;
    PyTuple::new(py, records)
}

pub(crate) fn test_file<'py>(
    paths: (&Bound<'py, PyAny>, &Bound<'py, PyAny>),
    file: DiscoveredSqlTestFile,
) -> PyResult<Bound<'py, PyAny>> {
    let (file_path, relative_path) = paths;
    let py: Python<'py> = file_path.py();
    new_record_with(
        py,
        RecordId::DiscoveredSqlTestFile,
        &[
            ("file_path", file_path.clone()),
            ("relative_path", relative_path.clone()),
            ("contents", file.contents.into_bound_py_any(py)?),
            (
                "blocks",
                test_blocks(file_path, file.blocks, file.failure)?.into_any(),
            ),
            (
                "ownership_root",
                ownership_root(py, SQL_TESTS_OWNERSHIP_ROOT)?,
            ),
        ],
    )
}

/// Python's `build_sql_scenario_file`: validate the header and build the record.
pub(crate) fn scenario_file<'py>(
    paths: (&Bound<'py, PyAny>, &Bound<'py, PyAny>),
    file: DiscoveredScenarioFile,
) -> PyResult<Bound<'py, PyAny>> {
    let (file_path, relative_path) = paths;
    let py: Python<'py> = file_path.py();
    let header_values: Bound<'py, PyDict> = map_to_python(py, &file.header_values)?
        .into_bound(py)
        .downcast_into::<PyDict>()?;
    let shown: Bound<'py, PyString> = file_path.str()?;
    let failure = |message: String| Err(discovery_error(py, SCENARIO_KIND, message, None));
    if let Some(description) = header_values.get_item(DESCRIPTION_KEY)?
        && !description.is_instance_of::<PyString>()
    {
        return failure(format!(
            "SCENARIO() description in '{shown}' must be a string"
        ));
    }
    if let Some(tags) = header_values.get_item(TAGS_KEY)? {
        let tags_are_strings: bool = match tags.downcast::<PyList>() {
            Ok(tags) => tags.iter().all(|tag| tag.is_instance_of::<PyString>()),
            Err(_) => false,
        };
        if !tags_are_strings {
            return failure(format!(
                "SCENARIO() tags in '{shown}' must be a list of strings"
            ));
        }
    }
    if let Some(timeout) = header_values.get_item(TIMEOUT_KEY)? {
        let valid: bool = match timeout.extract::<String>() {
            Ok(text) => parse_execution_limit(&text).is_some(),
            Err(_) => false,
        };
        if !valid {
            return failure(format!(
                "SCENARIO() timeout in '{shown}' must be a positive fixed duration such as \
                 \"10m\", or \"disabled\""
            ));
        }
    }
    if let Some(columns) = header_values.get_item(constants::SCENARIO_EXPECTED_COLUMNS_KEY)? {
        let valid: bool = columns.extract::<String>().is_ok_and(|value| {
            constants::SCENARIO_EXPECTED_COLUMNS_VALUES.contains(&value.as_str())
        });
        if !valid {
            return failure(format!(
                "SCENARIO() expected_columns in '{shown}' must be \"listed\" or \"all\""
            ));
        }
    }
    if file.sql_body.is_empty() {
        return failure(format!(
            "SQL scenario '{shown}' must define SQL after SCENARIO(...)"
        ));
    }
    new_record_with(
        py,
        RecordId::DiscoveredSqlScenarioFile,
        &[
            ("file_path", file_path.clone()),
            ("relative_path", relative_path.clone()),
            ("contents", file.contents.into_bound_py_any(py)?),
            ("header_values", header_values.into_any()),
            ("sql_body", file.sql_body.into_bound_py_any(py)?),
            ("sql_body_span", file.body_span.into_bound_py_any(py)?),
            ("name", native_field(file_path, "stem")?),
            (
                "ownership_root",
                ownership_root(py, SQL_SCENARIOS_OWNERSHIP_ROOT)?,
            ),
        ],
    )
}

/// Build each file, raising or reporting each failing one.
pub(crate) fn records<'py, T>(
    faults: &FileFaults<'py>,
    files: Vec<(Bound<'py, PyAny>, FileOutcome<T>)>,
    build: RecordBuilder<'py, T>,
) -> PyResult<Bound<'py, PyTuple>> {
    let py: Python<'py> = faults.project_dir.py();
    let mut built: Vec<Bound<'py, PyAny>> = Vec::with_capacity(files.len());
    for (relative_path, outcome) in files {
        let file_path: Bound<'py, PyAny> = faults.project_dir.div(&relative_path)?;
        let record: PyResult<Bound<'py, PyAny>> = match outcome {
            FileOutcome::Parsed(file) => build((&file_path, &relative_path), file),
            FileOutcome::Failed(failure) => Err(failure_error(py, failure)),
            FileOutcome::Unreadable(failure) => Err(read_error(failure, &file_path)),
        };
        built.extend(faults.kept(&relative_path, record)?);
    }
    record_tuple(py, built)
}

/// Parse in-memory scenario contents into the record scenario discovery builds for a file.
#[pyfunction]
#[pyo3(signature = (*, contents, file_path, relative_path))]
pub(crate) fn parse_sql_scenario_file<'py>(
    contents: String,
    file_path: &Bound<'py, PyAny>,
    relative_path: &Bound<'py, PyAny>,
) -> PyResult<Bound<'py, PyAny>> {
    let py: Python<'py> = file_path.py();
    let path: String = file_path.str()?.to_str()?.to_owned();
    let options: SqlTestFileOptions = options()?;
    let parsed = py.detach(|| parse_scenario_text(&path, contents, &options));
    let file: DiscoveredScenarioFile = parsed.map_err(|failure| failure_error(py, failure))?;
    scenario_file((file_path, relative_path), file)
}
