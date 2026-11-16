//! A discovered SQL model file as a native record whose Python attributes are built on demand.

use std::path::PathBuf;
use std::sync::Arc;

use crate::bindings::_helpers::sqltext::authored_values::map_to_python;
use pyo3::exceptions::{PyRuntimeError, PyTypeError};
use pyo3::prelude::{Bound, Py, PyAny, PyAnyMethods, PyResult, Python};
use pyo3::sync::PyOnceLock;
use pyo3::types::{PyDict, PyDictMethods, PyString, PyStringMethods, PyTuple};
use pyo3::{FromPyObject, IntoPyObject, IntoPyObjectExt, pyclass, pymethods};
use sqlbuild_discovery::model_files::main::model_file_stem::model_file_stem;
use sqlbuild_discovery::models::LineColumnSpan;
use sqlbuild_python_base::records::main::construct::new_record::new_record;
use sqlbuild_python_base::records::main::fields::field::field;
use sqlbuild_python_base::records::types::RecordId;
use sqlbuild_sqltext::compiler::models::AuthoredValue;

const FIELD_NAMES: [&str; 10] = [
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
struct Locations {
    rows: Vec<(String, LineColumnSpan)>,
    projected: PyOnceLock<Py<PyDict>>,
}

impl Locations {
    fn from_rows(rows: Vec<(String, LineColumnSpan)>) -> Self {
        Self {
            rows,
            projected: PyOnceLock::new(),
        }
    }

    /// These locations for another record: the rows, and the dict once Python has read it.
    fn shared(&self, py: Python<'_>) -> Self {
        let projected: PyOnceLock<Py<PyDict>> = PyOnceLock::new();
        if let Some(dict) = self.projected.get(py) {
            let _ = projected.set(py, dict.clone_ref(py));
        }
        Self {
            rows: self.rows.clone(),
            projected,
        }
    }

    fn from_dict(py: Python<'_>, dict: Py<PyDict>) -> Self {
        let projected: PyOnceLock<Py<PyDict>> = PyOnceLock::new();
        let _ = projected.set(py, dict);
        Self {
            rows: Vec::new(),
            projected,
        }
    }

    /// Python's `{name: SourceLocation(...)}`: a repeated name keeps its first position and last value.
    fn dict<'py>(&self, path: &Bound<'py, PyAny>) -> PyResult<Bound<'py, PyDict>> {
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

/// The names a model file is known by, as Python's `pathlib` spells them.
#[derive(Clone)]
pub(crate) struct ModelFileTexts {
    /// `file_path.stem`, the model's name.
    pub(crate) name: String,
    /// `relative_path.as_posix()`.
    pub(crate) posix_path: String,
    /// `str(relative_path)`.
    pub(crate) relative_path: String,
    /// `str(file_path)`.
    pub(crate) file_path: String,
}

/// A model file's paths: native text, with the Python path objects built on first read.
struct ModelFilePaths {
    native_file_path: PathBuf,
    /// `None` when a path is not text Python can encode, so readers raise as Python does.
    texts: Option<ModelFileTexts>,
    /// `(file_path, relative_path)` as Python path objects.
    objects: PyOnceLock<(Py<PyAny>, Py<PyAny>)>,
    /// The relative path the objects are built from, unless they were given.
    relative: Option<PathBuf>,
}

impl ModelFilePaths {
    /// Paths given as Python objects; their text is read once, here.
    fn given(py: Python<'_>, file_path: Py<PyAny>, relative_path: Py<PyAny>) -> PyResult<Self> {
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
    fn discovered(relative_path: &str, file_path: String) -> Self {
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

    fn shared(&self, py: Python<'_>) -> Self {
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
    fn objects<'py>(&self, py: Python<'py>) -> PyResult<(Bound<'py, PyAny>, Bound<'py, PyAny>)> {
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
    fn texts(&self, py: Python<'_>) -> PyResult<&ModelFileTexts> {
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

/// `text` as the Python string `cache` shares with every reader, built on first read.
pub(crate) fn projected_text<'py>(
    py: Python<'py>,
    cache: &PyOnceLock<Py<PyString>>,
    text: &str,
) -> Bound<'py, PyString> {
    cache
        .get_or_init(py, || PyString::new(py, text).unbind())
        .bind(py)
        .clone()
}

/// A model header's authored values: native entries, with the Python dict built on first read.
struct HeaderValues {
    native: Option<Arc<[(String, AuthoredValue)]>>,
    projected: PyOnceLock<Py<PyDict>>,
}

impl HeaderValues {
    fn native(values: Vec<(String, AuthoredValue)>) -> Self {
        Self {
            native: Some(values.into()),
            projected: PyOnceLock::new(),
        }
    }

    fn given(py: Python<'_>, values: Py<PyDict>) -> Self {
        let projected: PyOnceLock<Py<PyDict>> = PyOnceLock::new();
        let _ = projected.set(py, values);
        Self {
            native: None,
            projected,
        }
    }

    fn shared(&self, py: Python<'_>) -> Self {
        let projected: PyOnceLock<Py<PyDict>> = PyOnceLock::new();
        if let Some(dict) = self.projected.get(py) {
            let _ = projected.set(py, dict.clone_ref(py));
        }
        Self {
            native: self.native.clone(),
            projected,
        }
    }

    fn dict<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyDict>> {
        Ok(self
            .projected
            .get_or_try_init(py, || {
                let values: &[(String, AuthoredValue)] = self.native.as_deref().unwrap_or_default();
                map_to_python(py, values)?
                    .into_bound(py)
                    .downcast_into::<PyDict>()
                    .map(Bound::unbind)
                    .map_err(pyo3::PyErr::from)
            })?
            .bind(py)
            .clone())
    }
}

/// A discovered SQL model file and its raw contents.
#[pyclass(module = "sqlbuild._native", name = "DiscoveredSqlModelFile", frozen)]
pub(crate) struct NativeModelFile {
    paths: ModelFilePaths,
    contents: Arc<str>,
    /// `contents` and `query_sql` as the Python strings readers share, built on first read.
    projected_texts: (PyOnceLock<Py<PyString>>, PyOnceLock<Py<PyString>>),
    header_values: HeaderValues,
    header_column_locations: Locations,
    output_column_locations: Locations,
    query_sql: Arc<str>,
    enum_declarations: Py<PyTuple>,
    constant_declarations: Py<PyTuple>,
    extract_implicit_alias_columns: bool,
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
struct ModelFileFields {
    file_path: Py<PyAny>,
    relative_path: Py<PyAny>,
    contents: Py<PyString>,
    header_values: Py<PyDict>,
    header_column_locations: Py<PyDict>,
    output_column_locations: Py<PyDict>,
    query_sql: Py<PyString>,
    #[pyo3(default)]
    enum_declarations: Option<Py<PyTuple>>,
    #[pyo3(default)]
    constant_declarations: Option<Py<PyTuple>>,
    #[pyo3(default = true)]
    extract_implicit_alias_columns: bool,
}

impl NativeModelFile {
    pub(crate) fn header_locations<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyDict>> {
        self.header_column_locations
            .dict(&self.paths.objects(py)?.1)
    }

    /// The header's authored values as the Python dict config builds read.
    pub(crate) fn header_value_dict<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyDict>> {
        self.header_values.dict(py)
    }

    /// The header columns' authored locations, unless Python constructed this record.
    pub(crate) fn header_location_rows(&self) -> &[(String, LineColumnSpan)] {
        &self.header_column_locations.rows
    }

    /// The header's native authored values, unless Python constructed this record.
    pub(crate) fn native_header_values(&self) -> Option<&Arc<[(String, AuthoredValue)]>> {
        self.header_values.native.as_ref()
    }

    pub(crate) fn declaration_tuples<'py>(
        &self,
        py: Python<'py>,
    ) -> (&Bound<'py, PyTuple>, &Bound<'py, PyTuple>) {
        (
            self.enum_declarations.bind(py),
            self.constant_declarations.bind(py),
        )
    }

    /// The file path as a native path.
    pub(crate) fn file_path_buf(&self) -> PathBuf {
        self.paths.native_file_path.clone()
    }

    /// The file's names, or the error Python raises reading a name that is not UTF-8 text.
    pub(crate) fn path_texts(&self, py: Python<'_>) -> PyResult<ModelFileTexts> {
        self.paths.texts(py).cloned()
    }

    /// `relative_path.as_posix()`.
    pub(crate) fn posix_path(&self, py: Python<'_>) -> PyResult<String> {
        Ok(self.paths.texts(py)?.posix_path.clone())
    }

    /// `str(relative_path)`.
    pub(crate) fn relative_text(&self, py: Python<'_>) -> PyResult<String> {
        Ok(self.paths.texts(py)?.relative_path.clone())
    }

    /// `str(relative_path.parent)`, the folder project graphs group the model by.
    pub(crate) fn folder(&self, py: Python<'_>) -> PyResult<String> {
        Ok(self
            .paths
            .texts(py)?
            .relative_path
            .rsplit_once(std::path::MAIN_SEPARATOR)
            .map_or_else(|| ".".to_owned(), |(folder, _)| folder.to_owned()))
    }

    /// `(file_path, relative_path)` as Python path objects.
    pub(crate) fn path_objects<'py>(
        &self,
        py: Python<'py>,
    ) -> PyResult<(Bound<'py, PyAny>, Bound<'py, PyAny>)> {
        self.paths.objects(py)
    }

    /// The authored query below the header.
    pub(crate) fn query_text(&self) -> &Arc<str> {
        &self.query_sql
    }

    /// The whole authored file.
    pub(crate) fn contents_text(&self) -> &Arc<str> {
        &self.contents
    }

    fn fields<'py>(&self, py: Python<'py>) -> PyResult<Vec<Bound<'py, PyAny>>> {
        let (file_path, relative_path) = self.paths.objects(py)?;
        Ok(vec![
            file_path,
            relative_path.clone(),
            self.contents(py).into_any(),
            self.header_values.dict(py)?.into_any(),
            self.header_column_locations
                .dict(&relative_path)?
                .into_any(),
            self.output_column_locations
                .dict(&relative_path)?
                .into_any(),
            self.query_sql(py).into_any(),
            self.enum_declarations.bind(py).clone().into_any(),
            self.constant_declarations.bind(py).clone().into_any(),
            pyo3::types::PyBool::new(py, self.extract_implicit_alias_columns)
                .to_owned()
                .into_any(),
        ])
    }
}

#[pymethods]
impl NativeModelFile {
    #[new]
    #[pyo3(signature = (**fields))]
    fn new(py: Python<'_>, fields: Option<Bound<'_, PyDict>>) -> PyResult<Self> {
        let Some(fields) = fields else {
            return Err(PyTypeError::new_err(
                "DiscoveredSqlModelFile requires its fields",
            ));
        };
        for key in fields.keys() {
            let key: String = key.extract()?;
            if !FIELD_NAMES.contains(&key.as_str()) {
                return Err(PyTypeError::new_err(format!(
                    "DiscoveredSqlModelFile got an unexpected keyword argument '{key}'"
                )));
            }
        }
        let fields: ModelFileFields = fields.extract()?;
        Ok(Self {
            paths: ModelFilePaths::given(py, fields.file_path, fields.relative_path)?,
            contents: fields.contents.bind(py).to_str()?.into(),
            projected_texts: (PyOnceLock::new(), PyOnceLock::new()),
            header_values: HeaderValues::given(py, fields.header_values),
            header_column_locations: Locations::from_dict(py, fields.header_column_locations),
            output_column_locations: Locations::from_dict(py, fields.output_column_locations),
            query_sql: fields.query_sql.bind(py).to_str()?.into(),
            enum_declarations: fields
                .enum_declarations
                .unwrap_or_else(|| PyTuple::empty(py).unbind()),
            constant_declarations: fields
                .constant_declarations
                .unwrap_or_else(|| PyTuple::empty(py).unbind()),
            extract_implicit_alias_columns: fields.extract_implicit_alias_columns,
        })
    }

    /// This file with edited contents and query, as `dataclasses.replace` would make it.
    pub(crate) fn with_text(
        &self,
        py: Python<'_>,
        contents: Py<PyString>,
        query_sql: Py<PyString>,
    ) -> PyResult<Self> {
        Ok(Self {
            paths: self.paths.shared(py),
            contents: contents.bind(py).to_str()?.into(),
            projected_texts: (PyOnceLock::new(), PyOnceLock::new()),
            header_values: self.header_values.shared(py),
            header_column_locations: self.header_column_locations.shared(py),
            output_column_locations: self.output_column_locations.shared(py),
            query_sql: query_sql.bind(py).to_str()?.into(),
            enum_declarations: self.enum_declarations.clone_ref(py),
            constant_declarations: self.constant_declarations.clone_ref(py),
            extract_implicit_alias_columns: self.extract_implicit_alias_columns,
        })
    }

    #[getter]
    pub(crate) fn file_path(&self, py: Python<'_>) -> PyResult<Py<PyAny>> {
        Ok(self.paths.objects(py)?.0.unbind())
    }

    #[getter]
    pub(crate) fn relative_path(&self, py: Python<'_>) -> PyResult<Py<PyAny>> {
        Ok(self.paths.objects(py)?.1.unbind())
    }

    #[getter]
    pub(crate) fn contents<'py>(&self, py: Python<'py>) -> Bound<'py, PyString> {
        projected_text(py, &self.projected_texts.0, &self.contents)
    }

    #[getter]
    pub(crate) fn header_values(&self, py: Python<'_>) -> PyResult<Py<PyDict>> {
        Ok(self.header_values.dict(py)?.unbind())
    }

    #[getter]
    pub(crate) fn header_column_locations<'py>(
        &self,
        py: Python<'py>,
    ) -> PyResult<Bound<'py, PyDict>> {
        self.header_locations(py)
    }

    #[getter]
    pub(crate) fn output_column_locations<'py>(
        &self,
        py: Python<'py>,
    ) -> PyResult<Bound<'py, PyDict>> {
        self.output_column_locations
            .dict(&self.paths.objects(py)?.1)
    }

    #[getter]
    pub(crate) fn query_sql<'py>(&self, py: Python<'py>) -> Bound<'py, PyString> {
        projected_text(py, &self.projected_texts.1, &self.query_sql)
    }

    #[getter]
    pub(crate) fn enum_declarations(&self, py: Python<'_>) -> Py<PyTuple> {
        self.enum_declarations.clone_ref(py)
    }

    #[getter]
    pub(crate) fn constant_declarations(&self, py: Python<'_>) -> Py<PyTuple> {
        self.constant_declarations.clone_ref(py)
    }

    #[getter]
    pub(crate) fn extract_implicit_alias_columns(&self) -> bool {
        self.extract_implicit_alias_columns
    }

    fn __eq__(&self, py: Python<'_>, other: &Bound<'_, PyAny>) -> PyResult<Py<PyAny>> {
        let Ok(other) = other.downcast::<NativeModelFile>() else {
            return Ok(py.NotImplemented());
        };
        let other: &NativeModelFile = other.get();
        for (left, right) in self.fields(py)?.iter().zip(other.fields(py)?) {
            if !left.eq(right)? {
                return false.into_py_any(py);
            }
        }
        true.into_py_any(py)
    }

    fn __repr__(&self, py: Python<'_>) -> PyResult<String> {
        let fields: Vec<String> = FIELD_NAMES
            .iter()
            .zip(self.fields(py)?)
            .map(|(name, value)| Ok(format!("{name}={}", value.repr()?)))
            .collect::<PyResult<_>>()?;
        Ok(format!("DiscoveredSqlModelFile({})", fields.join(", ")))
    }

    /// The keyword fields `copy` and `pickle` rebuild this record from.
    fn __getnewargs_ex__<'py>(
        slf: &Bound<'py, Self>,
    ) -> PyResult<(Bound<'py, PyTuple>, Bound<'py, PyDict>)> {
        let py: Python<'py> = slf.py();
        let keywords: Bound<'py, PyDict> = PyDict::new(py);
        for (name, value) in FIELD_NAMES.iter().zip(slf.get().fields(py)?) {
            keywords.set_item(name, value)?;
        }
        Ok((PyTuple::empty(py), keywords))
    }
}
