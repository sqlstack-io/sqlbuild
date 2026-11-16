//! Sources attached by the native loop, held by the compiled project, and their views.

use pyo3::exceptions::PyIndexError;
use pyo3::prelude::{Bound, Py, PyAny, PyAnyMethods, PyResult, Python};
use pyo3::sync::PyOnceLock;
use pyo3::types::{PyDict, PyDictMethods, PyTuple};
use pyo3::{IntoPyObjectExt, pyclass, pymethods};
use sqlbuild_analysis::compiled_project::models::SchemaColumnFact;
use sqlbuild_python_base::positional_record;

use sqlbuild_project::models as project_models;

use crate::bindings::_helpers::compiled_project::project::NativeCompiledProject;
use crate::bindings::_helpers::compiled_project::schema_facts::python_column_facts;
use crate::bindings::_helpers::compiled_views::compiled_model_view::object_key;
use crate::bindings::_helpers::compiled_views::model_input_view::{InputViewClasses, view_classes};
use crate::bindings::_helpers::compiled_views::project_host::PyProjectHost;
use crate::bindings::_helpers::compiled_views::relation_keys::audit_key_rows;
use crate::bindings::_helpers::graph::project_graph::ResourceRow;
use sqlbuild_python_base::records::main::fields::field::field as native_field;
use sqlbuild_python_base::records::main::fields::replaced_keywords::replaced_keywords;

const SOURCE_RESOURCE: &str = "source";

positional_record! {
    into;
    /// `(name, has a loader, database, schema)` of one source.
    pub(crate) struct SourceAssemblyRow {
        name: String,
        managed: bool,
        database: Option<String>,
        schema: Option<String>,
    }
}

/// What the source loop attached for one source: its expanded metadata and usages.
pub(crate) type AttachedSource = project_models::AttachedSource<PyProjectHost>;

/// One attached source held by the compiled project.
pub(crate) type SourceRecord = project_models::SourceRecord<PyProjectHost>;

/// An attached source's user values.
pub(crate) struct SourceHandles {
    /// The discovered source file.
    pub(crate) source_file: Py<PyAny>,
    /// The discovered entry; its spec carries user-authored metadata.
    pub(crate) entry: Py<PyAny>,
    /// Template-expanded user metadata.
    pub(crate) meta: Py<PyAny>,
    /// Template-expanded column specs, which carry user metadata and audit arguments.
    pub(crate) columns: Py<PyTuple>,
    /// Template-expanded table audit specs, which carry user audit arguments.
    pub(crate) audits: Py<PyTuple>,
    /// Usage records, the identities the scope index is built from.
    pub(crate) usages: Py<PyTuple>,
}

/// What a source declares about its keys: its column facts and `(audit, column argument)` rows.
pub(crate) struct SourceKeyFacts {
    pub(crate) columns: Vec<SchemaColumnFact>,
    pub(crate) audits: Vec<(String, Option<String>)>,
}

/// A source record's key facts and entries, each built once from its Python values.
pub(crate) struct SourceProjections {
    key_facts: PyOnceLock<SourceKeyFacts>,
    attached_entry: PyOnceLock<Py<PyAny>>,
    assembled_entry: PyOnceLock<Py<PyAny>>,
    pub(crate) key: PyOnceLock<Py<PyAny>>,
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

/// What a source record projects from its Python values, each built once.
pub(crate) trait SourceViews {
    /// The source's declared column facts and audit key rows, read once from its expanded specs.
    fn key_facts(&self, py: Python<'_>) -> PyResult<&SourceKeyFacts>;

    /// The attached `SourceEntry`: the discovered entry with its expanded metadata.
    fn attached_entry(&self, py: Python<'_>) -> PyResult<Py<PyAny>>;

    /// The assembled `SourceEntry`: the attached entry in its resolved namespace.
    fn assembled_entry(&self, py: Python<'_>) -> PyResult<Py<PyAny>>;
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

/// `dataclasses.replace(value, **changes)`.
pub(crate) fn replaced(
    _py: Python<'_>,
    value: &Bound<'_, PyAny>,
    changes: &Bound<'_, PyDict>,
) -> PyResult<Py<PyAny>> {
    Ok(replaced_keywords(value, changes)?.unbind())
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

fn read_source<'py, T>(
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

/// One attached source, read from the compiled project that owns it.
#[pyclass(module = "sqlbuild._native", frozen)]
pub(crate) struct NativeSourceInput {
    owner: Py<NativeCompiledProject>,
    index: usize,
}

impl NativeSourceInput {
    pub(crate) fn new(owner: Py<NativeCompiledProject>, index: usize) -> Self {
        Self { owner, index }
    }
}

#[pymethods]
impl NativeSourceInput {
    #[getter]
    pub(crate) fn source_entry(&self, py: Python<'_>) -> PyResult<Py<PyAny>> {
        read_source(&self.owner, self.index, py, |source, py| {
            source.attached_entry(py)
        })
    }

    #[getter]
    pub(crate) fn source_file(&self, py: Python<'_>) -> PyResult<Py<PyAny>> {
        read_source(&self.owner, self.index, py, |source, py| {
            Ok(source.attached.host.source_file.clone_ref(py))
        })
    }

    #[getter]
    pub(crate) fn declaration_usages(&self, py: Python<'_>) -> PyResult<Py<PyTuple>> {
        read_source(&self.owner, self.index, py, |source, py| {
            Ok(source.attached.host.usages.clone_ref(py))
        })
    }

    /// `(name, has a loader, database, schema)`, what project assembly resolves a namespace from.
    #[getter]
    pub(crate) fn assembly_row(&self, py: Python<'_>) -> PyResult<SourceAssemblyRow> {
        read_source(&self.owner, self.index, py, |source, py| {
            let attached: &AttachedSource = &source.attached;
            Ok(SourceAssemblyRow {
                name: attached.name.clone(),
                managed: !native_field(attached.host.entry.bind(py), "loader")?.is_none(),
                database: attached.database.clone(),
                schema: attached.schema.clone(),
            })
        })
    }
}

/// One assembled source, read from the compiled project that owns it.
#[pyclass(module = "sqlbuild._native", frozen)]
pub(crate) struct NativeCompiledSource {
    owner: Py<NativeCompiledProject>,
    index: usize,
}

impl NativeCompiledSource {
    pub(crate) fn new(owner: Py<NativeCompiledProject>, index: usize) -> Self {
        Self { owner, index }
    }

    /// The source's project graph row: its key, with no dependencies, tags or folder.
    pub(crate) fn graph_row(&self, py: Python<'_>) -> PyResult<ResourceRow> {
        read_source(&self.owner, self.index, py, |source, _| {
            Ok(ResourceRow {
                key: (SOURCE_RESOURCE.to_owned(), source.attached.name.clone()),
                deps: Vec::new(),
                tags: Vec::new(),
                folder: None,
            })
        })
    }
}

#[pymethods]
impl NativeCompiledSource {
    /// The source's planner-facing `CompiledObjectKey`.
    #[getter]
    pub(crate) fn key(&self, py: Python<'_>) -> PyResult<Py<PyAny>> {
        let classes: InputViewClasses<'_> = view_classes(&self.owner, py)?;
        read_source(&self.owner, self.index, py, |source, py| {
            source
                .host
                .key
                .get_or_try_init(py, || {
                    object_key(&classes.compiled, SOURCE_RESOURCE, &source.attached.name)
                })
                .map(|key| key.clone_ref(py))
        })
    }

    #[getter]
    pub(crate) fn deps<'py>(&self, py: Python<'py>) -> Bound<'py, PyTuple> {
        PyTuple::empty(py)
    }

    #[getter]
    pub(crate) fn name(&self, py: Python<'_>) -> PyResult<String> {
        read_source(&self.owner, self.index, py, |source, _| {
            Ok(source.attached.name.clone())
        })
    }

    #[getter]
    pub(crate) fn source_entry(&self, py: Python<'_>) -> PyResult<Py<PyAny>> {
        read_source(&self.owner, self.index, py, |source, py| {
            source.assembled_entry(py)
        })
    }

    #[getter]
    pub(crate) fn source_file(&self, py: Python<'_>) -> PyResult<Py<PyAny>> {
        read_source(&self.owner, self.index, py, |source, py| {
            Ok(source.attached.host.source_file.clone_ref(py))
        })
    }
}

impl NativeCompiledSource {
    /// The `CompiledSource` field values, in field order, without building the record.
    pub(crate) fn record_values<'py>(&self, py: Python<'py>) -> PyResult<Vec<Bound<'py, PyAny>>> {
        Ok(vec![
            self.key(py)?.into_bound(py),
            self.deps(py).into_any(),
            self.name(py)?.into_bound_py_any(py)?,
            self.source_entry(py)?.into_bound(py),
            self.source_file(py)?.into_bound(py),
        ])
    }
}

/// Keep `attached` in `owner`; return each source's view.
pub(crate) fn retain_sources(
    owner: &Bound<'_, NativeCompiledProject>,
    attached: Vec<AttachedSource>,
) -> Vec<NativeSourceInput> {
    let mut project = owner.borrow_mut();
    let first: usize = project.sources().len();
    let count: usize = attached.len();
    for source in attached {
        project.retain_source(source_record(source));
    }
    drop(project);
    (first..first + count)
        .map(|index| NativeSourceInput::new(owner.clone().unbind(), index))
        .collect()
}
