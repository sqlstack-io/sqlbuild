//! Closed relation shapes of a compiled project, as binding and semantic checks read them.

use pyo3::prelude::{Bound, PyAny, PyAnyMethods, PyModule, PyModuleMethods, PyResult, Python};
use pyo3::types::{PyDict, PyDictMethods, PyFrozenSet, PyString, PyTuple};
use pyo3::{pyfunction, wrap_pyfunction};
use sqlbuild_host::entries::models::AdapterHost;

use crate::bindings::_helpers::analysis::binding_catalog::{
    BindingCatalog, CASE_SENSITIVE_BINDING_DIALECTS, CatalogKey,
};
use crate::bindings::_helpers::analysis::normalization::normalize_analysis_sql;
use crate::bindings::_helpers::analysis_session::stage_functions::infer_native_expression_source_shapes;
use crate::bindings::_helpers::compiled_project::rows::ReferenceTuple;
use crate::bindings::_helpers::compiled_views::compiled_model_view::{
    DeclaredRow, NativeCompiledModel,
};
use crate::bindings::_helpers::compiled_views::model_reads::{
    declared_rows, dynamic_columns_declared, inferred_rows, star_flags, unchecked_columns,
};
use sqlbuild_project::types::InferredRow;
use sqlbuild_python_base::boundary::models::Request;
use sqlbuild_python_base::native_host::main::host_entry::host_entry;
use sqlbuild_python_base::records::main::construct::enum_member::enum_member;
use sqlbuild_python_base::records::main::fields::field::field as native_field;
use sqlbuild_python_base::records::main::fields::mapping_get::mapping_get;
use sqlbuild_python_base::records::main::fields::replaced_keywords::replaced_keywords;
use sqlbuild_python_base::records::types::RecordId;

const UNKNOWN: &str = "UNKNOWN";
const GENERIC_DIALECT: &str = "generic";
/// Snapshot validity start column when the model names none.
pub(crate) const VALID_FROM_DEFAULT: &str = "valid_from";
/// Snapshot validity end column when the model names none.
pub(crate) const VALID_TO_DEFAULT: &str = "valid_to";
const TABLE_FUNCTION_PREFIX: &str = "__sqlbuild_table_function_";
const UDF_KIND: &str = "udf";
const TABLE_FUNCTION_KIND: &str = "table_fn";

/// The relation names `(kind, name, package, count)` reference rows bind, UDFs excluded.
pub(crate) fn relation_row_names<'py>(rows: &Bound<'py, PyAny>) -> PyResult<Vec<String>> {
    let mut pairs: Vec<(String, String)> = Vec::new();
    for row in rows.try_iter()? {
        let row: Bound<'py, PyAny> = row?;
        pairs.push((
            row.get_item(0)?.str()?.extract()?,
            row.get_item(1)?.extract()?,
        ));
    }
    Ok(relation_names(pairs))
}

/// The relations `(kind, name)` reference rows bind, each once, table functions by binding name.
fn relation_names(rows: Vec<(String, String)>) -> Vec<String> {
    let mut names: Vec<String> = Vec::new();
    for (kind, name) in rows {
        if kind == UDF_KIND {
            continue;
        }
        let name: String = if kind == TABLE_FUNCTION_KIND {
            format!("{TABLE_FUNCTION_PREFIX}{name}")
        } else {
            name
        };
        if !names.contains(&name) {
            names.push(name);
        }
    }
    names
}

/// The relations a model's reference rows bind; a native model answers from its record.
fn model_relation_names(model: &Bound<'_, PyAny>) -> PyResult<(Vec<String>, bool)> {
    if let Ok(view) = model.downcast::<NativeCompiledModel>() {
        let rows: Vec<ReferenceTuple> = view.get().reference_rows(model.py())?;
        let referenced: bool = !rows.is_empty();
        let names: Vec<String> = relation_names(
            rows.into_iter()
                .map(|ReferenceTuple { kind, name, .. }| (kind, name))
                .collect(),
        );
        return Ok((names, referenced));
    }
    let rows: Bound<'_, PyAny> = native_field(model, "reference_rows")?;
    Ok((relation_row_names(&rows)?, rows.is_truthy()?))
}

/// `{name: shapes.get(name, {})}` for each relation a model's reference rows bind.
pub(crate) fn model_inputs<'py>(
    model: &Bound<'py, PyAny>,
    shapes: &Bound<'py, PyDict>,
) -> PyResult<Bound<'py, PyDict>> {
    let py: Python<'py> = model.py();
    let inputs: Bound<'py, PyDict> = PyDict::new(py);
    for name in model_relation_names(model)?.0 {
        let shape: Bound<'py, PyAny> = match shapes.get_item(&name)? {
            Some(shape) => shape,
            None => PyDict::new(py).into_any(),
        };
        inputs.set_item(name, shape)?;
    }
    Ok(inputs)
}

/// A model relation's shape, with snapshot validity columns.
pub(crate) fn published_shape<'py>(
    model: &Bound<'py, PyAny>,
    columns: &Bound<'py, PyDict>,
    inputs: &Bound<'py, PyDict>,
    profile: &Bound<'py, PyAny>,
) -> PyResult<Bound<'py, PyDict>> {
    let shape: Bound<'py, PyDict> = binding_shape(model, columns, inputs, profile)?;
    for name in snapshot_validity_columns(model)? {
        shape.set_item(name, UNKNOWN)?;
    }
    Ok(shape)
}

/// A snapshot model's validity column names, which the snapshot adds to its query's columns.
pub(crate) fn snapshot_validity_columns(model: &Bound<'_, PyAny>) -> PyResult<Vec<String>> {
    let py: Python<'_> = model.py();
    let values: Bound<'_, PyAny> = native_field(&native_field(model, "config")?, "values")?;
    let snapshot: Bound<'_, PyAny> = enum_member(py, RecordId::MaterializationType, "SNAPSHOT")?;
    if !mapping_get(&values, "materialized")?.eq(snapshot)? {
        return Ok(Vec::new());
    }
    let mut names: Vec<String> = Vec::with_capacity(2);
    for (key, default) in [
        ("valid_from_column", VALID_FROM_DEFAULT),
        ("valid_to_column", VALID_TO_DEFAULT),
    ] {
        let name: Bound<'_, PyAny> = mapping_get(&values, key)?;
        names.push(if name.is_truthy()? {
            name.str()?.extract()?
        } else {
            default.to_owned()
        });
    }
    Ok(names)
}

/// Inferred columns, re-quoted from authored SQL where the dialect keeps quoted case.
fn binding_shape<'py>(
    model: &Bound<'py, PyAny>,
    columns: &Bound<'py, PyDict>,
    inputs: &Bound<'py, PyDict>,
    profile: &Bound<'py, PyAny>,
) -> PyResult<Bound<'py, PyDict>> {
    let py: Python<'py> = model.py();
    let dialect: Bound<'py, PyAny> = native_field(profile, "sql_analysis_dialect")?;
    let sensitive: bool = !dialect.is_none()
        && CASE_SENSITIVE_BINDING_DIALECTS.contains(&dialect.extract::<String>()?.as_str());
    if native_field(profile, "quoted_identifiers_ignore_case")?.is_truthy()? || !sensitive {
        return Ok(columns.clone());
    }
    let mut catalog: Bound<'py, PyAny> = native_field(profile, "binding_catalog")?;
    if !catalog.is_truthy()? {
        let created: BindingCatalog = BindingCatalog::create(
            py,
            CatalogKey {
                dialect: or_generic(&dialect)?.extract()?,
                quoted_ignore_case: false,
                known_functions: native_field(profile, "semantic_known_functions")?.extract()?,
                known_types: native_field(profile, "semantic_known_types")?.extract()?,
            },
            inputs.as_any(),
        )?;
        catalog = Bound::new(py, created)?.into_any();
    }
    let request: Bound<'py, PyDict> = PyDict::new(py);
    request.set_item("sql", native_field(model, "query_sql")?)?;
    request.set_item("dialect", or_generic(&dialect)?)?;
    request.set_item("stubs", PyDict::new(py))?;
    request.set_item("placeholders", PyDict::new(py))?;
    let sql: Bound<'py, PyAny> =
        PyString::new(py, &normalize_analysis_sql(py, request.extract()?)?).into_any();
    let arguments: Bound<'py, PyDict> = PyDict::new(py);
    arguments.set_item("sql", sql)?;
    arguments.set_item("columns", columns)?;
    arguments.set_item("inputs", inputs)?;
    Ok(native_field(&catalog, "inferred_schema")?
        .call((), Some(&arguments))?
        .downcast_into::<PyDict>()?)
}

fn or_generic<'py>(dialect: &Bound<'py, PyAny>) -> PyResult<Bound<'py, PyAny>> {
    if dialect.is_truthy()? {
        return Ok(dialect.clone());
    }
    Ok(PyString::new(dialect.py(), GENERIC_DIALECT).into_any())
}

/// `{name: type or "UNKNOWN"}` over `(name, type, ...)` rows.
fn typed_columns<'py>(py: Python<'py>, rows: &[InferredRow]) -> PyResult<Bound<'py, PyDict>> {
    let columns: Bound<'py, PyDict> = PyDict::new(py);
    for (name, column_type, _) in rows {
        columns.set_item(name, text_or_unknown(column_type.as_deref()))?;
    }
    Ok(columns)
}

/// `value or "UNKNOWN"` for an optional type name.
fn text_or_unknown(value: Option<&str>) -> &str {
    value.filter(|text| !text.is_empty()).unwrap_or(UNKNOWN)
}

fn or_unknown<'py>(value: &Bound<'py, PyAny>) -> PyResult<Bound<'py, PyAny>> {
    if value.is_truthy()? {
        return Ok(value.clone());
    }
    Ok(PyString::new(value.py(), UNKNOWN).into_any())
}

/// `{column.name: column.type or "UNKNOWN"}`.
fn declared_columns<'py>(
    py: Python<'py>,
    columns: &Bound<'py, PyAny>,
) -> PyResult<Bound<'py, PyDict>> {
    let shape: Bound<'py, PyDict> = PyDict::new(py);
    for column in columns.try_iter()? {
        let column: Bound<'py, PyAny> = column?;
        shape.set_item(
            native_field(&column, "name")?,
            or_unknown(&native_field(&column, "type")?)?,
        )?;
    }
    Ok(shape)
}

/// The profile shapes are read under, bound to the project's catalog.
pub(crate) fn project_profile<'py>(
    project: &Bound<'py, PyAny>,
    profile: Option<Bound<'py, PyAny>>,
) -> PyResult<Bound<'py, PyAny>> {
    let py: Python<'py> = project.py();
    let profile: Bound<'py, PyAny> = match profile {
        Some(profile) if profile.is_truthy()? => profile,
        _ => {
            let arguments: Bound<'py, PyDict> = PyDict::new(py);
            arguments.set_item(
                "sql_analysis_dialect",
                native_field(project, "sql_analysis_dialect")?,
            )?;
            host_entry(py, AdapterHost::ExpressionInferenceProfile)?.call((), Some(&arguments))?
        }
    };
    let arguments: Bound<'py, PyDict> = PyDict::new(py);
    arguments.set_item("binding_catalog", native_field(project, "binding_catalog")?)?;
    replaced_keywords(&profile, &arguments)
}

/// Closed shapes by relation name; absence means open, never an empty physical table.
pub(crate) fn project_shapes<'py>(
    project: &Bound<'py, PyAny>,
    profile: Option<Bound<'py, PyAny>>,
) -> PyResult<Bound<'py, PyDict>> {
    let py: Python<'py> = project.py();
    let profile: Bound<'py, PyAny> = project_profile(project, profile)?;
    let shapes: Bound<'py, PyDict> = PyDict::new(py);
    for seed in native_field(project, "seeds")?.try_iter()? {
        let seed: Bound<'py, PyAny> = seed?;
        shapes.set_item(
            native_field(&seed, "name")?,
            declared_columns(
                py,
                &native_field(&native_field(&seed, "schema_entry")?, "columns")?,
            )?,
        )?;
    }
    for function in native_field(project, "functions")?.try_iter()? {
        let function: Bound<'py, PyAny> = function?;
        let returns: Bound<'py, PyAny> = native_field(&function, "return_columns")?;
        if returns.is_truthy()? {
            let shape: Bound<'py, PyDict> = PyDict::new(py);
            for column in returns.try_iter()? {
                let column: Bound<'py, PyAny> = column?;
                shape.set_item(
                    native_field(&column, "name")?,
                    native_field(&column, "type")?,
                )?;
            }
            let name: String = native_field(&function, "name")?.extract()?;
            shapes.set_item(format!("{TABLE_FUNCTION_PREFIX}{name}"), shape)?;
        }
    }
    let enforced: Bound<'py, PyAny> = enum_member(py, RecordId::ContractPolicy, "ENFORCED")?;
    let mut expression_names: Vec<Bound<'py, PyAny>> = Vec::new();
    let mut expressions: Vec<Bound<'py, PyAny>> = Vec::new();
    for source in native_field(project, "sources")?.try_iter()? {
        let source: Bound<'py, PyAny> = source?;
        let entry: Bound<'py, PyAny> = native_field(&source, "source_entry")?;
        if native_field(&entry, "contract")?.eq(&enforced)? {
            shapes.set_item(
                native_field(&source, "name")?,
                declared_columns(py, &native_field(&entry, "columns")?)?,
            )?;
        } else {
            let expression: Bound<'py, PyAny> = native_field(&entry, "expression")?;
            if expression.is_truthy()? {
                expression_names.push(native_field(&source, "name")?);
                expressions.push(expression);
            }
        }
    }
    let arguments: Bound<'py, PyDict> = PyDict::new(py);
    arguments.set_item("expressions", PyTuple::new(py, expressions)?)?;
    arguments.set_item("profile", &profile)?;
    let source_shapes: Bound<'py, PyAny> =
        infer_native_expression_source_shapes(Some(&arguments))?.into_any();
    for (name, shape) in expression_names.iter().zip(source_shapes.try_iter()?) {
        let shape: Bound<'py, PyAny> = shape?;
        if !shape.is_none() {
            shapes.set_item(name, shape)?;
        }
    }
    model_shapes(project, &profile, &enforced, &shapes)?;
    Ok(shapes)
}

/// Publish model shapes in dependency rounds until no pending model can close.
fn model_shapes<'py>(
    project: &Bound<'py, PyAny>,
    profile: &Bound<'py, PyAny>,
    enforced: &Bound<'py, PyAny>,
    shapes: &Bound<'py, PyDict>,
) -> PyResult<()> {
    let py: Python<'py> = project.py();
    let mut pending: Vec<Bound<'py, PyAny>> = native_field(project, "models")?
        .try_iter()?
        .collect::<PyResult<_>>()?;
    crate::bindings::_helpers::semantic_shapes::delta_shapes::inject_model_shapes(
        &pending, shapes,
    )?;
    while !pending.is_empty() {
        let mut remaining: Vec<Bound<'py, PyAny>> = Vec::new();
        for model in &pending {
            let declared: Option<Vec<DeclaredRow>> = declared_rows(model)?;
            let inferred: Option<Vec<InferredRow>> = inferred_rows(model)?;
            let (relations, referenced): (Vec<String>, bool) = model_relation_names(model)?;
            let contract: Bound<'py, PyAny> =
                native_field(&native_field(model, "config")?, "values")?
                    .call_method1("get", ("contract",))?;
            if contract.eq(enforced)?
                && let Some(declared) = &declared
                && !dynamic_columns_declared(model)?
            {
                let unchecked: Vec<String> = unchecked_columns(model)?;
                let shape: Bound<'py, PyDict> = PyDict::new(py);
                for (name, column_type) in declared {
                    let column_type: &str = if unchecked.contains(name) {
                        UNKNOWN
                    } else {
                        text_or_unknown(column_type.as_deref())
                    };
                    shape.set_item(name, column_type)?;
                }
                shapes.set_item(native_field(model, "name")?, shape)?;
                continue;
            }
            let (has_star, star_resolved): (bool, bool) = star_flags(model)?;
            let closes: bool = inferred.as_ref().is_some_and(|rows| !rows.is_empty())
                && (!has_star || (star_resolved && referenced && all_known(&relations, shapes)?));
            if let (true, Some(inferred)) = (closes, &inferred) {
                let inputs: Bound<'py, PyDict> = model_inputs(model, shapes)?;
                let columns: Bound<'py, PyDict> = typed_columns(py, inferred)?;
                shapes.set_item(
                    native_field(model, "name")?,
                    published_shape(model, &columns, &inputs, profile)?,
                )?;
            } else {
                remaining.push(model.clone());
            }
        }
        if remaining.len() == pending.len() {
            break;
        }
        pending = remaining;
    }
    crate::bindings::_helpers::semantic_shapes::delta_shapes::record_model_shapes(project, shapes)
}

fn all_known(names: &[String], shapes: &Bound<'_, PyDict>) -> PyResult<bool> {
    for name in names {
        if !shapes.contains(name)? {
            return Ok(false);
        }
    }
    Ok(true)
}

/// Closed relation shapes; `profile` defaults to the project's dialect.
#[pyfunction]
#[pyo3(signature = (**request))]
fn semantic_shapes<'py>(request: Option<&Bound<'py, PyDict>>) -> PyResult<Bound<'py, PyDict>> {
    let request = Request::new("semantic_shapes", request)?;
    project_shapes(&request.get("project")?, request.optional("profile")?)
}

/// The analysis names the references bind, UDFs excluded.
#[pyfunction]
fn binding_relation_names<'py>(
    references: &Bound<'py, PyAny>,
) -> PyResult<Bound<'py, PyFrozenSet>> {
    let py: Python<'py> = references.py();
    let mut rows: Vec<Bound<'py, PyAny>> = Vec::new();
    for reference in references.try_iter()? {
        let reference: Bound<'py, PyAny> = reference?;
        rows.push(
            PyTuple::new(
                py,
                [
                    native_field(&reference, "ref_kind")?.str()?.into_any(),
                    native_field(&reference, "ref_name")?,
                ],
            )?
            .into_any(),
        );
    }
    let names: Vec<String> = relation_row_names(&PyTuple::new(py, rows)?.into_any())?;
    PyFrozenSet::new(py, names)
}

/// A model relation's published shape: `published_model_shape(*, model, columns, inputs, profile)`.
#[pyfunction]
#[pyo3(signature = (**request))]
fn published_model_shape<'py>(
    request: Option<&Bound<'py, PyDict>>,
) -> PyResult<Bound<'py, PyDict>> {
    let request = Request::new("published_model_shape", request)?;
    published_shape(
        &request.get("model")?,
        request.get("columns")?.downcast()?,
        request.get("inputs")?.downcast()?,
        &request.get("profile")?,
    )
}

pub(crate) fn register(module: &Bound<'_, PyModule>) -> PyResult<()> {
    module.add_function(wrap_pyfunction!(semantic_shapes, module)?)?;
    module.add_function(wrap_pyfunction!(published_model_shape, module)?)?;
    module.add_function(wrap_pyfunction!(binding_relation_names, module)?)
}
