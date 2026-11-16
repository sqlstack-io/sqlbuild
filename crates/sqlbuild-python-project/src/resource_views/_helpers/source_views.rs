//! Sources attached by the native loop, held by the compiled project, and their views.

use crate::resource_views::main::source_views::replaced::replaced;
use crate::resource_views::models::NativeCompiledSource;
use crate::resource_views::models::NativeSourceInput;
use crate::resource_views::models::SourceKeyFacts;
use crate::resource_views::models::SourceProjections;
use crate::resource_views::types::AttachedSource;
use crate::resource_views::types::SourceRecord;
use crate::resource_views::types::SourceViews;
use pyo3::exceptions::PyIndexError;
use pyo3::prelude::{Bound, Py, PyAny, PyResult, Python};
use pyo3::sync::PyOnceLock;
use pyo3::types::{PyDict, PyDictMethods};
use sqlbuild_python_base::positional_record;

use crate::compiled_project::main::python_column_facts::python_column_facts;
use crate::compiled_project::models::NativeCompiledProject;
use crate::compiled_views::main::compiled_models::audit_key_rows::audit_key_rows;

pub(crate) const SOURCE_RESOURCE: &str = "source";

positional_record! {
    into;
    /// `(name, has a loader, database, schema)` of one source.
    pub(crate) struct SourceAssemblyRow {
        pub(crate) name: String,
        pub(crate) managed: bool,
        pub(crate) database: Option<String>,
        pub(crate) schema: Option<String>,
    }
}

/// The source record of an attached source, before assembly resolves its namespace.
pub(crate) fn source_record(attached: AttachedSource) -> SourceRecord {
    SourceRecord {
        attached,
        namespace: None,
        host: SourceProjections {
            key_facts: PyOnceLock::new(),
            attached_entry: PyOnceLock::new(),
            assembled_entry: PyOnceLock::new(),
            key: PyOnceLock::new(),
        },
    }
}

impl SourceViews for SourceRecord {
    fn key_facts(&self, py: Python<'_>) -> PyResult<&SourceKeyFacts> {
        self.host.key_facts.get_or_try_init(py, || {
            Ok(SourceKeyFacts {
                columns: python_column_facts(self.attached.host.columns.bind(py).as_any(), None)?,
                audits: audit_key_rows(self.attached.host.audits.bind(py).as_any())?,
            })
        })
    }

    fn attached_entry(&self, py: Python<'_>) -> PyResult<Py<PyAny>> {
        self.host
            .attached_entry
            .get_or_try_init(py, || {
                let source: &AttachedSource = &self.attached;
                let changes: Bound<'_, PyDict> = PyDict::new(py);
                changes.set_item("contract", &source.contract)?;
                changes.set_item("database", &source.database)?;
                changes.set_item("schema", &source.schema)?;
                changes.set_item("table", &source.table)?;
                changes.set_item("expression", &source.expression)?;
                changes.set_item("description", &source.description)?;
                changes.set_item("meta", source.host.meta.bind(py))?;
                changes.set_item("columns", source.host.columns.bind(py))?;
                changes.set_item("audits", source.host.audits.bind(py))?;
                replaced(py, source.host.entry.bind(py), &changes)
            })
            .map(|entry| entry.clone_ref(py))
    }

    fn assembled_entry(&self, py: Python<'_>) -> PyResult<Py<PyAny>> {
        let attached: Py<PyAny> = self.attached_entry(py)?;
        let Some(Some((database, schema))) = &self.namespace else {
            return Ok(attached);
        };
        self.host
            .assembled_entry
            .get_or_try_init(py, || {
                let changes: Bound<'_, PyDict> = PyDict::new(py);
                changes.set_item("database", database)?;
                changes.set_item("schema", schema)?;
                replaced(py, attached.bind(py), &changes)
            })
            .map(|entry| entry.clone_ref(py))
    }
}

/// Read the source record behind a native source view, or `None` for any other object.
pub(crate) fn read_source_view<'py, T>(
    view: &Bound<'py, PyAny>,
    read: impl FnOnce(&SourceRecord, Python<'py>) -> PyResult<T>,
) -> Option<PyResult<T>> {
    let py: Python<'py> = view.py();
    if let Ok(input) = view.cast::<NativeSourceInput>() {
        let input: &NativeSourceInput = input.get();
        return Some(read_source(&input.owner, input.index, py, read));
    }
    let Ok(compiled) = view.cast::<NativeCompiledSource>() else {
        return None;
    };
    let compiled: &NativeCompiledSource = compiled.get();
    Some(read_source(&compiled.owner, compiled.index, py, read))
}

pub(crate) fn read_source<'py, T>(
    owner: &Py<NativeCompiledProject>,
    index: usize,
    py: Python<'py>,
    read: impl FnOnce(&SourceRecord, Python<'py>) -> PyResult<T>,
) -> PyResult<T> {
    let owner = owner.bind(py).borrow();
    let source: &SourceRecord = owner
        .sources()
        .get(index)
        .ok_or_else(|| PyIndexError::new_err(index))?;
    read(source, py)
}
