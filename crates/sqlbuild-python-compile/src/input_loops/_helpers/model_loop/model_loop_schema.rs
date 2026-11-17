//! A model's `model_schema`: resolved, checked for visibility, and merged into its header columns.

use pyo3::IntoPyObjectExt;
use pyo3::prelude::{Bound, PyAny, PyAnyMethods, PyResult, Python};
use pyo3::types::{PyDict, PyDictMethods, PyString, PyTuple};
use sqlbuild_analysis::compiled_project::models::SchemaColumnFact;

use crate::input_loops::_helpers::model::model_loop::LoopContext;
use crate::input_loops::_helpers::model::model_loop::compile_error;
use crate::input_loops::_helpers::model::model_loop_inputs::present;
use sqlbuild_python_base::records::main::fields::field::field as native_field;
use sqlbuild_python_front::model_config::models::HeaderColumn;
use sqlbuild_python_project::compiled_project::main::python_column_fact::python_column_fact;

const MODEL_SCHEMA_KEY: &str = "model_schema";
const NAME_FIELD: &str = "name";
const TYPE_FIELD: &str = "type";
const NULLABLE_FIELD: &str = "nullable";
const DESCRIPTION_FIELD: &str = "description";
const AUDITS_FIELD: &str = "audits";
const LOCATION_FIELD: &str = "location";
const PATH_FIELD: &str = "path";
const LINE_FIELD: &str = "line";
const COLUMNS_FIELD: &str = "columns";
const STRIP_METHOD: &str = "strip";

/// The reusable schema a model names: its columns, name, description and enum column types.
pub(crate) struct ModelSchema<'py> {
    pub(crate) declaration: Bound<'py, PyAny>,
    pub(crate) name: String,
}

/// `_resolve_model_schema`: `None`, or the public schema the model's config names.
pub(crate) fn model_schema<'py>(
    context: &LoopContext<'_, 'py>,
    values: &Bound<'py, PyDict>,
    model_name: &str,
) -> PyResult<Option<ModelSchema<'py>>> {
    let Some(raw) = present(values, MODEL_SCHEMA_KEY)? else {
        return Ok(None);
    };
    let Ok(name) = raw.downcast::<PyString>() else {
        return Err(invalid_name(context, model_name));
    };
    if !name.call_method0(STRIP_METHOD)?.is_truthy()? {
        return Err(invalid_name(context, model_name));
    }
    let schemas: &Bound<'py, PyDict> = &context.classes.model_schemas;
    let Some(declaration) = schemas.get_item(name)? else {
        let mut available: Vec<String> = schemas.keys().extract()?;
        available.sort_unstable();
        let listed: String = if available.is_empty() {
            "none".to_owned()
        } else {
            available.join(", ")
        };
        let message: String = format!(
            "model '{model_name}' references unknown model_schema '{name}'; available schemas: \
             {listed}"
        );
        return Err(compile_error(context.classes, &message, None));
    };
    Ok(Some(ModelSchema {
        declaration,
        name: name.extract()?,
    }))
}

/// The schema's column names, which validation reads as the declared columns.
pub(crate) fn schema_column_names(schema: &ModelSchema<'_>) -> PyResult<Vec<String>> {
    native_field(&schema.declaration, COLUMNS_FIELD)?
        .try_iter()?
        .map(|column| native_field(&column?, NAME_FIELD)?.extract())
        .collect()
}

/// `model_schema_enum_declarations`: the schema's column types that name a project enum.
pub(crate) fn schema_enums<'py>(
    context: &LoopContext<'_, 'py>,
    schema: &ModelSchema<'py>,
) -> PyResult<Bound<'py, PyDict>> {
    let py: Python<'py> = schema.declaration.py();
    let enums: Bound<'py, PyDict> = PyDict::new(py);
    for column in native_field(&schema.declaration, COLUMNS_FIELD)?.try_iter()? {
        let column_type: Bound<'py, PyAny> = native_field(&column?, TYPE_FIELD)?;
        if column_type.is_none() {
            continue;
        }
        if let Some(declaration) = context.classes.schema_enums.get_item(&column_type)? {
            enums.set_item(column_type, declaration)?;
        }
    }
    Ok(enums)
}

/// Schema columns as Python specs paired with their native facts, in declaration order.
pub(crate) struct DeclaredColumns {
    pub(crate) specs: Vec<HeaderColumn>,
    pub(crate) facts: Vec<SchemaColumnFact>,
}

/// `_merge_model_schema_columns`: named columns first, augmented by header audits, then the rest.
pub(crate) fn merged_columns<'py>(
    context: &LoopContext<'_, 'py>,
    schema: &ModelSchema<'py>,
    local: DeclaredColumns,
    model: (&str, &str, &Bound<'py, PyAny>),
) -> PyResult<DeclaredColumns> {
    let (model_name, relative_path, model_path) = model;
    let mut merged: DeclaredColumns = DeclaredColumns {
        specs: Vec::new(),
        facts: Vec::new(),
    };
    for column in native_field(&schema.declaration, COLUMNS_FIELD)?.try_iter()? {
        let column: Bound<'py, PyAny> = column?;
        merged
            .facts
            .push(python_column_fact(&column, Some(model_path))?);
        merged.specs.push(HeaderColumn::from_python(column)?);
    }
    let keys: Vec<String> = merged
        .facts
        .iter()
        .map(|fact| fact.name.to_lowercase())
        .collect();
    let mut additional: DeclaredColumns = DeclaredColumns {
        specs: Vec::new(),
        facts: Vec::new(),
    };
    for (column, fact) in local.specs.into_iter().zip(local.facts) {
        let key: String = fact.name.to_lowercase();
        let Some(index) = keys.iter().rposition(|candidate| *candidate == key) else {
            additional.specs.push(column);
            additional.facts.push(fact);
            continue;
        };
        check_augmentation(
            context,
            &fact,
            &merged.specs[index],
            (model_name, relative_path),
        )?;
        let (spec, augmented_fact) = augmented(
            context,
            (&merged.specs[index], &merged.facts[index]),
            (&column, fact),
        )?;
        merged.specs[index] = spec;
        merged.facts[index] = augmented_fact;
    }
    merged.specs.extend(additional.specs);
    merged.facts.extend(additional.facts);
    Ok(merged)
}

fn augmented<'py>(
    context: &LoopContext<'_, 'py>,
    (named, named_fact): (&HeaderColumn, &SchemaColumnFact),
    (local, local_fact): (&HeaderColumn, SchemaColumnFact),
) -> PyResult<(HeaderColumn, SchemaColumnFact)> {
    let py: Python<'py> = context.contracts.schema_column.py();
    let mut audits: Vec<Bound<'py, PyAny>> = named
        .field(py, AUDITS_FIELD)?
        .try_iter()?
        .collect::<PyResult<_>>()?;
    let mut fact: SchemaColumnFact = named_fact.clone();
    for (audit, definition) in local
        .field(py, AUDITS_FIELD)?
        .try_iter()?
        .zip(local_fact.audit_definitions)
    {
        let audit: Bound<'py, PyAny> = audit?;
        let mut present: bool = false;
        for existing in &audits {
            if existing.eq(&audit)? {
                present = true;
                break;
            }
        }
        if !present {
            audits.push(audit);
            fact.audit_definitions.push(definition);
        }
    }
    let migrate_from = local_fact.migrate_from.clone().into_bound_py_any(py)?;
    fact.migrate_from = local_fact.migrate_from;
    Ok((
        HeaderColumn::replaced(
            &context.contracts.schema_column,
            [
                named.field(py, NAME_FIELD)?,
                named.field(py, TYPE_FIELD)?,
                named.field(py, NULLABLE_FIELD)?,
                named.field(py, DESCRIPTION_FIELD)?,
                named.field(py, "meta")?,
                PyTuple::new(py, audits)?.into_any(),
                named.field(py, LOCATION_FIELD)?,
                migrate_from,
            ],
            [0, 1, 2, 3, 4, 6, 5, 7],
        )?,
        fact,
    ))
}

/// `_validate_model_schema_audit_augmentation`.
fn check_augmentation(
    context: &LoopContext<'_, '_>,
    local: &SchemaColumnFact,
    named: &HeaderColumn,
    model: (&str, &str),
) -> PyResult<()> {
    let (model_name, relative_path) = model;
    let name: &str = &local.name;
    let mut overridden: Vec<&str> = Vec::new();
    if local.data_type.is_some() {
        overridden.push(TYPE_FIELD);
    }
    if local.nullable.is_some() {
        overridden.push(NULLABLE_FIELD);
    }
    if local.described {
        overridden.push(DESCRIPTION_FIELD);
    }
    if !overridden.is_empty() {
        let message: String = format!(
            "model '{model_name}' in {relative_path} cannot override {} for named-schema column \
             '{name}' from {}; only audit augmentation is supported",
            overridden.join(", "),
            origin(&named.project(context.contracts.schema_column.py())?)?
        );
        return Err(compile_error(context.classes, &message, None));
    }
    if local.audit_definitions.is_empty() && local.migrate_from.is_none() {
        let message: String = format!(
            "model '{model_name}' in {relative_path} redeclares named-schema column '{name}' from \
             {} without audits or migrate_from; only audit augmentation and column \
             migrations are supported",
            origin(&named.project(context.contracts.schema_column.py())?)?
        );
        return Err(compile_error(context.classes, &message, None));
    }
    Ok(())
}

/// Where a named-schema column is declared, as its `path:line` or the named schema.
fn origin(named: &Bound<'_, PyAny>) -> PyResult<String> {
    let location: Bound<'_, PyAny> = native_field(named, LOCATION_FIELD)?;
    if location.is_none() {
        return Ok("the named schema".to_owned());
    }
    Ok(format!(
        "{}:{}",
        native_field(&location, PATH_FIELD)?.str()?,
        native_field(&location, LINE_FIELD)?.str()?
    ))
}

fn invalid_name(context: &LoopContext<'_, '_>, model_name: &str) -> pyo3::PyErr {
    let message: String = format!("model '{model_name}': model_schema must be a non-empty string");
    compile_error(context.classes, &message, None)
}
