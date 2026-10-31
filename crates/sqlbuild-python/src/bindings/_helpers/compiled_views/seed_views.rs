//! Seeds paired with their declarations, held by the compiled project, and their views.

use pyo3::exceptions::{PyIndexError, PyRuntimeError};
use pyo3::prelude::{Bound, Py, PyAny, PyAnyMethods, PyErr, PyResult, Python};
use pyo3::sync::PyOnceLock;
use pyo3::types::PyTuple;
use pyo3::{IntoPyObjectExt, pyclass, pymethods};
use sqlbuild_attachments::seeds::main::pair_seed_declarations::pair_seed_declarations;

use sqlbuild_project::models as project_models;

use crate::bindings::_helpers::compiled_project::project::NativeCompiledProject;
use crate::bindings::_helpers::compiled_views::compiled_model_view::{
    defaulted_destination, object_key, relation_location,
};
use crate::bindings::_helpers::compiled_views::model_input_view::{InputViewClasses, view_classes};
use crate::bindings::_helpers::compiled_views::project_host::PyProjectHost;
use crate::bindings::_helpers::graph::project_graph::ResourceRow;
use crate::bindings::_helpers::records::fields::field as native_field;
use sqlbuild_project::models::DestinationFact;

const SEED_RESOURCE: &str = "seed";
const TAGS_ATTRIBUTE: &str = "tags";
const NAME_ATTRIBUTE: &str = "name";
const FILE_PATH_ATTRIBUTE: &str = "file_path";
const STEM_ATTRIBUTE: &str = "stem";
const RELATIVE_PATH_ATTRIBUTE: &str = "relative_path";

positional_record! {
    extract;
    /// `(database, schema, logical database, logical schema)` of a seed's destination.
    pub(crate) struct SeedNamespace {
        database: Option<String>,
        schema: Option<String>,
        logical_database: Option<String>,
        logical_schema: Option<String>,
    }
}

/// A seed's name, CSV file, declaration and declaring schema file.
pub(crate) struct SeedParts<'a> {
    pub(crate) name: &'a str,
    pub(crate) seed_file: &'a Py<PyAny>,
    pub(crate) schema_entry: &'a Py<PyAny>,
    pub(crate) schema_file: &'a Py<PyAny>,
}

/// One seed CSV paired with its declaration; the discovered objects are discovery's.
pub(crate) type SeedRecord = project_models::SeedRecord<PyProjectHost>;

/// A seed's discovered objects and the key its views build.
pub(crate) struct SeedHandles {
    /// The discovered CSV file.
    seed_file: Py<PyAny>,
    /// The discovered declaration, whose spec carries user-authored column metadata.
    schema_entry: Py<PyAny>,
    /// The schema file declaring the seed.
    schema_file: Py<PyAny>,
    key: PyOnceLock<Py<PyAny>>,
}

/// What a seed record reads and keeps through its Python values.
pub(crate) trait SeedViews {
    /// The seed's name, CSV file, declaration and declaring schema file.
    fn parts(&self) -> SeedParts<'_>;

    /// Keep the destination native project assembly resolved for the seed.
    fn set_namespace(&mut self, namespace: SeedNamespace);
}

/// Pair each declaration with the CSV file of its name, raising for the first without one.
pub(crate) fn paired_seeds(
    declarations: Vec<(Py<PyAny>, Py<PyAny>)>,
    seed_files: &[Py<PyAny>],
    compile_error: &Bound<'_, PyAny>,
) -> PyResult<Vec<SeedRecord>> {
    let py: Python<'_> = compile_error.py();
    let mut names: Vec<String> = Vec::with_capacity(declarations.len());
    for (entry, _) in &declarations {
        names.push(native_field(entry.bind(py), NAME_ATTRIBUTE)?.extract()?);
    }
    let mut stems: Vec<String> = Vec::with_capacity(seed_files.len());
    for seed_file in seed_files {
        stems.push(
            native_field(
                &native_field(seed_file.bind(py), FILE_PATH_ATTRIBUTE)?,
                STEM_ATTRIBUTE,
            )?
            .extract()?,
        );
    }
    let pairs: Vec<usize> = match pair_seed_declarations(&names, &stems) {
        Ok(pairs) => pairs,
        Err(missing) => {
            let schema_file: &Py<PyAny> = &declarations[missing].1;
            let relative_path: String =
                native_field(schema_file.bind(py), RELATIVE_PATH_ATTRIBUTE)?
                    .str()?
                    .extract()?;
            let message: String = format!(
                "Seed declaration '{}' in {relative_path} has no matching CSV file under seeds/",
                names[missing]
            );
            return Err(PyErr::from_value(compile_error.call1((message,))?));
        }
    };
    let mut seeds: Vec<SeedRecord> = Vec::with_capacity(pairs.len());
    for (((schema_entry, schema_file), name), file_index) in
        declarations.into_iter().zip(names).zip(pairs)
    {
        seeds.push(SeedRecord {
            name,
            destination: None,
            host: SeedHandles {
                seed_file: seed_files[file_index].clone_ref(py),
                schema_entry,
                schema_file,
                key: PyOnceLock::new(),
            },
        });
    }
    Ok(seeds)
}

impl SeedViews for SeedRecord {
    fn parts(&self) -> SeedParts<'_> {
        SeedParts {
            name: &self.name,
            seed_file: &self.host.seed_file,
            schema_entry: &self.host.schema_entry,
            schema_file: &self.host.schema_file,
        }
    }

    fn set_namespace(&mut self, namespace: SeedNamespace) {
        let SeedNamespace {
            database,
            schema,
            logical_database,
            logical_schema,
        } = namespace;
        self.destination = Some(DestinationFact {
            database,
            schema,
            name: self.name.clone(),
            qualified_name: None,
            logical_schema,
            logical_database,
        });
    }
}

fn read_seed<'py, T>(
    owner: &Py<NativeCompiledProject>,
    index: usize,
    py: Python<'py>,
    read: impl FnOnce(&SeedRecord, Python<'py>) -> PyResult<T>,
) -> PyResult<T> {
    let owner = owner.bind(py).borrow();
    let seed: &SeedRecord = owner
        .seeds()
        .get(index)
        .ok_or_else(|| PyIndexError::new_err(index))?;
    read(seed, py)
}

/// One attached seed, read from the compiled project that owns it.
#[pyclass(module = "sqlbuild._native", frozen)]
pub(crate) struct NativeSeedInput {
    owner: Py<NativeCompiledProject>,
    index: usize,
}

impl NativeSeedInput {
    pub(crate) fn new(owner: Py<NativeCompiledProject>, index: usize) -> Self {
        Self { owner, index }
    }
}

#[pymethods]
impl NativeSeedInput {
    #[getter]
    pub(crate) fn seed_file(&self, py: Python<'_>) -> PyResult<Py<PyAny>> {
        read_seed(&self.owner, self.index, py, |seed, py| {
            Ok(seed.host.seed_file.clone_ref(py))
        })
    }

    #[getter]
    pub(crate) fn schema_entry(&self, py: Python<'_>) -> PyResult<Py<PyAny>> {
        read_seed(&self.owner, self.index, py, |seed, py| {
            Ok(seed.host.schema_entry.clone_ref(py))
        })
    }

    #[getter]
    pub(crate) fn schema_file(&self, py: Python<'_>) -> PyResult<Py<PyAny>> {
        read_seed(&self.owner, self.index, py, |seed, py| {
            Ok(seed.host.schema_file.clone_ref(py))
        })
    }
}

/// One assembled seed, read from the compiled project that owns it.
#[pyclass(module = "sqlbuild._native", frozen)]
pub(crate) struct NativeCompiledSeed {
    owner: Py<NativeCompiledProject>,
    index: usize,
    /// The destination with target defaults applied, replacing the assembled one.
    defaulted: Option<DestinationFact>,
    destination: PyOnceLock<Py<PyAny>>,
}

impl NativeCompiledSeed {
    pub(crate) fn new(owner: Py<NativeCompiledProject>, index: usize) -> Self {
        Self {
            owner,
            index,
            defaulted: None,
            destination: PyOnceLock::new(),
        }
    }

    /// The seed's project graph row: its key and its declared tags.
    pub(crate) fn graph_row(&self, py: Python<'_>) -> PyResult<ResourceRow> {
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

    pub(crate) fn current_destination(&self, py: Python<'_>) -> PyResult<DestinationFact> {
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
    pub(crate) fn key(&self, py: Python<'_>) -> PyResult<Py<PyAny>> {
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
    pub(crate) fn deps<'py>(&self, py: Python<'py>) -> Bound<'py, PyTuple> {
        PyTuple::empty(py)
    }

    #[getter]
    pub(crate) fn name(&self, py: Python<'_>) -> PyResult<String> {
        read_seed(&self.owner, self.index, py, |seed, _| Ok(seed.name.clone()))
    }

    #[getter]
    pub(crate) fn seed_file(&self, py: Python<'_>) -> PyResult<Py<PyAny>> {
        read_seed(&self.owner, self.index, py, |seed, py| {
            Ok(seed.host.seed_file.clone_ref(py))
        })
    }

    #[getter]
    pub(crate) fn schema_entry(&self, py: Python<'_>) -> PyResult<Py<PyAny>> {
        read_seed(&self.owner, self.index, py, |seed, py| {
            Ok(seed.host.schema_entry.clone_ref(py))
        })
    }

    #[getter]
    pub(crate) fn schema_file(&self, py: Python<'_>) -> PyResult<Py<PyAny>> {
        read_seed(&self.owner, self.index, py, |seed, py| {
            Ok(seed.host.schema_file.clone_ref(py))
        })
    }

    /// The seed's planner-facing `CompiledRelationLocation`.
    #[getter]
    pub(crate) fn destination(&self, py: Python<'_>) -> PyResult<Py<PyAny>> {
        let classes: InputViewClasses<'_> = view_classes(&self.owner, py)?;
        self.destination
            .get_or_try_init(py, || {
                relation_location(&classes.compiled, &self.current_destination(py)?)
            })
            .map(|destination| destination.clone_ref(py))
    }

    /// This seed with the adapter's default schema and database filled into its destination.
    pub(crate) fn with_target_defaults(
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
    pub(crate) fn is_external(&self) -> bool {
        false
    }
}

impl NativeCompiledSeed {
    /// The `CompiledSeed` field values, in field order, without building the record.
    pub(crate) fn record_values<'py>(&self, py: Python<'py>) -> PyResult<Vec<Bound<'py, PyAny>>> {
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
