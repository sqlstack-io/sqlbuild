//! Compile usage merging, placement validation and move relocation over a Python `ScopeIndex`.

use crate::scopes::main::checks::relocated_declarations::relocated_declarations;
use pyo3::prelude::{Bound, Py, PyAny, PyAnyMethods, PyResult, Python};
use pyo3::types::{PyDict, PyDictMethods, PyTuple, PyTupleMethods};
use pyo3::{IntoPyObjectExt, pyfunction};
use sqlbuild_scopes::compile_usages::models::UsageKey;
use sqlbuild_scopes::placement::main::diagnostic_order::diagnostic_order;
use sqlbuild_scopes::placement::main::placement_diagnostics::placement_diagnostics;
use sqlbuild_scopes::placement::models::{
    DiagnosticOrderKey, PlacementCode, PlacementDiagnostic, PlacementIndex,
};
use sqlbuild_scopes::scope_index::models::ResourceIdentity;

use crate::scopes::_helpers::declarations::placement::{QualifiedRow, resource_identity};
use crate::scopes::_helpers::index::scope_objects::{
    enum_text, field, identity_row, optional_item, qualified_row, read_completeness,
    read_placement_index, tuple_attribute, value_of,
};
use crate::scopes::_helpers::relationships::compile_usages::{
    MergeRows, SourceRow, merged_usage_rows,
};
use crate::scopes::models::ScopeClasses;
use sqlbuild_python_base::boundary::main::compiler_error::compiler_error;
use sqlbuild_python_base::records::main::fields::field::field as native_field;
use sqlbuild_python_base::records::main::fields::replaced_keywords::replaced_keywords;
use sqlbuild_python_base::records::types::RecordId;
use sqlbuild_python_base::types::CompilerDetach;

pub(crate) const RUNTIME_USAGE: usize = 2;
/// Compile input groups whose items carry declaration usages, after the model inputs.
pub(crate) const COMPILE_USAGE_GROUPS: [&str; 5] = [
    "source_inputs",
    "sql_function_inputs",
    "audit_inputs",
    "test_inputs",
    "scenario_inputs",
];

/// `dataclasses.replace(item, **changes)`.
pub(crate) fn replace<'py>(
    item: &Bound<'py, PyAny>,
    changes: &[(&str, Bound<'py, PyAny>)],
) -> PyResult<Bound<'py, PyAny>> {
    let py: Python<'py> = item.py();
    let keywords: Bound<'py, PyDict> = PyDict::new(py);
    for (name, value) in changes {
        keywords.set_item(*name, value)?;
    }
    replaced_keywords(item, &keywords)
}

fn tuple_from<'py>(py: Python<'py>, items: Vec<Bound<'py, PyAny>>) -> PyResult<Bound<'py, PyAny>> {
    Ok(PyTuple::new(py, items)?.into_any())
}

fn usage_row(usage: &Bound<'_, PyAny>) -> PyResult<UsageKey> {
    Ok(UsageKey {
        consumer: qualified_row(&native_field(usage, "consumer")?)?,
        declaration: qualified_row(&native_field(usage, "declaration")?)?,
        kind: value_of(usage, "kind")?,
        through: optional_item(usage, "through")?
            .map(|through| qualified_row(&through))
            .transpose()?,
        enum_member: field(usage, "enum_member")?,
    })
}

fn dependency_row(
    record: &Bound<'_, PyAny>,
) -> PyResult<(QualifiedRow, Option<Vec<QualifiedRow>>)> {
    let dependencies: Option<Vec<QualifiedRow>> = match optional_item(record, "macro")? {
        None => None,
        Some(metadata) => Some(
            native_field(&metadata, "dependencies")?
                .try_iter()?
                .map(|item| qualified_row(&item?))
                .collect::<PyResult<_>>()?,
        ),
    };
    Ok((
        qualified_row(&native_field(record, "identity")?)?,
        dependencies,
    ))
}

fn with_dependencies<'py>(
    record: Bound<'py, PyAny>,
    sources: Option<Vec<SourceRow>>,
    usages: &[Bound<'py, PyAny>],
) -> PyResult<Bound<'py, PyAny>> {
    let (Some(metadata), Some(sources)) = (optional_item(&record, "macro")?, sources) else {
        return Ok(record);
    };
    let own: Bound<'py, PyTuple> = tuple_attribute(&metadata, "dependencies")?;
    let dependencies: Vec<Bound<'py, PyAny>> = sources
        .into_iter()
        .map(|(is_own, position)| {
            if is_own {
                own.get_item(position)
            } else {
                native_field(&usages[position], "declaration")
            }
        })
        .collect::<PyResult<_>>()?;
    let py: Python<'py> = record.py();
    let metadata: Bound<'py, PyAny> = replace(
        &metadata,
        &[("dependencies", tuple_from(py, dependencies)?)],
    )?;
    replace(&record, &[("macro", metadata)])
}

/// Python's `merged_usage_index`: compile usages merged into usages and macro dependencies.
fn merged_usage_index<'py>(
    index: &Bound<'py, PyAny>,
    collected: &Bound<'py, PyTuple>,
) -> PyResult<Bound<'py, PyAny>> {
    let py: Python<'py> = index.py();
    let records: Bound<'py, PyTuple> = tuple_attribute(index, "declarations")?;
    let prior: Bound<'py, PyTuple> = tuple_attribute(index, "usages")?;
    let declaration_rows: Vec<(QualifiedRow, Option<Vec<QualifiedRow>>)> = records
        .iter()
        .map(|record| dependency_row(&record))
        .collect::<PyResult<_>>()?;
    let prior_rows: Vec<UsageKey> = prior
        .iter()
        .map(|usage| usage_row(&usage))
        .collect::<PyResult<_>>()?;
    let collected_rows: Vec<UsageKey> = collected
        .iter()
        .map(|usage| usage_row(&usage))
        .collect::<PyResult<_>>()?;
    let (distinct, merged, combined): MergeRows =
        merged_usage_rows(&declaration_rows, &prior_rows, &collected_rows);
    let usages: Vec<Bound<'py, PyAny>> = distinct
        .iter()
        .map(|position| collected.get_item(*position))
        .collect::<PyResult<_>>()?;
    let mut all_usages: Vec<Bound<'py, PyAny>> = prior.iter().collect();
    all_usages.extend(usages.iter().cloned());
    let declarations: Vec<Bound<'py, PyAny>> = records
        .iter()
        .zip(merged)
        .map(|(record, sources)| with_dependencies(record, sources, &usages))
        .collect::<PyResult<_>>()?;
    let true_value: Bound<'py, PyAny> = true.into_bound_py_any(py)?;
    let completeness: Bound<'py, PyAny> = replace(
        &native_field(index, "completeness")?,
        &[
            ("runtime_usage", true_value.clone()),
            ("promotion_impact", true_value),
        ],
    )?;
    replace(
        index,
        &[
            ("declarations", tuple_from(py, declarations)?),
            (
                "usages",
                tuple_from(
                    py,
                    combined
                        .iter()
                        .map(|position| all_usages[*position].clone())
                        .collect(),
                )?,
            ),
            ("completeness", completeness),
        ],
    )
}

fn order_key(diagnostic: &Bound<'_, PyAny>) -> PyResult<DiagnosticOrderKey> {
    Ok(DiagnosticOrderKey {
        path: field(diagnostic, "path")?,
        line: field(diagnostic, "line")?,
        column: field(diagnostic, "column")?,
        code: enum_text(&native_field(diagnostic, "code")?)?,
        message: field(diagnostic, "message")?,
    })
}

fn with_placement<'py>(
    index: &Bound<'py, PyAny>,
    diagnostics: Vec<Bound<'py, PyAny>>,
    placement: bool,
) -> PyResult<Bound<'py, PyAny>> {
    let py: Python<'py> = index.py();
    let completeness: Bound<'py, PyAny> = replace(
        &native_field(index, "completeness")?,
        &[("placement", placement.into_bound_py_any(py)?)],
    )?;
    replace(
        index,
        &[
            ("diagnostics", tuple_from(py, diagnostics)?),
            ("completeness", completeness),
        ],
    )
}

/// Python's `build_placement_validated_index`.
fn placement_validated_index<'py>(
    index: &Bound<'py, PyAny>,
    enforce_placement: bool,
) -> PyResult<Bound<'py, PyAny>> {
    let py: Python<'py> = index.py();
    let mut kept: Vec<Bound<'py, PyAny>> = Vec::new();
    let mut keys: Vec<DiagnosticOrderKey> = Vec::new();
    for diagnostic in tuple_attribute(index, "diagnostics")?.iter() {
        if PlacementCode::parse(&enum_text(&native_field(&diagnostic, "code")?)?).is_none() {
            keys.push(order_key(&diagnostic)?);
            kept.push(diagnostic);
        }
    }
    if !read_completeness(index)?[RUNTIME_USAGE] {
        return with_placement(index, kept, false);
    }
    let classes: ScopeClasses<'py> = ScopeClasses::new(py)?;
    let severity: Bound<'py, PyAny> = classes.member(
        RecordId::ScopeDiagnosticSeverity,
        if enforce_placement {
            "error"
        } else {
            "warning"
        },
    )?;
    let placement: PlacementIndex = read_placement_index(index)?;
    let found: Vec<PlacementDiagnostic> = py
        .compiler_detach(|| Ok(placement_diagnostics(&placement)))
        .map_err(compiler_error)?;
    let records: Bound<'py, PyTuple> = tuple_attribute(index, "declarations")?;
    let diagnostic_class: Bound<'py, PyAny> = classes.model(RecordId::ScopeDiagnostic)?;
    for item in found {
        let record: Bound<'py, PyAny> = records.get_item(item.declaration)?;
        let native = &placement.declarations[item.declaration];
        keys.push(DiagnosticOrderKey {
            path: Some(native.path.clone()),
            line: Some(native.line),
            column: Some(native.column),
            code: item.code.as_str().to_owned(),
            message: item.message.clone(),
        });
        kept.push(diagnostic_class.call1((
            classes.member(RecordId::ScopeDiagnosticCode, item.code.as_str())?,
            item.message,
            &severity,
            native_field(&record, "path")?,
            native_field(&record, "line")?,
            native_field(&record, "column")?,
            native_field(&record, "identity")?,
        ))?);
    }
    let ordered: Vec<Bound<'py, PyAny>> = diagnostic_order(&keys)
        .into_iter()
        .map(|position| kept[position].clone())
        .collect();
    with_placement(index, ordered, true)
}

/// Complete usage and placement diagnostics, merging any collected compile usages first.
#[pyfunction]
#[pyo3(
    name = "get_placement_validated_scope_index",
    signature = (*, index, enforce_placement = true, compile_usages = None)
)]
pub(crate) fn scope_placement_validated_index(
    index: &Bound<'_, PyAny>,
    enforce_placement: bool,
    compile_usages: Option<Bound<'_, PyTuple>>,
) -> PyResult<Py<PyAny>> {
    let merged: Bound<'_, PyAny> = match compile_usages {
        Some(collected) => merged_usage_index(index, &collected)?,
        None => index.clone(),
    };
    Ok(placement_validated_index(&merged, enforce_placement)?.unbind())
}

/// Declarations whose placement changes when `resource` moves, or `None` if unsettled.
#[pyfunction]
#[pyo3(
    name = "relocate_declarations_for_move",
    signature = (*, index, resource, destination)
)]
pub(crate) fn scope_relocated_declarations(
    index: &Bound<'_, PyAny>,
    resource: &Bound<'_, PyAny>,
    destination: &str,
) -> PyResult<Option<Py<PyTuple>>> {
    let resource: ResourceIdentity = resource_identity(identity_row(resource)?)?;
    let relocated = relocated_declarations(index, &[(resource, destination.to_owned())])?;
    Ok(relocated.map(|(records, _)| records.unbind()))
}

pub(crate) fn usage_records<'py>(records: &Bound<'py, PyAny>) -> PyResult<Vec<Bound<'py, PyAny>>> {
    records.try_iter()?.collect()
}
