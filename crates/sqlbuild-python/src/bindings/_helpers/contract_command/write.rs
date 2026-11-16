//! Typed, metadata-preserving repository edits for contract adoption.

use pyo3::exceptions::PyException;
use pyo3::prelude::{Bound, PyAny, PyAnyMethods, PyResult, Python};
use pyo3::types::{PyDict, PyDictMethods, PyList, PyListMethods, PyTuple};

use crate::bindings::_helpers::format_command::files::write_file_atomically;
use sqlbuild_python_base::boundary::main::imported::imported;
use sqlbuild_python_base::boundary::main::keywords::keywords;
use sqlbuild_python_base::records::main::construct::enum_member::enum_member;
use sqlbuild_python_base::records::main::construct::new_record::new_record;
use sqlbuild_python_base::records::main::fields::field::field as native_field;
use sqlbuild_python_base::records::main::fields::mapping_get::mapping_get;
use sqlbuild_python_base::records::main::fields::replaced_keywords::replaced_keywords;
use sqlbuild_python_base::records::types::RecordId;
use sqlbuild_python_compile::compile_driver::main::setup::build_compiled_project::build_compiled_project;
use sqlbuild_python_compile::compile_driver::main::stages::discover_project_inputs::discover_project_inputs;
use sqlbuild_python_front::refactor::main::edit_contract_model::edit_contract_model;
use sqlbuild_python_front::refactor::main::edit_contract_source::edit_contract_source;
use sqlbuild_python_front::refactor::main::remaining_contract_findings::remaining_contract_findings;

const UTF8: &str = "utf-8";

/// The repository the edits apply to and how they apply.
pub(crate) struct ContractWrite<'py> {
    pub(crate) project_dir: Bound<'py, PyAny>,
    pub(crate) overwrite: Bound<'py, PyAny>,
    pub(crate) adapter: Bound<'py, PyAny>,
    pub(crate) resolved_connection: Bound<'py, PyAny>,
    pub(crate) cli_vars: Bound<'py, PyAny>,
}

/// Apply safe source-only edits atomically and validate the resulting project.
pub(crate) fn write_contracts<'py>(
    write: &ContractWrite<'py>,
    graph: &Bound<'py, PyAny>,
    result: &Bound<'py, PyAny>,
) -> PyResult<Bound<'py, PyAny>> {
    let py: Python<'py> = graph.py();
    let project: Bound<'py, PyAny> = native_field(graph, "project")?;
    let models: Bound<'py, PyDict> = by_name(&native_field(&project, "models")?)?;
    let sources: Bound<'py, PyDict> = by_name(&native_field(&project, "sources")?)?;
    let model_type: Bound<'py, PyAny> = enum_member(py, RecordId::CompiledResourceType, "MODEL")?;
    let updates: Bound<'py, PyDict> = PyDict::new(py);
    let updated_evidence: Bound<'py, PyList> = PyList::empty(py);
    for evidence in native_field(result, "evidence")?.try_iter()? {
        let evidence: Bound<'py, PyAny> = evidence?;
        let physical_columns: Bound<'py, PyAny> = native_field(&evidence, "physical_columns")?;
        let findings: Bound<'py, PyAny> = native_field(&evidence, "findings")?;
        if physical_columns.is_none() || !findings.is_truthy()? {
            updated_evidence.append(evidence)?;
            continue;
        }
        let name: Bound<'py, PyAny> = native_field(&evidence, "resource_name")?;
        let (path, edited): (Bound<'py, PyAny>, Bound<'py, PyAny>) =
            if native_field(&evidence, "resource_type")?.eq(&model_type)? {
                let model: Bound<'py, PyAny> = models.as_any().get_item(&name)?;
                let edited: Bound<'py, PyAny> = edit_contract_model(
                    py,
                    &model.clone(),
                    &physical_columns,
                    write.overwrite.clone().extract::<bool>()?,
                )
                .and_then(|value| pyo3::IntoPyObjectExt::into_bound_py_any(value, py))?;
                (
                    write
                        .project_dir
                        .div(native_field(&model, "relative_path")?)?,
                    edited,
                )
            } else {
                let source: Bound<'py, PyAny> = sources.as_any().get_item(&name)?;
                let path: Bound<'py, PyAny> =
                    native_field(&source, "source_file")?.getattr("file_path")?;
                let edited: Bound<'py, PyAny> = edit_contract_source(
                    &source,
                    &physical_columns,
                    write.overwrite.clone().extract::<bool>()?,
                    mapping_get(updates.as_any(), &path)?.extract::<Option<String>>()?,
                )
                .and_then(|value| pyo3::IntoPyObjectExt::into_bound_py_any(value, py))?;
                (path, edited)
            };
        let (contents, conflict): (Bound<'py, PyAny>, Bound<'py, PyAny>) = edited.extract()?;
        if !contents.is_none() && contents.ne(read_text(&path)?)? {
            updates.set_item(&path, contents)?;
        }
        let remaining: Bound<'py, PyAny> = remaining_contract_findings(
            py,
            &findings,
            write.overwrite.clone().extract::<bool>()?,
            conflict.extract::<Option<String>>()?,
        )
        .and_then(|value| pyo3::IntoPyObjectExt::into_bound_py_any(value, py))?;
        updated_evidence.append(replaced(&evidence, &[("findings", remaining)])?)?;
    }
    let evidence: Bound<'py, PyAny> = PyTuple::new(py, updated_evidence)?.into_any();
    if updates.is_empty() {
        return replaced(result, &[("evidence", evidence)]);
    }
    let originals: Bound<'py, PyDict> = PyDict::new(py);
    for path in updates.keys() {
        originals.set_item(&path, read_text(&path)?)?;
    }
    if let Err(error) = applied(write, result, &updates) {
        if error.is_instance_of::<PyException>(py) {
            write_all(&originals)?;
        }
        return Err(error);
    }
    let relative: Vec<Bound<'py, PyAny>> = updates
        .keys()
        .iter()
        .map(|path| path.call_method1("relative_to", (&write.project_dir,)))
        .collect::<PyResult<_>>()?;
    let written: Bound<'py, PyAny> = imported(py, "builtins", "sorted")?.call1((relative,))?;
    replaced(
        result,
        &[
            ("evidence", evidence),
            (
                "written_paths",
                PyTuple::new(py, written.try_iter()?.collect::<PyResult<Vec<_>>>()?)?.into_any(),
            ),
        ],
    )
}

/// Write every update, rediscover the project and compile it when an adapter is available.
fn applied<'py>(
    write: &ContractWrite<'py>,
    result: &Bound<'py, PyAny>,
    updates: &Bound<'py, PyDict>,
) -> PyResult<()> {
    let py: Python<'py> = result.py();
    write_all(updates)?;
    let discovered: Bound<'py, PyAny> = discover_project_inputs(
        py,
        Some(keywords(py, &[("project_dir", write.project_dir.clone())])?),
    )?;
    if write.adapter.is_none() {
        return Ok(());
    }
    let selection_keywords: Bound<'py, PyDict> = PyDict::new(py);
    selection_keywords.set_item("no_cache", true)?;
    let selection: Bound<'py, PyAny> = new_record(
        py,
        RecordId::CompileAnalysisSelection,
        (),
        Some(&selection_keywords),
    )?;
    build_compiled_project(
        py,
        Some(keywords(
            py,
            &[
                ("discovered_inputs", discovered),
                ("adapter", write.adapter.clone()),
                ("selected_target", native_field(result, "from_target")?),
                ("resolved_connection", write.resolved_connection.clone()),
                ("cli_vars", write.cli_vars.clone()),
                ("analysis_selection", selection),
            ],
        )?),
    )?;
    Ok(())
}

/// Write each `path: contents` pair atomically, in order.
fn write_all(contents_by_path: &Bound<'_, PyDict>) -> PyResult<()> {
    for (path, contents) in contents_by_path.iter() {
        write_file_atomically(path.extract()?, &contents.extract::<String>()?)?;
    }
    Ok(())
}

fn read_text<'py>(path: &Bound<'py, PyAny>) -> PyResult<Bound<'py, PyAny>> {
    let keywords: Bound<'py, PyDict> = PyDict::new(path.py());
    keywords.set_item("encoding", UTF8)?;
    path.call_method("read_text", (), Some(&keywords))
}

/// `{item.name: item for item in items}`.
fn by_name<'py>(items: &Bound<'py, PyAny>) -> PyResult<Bound<'py, PyDict>> {
    let named: Bound<'py, PyDict> = PyDict::new(items.py());
    for item in items.try_iter()? {
        let item: Bound<'py, PyAny> = item?;
        named.set_item(native_field(&item, "name")?, &item)?;
    }
    Ok(named)
}

/// `dataclasses.replace(value, **fields)`.
fn replaced<'py>(
    value: &Bound<'py, PyAny>,
    fields: &[(&str, Bound<'py, PyAny>)],
) -> PyResult<Bound<'py, PyAny>> {
    let py: Python<'py> = value.py();
    let keywords: Bound<'py, PyDict> = PyDict::new(py);
    for (name, field) in fields {
        keywords.set_item(*name, field)?;
    }
    replaced_keywords(value, &keywords)
}
