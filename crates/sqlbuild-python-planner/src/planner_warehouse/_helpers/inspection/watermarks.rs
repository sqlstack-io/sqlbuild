//! Validate reliable watermark column contracts before warehouse inspection.

use crate::planner_strategy::main::cursor_types::classify_cursor_sql_type::classify_cursor_sql_type;
use crate::planner_warehouse::constants::{
    CURSOR_TYPE_INTEGER, CURSOR_TYPE_TIMESTAMP, WATERMARK_CONTRACT_CODE,
};
use pyo3::prelude::{Bound, PyAny, PyAnyMethods, PyErr, PyResult};
use pyo3::pyfunction;
use pyo3::types::{PyDict, PyDictMethods};
use sqlbuild_python_base::records::main::fields::field::field as native_field;
use sqlbuild_python_front::model_config::main::config::config_value::config_value;
use sqlbuild_python_front::model_config::main::config::model_values::model_values;
use sqlbuild_python_project::planner_phases::main::config_str::config_str;
use sqlbuild_python_project::planner_phases::main::object::object;
use sqlbuild_python_project::planner_phases::main::planner_error::planner_error;
use sqlbuild_python_project::planner_phases::main::text::text;
use sqlbuild_python_project::resource_views::main::seed_reads::schema_entry::schema_entry;
use sqlbuild_python_project::resource_views::main::source_reads::source_entry::source_entry;
use std::collections::HashMap;

const CONTRACT_KEY: &str = "contract";
const ENFORCED_CONTRACT: &str = "enforced";

/// Whether a declared column type can hold the model's cursor type.
#[pyfunction]
#[pyo3(signature = (*, declared_type, cursor_type))]
pub(crate) fn watermark_type_is_compatible(
    declared_type: &Bound<'_, PyAny>,
    cursor_type: Option<&str>,
) -> PyResult<bool> {
    let Some(expected) = cursor_type
        .filter(|value| *value == CURSOR_TYPE_TIMESTAMP || *value == CURSOR_TYPE_INTEGER)
    else {
        return Ok(true);
    };
    let classified = object(
        declared_type.py(),
        classify_cursor_sql_type(declared_type.py(), &declared_type.extract::<String>()?)?,
    )?;
    classified.eq(expected)
}

type DeclaredColumns<'py> = (Vec<String>, HashMap<String, Bound<'py, PyAny>>);

fn declared_columns<'py>(columns: &Bound<'py, PyAny>) -> PyResult<DeclaredColumns<'py>> {
    let mut names: Vec<_> = Vec::new();
    let mut types: HashMap<_, _> = HashMap::new();
    for column in columns.try_iter()? {
        let column = column?;
        let name: String = native_field(&column, "name")?.extract()?;
        let column_type = native_field(&column, "type")?;
        if !column_type.is_none() {
            types.insert(
                native_field(&column, "name")?
                    .call_method0("lower")?
                    .extract()?,
                column_type,
            );
        }
        names.push(name);
    }
    Ok((names, types))
}

/// Raise S302 when a reliable upstream contract lacks or mistypes a watermark column.
pub(crate) fn validate_watermark_column<'py>(
    model: &Bound<'py, PyAny>,
    reference: &Bound<'py, PyAny>,
    cursor_column: &str,
    (models, sources, seeds): (
        &Bound<'py, PyDict>,
        &Bound<'py, PyDict>,
        &Bound<'py, PyDict>,
    ),
) -> Result<(), PyErr> {
    let py = model.py();
    let kind = native_field(reference, "ref_kind")?;
    let ref_name: String = native_field(reference, "ref_name")?.extract()?;
    let model_name: String =
        sqlbuild_python_project::compiled_views::main::compiled_models::model_name::model_name(
            model,
        )?;
    let lowered =
        |value: &str| -> PyResult<String> { text(py, value).call_method0("lower")?.extract() };
    let (names, types) = if kind.eq("ref")? {
        let Some(upstream) = models.get_item(&ref_name)? else {
            return Ok(());
        };
        let upstream_cursor = config_str(&upstream, "cursor")?;
        let upstream_type = config_str(&upstream, "cursor_type")?;
        let consumer_type = config_str(model, "cursor_type")?;
        if let (Some(upstream_cursor), Some(upstream_type), Some(consumer_type)) =
            (&upstream_cursor, &upstream_type, &consumer_type)
            && lowered(upstream_cursor)? == lowered(cursor_column)?
            && upstream_type != consumer_type
        {
            return Err(planner_error(
                py,
                &format!(
                    "model '{model_name}': cursor_inputs watermark '{ref_name}.{cursor_column}' has cursor_type={upstream_type}, incompatible with model cursor_type={consumer_type}"
                ),
                (Some(WATERMARK_CONTRACT_CODE), None),
            ));
        }
        let contract = config_value(py, &model_values(&upstream)?, CONTRACT_KEY)?;
        if !contract.eq(ENFORCED_CONTRACT)? {
            return Ok(());
        }
        let schema = native_field(&upstream, "schema_entry")?;
        if schema.is_none() {
            (Vec::new(), HashMap::new())
        } else {
            declared_columns(&native_field(&schema, "columns")?)?
        }
    } else if kind.eq("source")? {
        let Some(source) = sources.get_item(&ref_name)? else {
            return Ok(());
        };
        let entry = source_entry(&source)?;
        if !native_field(&entry, "contract")?.eq("enforced")? {
            return Ok(());
        }
        declared_columns(&native_field(&entry, "columns")?)?
    } else if kind.eq("seed")? {
        let Some(seed) = seeds.get_item(&ref_name)? else {
            return Ok(());
        };
        declared_columns(&native_field(&schema_entry(&seed)?, "columns")?)?
    } else {
        return Ok(());
    };
    let wanted = lowered(cursor_column)?;
    let mut lowered_names: Vec<_> = Vec::new();
    for name in &names {
        lowered_names.push(lowered(name)?);
    }
    if lowered_names.contains(&wanted) {
        if let Some(declared_type) = types.get(&wanted) {
            let cursor_type = config_str(model, "cursor_type")?;
            if !watermark_type_is_compatible(declared_type, cursor_type.as_deref())? {
                return Err(planner_error(
                    py,
                    &format!(
                        "model '{model_name}': cursor_inputs watermark '{ref_name}.{cursor_column}' type {} is incompatible with cursor_type={}",
                        declared_type.str()?,
                        cursor_type.as_deref().unwrap_or("None")
                    ),
                    (Some(WATERMARK_CONTRACT_CODE), None),
                ));
            }
        }
        return Ok(());
    }
    let mut declared = names.join(", ");
    if declared.is_empty() {
        declared = "none".to_owned();
    }
    Err(planner_error(
        py,
        &format!(
            "model '{model_name}': cursor_inputs watermark references '{ref_name}' column '{cursor_column}', but its enforced contract does not expose the column. Declared contract columns: {declared}"
        ),
        (Some(WATERMARK_CONTRACT_CODE), None),
    ))
}
