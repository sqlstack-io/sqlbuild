//! Where project Python tests sit: each mirrors an existing folder, and each test module a module, under `python/nodes/`, `python/providers/` or `orchestration/`.

use std::collections::BTreeSet;

use crate::models::{Fault, PythonSource};
use crate::python_rules::_helpers::engine::faults::catalogue_fault;
use crate::python_rules::constants::{PYTHON_TEST_PLACEMENT_CODE, TEST_FILE_PREFIX};

/// A fault on each Python test whose mirrored folder, or for a test module whose module, is not in the project.
pub(crate) fn python_test_faults(
    sources: &[PythonSource],
    tests: &[PythonSource],
    codes: &BTreeSet<&str>,
) -> Vec<Fault> {
    if !codes.contains(PYTHON_TEST_PLACEMENT_CODE) {
        return Vec::new();
    }
    let paths: BTreeSet<&str> = sources
        .iter()
        .map(|source| source.relative_path.as_str())
        .collect();
    tests
        .iter()
        .filter_map(|test| {
            let detail: String = missing_counterpart(&test.relative_path, &paths)?;
            catalogue_fault(
                PYTHON_TEST_PLACEMENT_CODE,
                &test.relative_path,
                (1, 1),
                &detail,
            )
        })
        .collect()
}

/// The project folders Python tests mirror, each below `tests/`.
const MIRRORED_ROOTS: [&str; 3] = ["python/nodes", "python/providers", "orchestration"];

/// What the test at `path`, such as `tests/python/nodes/orders/test_export.py`, mirrors that the project lacks.
fn missing_counterpart(path: &str, paths: &BTreeSet<&str>) -> Option<String> {
    let below: &str = path.strip_prefix("tests/")?;
    let root: &str = MIRRORED_ROOTS
        .iter()
        .find(|root| below.strip_prefix(**root).is_some_and(|rest| rest.starts_with('/')))?;
    let parts: Vec<&str> = below[root.len() + 1..].split('/').collect();
    let [folders @ .., file] = parts.as_slice() else {
        return None;
    };
    if folders.is_empty() {
        return Some(format!(
            "it sits directly in tests/{root}/, not in a folder mirroring one under {root}/"
        ));
    }
    let folder: String = format!("{root}/{}", folders.join("/"));
    if !holds_files(paths, &folder) {
        return Some(format!("{folder}/ does not exist"));
    }
    let module: &str = file.strip_prefix(TEST_FILE_PREFIX)?;
    let tested: String = format!("{folder}/{module}");
    let package: &str = tested.trim_end_matches(".py");
    (!paths.contains(tested.as_str()) && !holds_files(paths, package))
        .then(|| format!("{tested} does not exist"))
}

/// Whether any project Python file sits below `folder`.
fn holds_files(paths: &BTreeSet<&str>, folder: &str) -> bool {
    let prefix: String = format!("{folder}/");
    paths
        .range(prefix.as_str()..)
        .next()
        .is_some_and(|path| path.starts_with(&prefix))
}
