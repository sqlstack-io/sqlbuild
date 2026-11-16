//! The Python models, enums and compile helpers the semantic completion stage reads and builds.

use sqlbuild_host::entries::models::AdapterHost;

use pyo3::prelude::{Bound, PyAny, PyAnyMethods, PyResult, Python};
use pyo3::types::{PyDict, PyDictMethods, PyString};
use sqlbuild_python_base::native_host::main::host_entry::host_entry;
use sqlbuild_python_base::records::main::construct::enum_member::enum_member;
use sqlbuild_python_base::records::main::construct::record_class::record_class;
use sqlbuild_python_base::records::main::fields::field::field as native_field;
use sqlbuild_python_base::records::main::fields::replaced::replaced;
use sqlbuild_python_base::records::types::RecordId;
use sqlbuild_python_project::compiled_views::main::project::project_with::project_with;

const COMPILE_TYPES_MODULE: &str = "sqlbuild.compiler.compile.types";

const UNNEEDED_SQL_ANALYSIS_OPT_OUT_CODE: &str = "P009";
const SQL_ANALYSIS_CONFIG_KEY: &str = "sql_analysis";

/// Imported once per stage run.
pub(crate) struct StageClasses<'py> {
    pub(crate) diagnostic: Bound<'py, PyAny>,
    pub(crate) location: Bound<'py, PyAny>,
    pub(crate) model_test_payload: Bound<'py, PyAny>,
    pub(crate) compile_phase: Bound<'py, PyAny>,
    pub(crate) error: Bound<'py, PyAny>,
    pub(crate) warning: Bound<'py, PyAny>,
    pub(crate) model_resource: Bound<'py, PyAny>,
    pub(crate) source_resource: Bound<'py, PyAny>,
    pub(crate) sql_test_resource: Bound<'py, PyAny>,
    pub(crate) opt_out_code: Bound<'py, PyAny>,
    pub(crate) normalize_type: Bound<'py, PyAny>,
    pub(crate) sql_analysis_key: Bound<'py, PyAny>,
    pub(crate) udf_kind: Bound<'py, PyAny>,
    pub(crate) table_function_kind: Bound<'py, PyAny>,
}

impl<'py> StageClasses<'py> {
    pub(crate) fn new(py: Python<'py>) -> PyResult<Self> {
        let types = py.import(COMPILE_TYPES_MODULE)?;
        let severity = record_class(types.py(), RecordId::DiagnosticSeverity)?;
        let resource = record_class(py, RecordId::CompiledResourceType)?;
        let kinds = record_class(py, RecordId::SqlReferenceKind)?;
        Ok(Self {
            diagnostic: record_class(py, RecordId::CompilerDiagnostic)?,
            location: record_class(py, RecordId::SourceLocation)?,
            model_test_payload: record_class(py, RecordId::CompiledModelSqlTestPayload)?,
            compile_phase: enum_member(types.py(), RecordId::DiagnosticPhase, "COMPILE")?,
            error: native_field(&severity, "ERROR")?,
            warning: native_field(&severity, "WARNING")?,
            model_resource: native_field(&resource, "MODEL")?,
            source_resource: native_field(&resource, "SOURCE")?,
            sql_test_resource: native_field(&resource, "SQL_TEST")?,
            opt_out_code: PyString::new(py, UNNEEDED_SQL_ANALYSIS_OPT_OUT_CODE).into_any(),
            normalize_type: host_entry(py, AdapterHost::NormalizeType)?,
            sql_analysis_key: PyString::new(py, SQL_ANALYSIS_CONFIG_KEY).into_any(),
            udf_kind: native_field(&kinds, "UDF")?,
            table_function_kind: native_field(&kinds, "TABLE_FUNCTION")?,
        })
    }

    pub(crate) fn py(&self) -> Python<'py> {
        self.diagnostic.py()
    }

    /// `callable(**pairs)`.
    pub(crate) fn call(
        &self,
        callable: &Bound<'py, PyAny>,
        pairs: &[(&str, &Bound<'py, PyAny>)],
    ) -> PyResult<Bound<'py, PyAny>> {
        let kwargs: Bound<'py, PyDict> = PyDict::new(self.py());
        for (name, value) in pairs {
            kwargs.set_item(*name, *value)?;
        }
        callable.call((), Some(&kwargs))
    }

    /// `value` with `pairs` replaced.
    pub(crate) fn replaced(
        &self,
        value: &Bound<'py, PyAny>,
        pairs: &[(&str, &Bound<'py, PyAny>)],
    ) -> PyResult<Bound<'py, PyAny>> {
        let owned: Vec<(&str, Bound<'py, PyAny>)> = pairs
            .iter()
            .map(|(name, changed)| (*name, (*changed).clone()))
            .collect();
        replaced(value, &owned)
    }

    /// `project_with(project=project, **pairs)`.
    pub(crate) fn project_with(
        &self,
        project: &Bound<'py, PyAny>,
        pairs: &[(&str, &Bound<'py, PyAny>)],
    ) -> PyResult<Bound<'py, PyAny>> {
        let kwargs: Bound<'py, PyDict> = PyDict::new(self.py());
        for (name, value) in pairs {
            kwargs.set_item(*name, *value)?;
        }
        project_with(project, Some(kwargs))
    }
}
