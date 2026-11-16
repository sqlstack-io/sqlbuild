//! A compiled model's analysis outcomes as Rust data: views read their owner, records their fields.

use pyo3::prelude::{Bound, PyAny, PyAnyMethods, PyResult, Python};

use crate::bindings::_helpers::compiled_views::compiled_model_view::{
    DeclaredRow, LineageInput, NativeCompiledModel,
};
use sqlbuild_project::types::InferredRow;
use sqlbuild_python_base::compile_driver::constants::COMPILED_VIEW_FALLBACKS;
use sqlbuild_python_base::compile_driver::main::count_stage::count_stage;
use sqlbuild_python_base::records::main::fields::field::field as native_field;

/// One lineage output with its transform kind and confidence values, and its sources.
pub(crate) struct LineageFactRow {
    pub(crate) output_column: String,
    pub(crate) transform: String,
    pub(crate) confidence: String,
    /// `(resource type, resource, column)` of each source.
    pub(crate) sources: Vec<(String, String, String)>,
}

const TRANSFORM_VALUES: [&str; 6] = [
    "direct",
    "cast",
    "expression",
    "aggregation",
    "star",
    "constant",
];
const CONFIDENCE_VALUES: [&str; 3] = ["unknown", "high", "medium"];

/// The native view behind a model object; any other object is a Python-built record, counted.
#[inline]
pub(crate) fn native_model<'a>(
    model: &'a Bound<'_, PyAny>,
) -> PyResult<Option<&'a NativeCompiledModel>> {
    if let Ok(view) = model.cast::<NativeCompiledModel>() {
        return Ok(Some(view.get()));
    }
    count_stage(model.py(), COMPILED_VIEW_FALLBACKS, 1)?;
    Ok(None)
}

/// Each inferred output column as `(name, type, nullability)`, `None` when none were inferred.
pub(crate) fn inferred_rows(model: &Bound<'_, PyAny>) -> PyResult<Option<Vec<InferredRow>>> {
    if let Some(view) = native_model(model)? {
        return view.inferred_rows(model.py());
    }
    let rows: Bound<'_, PyAny> = native_field(model, "inferred_rows")?;
    if rows.is_none() {
        return Ok(None);
    }
    let mut inferred: Vec<InferredRow> = Vec::new();
    for row in rows.try_iter()? {
        let (name, column_type, nullability): (String, Option<String>, Bound<'_, PyAny>) =
            row?.extract()?;
        inferred.push((name, column_type, nullability.str()?.extract()?));
    }
    Ok(Some(inferred))
}

/// The names of the inferred output columns, `None` when the shape is unknown.
pub(crate) fn inferred_names(model: &Bound<'_, PyAny>) -> PyResult<Option<Vec<String>>> {
    Ok(inferred_rows(model)?.map(column_names))
}

fn column_names(rows: Vec<InferredRow>) -> Vec<String> {
    rows.into_iter().map(|(name, _, _)| name).collect()
}

/// Each declared column of the attached schema as `(name, type)`, `None` without a schema.
pub(crate) fn declared_rows(model: &Bound<'_, PyAny>) -> PyResult<Option<Vec<DeclaredRow>>> {
    if let Some(view) = native_model(model)? {
        return view.schema_column_rows(model.py());
    }
    native_field(model, "schema_column_rows")?.extract()
}

/// Whether the attached schema declares dynamic column families.
pub(crate) fn dynamic_columns_declared(model: &Bound<'_, PyAny>) -> PyResult<bool> {
    if let Some(view) = native_model(model)? {
        return view.dynamic_columns_declared(model.py());
    }
    native_field(model, "has_dynamic_columns")?.is_truthy()
}

/// `(has star, star resolved)` of the model's analysed query.
pub(crate) fn star_flags(model: &Bound<'_, PyAny>) -> PyResult<(bool, bool)> {
    if let Some(view) = native_model(model)? {
        return Ok((
            view.fast_lineage_has_star(model.py())?,
            view.fast_lineage_star_resolved(model.py())?,
        ));
    }
    Ok((
        native_field(model, "fast_lineage_has_star")?.is_truthy()?,
        native_field(model, "fast_lineage_star_resolved")?.is_truthy()?,
    ))
}

/// Output columns whose types semantic recovery forgot, in any order.
pub(crate) fn unchecked_columns(model: &Bound<'_, PyAny>) -> PyResult<Vec<String>> {
    if let Some(view) = native_model(model)? {
        return Ok(view.unchecked_columns().to_vec());
    }
    native_field(model, "unchecked_output_columns")?
        .try_iter()?
        .map(|column| column?.extract())
        .collect()
}

/// Each lineage output with the `(resource, column)` pairs it reads; empty without lineage.
pub(crate) fn lineage_inputs(model: &Bound<'_, PyAny>) -> PyResult<Vec<LineageInput>> {
    if let Some(view) = native_model(model)? {
        return Ok(view.lineage_inputs(model.py())?.unwrap_or_default());
    }
    let columns: Bound<'_, PyAny> = native_field(model, "fast_lineage_columns")?;
    if !columns.is_truthy()? {
        return Ok(Vec::new());
    }
    let mut lineage: Vec<LineageInput> = Vec::new();
    for output in columns.try_iter()? {
        let output: Bound<'_, PyAny> = output?;
        let mut upstream: Vec<(String, String)> = Vec::new();
        for source in native_field(&output, "upstream_columns")?.try_iter()? {
            let source: Bound<'_, PyAny> = source?;
            upstream.push((
                native_field(&source, "resource_name")?.extract()?,
                native_field(&source, "column_name")?.extract()?,
            ));
        }
        lineage.push((native_field(&output, "output_column")?.extract()?, upstream));
    }
    Ok(lineage)
}

/// Whether the native session validated the model's bindings.
pub(crate) fn binding_validated(model: &Bound<'_, PyAny>) -> PyResult<bool> {
    if let Some(view) = native_model(model)? {
        return view.binding_validated(model.py());
    }
    native_field(model, "binding_validated")?.is_truthy()
}

/// Whether the model's dynamic pivot proof proved its output columns.
pub(crate) fn output_proven(model: &Bound<'_, PyAny>) -> PyResult<bool> {
    if let Some(view) = native_model(model)? {
        return view.output_proven(model.py());
    }
    let proof: Bound<'_, PyAny> = native_field(model, "dynamic_column_contract")?;
    Ok(!proof.is_none() && native_field(&proof, "output_proven")?.is_truthy()?)
}

/// Each lineage output with its transform kind and confidence values, `None` without lineage.
pub(crate) fn lineage_facts(model: &Bound<'_, PyAny>) -> PyResult<Option<Vec<LineageFactRow>>> {
    if let Some(view) = native_model(model)? {
        let Some(rows) = view.lineage_rows(model.py())? else {
            return Ok(None);
        };
        let mut facts: Vec<LineageFactRow> = Vec::with_capacity(rows.len());
        for row in rows {
            facts.push(LineageFactRow {
                output_column: row.output_column,
                transform: coded(&TRANSFORM_VALUES, row.transform_code)?,
                confidence: coded(&CONFIDENCE_VALUES, row.confidence_code)?,
                sources: row.sources,
            });
        }
        return Ok(Some(facts));
    }
    let columns: Bound<'_, PyAny> = native_field(model, "fast_lineage_columns")?;
    if columns.is_none() {
        return Ok(None);
    }
    let mut facts: Vec<LineageFactRow> = Vec::new();
    for output in columns.try_iter()? {
        let output: Bound<'_, PyAny> = output?;
        let mut sources: Vec<(String, String, String)> = Vec::new();
        for source in native_field(&output, "upstream_columns")?.try_iter()? {
            let source: Bound<'_, PyAny> = source?;
            sources.push((
                native_field(&source, "resource_type")?.extract()?,
                native_field(&source, "resource_name")?.extract()?,
                native_field(&source, "column_name")?.extract()?,
            ));
        }
        facts.push(LineageFactRow {
            output_column: native_field(&output, "output_column")?.extract()?,
            transform: native_field(&output, "transform_kind")?.str()?.extract()?,
            confidence: native_field(&output, "confidence")?.str()?.extract()?,
            sources,
        });
    }
    Ok(Some(facts))
}

fn coded(values: &[&str], code: u8) -> PyResult<String> {
    values
        .get(usize::from(code))
        .map(|value| (*value).to_owned())
        .ok_or_else(|| pyo3::exceptions::PyIndexError::new_err("tuple index out of range"))
}

/// The input relations of a dynamic pivot proof that proved the model's outputs, else `None`.
pub(crate) fn proven_inputs(model: &Bound<'_, PyAny>) -> PyResult<Option<Vec<String>>> {
    if let Some(view) = native_model(model)? {
        return view.proven_inputs(model.py());
    }
    let proof: Bound<'_, PyAny> = native_field(model, "dynamic_column_contract")?;
    if proof.is_none() || !native_field(&proof, "output_proven")?.is_truthy()? {
        return Ok(None);
    }
    native_field(&proof, "input_relations")?.extract().map(Some)
}

/// The model's name.
pub(crate) fn model_name(model: &Bound<'_, PyAny>) -> PyResult<String> {
    if let Some(view) = native_model(model)? {
        return view.name(model.py());
    }
    native_field(model, "name")?.extract()
}

/// The model's compiled query SQL.
pub(crate) fn query_sql<'py>(model: &Bound<'py, PyAny>) -> PyResult<Bound<'py, PyAny>> {
    if let Some(view) = native_model(model)? {
        return Ok(view.query_sql(model.py())?.into_any());
    }
    native_field(model, "query_sql")
}

/// The model's `CompiledObjectKey`.
pub(crate) fn model_key<'py>(model: &Bound<'py, PyAny>) -> PyResult<Bound<'py, PyAny>> {
    if let Some(view) = native_model(model)? {
        return Ok(view.key(model.py())?.into_bound(model.py()));
    }
    native_field(model, "key")
}

/// The model's destination `CompiledRelationLocation`.
pub(crate) fn destination<'py>(model: &Bound<'py, PyAny>) -> PyResult<Bound<'py, PyAny>> {
    if let Some(view) = native_model(model)? {
        return Ok(view.destination(model.py())?.into_bound(model.py()));
    }
    native_field(model, "destination")
}

/// The model's name as a Python string.
pub(crate) fn name_object<'py>(model: &Bound<'py, PyAny>) -> PyResult<Bound<'py, PyAny>> {
    if let Some(view) = native_model(model)? {
        let name: String = view.name(model.py())?;
        return Ok(pyo3::types::PyString::new(model.py(), &name).into_any());
    }
    native_field(model, "name")
}

/// The model's attached `SchemaModelEntry`, or `None`.
pub(crate) fn schema_entry<'py>(model: &Bound<'py, PyAny>) -> PyResult<Bound<'py, PyAny>> {
    let py: Python<'py> = model.py();
    if let Some(view) = native_model(model)? {
        return Ok(view.schema_entry(py)?.into_bound(py));
    }
    native_field(model, "schema_entry")
}

/// The model's `CompileModelConfig`.
pub(crate) fn config<'py>(model: &Bound<'py, PyAny>) -> PyResult<Bound<'py, PyAny>> {
    let py: Python<'py> = model.py();
    if let Some(view) = native_model(model)? {
        return Ok(view.config(py)?.into_bound(py));
    }
    native_field(model, "config")
}

/// The model's `CompileSqlReference` tuple.
pub(crate) fn references<'py>(model: &Bound<'py, PyAny>) -> PyResult<Bound<'py, PyAny>> {
    let py: Python<'py> = model.py();
    if let Some(view) = native_model(model)? {
        return Ok(view.references(py)?.into_bound(py).into_any());
    }
    native_field(model, "references")
}

/// The model file's relative path.
pub(crate) fn relative_path<'py>(model: &Bound<'py, PyAny>) -> PyResult<Bound<'py, PyAny>> {
    let py: Python<'py> = model.py();
    if let Some(view) = native_model(model)? {
        return Ok(view.relative_path(py)?.into_bound(py));
    }
    native_field(model, "relative_path")
}

/// The model's `InferredColumn` tuple, or `None`.
pub(crate) fn inferred_columns<'py>(model: &Bound<'py, PyAny>) -> PyResult<Bound<'py, PyAny>> {
    let py: Python<'py> = model.py();
    if let Some(view) = native_model(model)? {
        return Ok(view.inferred_columns(py)?.into_bound(py));
    }
    native_field(model, "inferred_columns")
}

/// `(resource type, name)` of each dependency.
pub(crate) fn dep_rows(model: &Bound<'_, PyAny>) -> PyResult<Vec<(String, String)>> {
    if let Some(view) = native_model(model)? {
        return view.dep_rows(model.py());
    }
    native_field(model, "dep_rows")?.extract()
}

/// The model's dependency `CompiledObjectKey` tuple.
pub(crate) fn deps<'py>(model: &Bound<'py, PyAny>) -> PyResult<Bound<'py, PyAny>> {
    let py: Python<'py> = model.py();
    if let Some(view) = native_model(model)? {
        return Ok(view.deps(py)?.into_bound(py).into_any());
    }
    native_field(model, "deps")
}

/// The model's enum columns by name.
pub(crate) fn enum_columns<'py>(model: &Bound<'py, PyAny>) -> PyResult<Bound<'py, PyAny>> {
    let py: Python<'py> = model.py();
    if let Some(view) = native_model(model)? {
        return Ok(view.enum_columns(py)?.into_bound(py).into_any());
    }
    native_field(model, "enum_columns")
}
