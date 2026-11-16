//! Typed handles to what `sqlbuild._native` registers, checked against the registration tables at init.

use std::collections::BTreeSet;

use pyo3::exceptions::{PyAttributeError, PyImportError};
use pyo3::prelude::{Bound, Py, PyAny, PyAnyMethods, PyModule, PyResult, Python};
use pyo3::sync::PyOnceLock;
use pyo3::types::PyString;

use crate::records::_helpers::native_classes::fields::all_specs;
use crate::records::constants::{ENUMS, ERRORS, NATIVE_MODULE};
use crate::records::types::{ErrorId, NativeAttr, RecordId};

/// The extension module, imported once for attribute lookups.
static MODULE: PyOnceLock<Py<PyModule>> = PyOnceLock::new();
/// Each `NativeAttr`'s interned attribute name, by variant position.
static ATTRIBUTE_NAMES: PyOnceLock<Vec<Py<PyString>>> = PyOnceLock::new();

fn module(py: Python<'_>) -> PyResult<&Py<PyModule>> {
    MODULE.get_or_try_init(py, || Ok(py.import(NATIVE_MODULE)?.unbind()))
}

/// The module attribute at `index` of `names`, interning the names on first use.
fn attribute<'py, 'a>(
    py: Python<'py>,
    names: &PyOnceLock<Vec<Py<PyString>>>,
    all: impl Iterator<Item = &'a str>,
    index: usize,
) -> PyResult<Bound<'py, PyAny>> {
    let names: &Vec<Py<PyString>> = names.get_or_init(py, || {
        all.map(|name| PyString::intern(py, name).unbind())
            .collect()
    });
    module(py)?.bind(py).getattr(names[index].bind(py))
}

/// Names that are in exactly one of two name sets, sorted.
fn mismatched(left: BTreeSet<&str>, right: BTreeSet<&str>) -> Vec<String> {
    left.symmetric_difference(&right)
        .map(|name| (*name).to_owned())
        .collect()
}

/// Fail loudly when the typed names and the registration tables disagree.
fn check_tables() -> PyResult<()> {
    let records: BTreeSet<&str> = all_specs()
        .map(|spec| spec.name)
        .chain(
            ENUMS
                .iter()
                .flat_map(|group| group.iter())
                .map(|spec| spec.name),
        )
        .collect();
    let errors: BTreeSet<&str> = ERRORS
        .iter()
        .flat_map(|group| group.iter())
        .map(|spec| spec.name)
        .collect();
    let mut problems: Vec<String> =
        mismatched(records, RecordId::ALL.iter().map(|id| id.name()).collect());
    problems.extend(mismatched(
        errors,
        ErrorId::ALL.iter().map(|id| id.name()).collect(),
    ));
    if problems.is_empty() {
        return Ok(());
    }
    Err(PyImportError::new_err(format!(
        "native registry and registration tables disagree: {}",
        problems.join(", ")
    )))
}

/// Fail module init when the typed names and the registration tables disagree.
pub(crate) fn verify_tables() -> PyResult<()> {
    check_tables()
}

/// Fail module init when a `NativeAttr` names an attribute the finished module lacks.
pub(crate) fn verify_attributes(module: &Bound<'_, PyModule>) -> PyResult<()> {
    let mut missing: Vec<&str> = Vec::new();
    for id in NativeAttr::ALL {
        if !module.hasattr(id.name())? {
            missing.push(id.name());
        }
    }
    if missing.is_empty() {
        return Ok(());
    }
    Err(PyImportError::new_err(format!(
        "the native module does not register: {}",
        missing.join(", ")
    )))
}

/// The module attribute `id`, read now so a replaced attribute is seen.
pub(crate) fn native(py: Python<'_>, id: NativeAttr) -> PyResult<Bound<'_, PyAny>> {
    attribute(
        py,
        &ATTRIBUTE_NAMES,
        NativeAttr::ALL.iter().map(|id| id.name()),
        id as usize,
    )
}

/// The native record class or string enum a pure-Rust crate names, failing as `getattr` would.
pub(crate) fn record_named(name: &str) -> PyResult<RecordId> {
    RecordId::from_name(name).ok_or_else(|| {
        PyAttributeError::new_err(format!(
            "module 'sqlbuild._native' has no attribute '{name}'"
        ))
    })
}
