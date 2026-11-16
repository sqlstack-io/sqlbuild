//! Query columns, expression-source shapes and dynamic pivot proofs read from Python objects.

use crate::analysis_session::main::case_sensitive_shapes::case_sensitive_shapes;
use crate::analysis_session::main::dialect_or_generic::dialect_or_generic;
use crate::analysis_session::main::nullability_rules::nullability_rules;
use crate::analysis_session::main::profile_dialect::profile_dialect;
use crate::analysis_session::main::prove_dynamic_column_contracts::prove_dynamic_column_contracts;
use crate::analysis_session::main::return_types::return_types;
use crate::analysis_session::main::shape_rows::shape_rows;
use pyo3::IntoPyObjectExt;
use pyo3::prelude::{Bound, PyAny, PyAnyMethods, PyRef, PyResult, Python};
use pyo3::pyfunction;
use pyo3::types::{PyDict, PyDictMethods, PyListMethods, PyTuple};
use sqlbuild_analysis::assembly::analysis_session::types::Pairs;

use crate::analysis::models::ProjectCatalog;
use crate::analysis_session::_helpers::profile_inputs::{aliased_dialect, profile_catalog};
use crate::analysis_session::_helpers::query_columns::{
    QueryRequestRow, QueryRow, infer_query_columns,
};
use crate::analysis_session::_helpers::session::{
    ContractRow, ExpressionShapesRow, PivotModelRow, infer_expression_source_shapes,
};
use crate::analysis_session::models::{FamilyRow, NativeModelAnalysisSession, PivotRequestRow};
use crate::analysis_session::types::ColumnsRow;
use sqlbuild_python_base::boundary::models::Request;
use sqlbuild_python_base::compile_driver::constants::EXPRESSION_SHAPE_BATCHES;
use sqlbuild_python_base::compile_driver::constants::EXPRESSION_SHAPES_INFERRED;
use sqlbuild_python_base::compile_driver::main::count_stage::count_stage;
use sqlbuild_python_base::records::main::fields::field::field as native_field;
use sqlbuild_python_base::records::main::fields::mapping_items::mapping_items;
use sqlbuild_python_front::model_config::main::headers::family_specs::family_specs;

const BATCH_MODE: &str = "batch";
const TABLE_FUNCTION_KIND: &str = "table_fn";
const TABLE_FUNCTION_ANALYSIS_PREFIX: &str = "__sqlbuild_table_function_";
/// Python's `_LINEAGE_RESOURCE_TYPES`: reference kind to compiled resource type value.
const LINEAGE_RESOURCE_TYPES: [(&str, &str); 4] = [
    ("ref", "model"),
    ("source", "source"),
    ("seed", "seed"),
    ("table_fn", "table_fn"),
];

/// Each query's `(succeeded, column rows, has star)`; a native internal failure raises.
#[pyfunction]
#[pyo3(signature = (**request))]
pub(crate) fn infer_native_query_column_rows<'py>(
    request: Option<&Bound<'py, PyDict>>,
) -> PyResult<Vec<ColumnsRow>> {
    let request = Request::new("infer_native_query_column_rows", request)?;
    let empty: Bound<'py, PyAny> = PyDict::new(request.py()).into_any();
    let nullability: Bound<'py, PyAny> =
        or_empty(request.optional("column_nullability_by_table")?, &empty)?;
    query_column_rows(
        &request.get("queries")?,
        &request.get("profile")?,
        (&empty, &nullability),
    )
}

/// One shape per expression, cached on `profile`'s binding catalog; a failure raises.
pub(crate) fn expression_source_shapes<'py>(
    expressions: &Bound<'py, PyAny>,
    profile: &Bound<'py, PyAny>,
) -> PyResult<Bound<'py, PyTuple>> {
    let py: Python<'py> = profile.py();
    let profile: Bound<'py, PyAny> = profile.clone();
    let dialect: String = aliased_dialect(py, profile_dialect(&profile)?)?;
    let catalog: Bound<'py, PyAny> = profile_catalog(&profile, &dialect)?;
    let cache: Bound<'py, PyAny> = native_field(&catalog, "expression_shapes")?;
    let pending: Bound<'py, PyDict> = PyDict::new(py);
    for expression in expressions.try_iter()? {
        let expression: Bound<'py, PyAny> = expression?;
        if !cache.contains(&expression)? {
            pending.set_item(expression, py.None())?;
        }
    }
    if !pending.is_empty() {
        count_stage(py, EXPRESSION_SHAPE_BATCHES, 1)?;
        count_stage(py, EXPRESSION_SHAPES_INFERRED, pending.len() as u64)?;
        let (rules, adapter_rules) = nullability_rules(&profile)?;
        let native: PyRef<'py, ProjectCatalog> = native_field(&catalog, "native")?.extract()?;
        let pending_names: Vec<String> = pending
            .keys()
            .iter()
            .map(|name| name.extract())
            .collect::<PyResult<_>>()?;
        let case_sensitive: bool =
            case_sensitive_shapes(&profile, &dialect.clone().into_bound_py_any(py)?)?;
        let shapes: Vec<Option<Pairs>> = infer_expression_source_shapes(
            py,
            native,
            ExpressionShapesRow {
                dialect,
                case_sensitive_shapes: case_sensitive,
                function_return_types: return_types(&profile)?,
                nullability_rules: Some(rules),
                expressions: pending_names,
            },
            adapter_rules,
        )?;
        for (expression, shape) in pending.keys().iter().zip(shapes) {
            let value: Bound<'py, PyAny> = match shape {
                Some(shape) => shape.into_py_dict_ordered(py)?,
                None => py.None().into_bound(py),
            };
            cache.set_item(expression, value)?;
        }
    }
    let mut shapes: Vec<Bound<'py, PyAny>> = Vec::new();
    for expression in expressions.try_iter()? {
        let cached: Bound<'py, PyAny> = cache.get_item(expression?)?;
        shapes.push(if cached.is_none() {
            cached
        } else {
            let copied: Bound<'py, PyDict> = PyDict::new(py);
            copied.update(cached.downcast::<PyDict>()?.as_mapping())?;
            copied.into_any()
        });
    }
    PyTuple::new(py, shapes)
}

/// The proof row per `(sql, families)` model from the session or `tables`, else None.
#[pyfunction]
#[pyo3(signature = (**request))]
pub(crate) fn prove_native_dynamic_contracts<'py>(
    request: Option<&Bound<'py, PyDict>>,
) -> PyResult<Bound<'py, PyTuple>> {
    let request = Request::new("prove_native_dynamic_contracts", request)?;
    dynamic_contract_proofs(
        &request.get("session")?,
        &request.get("tables")?,
        &request.get("models")?,
    )
}

/// The proof row per `(sql, families)` model from `session`, or from `tables` without one.
pub(crate) fn dynamic_contract_proofs<'py>(
    session: &Bound<'py, PyAny>,
    tables: &Bound<'py, PyAny>,
    models: &Bound<'py, PyAny>,
) -> PyResult<Bound<'py, PyTuple>> {
    let py: Python<'py> = models.py();
    if !models.is_truthy()? {
        return Ok(PyTuple::empty(py));
    }
    let mut rows: Vec<PivotModelRow> = Vec::new();
    for model in models.try_iter()? {
        let (sql, families): (String, Bound<'py, PyAny>) = model?.extract()?;
        rows.push((sql, family_rows(&families)?));
    }
    let contracts: Vec<ContractRow> = if session.is_none() {
        let by_table: Bound<'py, PyAny> = native_field(tables, "dynamic_families_by_table")?;
        let mut families: Vec<(String, Vec<FamilyRow>)> = Vec::new();
        for item in mapping_items(&by_table)?.try_iter()? {
            let (name, declared): (String, Bound<'py, PyAny>) = item?.extract()?;
            families.push((name, family_rows(&declared)?));
        }
        prove_dynamic_column_contracts(
            py,
            PivotRequestRow {
                dialect: dialect_or_generic(&native_field(tables, "dialect")?)?,
                column_types: shape_rows(&native_field(tables, "column_types_by_table")?)?,
                authoritative_types: shape_rows(&native_field(
                    tables,
                    "authoritative_column_types_by_table",
                )?)?,
                column_nullability: shape_rows(&native_field(
                    tables,
                    "column_nullability_by_table",
                )?)?,
                families_by_table: families,
                models: rows,
            },
        )?
    } else {
        let session: PyRef<'py, NativeModelAnalysisSession> = session.extract()?;
        session.prove_dynamic_contracts(py, rows)?
    };
    let mut proofs: Vec<Bound<'py, PyAny>> = Vec::with_capacity(contracts.len());
    for (_, proof) in contracts {
        proofs.push(proof.into_bound_py_any(py)?);
    }
    PyTuple::new(py, proofs)
}

/// Declared dynamic families as native request rows.
pub(crate) fn family_rows(families: &Bound<'_, PyAny>) -> PyResult<Vec<FamilyRow>> {
    Ok(family_specs(families)?
        .into_iter()
        .map(FamilyRow::from)
        .collect())
}

pub(crate) fn or_empty<'py>(
    value: Option<Bound<'py, PyAny>>,
    empty: &Bound<'py, PyAny>,
) -> PyResult<Bound<'py, PyAny>> {
    match value {
        Some(value) if value.is_truthy()? => Ok(value),
        _ => Ok(empty.clone()),
    }
}

pub(crate) fn query_column_rows(
    queries: &Bound<'_, PyAny>,
    profile: &Bound<'_, PyAny>,
    (types, nullability): (&Bound<'_, PyAny>, &Bound<'_, PyAny>),
) -> PyResult<Vec<ColumnsRow>> {
    let py: Python<'_> = profile.py();
    if !queries.is_truthy()? {
        return Ok(Vec::new());
    }
    let queries: Vec<Bound<'_, PyAny>> = queries.try_iter()?.collect::<PyResult<_>>()?;
    let normalization_dialect: String = profile_dialect(profile)?;
    let dialect: String = aliased_dialect(py, normalization_dialect.clone())?;
    let mut batch: bool = false;
    for query in &queries {
        if native_field(query, "mode")?.eq(BATCH_MODE)? {
            batch = true;
            break;
        }
    }
    let catalog: Option<Bound<'_, PyAny>> = if batch {
        Some(native_field(
            &profile_catalog(profile, &dialect)?,
            "native",
        )?)
    } else {
        None
    };
    let (rules, adapter_rules) = nullability_rules(profile)?;
    let mut rows: Vec<QueryRow> = Vec::with_capacity(queries.len());
    for query in &queries {
        rows.push(query_row(query)?);
    }
    let catalog: Option<PyRef<'_, ProjectCatalog>> =
        catalog.map(|native| native.extract()).transpose()?;
    infer_query_columns(
        py,
        catalog,
        QueryRequestRow {
            dialect,
            function_return_types: return_types(profile)?,
            nullability_rules: rules,
            types: shape_rows(types)?,
            nullability: shape_rows(nullability)?,
            queries: rows,
            normalization_dialect,
        },
        adapter_rules,
    )
}

fn query_row(query: &Bound<'_, PyAny>) -> PyResult<QueryRow> {
    let placeholders: Bound<'_, PyAny> = native_field(query, "placeholders")?;
    let placeholders: Pairs = if placeholders.is_truthy()? {
        mapping_items(&placeholders)?
            .try_iter()?
            .map(|item| item?.extract())
            .collect::<PyResult<_>>()?
    } else {
        Vec::new()
    };
    let mut analysis_names: Vec<String> = Vec::new();
    let mut lineage: Vec<(String, String, String)> = Vec::new();
    for reference in native_field(query, "references")?.try_iter()? {
        let reference: Bound<'_, PyAny> = reference?;
        let kind: String = native_field(&reference, "ref_kind")?.extract()?;
        let name: String = native_field(&reference, "ref_name")?.extract()?;
        let analysis_name: String = if kind == TABLE_FUNCTION_KIND {
            format!("{TABLE_FUNCTION_ANALYSIS_PREFIX}{name}")
        } else {
            name.clone()
        };
        analysis_names.push(analysis_name.clone());
        if let Some((_, resource_type)) = LINEAGE_RESOURCE_TYPES
            .iter()
            .find(|(reference_kind, _)| *reference_kind == kind)
        {
            let entry = (analysis_name, (*resource_type).to_owned(), name);
            match lineage
                .iter_mut()
                .find(|(existing, _, _)| *existing == entry.0)
            {
                Some(existing) => *existing = entry,
                None => lineage.push(entry),
            }
        }
    }
    Ok(QueryRow {
        sql: native_field(query, "sql")?.extract()?,
        placeholders,
        analysis_names,
        lineage_references: lineage,
        recover_cte_facts: native_field(query, "recover_cte_facts")?.extract()?,
        mode: native_field(query, "mode")?.extract()?,
    })
}

trait OrderedDict {
    fn into_py_dict_ordered<'py>(self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>>;
}

impl OrderedDict for Pairs {
    fn into_py_dict_ordered<'py>(self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        let dict: Bound<'py, PyDict> = PyDict::new(py);
        for (key, value) in self {
            dict.set_item(key, value)?;
        }
        Ok(dict.into_any())
    }
}
