//! A discovered SQL model file as a native record whose Python attributes are built on demand.

use crate::discovery::models::ModelFileTexts;
use crate::discovery::models::NativeModelFile;
use std::path::PathBuf;
use std::sync::Arc;

use crate::sqltext::main::map_to_python::map_to_python;
use pyo3::exceptions::PyRuntimeError;
use pyo3::prelude::{Bound, Py, PyAny, PyAnyMethods, PyResult, Python};
use pyo3::sync::PyOnceLock;
use pyo3::types::{PyDict, PyDictMethods, PyString, PyStringMethods, PyTuple};
use pyo3::{FromPyObject, IntoPyObject};
use sqlbuild_discovery::model_files::main::model_file_stem::model_file_stem;
use sqlbuild_discovery::models::LineColumnSpan;
use sqlbuild_python_base::records::main::construct::new_record::new_record;
use sqlbuild_python_base::records::main::fields::field::field;
use sqlbuild_python_base::records::types::RecordId;
use sqlbuild_sqltext::compiler::models::AuthoredValue;

pub(crate) const FIELD_NAMES: [&str; 10] = [
    "file_path",
    "relative_path",
    "contents",
    "header_values",
    "header_column_locations",
    "output_column_locations",
    "query_sql",
    "enum_declarations",
    "constant_declarations",
    "extract_implicit_alias_columns",
];
const STEM_ATTRIBUTE: &str = "stem";

/// Authored locations: native rows until Python reads them, then a cached dict.
pub(crate) struct Locations {
    pub(crate) rows: Vec<(String, LineColumnSpan)>,
    pub(crate) projected: PyOnceLock<Py<PyDict>>,
}

impl std::fmt::Debug for Locations {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.debug_struct("Locations").finish_non_exhaustive()
    }
}

impl Locations {
    pub(crate) fn from_rows(rows: Vec<(String, LineColumnSpan)>) -> Self {
        Self {
            rows,
            projected: PyOnceLock::new(),
        }
    }

    /// These locations for another record: the rows, and the dict once Python has read it.
    pub(crate) fn shared(&self, py: Python<'_>) -> Self {
        let projected: PyOnceLock<Py<PyDict>> = shared_projection(py, &self.projected);
        Self {
            rows: self.rows.clone(),
            projected,
        }
    }

    pub(crate) fn from_dict(py: Python<'_>, dict: Py<PyDict>) -> Self {
        let projected: PyOnceLock<Py<PyDict>> = PyOnceLock::new();
        let _ = projected.set(py, dict);
        Self {
            rows: Vec::new(),
            projected,
        }
    }

    /// Python's `{name: SourceLocation(...)}`: a repeated name keeps its first position and last value.
    pub(crate) fn dict<'py>(&self, path: &Bound<'py, PyAny>) -> PyResult<Bound<'py, PyDict>> {
        let py: Python<'py> = path.py();
        Ok(self
            .projected
            .get_or_try_init(py, || {
                let dict: Bound<'py, PyDict> = PyDict::new(py);
                for (name, span) in &self.rows {
                    let location: Bound<'py, PyAny> = new_record(
                        py,
                        RecordId::SourceLocation,
                        (path, span.line, span.column, span.end_line, span.end_column),
                        None,
                    )?;
                    dict.set_item(name, location)?;
                }
                Ok::<_, pyo3::PyErr>(dict.unbind())
            })?
            .bind(py)
            .clone())
    }
}

/// A model file's paths: native text, with the Python path objects built on first read.
pub(crate) struct ModelFilePaths {
    pub(crate) native_file_path: PathBuf,
    /// `None` when a path is not text Python can encode, so readers raise as Python does.
    pub(crate) texts: Option<ModelFileTexts>,
    /// `(file_path, relative_path)` as Python path objects.
    pub(crate) objects: PyOnceLock<(Py<PyAny>, Py<PyAny>)>,
    /// The relative path the objects are built from, unless they were given.
    pub(crate) relative: Option<PathBuf>,
}

impl std::fmt::Debug for ModelFilePaths {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("ModelFilePaths")
            .finish_non_exhaustive()
    }
}

impl ModelFilePaths {
    /// Paths given as Python objects; their text is read once, here.
    pub(crate) fn given(
        py: Python<'_>,
        file_path: Py<PyAny>,
        relative_path: Py<PyAny>,
    ) -> PyResult<Self> {
        let native_file_path: PathBuf = file_path.bind(py).extract()?;
        let relative: PathBuf = relative_path.bind(py).extract()?;
        let texts: Option<ModelFileTexts> =
            native_file_path
                .to_str()
                .zip(relative.to_str())
                .map(|(file_text, relative_text)| ModelFileTexts {
                    name: model_file_stem(&posix(file_text)).to_owned(),
                    posix_path: posix(relative_text),
                    relative_path: relative_text.to_owned(),
                    file_path: file_text.to_owned(),
                });
        let objects: PyOnceLock<(Py<PyAny>, Py<PyAny>)> = PyOnceLock::new();
        let _ = objects.set(py, (file_path, relative_path));
        Ok(Self {
            native_file_path,
            texts,
            objects,
            relative: None,
        })
    }

    /// Paths native discovery named; their Python path objects are built only when read.
    pub(crate) fn discovered(relative_path: &str, file_path: String) -> Self {
        let name: String = model_file_stem(relative_path).to_owned();
        Self {
            native_file_path: PathBuf::from(&file_path),
            texts: Some(ModelFileTexts {
                name,
                posix_path: relative_path.to_owned(),
                relative_path: relative_path.replace('/', std::path::MAIN_SEPARATOR_STR),
                file_path,
            }),
            objects: PyOnceLock::new(),
            relative: Some(PathBuf::from(relative_path)),
        }
    }

    pub(crate) fn shared(&self, py: Python<'_>) -> Self {
        let objects: PyOnceLock<(Py<PyAny>, Py<PyAny>)> = PyOnceLock::new();
        if let Some((file_path, relative_path)) = self.objects.get(py) {
            let _ = objects.set(py, (file_path.clone_ref(py), relative_path.clone_ref(py)));
        }
        Self {
            native_file_path: self.native_file_path.clone(),
            texts: self.texts.clone(),
            objects,
            relative: self.relative.clone(),
        }
    }

    /// `(file_path, relative_path)` as the `pathlib` objects discovery names the file by.
    pub(crate) fn objects<'py>(
        &self,
        py: Python<'py>,
    ) -> PyResult<(Bound<'py, PyAny>, Bound<'py, PyAny>)> {
        let (file_path, relative_path) = self.objects.get_or_try_init(py, || {
            let Some(relative) = &self.relative else {
                return Err(PyRuntimeError::new_err("a model file has no paths"));
            };
            Ok::<_, pyo3::PyErr>((
                self.native_file_path.as_path().into_pyobject(py)?.unbind(),
                relative.as_path().into_pyobject(py)?.unbind(),
            ))
        })?;
        Ok((file_path.bind(py).clone(), relative_path.bind(py).clone()))
    }

    /// The file's names, or the error Python raises reading a name that is not UTF-8 text.
    pub(crate) fn texts(&self, py: Python<'_>) -> PyResult<&ModelFileTexts> {
        if let Some(texts) = &self.texts {
            return Ok(texts);
        }
        let (file_path, relative_path) = self.objects(py)?;
        for path in [file_path, relative_path] {
            path.str()?.to_str()?;
        }
        Err(PyRuntimeError::new_err("a model file path is not text"))
    }
}

/// `text` with the platform's path separator spelled `/`, as `PurePath.as_posix` spells it.
fn posix(text: &str) -> String {
    text.replace(std::path::MAIN_SEPARATOR, "/")
}

/// A model header's authored values: native entries, with the Python dict built on first read.
pub(crate) struct HeaderValues {
    pub(crate) native: Option<Arc<[(String, AuthoredValue)]>>,
    pub(crate) projected: PyOnceLock<Py<PyDict>>,
}

impl std::fmt::Debug for HeaderValues {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("HeaderValues")
            .finish_non_exhaustive()
    }
}

impl HeaderValues {
    pub(crate) fn native(values: Vec<(String, AuthoredValue)>) -> Self {
        Self {
            native: Some(values.into()),
            projected: PyOnceLock::new(),
        }
    }

    pub(crate) fn given(py: Python<'_>, values: Py<PyDict>) -> Self {
        let projected: PyOnceLock<Py<PyDict>> = PyOnceLock::new();
        let _ = projected.set(py, values);
        Self {
            native: None,
            projected,
        }
    }

    pub(crate) fn shared(&self, py: Python<'_>) -> Self {
        let projected: PyOnceLock<Py<PyDict>> = shared_projection(py, &self.projected);
        Self {
            native: self.native.clone(),
            projected,
        }
    }

    pub(crate) fn dict<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyDict>> {
        Ok(self
            .projected
            .get_or_try_init(py, || {
                let values: &[(String, AuthoredValue)] = self.native.as_deref().unwrap_or_default();
                map_to_python(py, values)?
                    .into_bound(py)
                    .cast_into::<PyDict>()
                    .map(Bound::unbind)
                    .map_err(pyo3::PyErr::from)
            })?
            .bind(py)
            .clone())
    }
}

/// What native discovery read for one model file.
pub(crate) struct ModelFileParts {
    pub(crate) contents: String,
    pub(crate) header_values: Vec<(String, AuthoredValue)>,
    pub(crate) header_column_locations: Vec<(String, LineColumnSpan)>,
    pub(crate) output_column_locations: Vec<(String, LineColumnSpan)>,
    pub(crate) query_sql: String,
    pub(crate) extract_implicit_alias_columns: bool,
}

/// How discovery names a model file: natively, or by Python paths for names Python escapes.
pub(crate) enum DiscoveredPaths {
    /// The `/`-separated relative path and `str(project_dir / relative_path)`.
    Native(String, String),
    /// `(file_path, relative_path)` built by Python.
    Python(Py<PyAny>, Py<PyAny>),
}

/// A model file before its header declarations are attached.
pub(crate) struct PendingModelFile {
    paths: ModelFilePaths,
    parts: ModelFileParts,
}

impl PendingModelFile {
    pub(crate) fn new(
        py: Python<'_>,
        paths: DiscoveredPaths,
        parts: ModelFileParts,
    ) -> PyResult<Self> {
        let paths: ModelFilePaths = match paths {
            DiscoveredPaths::Native(relative_path, file_path) => {
                ModelFilePaths::discovered(&relative_path, file_path)
            }
            DiscoveredPaths::Python(file_path, relative_path) => {
                ModelFilePaths::given(py, file_path, relative_path)?
            }
        };
        Ok(Self { paths, parts })
    }

    /// The model's name, its file stem.
    pub(crate) fn name(&self, py: Python<'_>) -> PyResult<String> {
        match &self.paths.texts {
            Some(texts) => Ok(texts.name.clone()),
            None => field(&self.paths.objects(py)?.0, STEM_ATTRIBUTE)?.extract(),
        }
    }

    /// The relative path as the Python path object the record keeps.
    pub(crate) fn relative_path<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        Ok(self.paths.objects(py)?.1)
    }

    /// The authored header value of `key`.
    pub(crate) fn header_value(&self, key: &str) -> Option<&AuthoredValue> {
        self.parts
            .header_values
            .iter()
            .rev()
            .find(|(name, _)| name == key)
            .map(|(_, value)| value)
    }

    pub(crate) fn finished(self, declarations: (Py<PyTuple>, Py<PyTuple>)) -> NativeModelFile {
        let (enum_declarations, constant_declarations) = declarations;
        let parts: ModelFileParts = self.parts;
        NativeModelFile {
            paths: self.paths,
            contents: parts.contents.into(),
            projected_texts: (PyOnceLock::new(), PyOnceLock::new()),
            header_values: HeaderValues::native(parts.header_values),
            header_column_locations: Locations::from_rows(parts.header_column_locations),
            output_column_locations: Locations::from_rows(parts.output_column_locations),
            query_sql: parts.query_sql.into(),
            enum_declarations,
            constant_declarations,
            extract_implicit_alias_columns: parts.extract_implicit_alias_columns,
        }
    }
}

/// The keyword fields Python constructs a model file record with.
#[derive(FromPyObject)]
#[pyo3(from_item_all)]
pub(crate) struct ModelFileFields {
    pub(crate) file_path: Py<PyAny>,
    pub(crate) relative_path: Py<PyAny>,
    pub(crate) contents: Py<PyString>,
    pub(crate) header_values: Py<PyDict>,
    pub(crate) header_column_locations: Py<PyDict>,
    pub(crate) output_column_locations: Py<PyDict>,
    pub(crate) query_sql: Py<PyString>,
    #[pyo3(default)]
    pub(crate) enum_declarations: Option<Py<PyTuple>>,
    #[pyo3(default)]
    pub(crate) constant_declarations: Option<Py<PyTuple>>,
    #[pyo3(default = true)]
    pub(crate) extract_implicit_alias_columns: bool,
}

/// A new cache holding the dict `projected` already holds, if Python has read it.
fn shared_projection(py: Python<'_>, projected: &PyOnceLock<Py<PyDict>>) -> PyOnceLock<Py<PyDict>> {
    let shared: PyOnceLock<Py<PyDict>> = PyOnceLock::new();
    if let Some(dict) = projected.get(py) {
        let _ = shared.set(py, dict.clone_ref(py));
    }
    shared
}
