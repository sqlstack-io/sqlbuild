//! Public records of resource views.

use crate::compiled_project::models::NativeCompiledProject;
use crate::compiled_views::main::compiled_models::defaulted_destination::defaulted_destination;
use crate::compiled_views::main::compiled_models::object_key::object_key;
use crate::compiled_views::main::compiled_models::relation_location::relation_location;
use crate::compiled_views::main::model_inputs::view_classes::view_classes;
use crate::compiled_views::models::InputViewClasses;
use crate::graph::models::ResourceRow;
use crate::resource_views::_helpers::function_views::NOT_ASSEMBLED;
use crate::resource_views::_helpers::function_views::read_function;
use crate::resource_views::_helpers::seed_views::SEED_RESOURCE;
use crate::resource_views::_helpers::seed_views::TAGS_ATTRIBUTE;
use crate::resource_views::_helpers::seed_views::read_seed;
use crate::resource_views::_helpers::source_views::SourceAssemblyRow;
use crate::resource_views::_helpers::source_views::read_source;
use crate::resource_views::_helpers::source_views::{LOAD_FIELD, LOAD_RESOURCE, SOURCE_RESOURCE};
use crate::resource_views::types::AttachedSource;
use crate::resource_views::types::SourceViews;
use pyo3::IntoPyObjectExt;
use pyo3::exceptions::PyRuntimeError;
use pyo3::prelude::Bound;
use pyo3::prelude::Py;
use pyo3::prelude::PyAny;
use pyo3::prelude::PyAnyMethods;
use pyo3::prelude::PyResult;
use pyo3::prelude::Python;
use pyo3::pyclass;
use pyo3::pymethods;
use pyo3::sync::PyOnceLock;
use pyo3::types::PyTuple;
use sqlbuild_analysis::compiled_project::models::SchemaColumnFact;
use sqlbuild_project::models::DestinationFact;
use sqlbuild_project::types::ObjectKeyRow;
use sqlbuild_python_base::records::main::fields::field::field as native_field;

/// An attached audit's user values.
#[derive(Debug)]
pub struct AuditHandles {
    /// The discovered (or built-in) audit file and block the SQL comes from.
    pub audit_file: Py<PyAny>,
    pub audit_block: Py<PyAny>,
    /// The authored measurement policy specs, carrying user values.
    pub thresholds: Py<PyAny>,
    pub minimum_samples: Py<PyAny>,
    pub evidence_limit: Py<PyAny>,
    /// Usage records, the identities the scope index is built from.
    pub usages: Py<PyTuple>,
}

/// An audit record's Python projections, each built once on first access.
pub struct AuditProjections {
    pub references: PyOnceLock<Py<PyTuple>>,
    pub key: PyOnceLock<Py<PyAny>>,
}

/// One attached audit, read from the compiled project that owns it.
#[pyclass(module = "sqlbuild._native", frozen)]
#[derive(Debug)]
pub struct NativeAuditInput {
    pub(crate) owner: Py<NativeCompiledProject>,
    pub(crate) index: usize,
}

impl NativeAuditInput {
    pub fn new(owner: Py<NativeCompiledProject>, index: usize) -> Self {
        Self { owner, index }
    }
}

/// One assembled audit, read from the compiled project that owns it.
#[pyclass(module = "sqlbuild._native", frozen)]
#[derive(Debug)]
pub struct NativeCompiledAudit {
    pub(crate) owner: Py<NativeCompiledProject>,
    pub(crate) index: usize,
}

impl NativeCompiledAudit {
    pub fn new(owner: Py<NativeCompiledProject>, index: usize) -> Self {
        Self { owner, index }
    }
}

impl NativeCompiledAudit {
    /// The `CompiledAudit` field values, in field order, without building the record.
    pub fn record_values<'py>(&self, py: Python<'py>) -> PyResult<Vec<Bound<'py, PyAny>>> {
        Ok(vec![
            self.key(py)?.into_bound(py),
            self.scope_deps(py)?.into_any(),
            self.name(py)?.into_bound_py_any(py)?,
            self.definition_name(py)?.into_bound_py_any(py)?,
            self.audit_file(py)?.into_bound(py),
            self.audit_block(py)?.into_bound(py),
            self.sql_body(py)?.into_bound_py_any(py)?,
            self.evaluation_mode(py)?.into_bound(py),
            self.measurement_contract(py)?.into_bound(py),
            self.thresholds(py)?.into_bound(py),
            self.minimum_samples(py)?.into_bound(py),
            self.measure_sql(py)?.into_bound_py_any(py)?,
            self.evidence_sql(py)?.into_bound_py_any(py)?,
            self.evidence_limit(py)?.into_bound(py),
            self.references(py)?.into_bound(py).into_any(),
            self.attached_target_kind(py)?,
            self.attached_target_name(py)?.into_bound_py_any(py)?,
            self.attached_column_name(py)?.into_bound_py_any(py)?,
            self.severity(py)?,
            self.run_scope(py)?.into_bound_py_any(py)?,
            pyo3::types::PyBool::new(py, self.always_run(py)?)
                .to_owned()
                .into_any(),
            self.description(py)?.into_bound_py_any(py)?,
            self.meta(py)?.into_bound(py),
        ])
    }
}

/// An attached function's user values.
#[derive(Debug)]
pub struct FunctionHandles {
    /// The discovered function file.
    pub function_file: Py<PyAny>,
    /// Usage records, the identities the scope index is built from.
    pub usages: Py<PyTuple>,
}

/// A function record's Python projections, each built once on first access.
pub struct FunctionProjections {
    pub(crate) arguments: PyOnceLock<Py<PyTuple>>,
    pub(crate) return_columns: PyOnceLock<Py<PyTuple>>,
    pub references: PyOnceLock<Py<PyTuple>>,
    pub key: PyOnceLock<Py<PyAny>>,
}

/// One attached function, read from the compiled project that owns it.
#[pyclass(module = "sqlbuild._native", frozen)]
#[derive(Debug)]
pub struct NativeFunctionInput {
    pub(crate) owner: Py<NativeCompiledProject>,
    pub(crate) index: usize,
}

impl NativeFunctionInput {
    pub fn new(owner: Py<NativeCompiledProject>, index: usize) -> Self {
        Self { owner, index }
    }
}

/// One assembled function, read from the compiled project that owns it.
#[pyclass(module = "sqlbuild._native", frozen)]
pub struct NativeCompiledFunction {
    pub(crate) owner: Py<NativeCompiledProject>,
    pub(crate) index: usize,
    /// The destination and fingerprint destination with target defaults applied.
    pub(crate) defaulted: Option<(DestinationFact, DestinationFact)>,
    pub(crate) destination: PyOnceLock<Py<PyAny>>,
    pub(crate) fingerprint_destination: PyOnceLock<Py<PyAny>>,
}

impl std::fmt::Debug for NativeCompiledFunction {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("NativeCompiledFunction")
            .finish_non_exhaustive()
    }
}

impl NativeCompiledFunction {
    pub fn new(owner: Py<NativeCompiledProject>, index: usize) -> Self {
        Self::defaulted(owner, index, None)
    }

    /// The function's project graph row: its key, the resources it reads and its tags.
    pub fn graph_row(&self, py: Python<'_>) -> PyResult<ResourceRow> {
        read_function(&self.owner, self.index, py, |function, _| {
            let deps: &[ObjectKeyRow] = function
                .deps
                .as_deref()
                .ok_or_else(|| PyRuntimeError::new_err(NOT_ASSEMBLED))?;
            Ok(ResourceRow {
                key: (
                    function.attached.node_type().to_owned(),
                    function.attached.name.clone(),
                ),
                deps: deps.to_vec(),
                tags: function.attached.tags.clone(),
                folder: None,
            })
        })
    }

    pub(crate) fn defaulted(
        owner: Py<NativeCompiledProject>,
        index: usize,
        defaulted: Option<(DestinationFact, DestinationFact)>,
    ) -> Self {
        Self {
            owner,
            index,
            defaulted,
            destination: PyOnceLock::new(),
            fingerprint_destination: PyOnceLock::new(),
        }
    }

    /// The destination and fingerprint destination, with target defaults once applied.
    pub fn current_destinations(
        &self,
        py: Python<'_>,
    ) -> PyResult<(DestinationFact, DestinationFact)> {
        if let Some(defaulted) = &self.defaulted {
            return Ok(defaulted.clone());
        }
        read_function(&self.owner, self.index, py, |function, _| {
            Ok((
                function.attached.destination.clone(),
                function.attached.fingerprint_destination.clone(),
            ))
        })
    }
}

impl NativeCompiledFunction {
    /// The `CompiledFunction` field values, in field order, without building the record.
    pub fn record_values<'py>(&self, py: Python<'py>) -> PyResult<Vec<Bound<'py, PyAny>>> {
        Ok(vec![
            self.key(py)?.into_bound(py),
            self.deps(py)?.into_any(),
            self.name(py)?.into_bound_py_any(py)?,
            self.relative_path(py)?.into_bound(py),
            self.arguments(py)?.into_bound(py).into_any(),
            self.returns(py)?.into_bound_py_any(py)?,
            self.body_sql(py)?.into_bound_py_any(py)?,
            self.destination(py)?.into_bound(py),
            self.fingerprint_destination(py)?.into_bound(py),
            self.return_columns(py)?.into_bound(py).into_any(),
            self.references(py)?.into_bound(py).into_any(),
            self.language(py)?,
            self.source_file_path(py)?.into_bound(py),
            self.runtime_version(py)?.into_bound_py_any(py)?,
            self.entry_point(py)?.into_bound_py_any(py)?,
            self.packages(py)?.into_any(),
            self.dataframe(py)?.into_bound_py_any(py)?,
            self.tags(py)?.into_any(),
            self.description(py)?.into_bound_py_any(py)?,
        ])
    }
}

/// `(database, schema, logical database, logical schema)` of a seed's destination.
#[derive(Debug)]
pub struct SeedNamespace {
    pub database: Option<String>,
    pub schema: Option<String>,
    pub logical_database: Option<String>,
    pub logical_schema: Option<String>,
}

impl<'py> pyo3::FromPyObject<'py> for SeedNamespace {
    fn extract_bound(value: &pyo3::Bound<'py, pyo3::PyAny>) -> pyo3::PyResult<Self> {
        let (database, schema, logical_database, logical_schema): (
            Option<String>,
            Option<String>,
            Option<String>,
            Option<String>,
        ) = pyo3::types::PyAnyMethods::extract(value)?;
        Ok(Self {
            database,
            schema,
            logical_database,
            logical_schema,
        })
    }
}

/// A seed's name, CSV file, declaration and declaring schema file.
#[derive(Debug)]
pub struct SeedParts<'a> {
    pub name: &'a str,
    pub seed_file: &'a Py<PyAny>,
    pub schema_entry: &'a Py<PyAny>,
    pub schema_file: &'a Py<PyAny>,
}

/// A seed's discovered objects and the key its views build.
pub struct SeedHandles {
    /// The discovered CSV file.
    pub(crate) seed_file: Py<PyAny>,
    /// The discovered declaration, whose spec carries user-authored column metadata.
    pub(crate) schema_entry: Py<PyAny>,
    /// The schema file declaring the seed.
    pub(crate) schema_file: Py<PyAny>,
    pub(crate) key: PyOnceLock<Py<PyAny>>,
}

/// One attached seed, read from the compiled project that owns it.
#[pyclass(module = "sqlbuild._native", frozen)]
#[derive(Debug)]
pub struct NativeSeedInput {
    pub(crate) owner: Py<NativeCompiledProject>,
    pub(crate) index: usize,
}

impl NativeSeedInput {
    pub fn new(owner: Py<NativeCompiledProject>, index: usize) -> Self {
        Self { owner, index }
    }
}

#[pymethods]
impl NativeSeedInput {
    #[getter]
    pub fn seed_file(&self, py: Python<'_>) -> PyResult<Py<PyAny>> {
        read_seed(&self.owner, self.index, py, |seed, py| {
            Ok(seed.host.seed_file.clone_ref(py))
        })
    }

    #[getter]
    pub fn schema_entry(&self, py: Python<'_>) -> PyResult<Py<PyAny>> {
        read_seed(&self.owner, self.index, py, |seed, py| {
            Ok(seed.host.schema_entry.clone_ref(py))
        })
    }

    #[getter]
    pub fn schema_file(&self, py: Python<'_>) -> PyResult<Py<PyAny>> {
        read_seed(&self.owner, self.index, py, |seed, py| {
            Ok(seed.host.schema_file.clone_ref(py))
        })
    }
}

/// One assembled seed, read from the compiled project that owns it.
#[pyclass(module = "sqlbuild._native", frozen)]
pub struct NativeCompiledSeed {
    pub(crate) owner: Py<NativeCompiledProject>,
    pub(crate) index: usize,
    /// The destination with target defaults applied, replacing the assembled one.
    pub(crate) defaulted: Option<DestinationFact>,
    pub(crate) destination: PyOnceLock<Py<PyAny>>,
}

impl std::fmt::Debug for NativeCompiledSeed {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("NativeCompiledSeed")
            .finish_non_exhaustive()
    }
}

impl NativeCompiledSeed {
    pub fn new(owner: Py<NativeCompiledProject>, index: usize) -> Self {
        Self {
            owner,
            index,
            defaulted: None,
            destination: PyOnceLock::new(),
        }
    }

    /// The seed's project graph row: its key and its declared tags.
    pub fn graph_row(&self, py: Python<'_>) -> PyResult<ResourceRow> {
        read_seed(&self.owner, self.index, py, |seed, py| {
            let mut tags: Vec<String> = Vec::new();
            for tag in native_field(seed.host.schema_entry.bind(py), TAGS_ATTRIBUTE)?.try_iter()? {
                tags.push(tag?.str()?.extract()?);
            }
            Ok(ResourceRow {
                key: (SEED_RESOURCE.to_owned(), seed.name.clone()),
                deps: Vec::new(),
                tags,
                folder: None,
            })
        })
    }

    pub fn current_destination(&self, py: Python<'_>) -> PyResult<DestinationFact> {
        if let Some(defaulted) = &self.defaulted {
            return Ok(defaulted.clone());
        }
        read_seed(&self.owner, self.index, py, |seed, _| {
            seed.destination
                .clone()
                .ok_or_else(|| PyRuntimeError::new_err("the seed was not assembled"))
        })
    }
}

#[pymethods]
impl NativeCompiledSeed {
    /// The seed's planner-facing `CompiledObjectKey`.
    #[getter]
    pub fn key(&self, py: Python<'_>) -> PyResult<Py<PyAny>> {
        let classes: InputViewClasses<'_> = view_classes(&self.owner, py)?;
        read_seed(&self.owner, self.index, py, |seed, py| {
            seed.host
                .key
                .get_or_try_init(py, || {
                    object_key(&classes.compiled, SEED_RESOURCE, &seed.name)
                })
                .map(|key| key.clone_ref(py))
        })
    }

    #[getter]
    pub fn deps<'py>(&self, py: Python<'py>) -> Bound<'py, PyTuple> {
        PyTuple::empty(py)
    }

    #[getter]
    pub fn name(&self, py: Python<'_>) -> PyResult<String> {
        read_seed(&self.owner, self.index, py, |seed, _| Ok(seed.name.clone()))
    }

    #[getter]
    pub fn seed_file(&self, py: Python<'_>) -> PyResult<Py<PyAny>> {
        read_seed(&self.owner, self.index, py, |seed, py| {
            Ok(seed.host.seed_file.clone_ref(py))
        })
    }

    #[getter]
    pub fn schema_entry(&self, py: Python<'_>) -> PyResult<Py<PyAny>> {
        read_seed(&self.owner, self.index, py, |seed, py| {
            Ok(seed.host.schema_entry.clone_ref(py))
        })
    }

    #[getter]
    pub fn schema_file(&self, py: Python<'_>) -> PyResult<Py<PyAny>> {
        read_seed(&self.owner, self.index, py, |seed, py| {
            Ok(seed.host.schema_file.clone_ref(py))
        })
    }

    /// The seed's planner-facing `CompiledRelationLocation`.
    #[getter]
    pub fn destination(&self, py: Python<'_>) -> PyResult<Py<PyAny>> {
        let classes: InputViewClasses<'_> = view_classes(&self.owner, py)?;
        self.destination
            .get_or_try_init(py, || {
                relation_location(&classes.compiled, &self.current_destination(py)?)
            })
            .map(|destination| destination.clone_ref(py))
    }

    /// This seed with the adapter's default schema and database filled into its destination.
    pub fn with_target_defaults(
        &self,
        py: Python<'_>,
        defaults: (Option<String>, Option<String>),
        render: Bound<'_, PyAny>,
    ) -> PyResult<Self> {
        let defaulted: DestinationFact =
            defaulted_destination(&self.current_destination(py)?, defaults, &render)?;
        Ok(Self {
            owner: self.owner.clone_ref(py),
            index: self.index,
            defaulted: Some(defaulted),
            destination: PyOnceLock::new(),
        })
    }

    #[getter(external)]
    pub fn is_external(&self) -> bool {
        false
    }
}

impl NativeCompiledSeed {
    /// The `CompiledSeed` field values, in field order, without building the record.
    pub fn record_values<'py>(&self, py: Python<'py>) -> PyResult<Vec<Bound<'py, PyAny>>> {
        Ok(vec![
            self.key(py)?.into_bound(py),
            self.deps(py).into_any(),
            self.name(py)?.into_bound_py_any(py)?,
            self.seed_file(py)?.into_bound(py),
            self.schema_entry(py)?.into_bound(py),
            self.schema_file(py)?.into_bound(py),
            self.destination(py)?.into_bound(py),
            pyo3::types::PyBool::new(py, self.is_external())
                .to_owned()
                .into_any(),
        ])
    }
}

/// An attached source's user values.
#[derive(Debug)]
pub struct SourceHandles {
    /// The discovered source file.
    pub source_file: Py<PyAny>,
    /// The discovered entry; its spec carries user-authored metadata.
    pub entry: Py<PyAny>,
    /// Template-expanded user metadata.
    pub meta: Py<PyAny>,
    /// Template-expanded column specs, which carry user metadata and audit arguments.
    pub columns: Py<PyTuple>,
    /// Template-expanded table audit specs, which carry user audit arguments.
    pub audits: Py<PyTuple>,
    /// Usage records, the identities the scope index is built from.
    pub usages: Py<PyTuple>,
}

/// What a source declares about its keys: its column facts and `(audit, column argument)` rows.
#[derive(Debug)]
pub struct SourceKeyFacts {
    pub columns: Vec<SchemaColumnFact>,
    pub audits: Vec<(String, Option<String>)>,
}

/// A source record's key facts and entries, each built once from its Python values.
pub struct SourceProjections {
    pub(crate) key_facts: PyOnceLock<SourceKeyFacts>,
    pub(crate) attached_entry: PyOnceLock<Py<PyAny>>,
    pub(crate) assembled_entry: PyOnceLock<Py<PyAny>>,
    pub key: PyOnceLock<Py<PyAny>>,
}

/// One attached source, read from the compiled project that owns it.
#[pyclass(module = "sqlbuild._native", frozen)]
#[derive(Debug)]
pub struct NativeSourceInput {
    pub(crate) owner: Py<NativeCompiledProject>,
    pub(crate) index: usize,
}

impl NativeSourceInput {
    pub fn new(owner: Py<NativeCompiledProject>, index: usize) -> Self {
        Self { owner, index }
    }
}

#[pymethods]
impl NativeSourceInput {
    #[getter]
    pub fn source_entry(&self, py: Python<'_>) -> PyResult<Py<PyAny>> {
        read_source(&self.owner, self.index, py, |source, py| {
            source.attached_entry(py)
        })
    }

    #[getter]
    pub fn source_file(&self, py: Python<'_>) -> PyResult<Py<PyAny>> {
        read_source(&self.owner, self.index, py, |source, py| {
            Ok(source.attached.host.source_file.clone_ref(py))
        })
    }

    #[getter]
    pub fn declaration_usages(&self, py: Python<'_>) -> PyResult<Py<PyTuple>> {
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
#[derive(Debug)]
pub struct NativeCompiledSource {
    pub(crate) owner: Py<NativeCompiledProject>,
    pub(crate) index: usize,
}

impl NativeCompiledSource {
    pub fn new(owner: Py<NativeCompiledProject>, index: usize) -> Self {
        Self { owner, index }
    }

    /// The source's project graph row: its key and the load that fills it, with no tags or folder.
    pub fn graph_row(&self, py: Python<'_>) -> PyResult<ResourceRow> {
        read_source(&self.owner, self.index, py, |source, py| {
            let load: Option<String> =
                native_field(source.attached.host.entry.bind(py), LOAD_FIELD)?.extract()?;
            Ok(ResourceRow {
                key: (SOURCE_RESOURCE.to_owned(), source.attached.name.clone()),
                deps: load
                    .map(|load| (LOAD_RESOURCE.to_owned(), load))
                    .into_iter()
                    .collect(),
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
    pub fn key(&self, py: Python<'_>) -> PyResult<Py<PyAny>> {
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

    /// The load that fills the source, the only resource a source depends on.
    #[getter]
    pub fn deps<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyTuple>> {
        let classes: InputViewClasses<'_> = view_classes(&self.owner, py)?;
        let load: Option<String> = read_source(&self.owner, self.index, py, |source, py| {
            native_field(source.attached.host.entry.bind(py), LOAD_FIELD)?.extract()
        })?;
        match load {
            Some(load) => PyTuple::new(py, [object_key(&classes.compiled, LOAD_RESOURCE, &load)?]),
            None => Ok(PyTuple::empty(py)),
        }
    }

    #[getter]
    pub fn name(&self, py: Python<'_>) -> PyResult<String> {
        read_source(&self.owner, self.index, py, |source, _| {
            Ok(source.attached.name.clone())
        })
    }

    #[getter]
    pub fn source_entry(&self, py: Python<'_>) -> PyResult<Py<PyAny>> {
        read_source(&self.owner, self.index, py, |source, py| {
            source.assembled_entry(py)
        })
    }

    #[getter]
    pub fn source_file(&self, py: Python<'_>) -> PyResult<Py<PyAny>> {
        read_source(&self.owner, self.index, py, |source, py| {
            Ok(source.attached.host.source_file.clone_ref(py))
        })
    }
}

impl NativeCompiledSource {
    /// The `CompiledSource` field values, in field order, without building the record.
    pub fn record_values<'py>(&self, py: Python<'py>) -> PyResult<Vec<Bound<'py, PyAny>>> {
        Ok(vec![
            self.key(py)?.into_bound(py),
            self.deps(py)?.into_any(),
            self.name(py)?.into_bound_py_any(py)?,
            self.source_entry(py)?.into_bound(py),
            self.source_file(py)?.into_bound(py),
        ])
    }
}
