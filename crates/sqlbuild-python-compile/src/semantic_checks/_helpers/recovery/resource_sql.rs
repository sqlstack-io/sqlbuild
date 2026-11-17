//! Native audit, SQL test and SQL hook SQL checks against the closed model shapes.

use std::sync::Arc;

use pyo3::prelude::{Bound, PyAny, PyAnyMethods, PyErr, PyRef, PyResult, Python};
use pyo3::pyfunction;
use pyo3::types::{PyBool, PyDict, PyDictMethods, PyList, PyString, PyTuple};
use sqlbuild_analysis::semantic_checks::main::resource_sql_diagnostics::resource_sql_diagnostics;
use sqlbuild_analysis::semantic_checks::models::{
    AuditSqlInput, HookModelInput, HookSqlInput, ResourceSpan, ResourceSqlDiagnostic,
    ResourceSqlFailure, ResourceSqlKind, ResourceSqlRequest, SqlTestSqlInput,
};

use sqlbuild_project::models::ReferenceRow;
use sqlbuild_python_base::boundary::main::compiler_error::compiler_error;
use sqlbuild_python_base::records::main::construct::error_class::error_class;
use sqlbuild_python_base::records::main::construct::record_class::record_class;
use sqlbuild_python_base::records::main::fields::field::field as native_field;
use sqlbuild_python_base::records::main::fields::mapping_get::mapping_get;
use sqlbuild_python_base::records::main::fields::mapping_items::mapping_items;
use sqlbuild_python_base::records::types::ErrorId;
use sqlbuild_python_base::records::types::RecordId;
use sqlbuild_python_base::types::CompilerDetach;
use sqlbuild_python_project::analysis::models::ProjectCatalog;
use sqlbuild_python_project::compiled_views::models::NativeCompiledModel;
use sqlbuild_python_project::resource_views::main::audit_views::audit_reference_rows::audit_reference_rows;

const SQL_ANALYSIS_CONFIG_KEY: &str = "sql_analysis";
const MODEL_HOOK_KEYS: [&str; 2] = ["pre_hooks", "post_hooks"];
const BUILT_IN_AUDIT_ROOT: &str = "<built-in>";
const UDF_KIND: &str = "udf";
const TABLE_FUNCTION_KIND: &str = "table_fn";
const TABLE_FUNCTION_STUB_PREFIX: &str = "__sqlbuild_table_function_";

/// The compile models a resource SQL diagnostic is built from.
struct DiagnosticClasses<'py> {
    compiler_diagnostic: Bound<'py, PyAny>,
    source_location: Bound<'py, PyAny>,
    path: Bound<'py, PyAny>,
}

/// Bind the project's audit, SQL test and SQL hook bodies on `catalog` against `shapes`.
#[pyfunction]
pub(crate) fn resource_sql_diagnostics_of<'py>(
    catalog: PyRef<'_, ProjectCatalog>,
    project: &Bound<'py, PyAny>,
    shapes: &Bound<'py, PyDict>,
    profile: &Bound<'py, PyAny>,
) -> PyResult<Bound<'py, PyTuple>> {
    let py: Python<'py> = project.py();
    let request: ResourceSqlRequest = request(project, shapes, profile)?;
    let catalog = &catalog.inner;
    let outcome = py
        .compiler_detach(|| Ok(resource_sql_diagnostics(&request, catalog)))
        .map_err(compiler_error)?;
    let diagnostics: Vec<ResourceSqlDiagnostic> = match outcome {
        Ok(diagnostics) => diagnostics,
        Err(failure) => return Err(failure_error(py, failure)?),
    };
    let classes = DiagnosticClasses {
        compiler_diagnostic: record_class(py, RecordId::CompilerDiagnostic)?,
        source_location: record_class(py, RecordId::SourceLocation)?,
        path: py.import("pathlib")?.getattr("Path")?,
    };
    let built: Vec<Bound<'py, PyAny>> = diagnostics
        .into_iter()
        .map(|diagnostic| python_diagnostic(py, &classes, diagnostic))
        .collect::<PyResult<_>>()?;
    PyTuple::new(py, built)
}

fn request(
    project: &Bound<'_, PyAny>,
    shapes: &Bound<'_, PyDict>,
    profile: &Bound<'_, PyAny>,
) -> PyResult<ResourceSqlRequest> {
    let py: Python<'_> = project.py();
    let reference_class: Bound<'_, PyAny> = record_class(py, RecordId::CompileSqlReference)?;
    let mut audits: Vec<AuditSqlInput> = Vec::new();
    for audit in native_field(project, "audits")?.try_iter()? {
        audits.push(audit_input(&audit?, &reference_class)?);
    }
    let mut sql_tests: Vec<SqlTestSqlInput> = Vec::new();
    for test in native_field(project, "sql_tests")?.try_iter()? {
        sql_tests.push(sql_test_input(&test?)?);
    }
    let mut models: Vec<HookModelInput> = Vec::new();
    for model in native_field(project, "models")?.try_iter()? {
        models.push(model_input(&model?, &reference_class)?);
    }
    let mut hook_files: Vec<(String, Arc<str>)> = Vec::new();
    for file in native_field(project, "sql_hook_files")?.try_iter()? {
        let file: Bound<'_, PyAny> = file?;
        hook_files.push((
            native_field(&file, "relative_path")?.str()?.extract()?,
            Arc::from(native_field(&file, "contents")?.extract::<String>()?),
        ));
    }
    Ok(ResourceSqlRequest {
        dialect: native_field(profile, "sql_analysis_dialect")?.extract()?,
        quoted_ignore_case: native_field(profile, "quoted_identifiers_ignore_case")?.extract()?,
        require_sql_analysis: native_field(
            &native_field(project, "settings")?,
            "require_sql_analysis",
        )?
        .extract()?,
        shapes: shapes
            .iter()
            .map(|(name, columns)| shape(&name, &columns))
            .collect::<PyResult<_>>()?,
        audits,
        sql_tests,
        models,
        hook_files,
    })
}

/// One closed relation shape: its name and `(column, type)` pairs in order.
fn shape(
    name: &Bound<'_, PyAny>,
    columns: &Bound<'_, PyAny>,
) -> PyResult<(String, Vec<(String, String)>)> {
    let mut pairs: Vec<(String, String)> = Vec::new();
    for pair in mapping_items(columns)?.try_iter()? {
        pairs.push(pair?.extract()?);
    }
    Ok((name.extract()?, pairs))
}

fn audit_input(
    audit: &Bound<'_, PyAny>,
    reference_class: &Bound<'_, PyAny>,
) -> PyResult<AuditSqlInput> {
    let file: Bound<'_, PyAny> = native_field(audit, "audit_file")?;
    let block: Bound<'_, PyAny> = native_field(audit, "audit_block")?;
    let target_name: Bound<'_, PyAny> = native_field(audit, "attached_target_name")?;
    let attached_target: Option<(String, String)> = if target_name.is_none() {
        None
    } else {
        Some((
            native_field(audit, "attached_target_kind")?
                .str()?
                .extract()?,
            target_name.extract()?,
        ))
    };
    let first_part: Option<String> = native_field(&native_field(&file, "file_path")?, "parts")?
        .try_iter()?
        .next()
        .transpose()?
        .map(|part| part.extract())
        .transpose()?;
    let mut bodies: Vec<(Option<String>, Option<String>)> = Vec::new();
    for attribute in ["sql_body", "measure_sql", "evidence_sql"] {
        bodies.push((
            native_field(audit, attribute)?.extract()?,
            native_field(&block, attribute)?.extract()?,
        ));
    }
    Ok(AuditSqlInput {
        name: native_field(audit, "name")?.extract()?,
        definition_name: native_field(audit, "definition_name")?.extract()?,
        attached_target,
        attached_column: native_field(audit, "attached_column_name")?.extract()?,
        built_in: first_part.as_deref() == Some(BUILT_IN_AUDIT_ROOT),
        path: native_field(&file, "relative_path")?.str()?.extract()?,
        contents: Arc::from(native_field(&file, "contents")?.extract::<String>()?),
        bodies,
        opts_out: opts_out(&native_field(&block, "header_values")?)?,
        relations: match audit_reference_rows(audit)? {
            Some(rows) => row_relations(rows),
            None => relations(&native_field(audit, "references")?, reference_class)?,
        },
    })
}

fn sql_test_input(test: &Bound<'_, PyAny>) -> PyResult<SqlTestSqlInput> {
    let file: Bound<'_, PyAny> = native_field(test, "test_file")?;
    let block: Bound<'_, PyAny> = native_field(test, "test_block")?;
    Ok(SqlTestSqlInput {
        name: native_field(test, "name")?.extract()?,
        expected_model_names: native_field(test, "expected_model_names")?
            .try_iter()?
            .map(|name| name?.extract())
            .collect::<PyResult<_>>()?,
        path: native_field(&file, "relative_path")?.str()?.extract()?,
        contents: Arc::from(native_field(&file, "contents")?.extract::<String>()?),
        sql_body: native_field(test, "sql_body")?.extract()?,
        authored_body: native_field(&block, "sql_body")?.extract()?,
        opts_out: opts_out(&native_field(&block, "header_values")?)?,
    })
}

fn model_input(
    model: &Bound<'_, PyAny>,
    reference_class: &Bound<'_, PyAny>,
) -> PyResult<HookModelInput> {
    let values: Bound<'_, PyAny> = native_field(&native_field(model, "config")?, "values")?;
    let mut hooks: Vec<HookSqlInput> = Vec::new();
    for key in MODEL_HOOK_KEYS {
        let entries: Bound<'_, PyAny> = mapping_get(&values, key)?;
        if !(entries.is_instance_of::<PyList>() || entries.is_instance_of::<PyTuple>()) {
            continue;
        }
        for (index, entry) in entries.try_iter()?.enumerate() {
            let entry: Bound<'_, PyAny> = entry?;
            let Some(statement) = text(&optional_attribute(&entry, "statement")?)? else {
                continue;
            };
            let hook_path: Bound<'_, PyAny> = optional_attribute(&entry, "relative_path")?;
            let reads: Bound<'_, PyAny> = optional_attribute(&entry, "reads")?;
            hooks.push(HookSqlInput {
                key: key.to_owned(),
                index,
                statement,
                hook_path: if hook_path.is_none() {
                    None
                } else {
                    Some(hook_path.str()?.extract()?)
                },
                definition: text(&optional_attribute(&entry, "definition_sql")?)?,
                relations: if reads.is_truthy()? {
                    relations(&reads, reference_class)?
                } else {
                    Vec::new()
                },
            });
        }
    }
    let (name, path, authored_sql): (String, String, Arc<str>) =
        match model.cast::<NativeCompiledModel>() {
            Ok(native) => native.get().file_texts(model.py())?,
            Err(_) => (
                native_field(model, "name")?.extract()?,
                native_field(model, "relative_path")?.str()?.extract()?,
                Arc::from(native_field(model, "authored_sql")?.extract::<String>()?),
            ),
        };
    Ok(HookModelInput {
        name,
        path,
        authored_sql,
        analysis_disabled: opts_out(&values)?,
        hooks,
    })
}

/// The value when it is a `str`, otherwise `None`.
fn text(value: &Bound<'_, PyAny>) -> PyResult<Option<String>> {
    if value.is_instance_of::<PyString>() {
        return value.extract().map(Some);
    }
    Ok(None)
}

/// `getattr(value, name, None)`.
fn optional_attribute<'py>(value: &Bound<'py, PyAny>, name: &str) -> PyResult<Bound<'py, PyAny>> {
    if value.hasattr(name)? {
        native_field(value, name)
    } else {
        Ok(value.py().None().into_bound(value.py()))
    }
}

/// Whether the values set `sql_analysis = false`.
fn opts_out(values: &Bound<'_, PyAny>) -> PyResult<bool> {
    Ok(mapping_get(values, SQL_ANALYSIS_CONFIG_KEY)?.is(PyBool::new(values.py(), false)))
}

/// `sorted(binding_relation_names(...))` of a native view's reference rows.
fn row_relations(rows: Vec<ReferenceRow>) -> Vec<String> {
    let mut names: Vec<String> = rows
        .into_iter()
        .filter_map(|ReferenceRow { kind, name, .. }| relation_name(&kind, name))
        .collect();
    names.sort_unstable();
    names.dedup();
    names
}

/// The binding relation a reference of `kind` to `name` reads, or `None` for a UDF call.
fn relation_name(kind: &str, name: String) -> Option<String> {
    if kind == UDF_KIND {
        return None;
    }
    Some(if kind == TABLE_FUNCTION_KIND {
        format!("{TABLE_FUNCTION_STUB_PREFIX}{name}")
    } else {
        name
    })
}

/// `sorted(binding_relation_names(...))` of the `CompileSqlReference`s among `references`.
fn relations(
    references: &Bound<'_, PyAny>,
    reference_class: &Bound<'_, PyAny>,
) -> PyResult<Vec<String>> {
    let mut names: Vec<String> = Vec::new();
    for reference in references.try_iter()? {
        let reference: Bound<'_, PyAny> = reference?;
        if !reference.is_instance(reference_class)? {
            continue;
        }
        let kind: String = native_field(&reference, "ref_kind")?.str()?.extract()?;
        names.extend(relation_name(
            &kind,
            native_field(&reference, "ref_name")?.extract()?,
        ));
    }
    names.sort_unstable();
    names.dedup();
    Ok(names)
}

fn python_diagnostic<'py>(
    py: Python<'py>,
    classes: &DiagnosticClasses<'py>,
    diagnostic: ResourceSqlDiagnostic,
) -> PyResult<Bound<'py, PyAny>> {
    let ResourceSpan {
        line,
        column,
        end_line,
        end_column,
    } = diagnostic.span;
    let location: Bound<'py, PyDict> = PyDict::new(py);
    location.set_item("path", classes.path.call1((&diagnostic.path,))?)?;
    location.set_item("line", line)?;
    location.set_item("column", column)?;
    location.set_item("end_line", end_line)?;
    location.set_item("end_column", end_column)?;
    let fields: Bound<'py, PyDict> = PyDict::new(py);
    fields.set_item("phase", "compile")?;
    fields.set_item(
        "severity",
        if diagnostic.is_error {
            "error"
        } else {
            "warning"
        },
    )?;
    fields.set_item("code", diagnostic.code)?;
    fields.set_item("message", diagnostic.message)?;
    fields.set_item(
        "resource_type",
        match diagnostic.kind {
            ResourceSqlKind::Audit => "audit",
            ResourceSqlKind::SqlTest => "sql_test",
            ResourceSqlKind::Model => "model",
        },
    )?;
    fields.set_item("resource_name", diagnostic.resource_name)?;
    fields.set_item(
        "location",
        classes.source_location.call((), Some(&location))?,
    )?;
    fields.set_item("notes", PyTuple::new(py, diagnostic.note)?)?;
    fields.set_item("help", diagnostic.help)?;
    classes.compiler_diagnostic.call((), Some(&fields))
}

fn failure_error(py: Python<'_>, failure: ResourceSqlFailure) -> PyResult<PyErr> {
    Ok(match failure {
        ResourceSqlFailure::CursorIntrinsic(message) => {
            PyErr::from_value(error_class(py, ErrorId::CompileInput)?.call1((message,))?)
        }
        ResourceSqlFailure::Normalization(message) | ResourceSqlFailure::Internal(message) => {
            compiler_error(message)
        }
    })
}
