//! The Python model validators, run natively up to the first error they raise.

use sqlbuild_python_base::positional_record;
use std::collections::HashSet;

use pyo3::prelude::{Bound, Py, PyAny, PyAnyMethods, PyModule, PyModuleMethods, PyResult, Python};
use pyo3::types::{PyDict, PyDictMethods};
use pyo3::{IntoPyObject, pyclass, pymethods};
use sqlbuild_attachments::functions::models::KnownResources;
use sqlbuild_model_config::model_validation::main::validate_model_config::validate_model_config;
use sqlbuild_model_config::model_validation::main::validate_model_references::validate_model_references;
use sqlbuild_model_config::model_validation::models::{
    ModelReference, ModelValidationFacts, ProjectValidationFacts, ValidationStop,
};

use crate::bindings::_helpers::discovery::project_facts::{column_name, each, stem};
use crate::bindings::_helpers::model_config::authored_nodes::PyNode;
use crate::bindings::_helpers::model_config::config_errors::native_config_error;
use sqlbuild_python_base::boundary::main::compiler_guard::compiler_guard;
use sqlbuild_python_base::records::main::fields::field::field as native_field;
use sqlbuild_python_base::records::main::fields::mapping_get::mapping_get;

const FILE_PATH_ATTRIBUTE: &str = "file_path";
const RETURNS_KEY: &str = "returns";
/// The only key of a table function's `returns` mapping.
const TABLE_RETURNS_KEY: &str = "table";

positional_record! {
    extract;
    /// Model, seed, source, function and table function names.
    struct ResourceNames {
        models: HashSet<String>,
        seeds: HashSet<String>,
        sources: HashSet<String>,
        functions: HashSet<String>,
        table_functions: HashSet<String>,
    }
}

positional_record! {
    extract;
    /// References `(kind, name, rejected)`, schema columns, unmanaged retention and table type.
    struct ModelFacts {
        references: Vec<(String, String, bool)>,
        declared_columns: Option<Vec<String>>,
        retention_unmanaged: bool,
        table_type_declared: bool,
    }
}

/// The project facts every model's validation reads, captured once per compile.
#[pyclass(module = "sqlbuild._native", frozen)]
pub(crate) struct NativeModelValidator {
    project: ProjectValidationFacts,
}

impl NativeModelValidator {
    /// The project's discovered resource names, which every reference resolves against.
    pub(crate) fn known_resources(&self) -> KnownResources<'_> {
        KnownResources {
            models: &self.project.models,
            seeds: &self.project.seeds,
            sources: &self.project.sources,
            functions: &self.project.functions,
            table_functions: &self.project.table_functions,
        }
    }
}

/// Model, seed, source, function and table function names discovered in `inputs`.
fn discovered_names(inputs: &Bound<'_, PyAny>) -> PyResult<ResourceNames> {
    let model_stem = |file: &Bound<'_, PyAny>| stem(file, FILE_PATH_ATTRIBUTE);
    let mut models: HashSet<String> = each(inputs, "model_files", model_stem)?
        .into_iter()
        .collect();
    if let Some(injection) =
        sqlbuild_python_base::compile_context::main::delta::injection::injection()
    {
        models.extend(injection.model_names.iter().cloned());
    }
    let mut seeds: HashSet<String> = HashSet::new();
    for schema in native_field(inputs, "schema_files")?.try_iter()? {
        seeds.extend(each(&schema?, "seed_entries", column_name)?);
    }
    let mut sources: HashSet<String> = HashSet::new();
    for file in native_field(inputs, "source_files")?.try_iter()? {
        sources.extend(each(&file?, "source_entries", column_name)?);
    }
    let mut functions: HashSet<String> = HashSet::new();
    let mut table_functions: HashSet<String> = HashSet::new();
    for file in native_field(inputs, "sql_function_files")?.try_iter()? {
        let file: Bound<'_, PyAny> = file?;
        let name: String = model_stem(&file)?;
        if returns_table(&file)? {
            table_functions.insert(name.clone());
        }
        functions.insert(name);
    }
    functions.extend(each(inputs, "python_function_files", model_stem)?);
    Ok(ResourceNames {
        models,
        seeds,
        sources,
        functions,
        table_functions,
    })
}

/// Whether a SQL function's header `returns` is a mapping whose only key is `table`.
fn returns_table(file: &Bound<'_, PyAny>) -> PyResult<bool> {
    let headers: Bound<'_, PyAny> = native_field(file, "header_values")?;
    let returns: Bound<'_, PyAny> = mapping_get(&headers, RETURNS_KEY)?;
    let Ok(returns) = returns.downcast::<PyDict>() else {
        return Ok(false);
    };
    let keys: Vec<Bound<'_, PyAny>> = returns.keys().into_iter().collect();
    Ok(keys.len() == 1 && keys[0].eq(TABLE_RETURNS_KEY)?)
}

#[pymethods]
impl NativeModelValidator {
    /// The validator for the resources and materializations `inputs` discovered.
    #[staticmethod]
    pub(crate) fn from_inputs(
        inputs: Bound<'_, PyAny>,
        microbatch_concurrency: bool,
    ) -> PyResult<Self> {
        let names: ResourceNames = discovered_names(&inputs)?;
        let custom_materializations: HashSet<String> =
            each(&inputs, "materialization_files", |file| {
                native_field(file, "name")?.extract()
            })?
            .into_iter()
            .collect();
        Self::new(names, custom_materializations, microbatch_concurrency)
    }

    /// The discovered model names references resolve against.
    fn model_names(&self) -> HashSet<String> {
        self.project.models.clone()
    }

    #[new]
    fn new(
        names: ResourceNames,
        custom_materializations: HashSet<String>,
        microbatch_concurrency: bool,
    ) -> PyResult<Self> {
        let ResourceNames {
            models,
            seeds,
            sources,
            functions,
            table_functions,
        } = names;
        Ok(Self {
            project: ProjectValidationFacts {
                custom_materializations,
                microbatch_concurrency,
                models,
                seeds,
                sources,
                functions,
                table_functions,
            },
        })
    }

    /// `None` when accepted, the first error, or the first externally rejected reference.
    fn validate(
        &self,
        py: Python<'_>,
        values: Bound<'_, PyDict>,
        model: (String, String, String),
        facts: ModelFacts,
    ) -> PyResult<Py<PyAny>> {
        compiler_guard(|| {
            let (model_name, query_sql, relative_path) = model;
            let ModelFacts {
                references,
                declared_columns,
                retention_unmanaged,
                table_type_declared,
            } = facts;
            let references: Vec<ModelReference> = references
                .into_iter()
                .map(|(kind, name, externally_rejected)| ModelReference {
                    kind,
                    name,
                    externally_rejected,
                })
                .collect();
            let entries: Vec<(PyNode<'_>, PyNode<'_>)> = values
                .iter()
                .map(|(key, value)| (PyNode(key), PyNode(value)))
                .collect();
            let facts = ModelValidationFacts {
                model_name: &model_name,
                relative_path: &relative_path,
                references: &references,
                declared_columns: declared_columns.as_deref(),
                query_sql: &query_sql,
                retention_unmanaged,
                table_type_declared,
            };
            match validate_model_config(entries, &self.project, &facts) {
                Ok(()) => Ok(py.None()),
                Err(stop) => stop_object(py, stop),
            }
        })
    }

    /// Check references alone: `None`, the first error, or a rejected dbt reference's index.
    fn references(
        &self,
        py: Python<'_>,
        model: (String, String),
        references: Vec<(String, String, bool)>,
    ) -> PyResult<Py<PyAny>> {
        compiler_guard(|| {
            let (model_name, relative_path) = model;
            let references: Vec<ModelReference> = references
                .into_iter()
                .map(|(kind, name, externally_rejected)| ModelReference {
                    kind,
                    name,
                    externally_rejected,
                })
                .collect();
            let facts = ModelValidationFacts {
                model_name: &model_name,
                relative_path: &relative_path,
                references: &references,
                declared_columns: None,
                query_sql: "",
                retention_unmanaged: false,
                table_type_declared: false,
            };
            match validate_model_references(&self.project, &facts) {
                Ok(()) => Ok(py.None()),
                Err(stop) => stop_object(py, stop),
            }
        })
    }
}

fn stop_object(py: Python<'_>, stop: ValidationStop) -> PyResult<Py<PyAny>> {
    match stop.into_error() {
        Err(index) => Ok(index.into_pyobject(py)?.into_any().unbind()),
        Ok(error) => Ok(native_config_error(py, error)?.into_any()),
    }
}

pub(crate) fn register(module: &Bound<'_, PyModule>) -> PyResult<()> {
    module.add_class::<NativeModelValidator>()?;
    Ok(())
}
