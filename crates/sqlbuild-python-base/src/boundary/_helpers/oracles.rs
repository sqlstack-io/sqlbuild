//! Internal hooks that let Python tests compare native foundations with Python byte for byte.

use pyo3::exceptions::PyValueError;
use pyo3::prelude::{Bound, PyModule, PyModuleMethods, PyResult};
use pyo3::{PyErr, pyfunction, wrap_pyfunction};
use serde_json::{Value, json};
use sqlbuild_config::errors::ConfigError;
use sqlbuild_config::models::{ConfigDate, ConfigTime, ConfigValue};
use sqlbuild_config::toml::main::load_toml::load_toml;
use sqlbuild_config::yaml::main::safe_load::safe_load;
use sqlbuild_core::json::main::dumps::dumps;
use sqlbuild_core::json::models::{
    JsonDialect, JsonInteger, JsonValue, OrjsonOptions, StdlibJsonOptions,
};
use sqlbuild_core::text::main::decode_text::decode_text;
use sqlbuild_core::text::models::LineIndex;

fn oracle_error(message: impl std::fmt::Display) -> PyErr {
    PyValueError::new_err(message.to_string())
}

fn text_field<'value>(value: &'value Value, key: &str) -> PyResult<&'value str> {
    value[key]
        .as_str()
        .ok_or_else(|| oracle_error(format!("expected string field {key}")))
}

fn flag(value: &Value, key: &str) -> PyResult<bool> {
    value[key]
        .as_bool()
        .ok_or_else(|| oracle_error(format!("expected boolean field {key}")))
}

fn dialect(spec: &Value) -> PyResult<JsonDialect> {
    if let Some(options) = spec.get("orjson") {
        return Ok(JsonDialect::Orjson(OrjsonOptions {
            indent_2: flag(options, "indent_2")?,
            sort_keys: flag(options, "sort_keys")?,
        }));
    }
    Ok(JsonDialect::Stdlib(StdlibJsonOptions {
        indent: spec["indent"].as_str().map(str::to_owned),
        item_separator: text_field(&spec["separators"], "item")?.to_owned(),
        key_separator: text_field(&spec["separators"], "key")?.to_owned(),
        ensure_ascii: flag(spec, "ensure_ascii")?,
        sort_keys: flag(spec, "sort_keys")?,
        allow_nan: flag(spec, "allow_nan")?,
    }))
}

fn float_from_bits(bits: &Value) -> PyResult<f64> {
    let text = bits
        .as_str()
        .ok_or_else(|| oracle_error("expected float bits"))?;
    text.parse::<u64>()
        .map(f64::from_bits)
        .map_err(oracle_error)
}

/// Decode `[tag, payload]`, the tagged form the Python oracle tests write.
fn tagged_value(tagged: &Value) -> PyResult<JsonValue> {
    let payload = &tagged[1];
    match tagged[0].as_str() {
        Some("null") => Ok(JsonValue::Null),
        Some("bool") => payload
            .as_bool()
            .map(JsonValue::Bool)
            .ok_or_else(|| oracle_error("expected boolean")),
        Some("int") => payload
            .as_str()
            .and_then(JsonInteger::parse)
            .map(JsonValue::Integer)
            .ok_or_else(|| oracle_error("expected decimal integer")),
        Some("float") => float_from_bits(payload).map(JsonValue::Float),
        Some("str") => payload
            .as_str()
            .map(|text| JsonValue::String(text.to_owned()))
            .ok_or_else(|| oracle_error("expected string")),
        Some("list") => tagged_items(payload)?
            .iter()
            .map(tagged_value)
            .collect::<PyResult<Vec<_>>>()
            .map(JsonValue::Array),
        Some("dict") => tagged_items(payload)?
            .iter()
            .map(|entry| {
                let key = entry[0]
                    .as_str()
                    .ok_or_else(|| oracle_error("expected string key"))?;
                Ok((key.to_owned(), tagged_value(&entry[1])?))
            })
            .collect::<PyResult<Vec<_>>>()
            .map(JsonValue::Object),
        _ => Err(oracle_error("unknown tagged value")),
    }
}

fn tagged_items(payload: &Value) -> PyResult<&Vec<Value>> {
    payload
        .as_array()
        .ok_or_else(|| oracle_error("expected tagged items"))
}

/// Serialize a tagged value natively, or return `error:<name>` for a refused value.
#[pyfunction]
fn _oracle_json_dumps(dialect_json: &str, value_json: &str) -> PyResult<String> {
    let spec: Value = serde_json::from_str(dialect_json).map_err(oracle_error)?;
    let tagged: Value = serde_json::from_str(value_json).map_err(oracle_error)?;
    Ok(dumps(&tagged_value(&tagged)?, &dialect(&spec)?)
        .unwrap_or_else(|error| format!("error:{error:?}")))
}

type OraclePosition = Option<(usize, usize, usize)>;

fn oracle_position(index: &LineIndex<'_>, byte_offset: usize) -> OraclePosition {
    let position = index.position_at_byte(byte_offset)?;
    Some((position.char_offset, position.line, position.column))
}

/// Decode authored bytes and report `(char_offset, line, column)` for each byte offset.
#[pyfunction]
fn _oracle_text_positions(
    data: &[u8],
    byte_offsets: Vec<usize>,
) -> PyResult<(String, Vec<OraclePosition>)> {
    let text = decode_text(data).map_err(|error| oracle_error(error.valid_up_to))?;
    let positions = {
        let index = LineIndex::new(&text);
        byte_offsets
            .into_iter()
            .map(|offset| oracle_position(&index, offset))
            .collect()
    };
    Ok((text, positions))
}

fn date_parts(date: ConfigDate) -> Value {
    json!([date.year, date.month, date.day])
}

fn time_parts(time: ConfigTime) -> Value {
    json!([time.hour, time.minute, time.second, time.microsecond])
}

fn float_canonical(value: f64) -> Value {
    if value.is_nan() {
        return json!(["float", "nan"]);
    }
    json!(["float", value.to_bits().to_string()])
}

/// The tagged canonical form the Python oracle tests build from `tomllib` and PyYAML values.
fn canonical(value: &ConfigValue) -> Value {
    match value {
        ConfigValue::Null => json!(["null"]),
        ConfigValue::Bool(flag) => json!(["bool", flag]),
        ConfigValue::Integer(number) => json!(["int", number.to_string()]),
        ConfigValue::BigInteger(text) => json!(["int", text]),
        ConfigValue::Float(number) => float_canonical(*number),
        ConfigValue::String(text) => json!(["str", text]),
        ConfigValue::Date(date) => json!(["date", date_parts(*date)]),
        ConfigValue::DateTime(moment) => json!([
            "datetime",
            date_parts(moment.date),
            time_parts(moment.time),
            moment.utc_offset_seconds
        ]),
        ConfigValue::Time(time) => json!(["time", time_parts(*time)]),
        ConfigValue::List(items) => {
            json!(["list", items.iter().map(canonical).collect::<Vec<_>>()])
        }
        ConfigValue::Map(entries) => json!([
            "map",
            entries
                .iter()
                .map(|(key, item)| json!([canonical(key), canonical(item)]))
                .collect::<Vec<_>>()
        ]),
    }
}

fn outcome(result: Result<Value, ConfigError>) -> String {
    result
        .unwrap_or_else(|error| json!({"error": format!("{:?}", error.kind)}))
        .to_string()
}

/// Load YAML natively and return its canonical form, or `{"error": kind}`.
#[pyfunction]
fn _oracle_yaml_load(text: &str) -> String {
    outcome(safe_load(text).map(|value| canonical(&value)))
}

/// Load TOML natively and return its canonical form, or `{"error": kind}`.
#[pyfunction]
fn _oracle_toml_load(text: &str) -> String {
    outcome(load_toml(text).map(|value| canonical(&value)))
}

pub(crate) fn register(module: &Bound<'_, PyModule>) -> PyResult<()> {
    module.add_function(wrap_pyfunction!(_oracle_json_dumps, module)?)?;
    module.add_function(wrap_pyfunction!(_oracle_text_positions, module)?)?;
    module.add_function(wrap_pyfunction!(_oracle_yaml_load, module)?)?;
    module.add_function(wrap_pyfunction!(_oracle_toml_load, module)?)?;
    Ok(())
}
