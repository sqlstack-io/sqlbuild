//! Public records of discovery.

use crate::discovery::_helpers::project::project_facts::DependencyRow;
use crate::discovery::_helpers::records::model_file_record::FIELD_NAMES;
use crate::discovery::_helpers::records::model_file_record::HeaderValues;
use crate::discovery::_helpers::records::model_file_record::Locations;
use crate::discovery::_helpers::records::model_file_record::ModelFileFields;
use crate::discovery::_helpers::records::model_file_record::ModelFilePaths;
use crate::discovery::_helpers::sources::provider_secrets::{
    INVOCATION_ALLOWS_MISSING_SECRETS, INVOCATION_TAKES_ALLOW_FLAG, INVOCATION_TARGET,
    TARGET_ALLOWED_MISSING_SECRETS,
};
use crate::discovery::main::records::projected_text::projected_text;
use pyo3::FromPyObject;
use pyo3::IntoPyObjectExt;
use pyo3::exceptions::PyTypeError;
use pyo3::exceptions::PyValueError;
use pyo3::prelude::Bound;
use pyo3::prelude::Py;
use pyo3::prelude::PyAny;
use pyo3::prelude::PyAnyMethods;
use pyo3::prelude::PyResult;
use pyo3::prelude::Python;
use pyo3::pyclass;
use pyo3::pymethods;
use pyo3::sync::PyOnceLock;
use pyo3::types::PyDict;
use pyo3::types::PyDictMethods;
use pyo3::types::PyString;
use pyo3::types::PyStringMethods;
use pyo3::types::PyTuple;
use sqlbuild_discovery::models::LineColumnSpan;
use sqlbuild_sqltext::compiler::models::AuthoredValue;
use std::path::PathBuf;
use std::sync::Arc;

/// The names a model file is known by, as Python's `pathlib` spells them.
#[derive(Debug, Clone)]
pub struct ModelFileTexts {
    /// `file_path.stem`, the model's name.
    pub name: String,
    /// `relative_path.as_posix()`.
    pub posix_path: String,
    /// `str(relative_path)`.
    pub relative_path: String,
    /// `str(file_path)`.
    pub file_path: String,
}

/// A discovered SQL model file and its raw contents.
#[pyclass(module = "sqlbuild._native", name = "DiscoveredSqlModelFile", frozen)]
pub struct NativeModelFile {
    pub(crate) paths: ModelFilePaths,
    pub(crate) contents: Arc<str>,
    /// `contents` and `query_sql` as the Python strings readers share, built on first read.
    pub(crate) projected_texts: (PyOnceLock<Py<PyString>>, PyOnceLock<Py<PyString>>),
    pub(crate) header_values: HeaderValues,
    pub(crate) header_column_locations: Locations,
    pub(crate) output_column_locations: Locations,
    pub(crate) query_sql: Arc<str>,
    pub(crate) enum_declarations: Py<PyTuple>,
    pub(crate) constant_declarations: Py<PyTuple>,
    pub(crate) extract_implicit_alias_columns: bool,
}

impl std::fmt::Debug for NativeModelFile {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("NativeModelFile")
            .finish_non_exhaustive()
    }
}

impl NativeModelFile {
    pub fn header_locations<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyDict>> {
        self.header_column_locations
            .dict(&self.paths.objects(py)?.1)
    }

    /// The header's authored values as the Python dict config builds read.
    pub fn header_value_dict<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyDict>> {
        self.header_values.dict(py)
    }

    /// The header columns' authored locations, unless Python constructed this record.
    pub fn header_location_rows(&self) -> &[(String, LineColumnSpan)] {
        &self.header_column_locations.rows
    }

    /// The header's native authored values, unless Python constructed this record.
    pub fn native_header_values(&self) -> Option<&Arc<[(String, AuthoredValue)]>> {
        self.header_values.native.as_ref()
    }

    pub fn declaration_tuples<'py>(
        &self,
        py: Python<'py>,
    ) -> (&Bound<'py, PyTuple>, &Bound<'py, PyTuple>) {
        (
            self.enum_declarations.bind(py),
            self.constant_declarations.bind(py),
        )
    }

    /// The file path as a native path.
    pub fn file_path_buf(&self) -> PathBuf {
        self.paths.native_file_path.clone()
    }

    /// The file's names, or the error Python raises reading a name that is not UTF-8 text.
    pub fn path_texts(&self, py: Python<'_>) -> PyResult<ModelFileTexts> {
        self.paths.texts(py).cloned()
    }

    /// `relative_path.as_posix()`.
    pub fn posix_path(&self, py: Python<'_>) -> PyResult<String> {
        Ok(self.paths.texts(py)?.posix_path.clone())
    }

    /// `str(relative_path)`.
    pub fn relative_text(&self, py: Python<'_>) -> PyResult<String> {
        Ok(self.paths.texts(py)?.relative_path.clone())
    }

    /// `str(relative_path.parent)`, the folder project graphs group the model by.
    pub fn folder(&self, py: Python<'_>) -> PyResult<String> {
        Ok(self
            .paths
            .texts(py)?
            .relative_path
            .rsplit_once(std::path::MAIN_SEPARATOR)
            .map_or_else(|| ".".to_owned(), |(folder, _)| folder.to_owned()))
    }

    /// `(file_path, relative_path)` as Python path objects.
    pub fn path_objects<'py>(
        &self,
        py: Python<'py>,
    ) -> PyResult<(Bound<'py, PyAny>, Bound<'py, PyAny>)> {
        self.paths.objects(py)
    }

    /// The authored query below the header.
    pub fn query_text(&self) -> &Arc<str> {
        &self.query_sql
    }

    /// The whole authored file.
    pub fn contents_text(&self) -> &Arc<str> {
        &self.contents
    }

    pub(crate) fn fields<'py>(&self, py: Python<'py>) -> PyResult<Vec<Bound<'py, PyAny>>> {
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
    pub(crate) fn new(py: Python<'_>, fields: Option<Bound<'_, PyDict>>) -> PyResult<Self> {
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
    pub fn with_text(
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
    pub fn file_path(&self, py: Python<'_>) -> PyResult<Py<PyAny>> {
        Ok(self.paths.objects(py)?.0.unbind())
    }

    #[getter]
    pub fn relative_path(&self, py: Python<'_>) -> PyResult<Py<PyAny>> {
        Ok(self.paths.objects(py)?.1.unbind())
    }

    #[getter]
    pub fn contents<'py>(&self, py: Python<'py>) -> Bound<'py, PyString> {
        projected_text(py, &self.projected_texts.0, &self.contents)
    }

    #[getter]
    pub fn header_values(&self, py: Python<'_>) -> PyResult<Py<PyDict>> {
        Ok(self.header_values.dict(py)?.unbind())
    }

    #[getter]
    pub fn header_column_locations<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyDict>> {
        self.header_locations(py)
    }

    #[getter]
    pub fn output_column_locations<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyDict>> {
        self.output_column_locations
            .dict(&self.paths.objects(py)?.1)
    }

    #[getter]
    pub fn query_sql<'py>(&self, py: Python<'py>) -> Bound<'py, PyString> {
        projected_text(py, &self.projected_texts.1, &self.query_sql)
    }

    #[getter]
    pub fn enum_declarations(&self, py: Python<'_>) -> Py<PyTuple> {
        self.enum_declarations.clone_ref(py)
    }

    #[getter]
    pub fn constant_declarations(&self, py: Python<'_>) -> Py<PyTuple> {
        self.constant_declarations.clone_ref(py)
    }

    #[getter]
    pub fn extract_implicit_alias_columns(&self) -> bool {
        self.extract_implicit_alias_columns
    }

    pub(crate) fn __eq__(&self, py: Python<'_>, other: &Bound<'_, PyAny>) -> PyResult<Py<PyAny>> {
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

    pub(crate) fn __repr__(&self, py: Python<'_>) -> PyResult<String> {
        let fields: Vec<String> = FIELD_NAMES
            .iter()
            .zip(self.fields(py)?)
            .map(|(name, value)| Ok(format!("{name}={}", value.repr()?)))
            .collect::<PyResult<_>>()?;
        Ok(format!("DiscoveredSqlModelFile({})", fields.join(", ")))
    }

    /// The keyword fields `copy` and `pickle` rebuild this record from.
    pub(crate) fn __getnewargs_ex__<'py>(
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

/// Which discovered model files to build, and where failing files are reported.
#[derive(Debug)]
pub struct ModelFileSelection<'py> {
    pub selected_model_names: Option<Bound<'py, PyAny>>,
    pub on_fault: Option<Bound<'py, PyAny>>,
}

/// A decorated Python node as the facade describes it.
#[derive(Debug, FromPyObject)]
#[pyo3(from_item_all)]
pub struct NodeRow {
    pub(crate) kind: String,
    pub(crate) name: String,
    pub(crate) path: String,
    pub(crate) decorator_config: bool,
    pub(crate) depends_on: Vec<DependencyRow>,
}

/// `--allow-missing-secrets`, whether the command takes it, and `--target` for one invocation, restored when the invocation ends.
#[derive(Debug)]
pub struct MissingSecretsScope {
    pub(crate) previous: (bool, bool, Option<String>),
}

impl MissingSecretsScope {
    pub fn enter((allowed, takes_flag): (bool, bool), target: Option<String>) -> Self {
        Self {
            previous: (
                INVOCATION_ALLOWS_MISSING_SECRETS.replace(allowed),
                INVOCATION_TAKES_ALLOW_FLAG.replace(takes_flag),
                INVOCATION_TARGET.replace(target),
            ),
        }
    }
}

impl Drop for MissingSecretsScope {
    fn drop(&mut self) {
        INVOCATION_ALLOWS_MISSING_SECRETS.set(self.previous.0);
        INVOCATION_TAKES_ALLOW_FLAG.set(self.previous.1);
        INVOCATION_TARGET.replace(self.previous.2.take());
    }
}

/// The providers a target's `allow_missing_secrets` lists, for one discovery, restored when it ends.
#[derive(Debug)]
pub struct TargetAllowedSecretsScope {
    pub(crate) previous: (Option<String>, Vec<String>),
}

impl TargetAllowedSecretsScope {
    pub fn enter(target: Option<String>, providers: Vec<String>) -> Self {
        Self {
            previous: TARGET_ALLOWED_MISSING_SECRETS.replace((target, providers)),
        }
    }
}

impl Drop for TargetAllowedSecretsScope {
    fn drop(&mut self) {
        TARGET_ALLOWED_MISSING_SECRETS.replace(std::mem::take(&mut self.previous));
    }
}

/// What discovery does when a provider's required secrets are unset.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SecretPolicy {
    /// Fail with D012: static commands by default.
    Require,
    /// Continue and list the providers on stderr: static commands that opted out.
    Allow,
    /// Continue silently; the command checks the providers it uses before running anything.
    Defer,
}

impl SecretPolicy {
    /// The keyword value that selects the policy.
    pub const fn name(self) -> &'static str {
        match self {
            Self::Require => "require",
            Self::Allow => "allow",
            Self::Defer => "defer",
        }
    }

    pub fn parse(value: &str) -> PyResult<Self> {
        [Self::Require, Self::Allow, Self::Defer]
            .into_iter()
            .find(|policy| policy.name() == value)
            .ok_or_else(|| PyValueError::new_err(format!("unknown secret policy '{value}'")))
    }
}

/// The context manager that makes one snapshot active for a project until it exits.
#[pyclass(module = "sqlbuild._native")]
#[derive(Debug)]
pub struct NativeDirectorySnapshotScope {
    pub project_dir: pyo3::Py<pyo3::PyAny>,
    /// The context-variable token of the snapshot this scope activated, if it activated one.
    pub token: Option<pyo3::Py<pyo3::PyAny>>,
}
