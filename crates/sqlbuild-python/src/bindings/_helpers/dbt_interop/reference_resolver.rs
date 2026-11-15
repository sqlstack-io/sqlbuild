//! The compile-time external reference resolver backed by a dbt manifest index.

use pyo3::prelude::{
    Bound, Py, PyAny, PyAnyMethods, PyErr, PyModule, PyModuleMethods, PyResult, Python,
};
use pyo3::types::{PyDict, PySet, PySetMethods, PyString, PyTuple, PyTupleMethods};
use pyo3::{pyclass, pyfunction, pymethods, wrap_pyfunction};
use sqlbuild_host::entries::models::ExecutorHost;
use std::collections::HashSet;

use crate::bindings::_helpers::dbt_interop::manifest::build_compile_dbt_manifest_index;
use crate::bindings::_helpers::dbt_interop::manifest::resolve_dbt_reference_relation;
use crate::bindings::_helpers::dbt_interop::manifest::validate_compile_dbt_model_names;
use crate::bindings::_helpers::dbt_interop::manifest::validate_compile_dbt_model_reference;
use sqlbuild_python_base::boundary::main::call_with::call_with;
use sqlbuild_python_base::boundary::main::imported::imported;
use sqlbuild_python_base::boundary::main::keywords::keywords;
use sqlbuild_python_base::boundary::main::within::within;
use sqlbuild_python_base::boundary::models::KeywordArguments;
use sqlbuild_python_base::native_host::main::host_entry::host_entry;
use sqlbuild_python_base::records::main::construct::error_class::error_class;
use sqlbuild_python_base::records::main::construct::new_record_with::new_record_with;
use sqlbuild_python_base::records::main::fields::field::field as native_field;
use sqlbuild_python_base::records::types::ErrorId;
use sqlbuild_python_base::records::types::RecordId;

const VALIDATE_OPTIONS: (&[&str], &[&str]) = (
    &[
        "ref_kind",
        "ref_name",
        "ref_package",
        "owner_relative_sql_path",
    ],
    &[],
);
const MANIFEST_FILENAME: &str = "manifest.json";
const MODEL_CONFLICT_CODE: &str = "C218";
const SOURCE_CONFLICT_CODE: &str = "C216";
const SEED_CONFLICT_CODE: &str = "C217";

fn compile_input_error(py: Python<'_>, message: String, code: &str) -> PyErr {
    let created = || -> PyResult<PyErr> {
        let error: Bound<'_, PyAny> = error_class(py, ErrorId::CompileInput)?.call(
            (message,),
            Some(&keywords(
                py,
                &[("code", PyString::new(py, code).into_any())],
            )?),
        )?;
        Ok(PyErr::from_value(error))
    };
    created().unwrap_or_else(|failure| failure)
}

fn text(value: &Bound<'_, PyAny>, name: &str) -> PyResult<String> {
    native_field(value, name)?.extract()
}

/// External reference resolver backed by a dbt manifest index.
#[pyclass(module = "sqlbuild._native", frozen)]
pub(crate) struct DbtCompileReferenceResolver {
    dbt_manifest: Option<Py<PyAny>>,
}

impl DbtCompileReferenceResolver {
    pub(crate) fn new(dbt_manifest: Option<Py<PyAny>>) -> Self {
        Self { dbt_manifest }
    }

    fn manifest<'py>(&self, py: Python<'py>) -> Option<Bound<'py, PyAny>> {
        self.dbt_manifest
            .as_ref()
            .map(|manifest| manifest.bind(py).clone())
            .filter(|manifest| !manifest.is_none())
    }

    fn manifest_or_none<'py>(&self, py: Python<'py>) -> Bound<'py, PyAny> {
        self.manifest(py)
            .unwrap_or_else(|| py.None().into_bound(py))
    }
}

/// Bare `(source_name, name)` or seed names that occur more than once, in a first pass.
fn duplicates(keys: Vec<String>) -> HashSet<String> {
    let mut seen: HashSet<String> = HashSet::new();
    let mut duplicate: HashSet<String> = HashSet::new();
    for key in keys {
        if !seen.insert(key.clone()) {
            duplicate.insert(key);
        }
    }
    duplicate
}

#[pymethods]
impl DbtCompileReferenceResolver {
    #[new]
    #[pyo3(signature = (*, dbt_manifest))]
    fn py_new(dbt_manifest: Option<Py<PyAny>>) -> Self {
        Self::new(dbt_manifest)
    }

    #[getter(_dbt_manifest)]
    pub(crate) fn dbt_manifest<'py>(&self, py: Python<'py>) -> Bound<'py, PyAny> {
        self.dbt_manifest.as_ref().map_or_else(
            || py.None().into_bound(py),
            |manifest| manifest.bind(py).clone(),
        )
    }

    #[pyo3(signature = (*, known_model_names))]
    fn validate_model_names(
        &self,
        py: Python<'_>,
        known_model_names: Bound<'_, PyAny>,
    ) -> Result<(), PyErr> {
        validate_compile_dbt_model_names(py, known_model_names, self.manifest_or_none(py))
            .and_then(|value| pyo3::IntoPyObjectExt::into_bound_py_any(value, py))?;
        Ok(())
    }

    #[pyo3(signature = (*, known_model_names))]
    fn extend_sql_test_model_names<'py>(
        &self,
        py: Python<'py>,
        known_model_names: Bound<'py, PyAny>,
    ) -> PyResult<Bound<'py, PySet>> {
        let names: Bound<'py, PySet> = PySet::empty(py)?;
        let Some(manifest) = self.manifest(py) else {
            return Ok(names);
        };
        for item in native_field(&manifest, "models_by_name")?
            .call_method0("items")?
            .try_iter()?
        {
            let (model_name, matches): (Bound<'py, PyAny>, Bound<'py, PyAny>) = item?.extract()?;
            if matches.len()? != 1 {
                continue;
            }
            if known_model_names.contains(&model_name)? {
                return Err(compile_input_error(
                    py,
                    format!(
                        "dbt model fixture '__ref__{}' conflicts with a SQLBuild model",
                        model_name.str()?
                    ),
                    MODEL_CONFLICT_CODE,
                ));
            }
            names.add(model_name)?;
        }
        for key in native_field(&manifest, "models_by_package_and_name")?.try_iter()? {
            let key: Bound<'py, PyTuple> = key?.cast_into()?;
            let qualified: String =
                format!("{}__{}", key.get_item(0)?.str()?, key.get_item(1)?.str()?);
            if known_model_names.contains(&qualified)? {
                return Err(compile_input_error(
                    py,
                    format!(
                        "dbt model fixture '__ref__{qualified}' conflicts with a SQLBuild model"
                    ),
                    MODEL_CONFLICT_CODE,
                ));
            }
            names.add(qualified)?;
        }
        Ok(names)
    }

    #[pyo3(signature = (*, known_source_names))]
    fn extend_sql_test_source_names<'py>(
        &self,
        py: Python<'py>,
        known_source_names: Bound<'py, PyAny>,
    ) -> PyResult<Bound<'py, PySet>> {
        let names: Bound<'py, PySet> = PySet::empty(py)?;
        let Some(manifest) = self.manifest(py) else {
            return Ok(names);
        };
        let sources: Vec<Bound<'py, PyAny>> = native_field(&manifest, "sources_by_unique_id")?
            .call_method0("values")?
            .try_iter()?
            .collect::<PyResult<_>>()?;
        let mut keys: Vec<String> = Vec::with_capacity(sources.len());
        for source in &sources {
            keys.push(format!(
                "{}\0{}",
                text(source, "source_name")?,
                text(source, "name")?
            ));
        }
        let duplicate: HashSet<String> = duplicates(keys.clone());
        for (source, key) in sources.iter().zip(keys) {
            let source_name: String = text(source, "source_name")?;
            let name: String = text(source, "name")?;
            let bare: String = format!("{source_name}__{name}");
            if known_source_names.contains(&bare)? {
                return Err(compile_input_error(
                    py,
                    format!(
                        "dbt source fixture '__source__{bare}' conflicts with a SQLBuild source"
                    ),
                    SOURCE_CONFLICT_CODE,
                ));
            }
            if !duplicate.contains(&key) {
                names.add(bare)?;
            }
            names.add(format!(
                "{}__{source_name}__{name}",
                text(source, "package_name")?
            ))?;
        }
        Ok(names)
    }

    #[pyo3(signature = (*, known_seed_names))]
    fn extend_sql_test_seed_names<'py>(
        &self,
        py: Python<'py>,
        known_seed_names: Bound<'py, PyAny>,
    ) -> PyResult<Bound<'py, PySet>> {
        let names: Bound<'py, PySet> = PySet::empty(py)?;
        let Some(manifest) = self.manifest(py) else {
            return Ok(names);
        };
        let seeds: Vec<Bound<'py, PyAny>> = native_field(&manifest, "seeds_by_unique_id")?
            .call_method0("values")?
            .try_iter()?
            .collect::<PyResult<_>>()?;
        let mut seed_names: Vec<String> = Vec::with_capacity(seeds.len());
        for seed in &seeds {
            seed_names.push(text(seed, "name")?);
        }
        let duplicate: HashSet<String> = duplicates(seed_names.clone());
        for (seed, name) in seeds.iter().zip(seed_names) {
            if known_seed_names.contains(&name)? {
                return Err(compile_input_error(
                    py,
                    format!("dbt seed fixture '__seed__{name}' conflicts with a SQLBuild seed"),
                    SEED_CONFLICT_CODE,
                ));
            }
            if !duplicate.contains(&name) {
                names.add(&name)?;
            }
            names.add(format!("{}__{name}", text(seed, "package_name")?))?;
        }
        Ok(names)
    }

    #[pyo3(signature = (**options))]
    fn validate_reference(
        &self,
        py: Python<'_>,
        options: Option<Bound<'_, PyDict>>,
    ) -> Result<(), PyErr> {
        let arguments: KeywordArguments<'_> =
            KeywordArguments::read(py, "validate_reference", options, VALIDATE_OPTIONS)?;
        let reference: Bound<'_, PyAny> = new_record_with(
            py,
            RecordId::CompileSqlReference,
            &[
                ("ref_kind", arguments.value("ref_kind")?),
                ("ref_name", arguments.value("ref_name")?),
                ("ref_package", arguments.value("ref_package")?),
            ],
        )?;
        validate_compile_dbt_model_reference(
            py,
            reference,
            arguments.value("owner_relative_sql_path")?,
            self.manifest_or_none(py),
        )
        .and_then(|value| pyo3::IntoPyObjectExt::into_bound_py_any(value, py))?;
        Ok(())
    }

    #[pyo3(signature = (*, ref_kind, ref_name, ref_package))]
    fn resolve_reference<'py>(
        &self,
        py: Python<'py>,
        ref_kind: Bound<'py, PyAny>,
        ref_name: Bound<'py, PyAny>,
        ref_package: Bound<'py, PyAny>,
    ) -> PyResult<Bound<'py, PyAny>> {
        resolve_dbt_reference_relation(
            py,
            Some(keywords(
                py,
                &[
                    ("manifest", self.manifest_or_none(py)),
                    ("ref_kind", ref_kind),
                    ("ref_name", ref_name),
                    ("ref_package", ref_package),
                ],
            )?),
        )
    }
}

/// A dbt-backed external reference resolver from manifest contents, or `None` without them.
#[pyfunction]
#[pyo3(signature = (*, manifest_contents))]
pub(crate) fn build_compile_reference_resolver<'py>(
    py: Python<'py>,
    manifest_contents: Bound<'py, PyAny>,
) -> PyResult<Bound<'py, PyAny>> {
    if manifest_contents.is_none() {
        return Ok(py.None().into_bound(py));
    }
    let manifest: Bound<'py, PyAny> = build_compile_dbt_manifest_index(py, manifest_contents)?;
    Ok(Bound::new(
        py,
        DbtCompileReferenceResolver::new(Some(manifest.unbind())),
    )?
    .into_any())
}

/// `<target_path>/manifest.json` under the project, when that file exists.
fn manifest_contents<'py>(
    project_dir: &Bound<'py, PyAny>,
    target_path: &Bound<'py, PyAny>,
) -> PyResult<Bound<'py, PyAny>> {
    let py: Python<'py> = project_dir.py();
    let raw: Bound<'py, PyAny> = imported(py, "pathlib", "Path")?.call1((target_path,))?;
    let resolved: Bound<'py, PyAny> = if raw.call_method0("is_absolute")?.is_truthy()? {
        raw
    } else {
        project_dir.div(raw)?
    };
    let manifest: Bound<'py, PyAny> = resolved.div(MANIFEST_FILENAME)?;
    if !manifest.call_method0("is_file")?.is_truthy()? {
        return Ok(py.None().into_bound(py));
    }
    call_with(
        &native_field(&manifest, "read_text")?,
        &[("encoding", PyString::new(py, "utf-8").into_any())],
    )
}

/// Build the external reference resolver configured for a discovered project.
#[pyfunction]
#[pyo3(signature = (*, project_dir, discovered_inputs))]
pub(crate) fn resolve_external_sql_reference_resolver<'py>(
    project_dir: Bound<'py, PyAny>,
    discovered_inputs: Bound<'py, PyAny>,
) -> PyResult<Bound<'py, PyAny>> {
    let py: Python<'py> = project_dir.py();
    let target_path: Bound<'py, PyAny> = native_field(&discovered_inputs, "project_config")?
        .getattr("dbt")?
        .getattr("target_path")?;
    let contents: Bound<'py, PyAny> = if target_path.is_none() {
        py.None().into_bound(py)
    } else {
        let lifecycle: Bound<'py, PyAny> = call_with(
            &host_entry(py, ExecutorHost::OperationLifecycle)?,
            &[
                ("operation_kind", PyString::new(py, "project").into_any()),
                (
                    "operation_name",
                    PyString::new(py, "external_manifest_discovery").into_any(),
                ),
            ],
        )?;
        within(&lifecycle, |_| {
            manifest_contents(&project_dir, &target_path)
        })?
    };
    build_compile_reference_resolver(py, contents)
}

pub(crate) fn register(module: &Bound<'_, PyModule>) -> PyResult<()> {
    module.add_class::<DbtCompileReferenceResolver>()?;
    module.add_function(wrap_pyfunction!(build_compile_reference_resolver, module)?)?;
    module.add_function(wrap_pyfunction!(
        resolve_external_sql_reference_resolver,
        module
    )?)
}
