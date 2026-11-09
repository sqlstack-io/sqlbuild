//! Model contract and promotion-conflict diagnostics, built as compile's `CompilerDiagnostic`s.

use std::collections::HashMap;

use pyo3::prelude::{
    Bound, PyAny, PyAnyMethods, PyModule, PyModuleMethods, PyRef, PyResult, Python,
};
use pyo3::types::{PyDict, PyDictMethods, PyTuple};
use pyo3::{IntoPyObject, pyfunction, wrap_pyfunction};
use serde_json::Value;
use sqlbuild_analysis::compiled_project::main::project_models::project_models;
use sqlbuild_analysis::compiled_project::models::CompiledModelFacts;
use sqlbuild_analysis::contracts::constants::CONTRACT_ENFORCED;
use sqlbuild_analysis::contracts::main::enum_literal_violations::enum_literal_violations;
use sqlbuild_analysis::contracts::models::{
    EnumColumn, EnumLiteralModel, EnumLiteralViolation, LiteralLocation,
};
use sqlbuild_core::constants::STAGED_PROMOTION_MODE;
use sqlbuild_sqltext::compiler::main::model_output_columns::model_output_columns;

use crate::bindings::_helpers::compiled_project::project::NativeCompiledProject;
use crate::bindings::_helpers::compiled_views::model_reads::enum_columns;
use crate::bindings::_helpers::compiled_views::records::compiled_project_facts;
use crate::bindings::_helpers::contracts::model_contracts::{
    ConflictRow, DiagnosticRow, evaluate_project_model_contracts, project_promotion_conflicts,
};
use crate::bindings::_helpers::records::construct::{enum_from_value, record_class};
use crate::bindings::_helpers::records::fields::field as native_field;
use crate::bindings::_helpers::records::ids::RecordId;
use crate::bindings::_helpers::sqltext::model_headers::{ColumnLocation, location_rows};

const PATHLIB_MODULE: &str = "pathlib";
const IMPLICIT_COLUMN_CONTRACTS: &str = "implicit";
const GENERIC_DIALECT: &str = "generic";
const OUTPUT_LABEL: &str = "output";
const CONTRACT_PHASE: &str = "contract";
const MODEL_RESOURCE: &str = "model";
const SQL_ANALYSIS_KEY: &str = "sql_analysis";

fn keyword_call<'py>(
    callable: &Bound<'py, PyAny>,
    pairs: Vec<(&str, Bound<'py, PyAny>)>,
) -> PyResult<Bound<'py, PyAny>> {
    let keywords: Bound<'py, PyDict> = PyDict::new(callable.py());
    for (name, value) in pairs {
        keywords.set_item(name, value)?;
    }
    callable.call((), Some(&keywords))
}

/// The compile's retained native project, read through `_native.compiled_project_facts`.
fn native_project<'py>(project: &Bound<'py, PyAny>) -> PyResult<Bound<'py, PyAny>> {
    Ok(compiled_project_facts(project)?.into_any())
}

/// Where the model's SQL produces `column_name`, recovered from the authored SQL when needed.
fn output_column_location<'py>(
    model: &Bound<'py, PyAny>,
    column_name: &str,
) -> PyResult<Option<Bound<'py, PyAny>>> {
    let recorded: Bound<'py, PyAny> =
        native_field(model, "output_column_locations")?.call_method1("get", (column_name,))?;
    let authored: Option<String> = native_field(model, "authored_sql")?.extract()?;
    let authored: String = authored.unwrap_or_default();
    if !recorded.is_none() || authored.is_empty() {
        return Ok(Some(recorded).filter(|location| !location.is_none()));
    }
    let implicit: bool = native_field(model, "extract_implicit_alias_columns")?.is_truthy()?;
    let rows: Vec<ColumnLocation> = location_rows(model_output_columns(&authored, implicit));
    let Some(ColumnLocation {
        line,
        column,
        end_line,
        end_column,
        ..
    }) = rows
        .into_iter()
        .rev()
        .find(|location| location.name == column_name)
    else {
        return Ok(None);
    };
    let py: Python<'py> = model.py();
    let location: Bound<'py, PyAny> = keyword_call(
        &record_class(py, RecordId::SourceLocation)?,
        vec![
            ("path", native_field(model, "relative_path")?),
            ("line", line.into_pyobject(py)?.into_any()),
            ("column", column.into_pyobject(py)?.into_any()),
            ("end_line", end_line.into_pyobject(py)?.into_any()),
            ("end_column", end_column.into_pyobject(py)?.into_any()),
        ],
    )?;
    Ok(Some(location))
}

/// The diagnostic's primary location: a declared column, else the output column.
fn diagnostic_location<'py>(
    model: &Bound<'py, PyAny>,
    (declared_index, output_column): (Option<usize>, Option<&str>),
) -> PyResult<Bound<'py, PyAny>> {
    let py: Python<'py> = model.py();
    let schema_entry: Bound<'py, PyAny> = native_field(model, "schema_entry")?;
    if let Some(index) = declared_index
        && !schema_entry.is_none()
    {
        return native_field(&schema_entry, "columns")?
            .get_item(index)?
            .getattr("location");
    }
    if let Some(column_name) = output_column
        && let Some(location) = output_column_location(model, column_name)?
    {
        return Ok(location);
    }
    Ok(py.None().into_bound(py))
}

fn related_locations<'py>(
    model: &Bound<'py, PyAny>,
    related: Option<(String, String)>,
) -> PyResult<Bound<'py, PyTuple>> {
    let py: Python<'py> = model.py();
    let Some((column_name, message)) = related else {
        return Ok(PyTuple::empty(py));
    };
    let Some(location) = output_column_location(model, &column_name)? else {
        return Ok(PyTuple::empty(py));
    };
    let related: Bound<'py, PyAny> = keyword_call(
        &record_class(py, RecordId::RelatedLocation)?,
        vec![
            ("label", OUTPUT_LABEL.into_pyobject(py)?.into_any()),
            ("location", location),
            ("message", message.into_pyobject(py)?.into_any()),
        ],
    )?;
    PyTuple::new(py, [related])
}

fn contract_diagnostic<'py>(
    model: &Bound<'py, PyAny>,
    row: DiagnosticRow,
) -> PyResult<Bound<'py, PyAny>> {
    let py: Python<'py> = model.py();
    let DiagnosticRow {
        code,
        is_error,
        message,
        column_name,
        declared_index,
        output_column,
        related,
        help,
    } = row;
    let severity: &str = if is_error { "error" } else { "warning" };
    keyword_call(
        &record_class(py, RecordId::CompilerDiagnostic)?,
        vec![
            ("phase", CONTRACT_PHASE.into_pyobject(py)?.into_any()),
            ("severity", severity.into_pyobject(py)?.into_any()),
            ("code", code.into_pyobject(py)?.into_any()),
            ("message", message.into_pyobject(py)?.into_any()),
            ("resource_type", model_resource_type(py)?),
            ("resource_name", native_field(model, "name")?),
            ("column_name", column_name.into_pyobject(py)?.into_any()),
            ("path", native_field(model, "relative_path")?),
            (
                "location",
                diagnostic_location(model, (declared_index, output_column.as_deref()))?,
            ),
            (
                "related_locations",
                related_locations(model, related)?.into_any(),
            ),
            ("help", help.into_pyobject(py)?.into_any()),
        ],
    )
}

/// The model's enum-typed columns whose enum has only string members, in column order.
fn string_enum_columns(model: &Bound<'_, PyAny>) -> PyResult<Vec<EnumColumn>> {
    let mut columns: Vec<EnumColumn> = Vec::new();
    for (name, declaration) in enum_columns(model)?.downcast_into::<PyDict>()?.iter() {
        let name: String = name.extract()?;
        let mut members: Vec<String> = Vec::new();
        for member in native_field(&declaration, "members")?.try_iter()? {
            match native_field(&member?, "value")?.extract::<String>() {
                Ok(value) => members.push(value),
                Err(_) => {
                    members.clear();
                    break;
                }
            }
        }
        if !members.is_empty() {
            columns.push(EnumColumn {
                name,
                enum_name: native_field(&declaration, "name")?.extract()?,
                members,
            });
        }
    }
    Ok(columns)
}

/// The K013 violations of a model with an enforced contract and SQL analysis left on.
fn model_enum_violations(
    model: &Bound<'_, PyAny>,
    facts: Option<&CompiledModelFacts>,
    dialect: &str,
) -> PyResult<Vec<EnumLiteralViolation>> {
    let Some(facts) = facts else {
        return Ok(Vec::new());
    };
    let enforced: bool = facts.config.contract.as_deref() == Some(CONTRACT_ENFORCED);
    let analysed: bool = facts.config.values.get(SQL_ANALYSIS_KEY) != Some(&Value::Bool(false));
    if !enforced || !analysed {
        return Ok(Vec::new());
    }
    let columns: Vec<EnumColumn> = string_enum_columns(model)?;
    if columns.is_empty() {
        return Ok(Vec::new());
    }
    Ok(enum_literal_violations(&EnumLiteralModel {
        name: &facts.name,
        dialect,
        query_sql: &facts.query_sql,
        authored_sql: &facts.authored_sql,
        authored_query_sql: &facts.authored_query_sql,
        columns,
    }))
}

fn literal_source_location<'py>(
    model: &Bound<'py, PyAny>,
    location: LiteralLocation,
) -> PyResult<Bound<'py, PyAny>> {
    let py: Python<'py> = model.py();
    keyword_call(
        &record_class(py, RecordId::SourceLocation)?,
        vec![
            ("path", native_field(model, "relative_path")?),
            ("line", location.line.into_pyobject(py)?.into_any()),
            ("column", location.column.into_pyobject(py)?.into_any()),
            ("end_line", location.end_line.into_pyobject(py)?.into_any()),
            (
                "end_column",
                location.end_column.into_pyobject(py)?.into_any(),
            ),
        ],
    )
}

fn enum_literal_diagnostic<'py>(
    model: &Bound<'py, PyAny>,
    violation: EnumLiteralViolation,
) -> PyResult<Bound<'py, PyAny>> {
    let py: Python<'py> = model.py();
    let location: Bound<'py, PyAny> = match violation.location {
        Some(location) => literal_source_location(model, location)?,
        None => diagnostic_location(model, (None, Some(&violation.column_name)))?,
    };
    keyword_call(
        &record_class(py, RecordId::CompilerDiagnostic)?,
        vec![
            ("phase", CONTRACT_PHASE.into_pyobject(py)?.into_any()),
            ("severity", "error".into_pyobject(py)?.into_any()),
            ("code", violation.code.into_pyobject(py)?.into_any()),
            ("message", violation.message.into_pyobject(py)?.into_any()),
            ("resource_type", model_resource_type(py)?),
            ("resource_name", native_field(model, "name")?),
            (
                "column_name",
                violation.column_name.into_pyobject(py)?.into_any(),
            ),
            ("path", native_field(model, "relative_path")?),
            ("location", location),
            ("help", violation.help.into_pyobject(py)?.into_any()),
        ],
    )
}

fn model_resource_type(py: Python<'_>) -> PyResult<Bound<'_, PyAny>> {
    enum_from_value(py, RecordId::CompiledResourceType, MODEL_RESOURCE)
}

/// Every model's column contract diagnostics, in model order.
///
/// `adapter_default` is the adapter's table promotion mode; without it no build counts as staged.
#[pyfunction]
#[pyo3(signature = (*, project, dialect = None, adapter_default = None))]
pub(crate) fn evaluate_model_contracts<'py>(
    py: Python<'py>,
    project: &Bound<'py, PyAny>,
    dialect: Option<&Bound<'py, PyAny>>,
    adapter_default: Option<&Bound<'py, PyAny>>,
) -> PyResult<Bound<'py, PyAny>> {
    let settings: Bound<'py, PyAny> = native_field(project, "settings")?;
    let mode: String = settings.getattr("column_contract_mode")?.str()?.extract()?;
    let explicit_promotion: Option<String> = settings.getattr("table_promotion_mode")?.extract()?;
    let promotion: Option<String> = match explicit_promotion {
        Some(explicit) => Some(explicit),
        None => adapter_default
            .map(|default| default.str()?.extract())
            .transpose()?,
    };
    let mut dialect_name: String = GENERIC_DIALECT.to_owned();
    if let Some(dialect) = dialect
        && dialect.is_truthy()?
    {
        dialect_name = dialect.str()?.extract()?;
    }
    let native: Bound<'py, PyAny> = native_project(project)?;
    let native: PyRef<'py, NativeCompiledProject> = native.extract()?;
    let outcomes: Vec<(String, Vec<DiagnosticRow>)> = evaluate_project_model_contracts(
        py,
        (
            dialect_name.clone(),
            mode == IMPLICIT_COLUMN_CONTRACTS,
            promotion.as_deref() == Some(STAGED_PROMOTION_MODE),
        ),
        &native,
    )?;
    let facts: HashMap<&str, &CompiledModelFacts> = project_models(native.facts())
        .iter()
        .map(|facts| (facts.name.as_str(), facts))
        .collect();
    let latest: HashMap<&str, &[DiagnosticRow]> = outcomes
        .iter()
        .map(|(owner, rows)| (owner.as_str(), rows.as_slice()))
        .collect();
    let mut diagnostics: Vec<Bound<'py, PyAny>> = Vec::new();
    for model in native_field(project, "models")?.try_iter()? {
        let model: Bound<'py, PyAny> = model?;
        let name: String = native_field(&model, "name")?.extract()?;
        for row in latest.get(name.as_str()).copied().unwrap_or_default() {
            diagnostics.push(contract_diagnostic(&model, row.clone())?);
        }
        let model_facts: Option<&CompiledModelFacts> = facts.get(name.as_str()).copied();
        for violation in model_enum_violations(&model, model_facts, &dialect_name)? {
            diagnostics.push(enum_literal_diagnostic(&model, violation)?);
        }
    }
    keyword_call(
        &record_class(py, RecordId::ContractValidationResult)?,
        vec![("diagnostics", PyTuple::new(py, diagnostics)?.into_any())],
    )
}

/// One K011 error per enforced-contract table model under immediate promotion.
#[pyfunction]
#[pyo3(signature = (*, project, adapter_default, settings_file))]
pub(crate) fn promotion_conflict_diagnostics<'py>(
    py: Python<'py>,
    project: &Bound<'py, PyAny>,
    adapter_default: &Bound<'py, PyAny>,
    settings_file: String,
) -> PyResult<Bound<'py, PyTuple>> {
    let explicit_mode: Option<String> = native_field(project, "settings")?
        .getattr("table_promotion_mode")?
        .extract()?;
    let native: Bound<'py, PyAny> = native_project(project)?;
    let native: PyRef<'py, NativeCompiledProject> = native.extract()?;
    let conflicts: Vec<ConflictRow> = project_promotion_conflicts(
        py,
        (
            explicit_mode,
            adapter_default.str()?.extract()?,
            settings_file,
        ),
        &native,
    )?;
    let diagnostic_type: Bound<'py, PyAny> = record_class(py, RecordId::CompilerDiagnostic)?;
    let path_type: Bound<'py, PyAny> = py.import(PATHLIB_MODULE)?.getattr("Path")?;
    let mut diagnostics: Vec<Bound<'py, PyAny>> = Vec::with_capacity(conflicts.len());
    for ConflictRow {
        name,
        relative_path,
        code,
        message,
        help,
    } in conflicts
    {
        diagnostics.push(keyword_call(
            &diagnostic_type,
            vec![
                ("phase", CONTRACT_PHASE.into_pyobject(py)?.into_any()),
                ("severity", "error".into_pyobject(py)?.into_any()),
                ("code", code.into_pyobject(py)?.into_any()),
                ("message", message.into_pyobject(py)?.into_any()),
                ("resource_type", model_resource_type(py)?),
                ("resource_name", name.into_pyobject(py)?.into_any()),
                ("path", path_type.call1((relative_path,))?),
                ("help", help.into_pyobject(py)?.into_any()),
            ],
        )?);
    }
    PyTuple::new(py, diagnostics)
}

pub(crate) fn register(module: &Bound<'_, PyModule>) -> PyResult<()> {
    module.add_function(wrap_pyfunction!(evaluate_model_contracts, module)?)?;
    module.add_function(wrap_pyfunction!(promotion_conflict_diagnostics, module)?)?;
    Ok(())
}
