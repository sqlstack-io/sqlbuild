//! Unit-test selector splitting over discovered project names.

use std::collections::BTreeSet;

use pyo3::IntoPyObject;
use pyo3::prelude::{Bound, PyAny, PyAnyMethods, PyResult, Python};
use pyo3::types::{PyFrozenSet, PyString, PyTuple};
use sqlbuild_analysis::graph::main::parse_selector::parse_selector;
use sqlbuild_analysis::graph::models::ParsedSelector;
use sqlbuild_analysis::planner_selection::main::split_unit_test_selectors::split_unit_test_selectors as split;
use sqlbuild_analysis::planner_selection::models::{
    SelectorGroups, UnitTestSelectorNames, UnitTestSelectorSplit,
};

use crate::bindings::_helpers::compiled_project::job_catalog::job_names;
use crate::bindings::_helpers::discovery::model_file_record::NativeModelFile;
use crate::bindings::_helpers::planner_selection::carrier::{Carrier, py_strings};
use crate::bindings::_helpers::records::construct::new_record_with;
use crate::bindings::_helpers::records::fields::{field as native_field, path_stem};
use crate::bindings::_helpers::records::ids::RecordId;

/// A bare selector's kind.
const NAME_SELECTOR_KIND: &str = "name";
/// Characters that make a selector value a glob pattern.
const PATTERN_CHARACTERS: [char; 3] = ['*', '?', '['];

const RESOURCE_ENTRY_FILES: [(&str, &str); 3] = [
    ("schema_files", "model_entries"),
    ("schema_files", "seed_entries"),
    ("source_files", "source_entries"),
];
const RESOURCE_PATH_FILES: [&str; 4] = [
    "model_files",
    "seed_files",
    "sql_function_files",
    "python_function_files",
];
const RESOURCE_NODES: [&str; 4] = [
    "loader_functions",
    "task_functions",
    "asset_functions",
    "check_functions",
];

fn stem(path: &Bound<'_, PyAny>) -> PyResult<String> {
    path_stem(path)
}

fn unit_test_names(discovered: &Bound<'_, PyAny>) -> PyResult<BTreeSet<String>> {
    let mut names: BTreeSet<String> = BTreeSet::new();
    for test_file in native_field(discovered, "test_files")?.try_iter()? {
        let test_file: Bound<'_, PyAny> = test_file?;
        for block in native_field(&test_file, "blocks")?.try_iter()? {
            let name: Bound<'_, PyAny> = native_field(&block?, "name")?;
            names.insert(if name.is_truthy()? {
                name.extract()?
            } else {
                stem(&native_field(&test_file, "relative_path")?)?
            });
        }
    }
    Ok(names)
}

fn resource_names(discovered: &Bound<'_, PyAny>) -> PyResult<BTreeSet<String>> {
    let mut names: BTreeSet<String> = BTreeSet::new();
    for field in RESOURCE_PATH_FILES {
        for file in native_field(discovered, field)?.try_iter()? {
            let file: Bound<'_, PyAny> = file?;
            names.insert(match file.cast::<NativeModelFile>() {
                Ok(native) => native.get().path_texts(file.py())?.name,
                Err(_) => stem(&native_field(&file, "file_path")?)?,
            });
        }
    }
    for (field, entries) in RESOURCE_ENTRY_FILES {
        for file in native_field(discovered, field)?.try_iter()? {
            for entry in file?.getattr(entries)?.try_iter()? {
                names.insert(native_field(&entry?, "name")?.extract()?);
            }
        }
    }
    for field in RESOURCE_NODES {
        for node in native_field(discovered, field)?.try_iter()? {
            names.insert(native_field(&node?, "name")?.extract()?);
        }
    }
    Ok(names)
}

fn restored<'py>(carrier: &Carrier<'py>, texts: &[String]) -> PyResult<Bound<'py, PyTuple>> {
    let texts: Vec<Bound<'py, PyString>> = texts
        .iter()
        .map(|text| carrier.back(text))
        .collect::<PyResult<_>>()?;
    PyTuple::new(carrier.py(), texts)
}

/// Whether a bare, non-pattern name in `selectors` is neither a unit test nor a resource.
fn names_unknown(selectors: &[String], names: &UnitTestSelectorNames) -> bool {
    selectors
        .iter()
        .flat_map(|selector| selector.split(char::is_whitespace))
        .flat_map(|token| token.split(','))
        .filter(|part| !part.is_empty())
        .any(|part| match parse_selector(part) {
            Ok(ParsedSelector::Kind { kind, value, .. }) => {
                kind == NAME_SELECTOR_KIND
                    && !value.contains(PATTERN_CHARACTERS)
                    && !names.tests.contains(&value)
                    && !names.resources.contains(&value)
            }
            _ => false,
        })
}

fn split_python<'py>(
    (select, exclude): (&Bound<'py, PyAny>, &Bound<'py, PyAny>),
    discovered: &Bound<'py, PyAny>,
    accepts_unit_tests: bool,
) -> PyResult<Bound<'py, PyAny>> {
    let py: Python<'py> = select.py();
    let mut names = UnitTestSelectorNames {
        tests: unit_test_names(discovered)?,
        resources: resource_names(discovered)?,
    };
    let select: Vec<Bound<'py, PyString>> = py_strings(select)?;
    let exclude: Vec<Bound<'py, PyString>> = py_strings(exclude)?;
    let texts: Vec<Bound<'py, PyString>> = select.iter().chain(&exclude).cloned().collect();
    let carrier: Carrier<'py> = Carrier::new(py, &texts, || {
        Ok(names.tests.union(&names.resources).cloned().collect())
    })?;
    let (select, exclude): (Vec<String>, Vec<String>) =
        (carrier.send_all(&select)?, carrier.send_all(&exclude)?);
    let sent: Vec<String> = select.iter().chain(&exclude).cloned().collect();
    if accepts_unit_tests && names_unknown(&sent, &names) {
        names.resources.extend(job_names(py)?);
    }
    let split: UnitTestSelectorSplit = split(
        SelectorGroups {
            select: &select,
            exclude: &exclude,
        },
        &names,
        accepts_unit_tests,
    )
    .map_err(|error| carrier.error(error))?;
    let selection: Bound<'py, PyAny> = new_record_with(
        py,
        RecordId::SqlTestSelection,
        &[
            ("names", PyFrozenSet::new(py, &split.test_names)?.into_any()),
            (
                "excluded_names",
                PyFrozenSet::new(py, &split.excluded_test_names)?.into_any(),
            ),
            (
                "models_selected",
                split
                    .models_selected
                    .into_pyobject(py)?
                    .to_owned()
                    .into_any(),
            ),
        ],
    )?;
    new_record_with(
        py,
        RecordId::UnitTestSelectorSplit,
        &[
            ("select", restored(&carrier, &split.select)?.into_any()),
            ("exclude", restored(&carrier, &split.exclude)?.into_any()),
            ("sql_test_selection", selection),
        ],
    )
}

/// Move unit-test selectors out of resource selectors, or reject them for this command.
pub(crate) fn split_project_unit_test_selectors<'py>(
    select: &Bound<'py, PyAny>,
    exclude: &Bound<'py, PyAny>,
    discovered_inputs: &Bound<'py, PyAny>,
    accepts_unit_tests: bool,
) -> PyResult<Bound<'py, PyAny>> {
    split_python((select, exclude), discovered_inputs, accepts_unit_tests)
}

/// Split unit-test selectors from resource selectors for a command that runs unit tests.
pub(crate) fn split_unit_test_selectors<'py>(
    select: &Bound<'py, PyAny>,
    exclude: &Bound<'py, PyAny>,
    discovered_inputs: &Bound<'py, PyAny>,
) -> PyResult<Bound<'py, PyAny>> {
    split_python((select, exclude), discovered_inputs, true)
}

/// Raise when a selector names a unit test; return the unchanged resource selectors.
pub(crate) fn reject_unit_test_selectors<'py>(
    select: &Bound<'py, PyAny>,
    exclude: &Bound<'py, PyAny>,
    discovered_inputs: &Bound<'py, PyAny>,
) -> PyResult<Bound<'py, PyAny>> {
    split_python((select, exclude), discovered_inputs, false)
}
