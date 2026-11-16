//! A `CompiledModel` for a model input built outside the native compile-input stage.

use crate::analysis::main::locate_model_binding_diagnostics::locate_model_binding_diagnostics;
use crate::analysis::models::LocatedRow;
use crate::analysis::types::ExpansionRow;
use crate::analysis_session::main::dynamic_contract_proof::dynamic_contract_proof;
use crate::analysis_session::main::session_outcome_analysis::session_outcome_analysis;
use pyo3::IntoPyObjectExt;
use pyo3::prelude::{Bound, PyAny, PyAnyMethods, PyResult, Python};
use pyo3::types::{PyDict, PyList, PyString, PyTuple};
use sqlbuild_analysis::semantic_validation::models::DiagnosticRow;
use sqlbuild_python_base::records::main::construct::enum_from_value::enum_from_value;
use sqlbuild_python_base::records::main::construct::enum_member::enum_member;
use sqlbuild_python_base::records::main::construct::record_class::record_class;
use sqlbuild_python_base::records::main::fields::field::field as native_field;
use sqlbuild_python_base::records::main::fields::mapping_get::mapping_get;
use sqlbuild_python_base::records::types::RecordId;
use sqlbuild_python_project::analysis::models::Positional;

pub(crate) type Object<'py> = Bound<'py, PyAny>;

/// The compiled model of `model_input` from its session outcome, fallback rows and proof.
pub(crate) fn compiled_model<'py>(
    model_input: &Object<'py>,
    deps: &Object<'py>,
    analysis: &Object<'py>,
    macro_deps: &Object<'py>,
) -> PyResult<Object<'py>> {
    let py: Python<'py> = model_input.py();
    let model_file: Object<'py> = native_field(model_input, "model_file")?;
    let model_name: Object<'py> = native_field(&model_file, "file_path")?.getattr("stem")?;
    let none: Object<'py> = py.None().into_bound(py);
    let mut inferred: Object<'py> = none.clone();
    let mut lineage: Object<'py> = none.clone();
    let mut has_star: Object<'py> = false_object(py);
    let mut star_resolved: Object<'py> = false_object(py);
    let mut diagnostics: Object<'py> = PyTuple::empty(py).into_any();
    let mut validated: Object<'py> = false_object(py);
    let mut proof: Object<'py> =
        dynamic_contract_proof(&native_field(analysis, "standalone_proof")?)?;
    let session: Object<'py> = native_field(analysis, "session")?;
    if !session.is_none() {
        let (session, position): (Object<'py>, Object<'py>) = session.extract()?;
        let arguments: Bound<'py, PyDict> = PyDict::new(py);
        arguments.set_item("session", session)?;
        arguments.set_item("position", position)?;
        arguments.set_item("placeholders", &none)?;
        let outcome: Object<'py> = session_outcome_analysis(Some(&arguments))?;
        let result: Object<'py> = native_field(&outcome, "polyglot_analysis")?;
        if native_field(&result, "analysis_succeeded")?.is_truthy()? {
            inferred = native_field(&result, "columns")?;
            lineage = native_field(&result, "lineage_columns")?;
            has_star = native_field(&result, "has_star")?;
            star_resolved = native_field(&result, "star_resolved")?;
        } else {
            inferred = fallback_columns(&native_field(analysis, "fallback")?)?;
        }
        let session_proof: Object<'py> = native_field(&outcome, "dynamic_column_contract")?;
        if session_proof.is_truthy()? {
            proof = session_proof;
        }
        validated = native_field(&result, "binding_validated")?;
        let cleaned: Object<'py> = native_field(&outcome, "cleaned_sql")?;
        let cleaned: Object<'py> = if cleaned.is_truthy()? {
            cleaned
        } else {
            PyString::new(py, "").into_any()
        };
        diagnostics = located(
            model_input,
            &native_field(&result, "binding_diagnostics")?,
            &cleaned,
        )?;
    }
    if !proof.is_none() && native_field(&proof, "output_proven")?.is_truthy()? {
        inferred = native_field(&proof, "fixed_columns")?;
        has_star = true_object(py);
    }
    let resource_type: Object<'py> = record_class(py, RecordId::CompiledResourceType)?;
    let key_class: Object<'py> = record_class(py, RecordId::CompiledObjectKey)?;
    let mut dependency_keys: Vec<Object<'py>> = Vec::new();
    for dependency in deps.try_iter()? {
        let (kind, name): (Object<'py>, Object<'py>) = dependency?.extract()?;
        dependency_keys.push(call(
            &key_class,
            &[
                (
                    "resource_type",
                    enum_from_value(py, RecordId::CompiledResourceType, kind)?,
                ),
                ("name", name),
            ],
        )?);
    }
    let config: Object<'py> = native_field(model_input, "config")?;
    let key: Object<'py> = call(
        &key_class,
        &[
            ("resource_type", native_field(&resource_type, "MODEL")?),
            ("name", model_name.clone()),
        ],
    )?;
    call(
        &record_class(py, RecordId::CompiledModel)?,
        &[
            ("key", key),
            ("deps", PyTuple::new(py, dependency_keys)?.into_any()),
            ("name", model_name.clone()),
            ("relative_path", native_field(&model_file, "relative_path")?),
            ("query_sql", native_field(model_input, "query_sql")?),
            ("config", config.clone()),
            ("destination", destination(&config, &model_name)?),
            ("references", native_field(model_input, "references")?),
            ("schema_entry", native_field(model_input, "schema_entry")?),
            ("inferred_columns", inferred),
            ("fast_lineage_columns", lineage),
            ("fast_lineage_has_star", has_star),
            ("fast_lineage_star_resolved", star_resolved),
            ("authored_sql", native_field(&model_file, "contents")?),
            (
                "authored_query_sql",
                native_field(&model_file, "query_sql")?,
            ),
            (
                "output_column_locations",
                native_field(&model_file, "output_column_locations")?,
            ),
            (
                "extract_implicit_alias_columns",
                native_field(&model_file, "extract_implicit_alias_columns")?,
            ),
            ("macro_deps", macro_deps.clone()),
            (
                "enum_declarations",
                native_field(model_input, "enum_declarations")?,
            ),
            (
                "constant_declarations",
                native_field(model_input, "constant_declarations")?,
            ),
            ("enum_columns", native_field(model_input, "enum_columns")?),
            ("binding_diagnostics", diagnostics),
            ("binding_validated", validated),
            ("dynamic_column_contract", proof),
            (
                "rejected_sql_analysis_opt_out",
                native_field(model_input, "rejected_sql_analysis_opt_out")?,
            ),
        ],
    )
}

fn false_object(py: Python<'_>) -> Object<'_> {
    pyo3::types::PyBool::new(py, false).to_owned().into_any()
}

fn true_object(py: Python<'_>) -> Object<'_> {
    pyo3::types::PyBool::new(py, true).to_owned().into_any()
}

/// A model's configured physical and logical destination.
fn destination<'py>(config: &Object<'py>, model_name: &Object<'py>) -> PyResult<Object<'py>> {
    let py: Python<'py> = config.py();
    let values: Object<'py> = native_field(config, "values")?;
    let text = |key: &str| -> PyResult<Option<Object<'py>>> {
        let value: Object<'py> = mapping_get(&values, key)?;
        Ok(value.is_instance_of::<PyString>().then_some(value))
    };
    let none: Object<'py> = py.None().into_bound(py);
    call(
        &record_class(py, RecordId::CompiledRelationLocation)?,
        &[
            (
                "database",
                text("database")?.unwrap_or_else(|| none.clone()),
            ),
            ("schema", text("schema")?.unwrap_or_else(|| none.clone())),
            ("name", text("alias")?.unwrap_or_else(|| model_name.clone())),
            ("qualified_name", none.clone()),
            ("logical_schema", native_field(config, "logical_schema")?),
            (
                "logical_database",
                native_field(config, "logical_database")?,
            ),
        ],
    )
}

/// A failed analysis's parse-mode column rows as inferred columns, None without rows.
fn fallback_columns<'py>(rows: &Object<'py>) -> PyResult<Object<'py>> {
    let py: Python<'py> = rows.py();
    if rows.is_none() {
        return Ok(rows.clone());
    }
    let column_class: Object<'py> = record_class(py, RecordId::InferredColumn)?;
    let mut columns: Vec<Object<'py>> = Vec::new();
    for row in rows.try_iter()? {
        let (name, data_type, value): (Object<'py>, Object<'py>, Object<'py>) = row?.extract()?;
        columns.push(call(
            &column_class,
            &[
                ("name", name),
                ("type", data_type),
                (
                    "nullability",
                    enum_from_value(py, RecordId::InferredNullability, value)?,
                ),
            ],
        )?);
    }
    Ok(PyTuple::new(py, columns)?.into_any())
}

/// Binding diagnostics located in the authored model file.
fn located<'py>(
    model_input: &Object<'py>,
    diagnostics: &Object<'py>,
    cleaned: &Object<'py>,
) -> PyResult<Object<'py>> {
    let py: Python<'py> = model_input.py();
    if !diagnostics.is_truthy()? {
        return Ok(PyTuple::empty(py).into_any());
    }
    let model_file: Object<'py> = native_field(model_input, "model_file")?;
    let path: Object<'py> = native_field(&model_file, "relative_path")?;
    let mut rows: Vec<Object<'py>> = Vec::new();
    for item in diagnostics.try_iter()? {
        let item: Object<'py> = item?;
        let fields: Vec<Object<'py>> = [
            "code", "message", "line", "column", "start", "end", "severity",
        ]
        .into_iter()
        .map(|name| native_field(&item, name))
        .collect::<PyResult<_>>()?;
        rows.push(PyTuple::new(py, fields)?.into_any());
    }
    let texts: Object<'py> = PyTuple::new(
        py,
        [
            native_field(&model_file, "contents")?,
            native_field(&model_file, "query_sql")?,
            cleaned.clone(),
        ],
    )?
    .into_any();
    let expansion: Object<'py> = native_field(model_input, "sql_expansion")?;
    let expansion_row: Object<'py> = if expansion.is_none() {
        expansion
    } else {
        let mut passes: Vec<Object<'py>> = Vec::new();
        for spans in native_field(&expansion, "passes")?.try_iter()? {
            let mut rows: Vec<Object<'py>> = Vec::new();
            for span in spans?.try_iter()? {
                let span: Object<'py> = span?;
                let fields: Vec<Object<'py>> =
                    ["source_start", "source_end", "output_start", "output_end"]
                        .into_iter()
                        .map(|name| native_field(&span, name))
                        .collect::<PyResult<_>>()?;
                rows.push(PyTuple::new(py, fields)?.into_any());
            }
            passes.push(PyList::new(py, rows)?.into_any());
        }
        PyTuple::new(
            py,
            [
                native_field(&expansion, "expanded_sql")?,
                PyList::new(py, passes)?.into_any(),
            ],
        )?
        .into_any()
    };
    let located: Vec<LocatedRow> = locate_model_binding_diagnostics(
        PyList::new(py, rows)?.extract::<Vec<Positional<DiagnosticRow>>>()?,
        texts.extract::<(String, String, String)>()?,
        expansion_row.extract::<ExpansionRow>()?,
    )?;
    let diagnostic_class: Object<'py> = record_class(py, RecordId::CompilerDiagnostic)?;
    let location_class: Object<'py> = record_class(py, RecordId::SourceLocation)?;
    let phase: Object<'py> = enum_member(py, RecordId::DiagnosticPhase, "COMPILE")?;
    let model: Object<'py> = enum_member(py, RecordId::CompiledResourceType, "MODEL")?;
    let name: Object<'py> = native_field(&model_file, "file_path")?.getattr("stem")?;
    let mut built: Vec<Object<'py>> = Vec::new();
    for row in located {
        let location: Object<'py> = match row.span {
            None => py.None().into_bound(py),
            Some(Positional(span)) => call(
                &location_class,
                &[
                    ("path", path.clone()),
                    ("line", span.line.into_bound_py_any(py)?),
                    ("column", span.column.into_bound_py_any(py)?),
                    ("end_line", span.end_line.into_bound_py_any(py)?),
                    ("end_column", span.end_column.into_bound_py_any(py)?),
                ],
            )?,
        };
        built.push(call(
            &diagnostic_class,
            &[
                ("phase", phase.clone()),
                ("severity", row.severity.into_bound_py_any(py)?),
                ("code", row.code.into_bound_py_any(py)?),
                ("message", row.message.into_bound_py_any(py)?),
                ("resource_type", model.clone()),
                ("resource_name", name.clone()),
                ("path", path.clone()),
                ("line", row.line.into_bound_py_any(py)?),
                ("column", row.column.into_bound_py_any(py)?),
                ("location", location),
            ],
        )?);
    }
    Ok(PyTuple::new(py, built)?.into_any())
}

fn call<'py>(callable: &Object<'py>, pairs: &[(&str, Object<'py>)]) -> PyResult<Object<'py>> {
    let arguments: Bound<'py, PyDict> = PyDict::new(callable.py());
    for (name, value) in pairs {
        arguments.set_item(*name, value)?;
    }
    callable.call((), Some(&arguments))
}
