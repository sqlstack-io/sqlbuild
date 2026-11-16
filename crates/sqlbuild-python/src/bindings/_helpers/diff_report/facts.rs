//! Python diff results read as plain diff facts.

use pyo3::prelude::{Bound, PyAny, PyAnyMethods, PyResult, Python};
use pyo3::types::{
    PyBool, PyBoolMethods, PyDict, PyDictMethods, PyFloat, PyInt, PyList, PyString,
    PyStringMethods, PyTuple,
};
use sqlbuild_cli::diff_report::models::{
    DiffChangedCell, DiffColumnFacts, DiffCoverageFacts, DiffModelFacts, DiffRowFacts,
    DiffSampleRow, DiffValue,
};
use sqlbuild_cli::diff_report::types::DiffNamedValue;
use sqlbuild_core::json::main::python_float_repr::python_float_repr;
use sqlbuild_core::json::models::{JsonInteger, JsonValue};

use sqlbuild_python_base::boundary::main::imported::imported;
use sqlbuild_python_base::records::main::fields::field::field as native_field;

fn text_of(value: &Bound<'_, PyAny>) -> PyResult<String> {
    Ok(value.str()?.to_string_lossy().into_owned())
}

fn strings(value: &Bound<'_, PyAny>) -> PyResult<Vec<String>> {
    let mut items: Vec<String> = Vec::new();
    for item in value.try_iter()? {
        items.push(text_of(&item?)?);
    }
    Ok(items)
}

fn optional_text(value: Bound<'_, PyAny>) -> PyResult<Option<String>> {
    if value.is_none() {
        return Ok(None);
    }
    text_of(&value).map(Some)
}

/// A JSON object key as `json.dumps` writes it.
fn json_key(key: &Bound<'_, PyAny>) -> PyResult<String> {
    if let Ok(flag) = key.downcast::<PyBool>() {
        return Ok(if flag.is_true() { "true" } else { "false" }.to_owned());
    }
    if key.is_none() {
        return Ok("null".to_owned());
    }
    if key.is_instance_of::<PyFloat>() {
        return Ok(python_float_repr(key.extract()?));
    }
    text_of(key)
}

/// One value as `json.dumps(default=...)` encodes it: dates as ISO text, other objects as `str()`.
pub(crate) fn json_of(value: &Bound<'_, PyAny>) -> PyResult<JsonValue> {
    if value.is_none() {
        return Ok(JsonValue::Null);
    }
    if let Ok(text) = value.downcast::<PyString>() {
        return Ok(JsonValue::String(text.to_string_lossy().into_owned()));
    }
    if let Ok(flag) = value.downcast::<PyBool>() {
        return Ok(JsonValue::Bool(flag.is_true()));
    }
    if value.is_instance_of::<PyInt>() {
        let decimal: String = value
            .call_method0("__index__")?
            .str()?
            .to_string_lossy()
            .into_owned();
        if let Some(integer) = JsonInteger::parse(&decimal) {
            return Ok(JsonValue::Integer(integer));
        }
    }
    if value.is_instance_of::<PyFloat>() {
        return Ok(JsonValue::Float(value.extract()?));
    }
    if value.is_instance_of::<PyList>() || value.is_instance_of::<PyTuple>() {
        let mut items: Vec<JsonValue> = Vec::new();
        for item in value.try_iter()? {
            items.push(json_of(&item?)?);
        }
        return Ok(JsonValue::Array(items));
    }
    if let Ok(mapping) = value.downcast::<PyDict>() {
        let mut entries: Vec<(String, JsonValue)> = Vec::new();
        for (key, item) in mapping.iter() {
            entries.push((json_key(&key)?, json_of(&item)?));
        }
        return Ok(JsonValue::Object(entries));
    }
    let py: Python<'_> = value.py();
    if value.is_instance(&imported(py, "datetime", "date")?)? {
        return Ok(JsonValue::String(text_of(
            &value.call_method0("isoformat")?,
        )?));
    }
    Ok(JsonValue::String(text_of(value)?))
}

fn diff_value(value: &Bound<'_, PyAny>) -> PyResult<DiffValue> {
    Ok(DiffValue {
        text: text_of(value)?,
        json: json_of(value)?,
    })
}

fn optional_value(value: Bound<'_, PyAny>) -> PyResult<Option<DiffValue>> {
    if value.is_none() {
        return Ok(None);
    }
    diff_value(&value).map(Some)
}

fn named_values(value: &Bound<'_, PyAny>) -> PyResult<Vec<DiffNamedValue>> {
    let mut pairs: Vec<DiffNamedValue> = Vec::new();
    for pair in value.try_iter()? {
        let (name, item): (Bound<'_, PyAny>, Bound<'_, PyAny>) = pair?.extract()?;
        pairs.push((text_of(&name)?, diff_value(&item)?));
    }
    Ok(pairs)
}

fn key_samples(value: &Bound<'_, PyAny>) -> PyResult<Vec<Vec<DiffNamedValue>>> {
    let mut samples: Vec<Vec<DiffNamedValue>> = Vec::new();
    for sample in value.try_iter()? {
        samples.push(named_values(&sample?)?);
    }
    Ok(samples)
}

fn coverage(value: Bound<'_, PyAny>) -> PyResult<Option<DiffCoverageFacts>> {
    if value.is_none() {
        return Ok(None);
    }
    Ok(Some(DiffCoverageFacts {
        row_count: native_field(&value, "row_count")?.extract()?,
        minimum_cursor: optional_value(native_field(&value, "minimum_cursor")?)?,
        maximum_cursor: optional_value(native_field(&value, "maximum_cursor")?)?,
    }))
}

fn column(value: &Bound<'_, PyAny>) -> PyResult<DiffColumnFacts> {
    let tolerance: Bound<'_, PyAny> = native_field(value, "tolerance")?;
    Ok(DiffColumnFacts {
        name: text_of(&native_field(value, "name")?)?,
        mismatched_count: native_field(value, "mismatched_count")?.extract()?,
        tolerance: if tolerance.is_none() {
            None
        } else {
            Some((
                optional_text(native_field(&tolerance, "absolute")?)?,
                optional_text(native_field(&tolerance, "relative")?)?,
            ))
        },
    })
}

fn coverage_differs(rows: &Bound<'_, PyAny>) -> PyResult<bool> {
    let left: Bound<'_, PyAny> = native_field(rows, "left_coverage")?;
    let right: Bound<'_, PyAny> = native_field(rows, "right_coverage")?;
    if left.is_none() || right.is_none() {
        return Ok(false);
    }
    Ok(
        native_field(&left, "minimum_cursor")?.ne(native_field(&right, "minimum_cursor")?)?
            || native_field(&left, "maximum_cursor")?
                .ne(native_field(&right, "maximum_cursor")?)?,
    )
}

fn row_facts(rows: Bound<'_, PyAny>) -> PyResult<Option<DiffRowFacts>> {
    if rows.is_none() {
        return Ok(None);
    }
    let count = |name: &str| -> PyResult<i64> { native_field(&rows, name)?.extract() };
    let mut columns: Vec<DiffColumnFacts> = Vec::new();
    for item in native_field(&rows, "column_results")?.try_iter()? {
        columns.push(column(&item?)?);
    }
    let sampling: Bound<'_, PyAny> = native_field(&rows, "sampling")?;
    Ok(Some(DiffRowFacts {
        left_count: count("left_count")?,
        right_count: count("right_count")?,
        joined_count: count("joined_count")?,
        equal_count: count("equal_count")?,
        unequal_count: count("unequal_count")?,
        left_only_count: count("left_only_count")?,
        right_only_count: count("right_only_count")?,
        columns,
        population_count: count("population_count")?,
        compared_count: count("compared_count")?,
        sampling: if sampling.is_none() {
            None
        } else {
            Some((
                native_field(&sampling, "row_limit")?.extract()?,
                native_field(&sampling, "seed")?.extract()?,
            ))
        },
        left_coverage: coverage(native_field(&rows, "left_coverage")?)?,
        right_coverage: coverage(native_field(&rows, "right_coverage")?)?,
        cursor_column: optional_text(native_field(&rows, "cursor_column")?)?,
        cursor_start: optional_value(native_field(&rows, "cursor_start")?)?,
        cursor_end: optional_value(native_field(&rows, "cursor_end")?)?,
        exhaustive: native_field(&rows, "is_exhaustive")?.is_truthy()?,
        coverage_differs: coverage_differs(&rows)?,
    }))
}

fn sample_row(value: &Bound<'_, PyAny>) -> PyResult<DiffSampleRow> {
    let mut changed_cells: Vec<DiffChangedCell> = Vec::new();
    for cell in native_field(value, "changed_cells")?.try_iter()? {
        let cell: Bound<'_, PyAny> = cell?;
        changed_cells.push(DiffChangedCell {
            name: text_of(&native_field(&cell, "name")?)?,
            left: diff_value(&native_field(&cell, "left_value")?)?,
            right: diff_value(&native_field(&cell, "right_value")?)?,
        });
    }
    Ok(DiffSampleRow {
        key_values: named_values(&native_field(value, "key_values")?)?,
        changed_cells,
    })
}

fn model_facts(model: &Bound<'_, PyAny>) -> PyResult<DiffModelFacts> {
    let schema: Bound<'_, PyAny> = native_field(model, "schema_result")?;
    let names = |attribute: &str| -> PyResult<Vec<String>> {
        let mut names: Vec<String> = Vec::new();
        for column in native_field(&schema, attribute)?.try_iter()? {
            names.push(text_of(&native_field(&column?, "name")?)?);
        }
        Ok(names)
    };
    let mut type_changes: Vec<(String, String, String)> = Vec::new();
    for pair in native_field(&schema, "type_changed_columns")?.try_iter()? {
        let (left, right): (Bound<'_, PyAny>, Bound<'_, PyAny>) = pair?.extract()?;
        type_changes.push((
            text_of(&native_field(&left, "name")?)?,
            text_of(&native_field(&left, "type")?)?,
            text_of(&native_field(&right, "type")?)?,
        ));
    }
    let mut unequal_samples: Vec<DiffSampleRow> = Vec::new();
    for sample in native_field(model, "unequal_row_samples")?.try_iter()? {
        unequal_samples.push(sample_row(&sample?)?);
    }
    Ok(DiffModelFacts {
        name: text_of(&native_field(model, "name")?)?,
        left_relation: text_of(&native_field(model, "left_relation")?)?,
        right_relation: text_of(&native_field(model, "right_relation")?)?,
        added_columns: names("added_columns")?,
        removed_columns: names("removed_columns")?,
        type_changes,
        unique_key: strings(&native_field(model, "unique_key")?)?,
        rows: row_facts(native_field(model, "row_result")?)?,
        unequal_samples,
        left_only_keys: key_samples(&native_field(model, "left_only_key_samples")?)?,
        right_only_keys: key_samples(&native_field(model, "right_only_key_samples")?)?,
        bounded_fallback: native_field(model, "bounded_fallback")?.is_truthy()?,
        excluded_columns: strings(&native_field(model, "excluded_columns")?)?,
        input_kind: text_of(&native_field(model, "input_kind")?)?,
        unkeyed: native_field(model, "unkeyed")?.is_truthy()?,
    })
}

/// Every model result of a `DiffExecutionResult`.
pub(crate) fn diff_facts(result: &Bound<'_, PyAny>) -> PyResult<Vec<DiffModelFacts>> {
    let mut models: Vec<DiffModelFacts> = Vec::new();
    for model in native_field(result, "model_results")?.try_iter()? {
        models.push(model_facts(&model?)?);
    }
    Ok(models)
}
