//! The custom-rule host payload: the facts rules read, written natively for hosts to decode lazily.

use std::io::Write;
use std::sync::Arc;

use pyo3::prelude::{Bound, PyAny, PyAnyMethods, PyResult, PyTypeMethods, Python};
use pyo3::pyfunction;
use pyo3::types::{
    PyBytes, PyBytesMethods, PyDict, PyDictMethods, PyString, PyStringMethods, PyTuple,
    PyTupleMethods,
};
use serde_json::{Map, Value, json};
use sqlbuild_discovery::models::LineColumnSpan;

use crate::rules::_helpers::custom::custom_host_columns::PayloadColumn;
use crate::rules::_helpers::custom::custom_hosts::pickled;
use crate::rules::_helpers::fact::fact_replay::ModelEntry;
use crate::rules::_helpers::fact::fact_replay::project_attribute;
use crate::rules::_helpers::fact::fact_replay::project_models;
use sqlbuild_python_base::records::main::fields::field::field as native_field;
use sqlbuild_python_project::compiled_views::main::compiled_models::enum_columns::enum_columns;
use sqlbuild_python_project::compiled_views::main::project::compiled_audit_record::compiled_audit_record;
use sqlbuild_python_project::compiled_views::main::project::compiled_function_record::compiled_function_record;
use sqlbuild_python_project::compiled_views::main::project::compiled_seed_record::compiled_seed_record;
use sqlbuild_python_project::compiled_views::main::project::compiled_source_record::compiled_source_record;
use sqlbuild_python_project::compiled_views::models::NativeCompiledModel;

/// The first bytes of every payload file; hosts reject any other.
pub(crate) const CUSTOM_HOST_PAYLOAD_MAGIC: &[u8; 8] = b"SQBHOST1";
/// The config values model facts read.
const CONFIG_KEYS: [&str; 3] = ["materialized", "contract", "grain"];
const PICKLE_TAG: &str = "p";
const TEXT_TAG: &str = "s";
const TEXTS_TAG: &str = "t";
const INFERRED_TAG: &str = "r";
const COLUMNS_TAG: &str = "c";
const PATHLIB_MODULE: &str = "pathlib";
const PATH_CLASSES: [&str; 2] = ["PosixPath", "WindowsPath"];

type RecordConversion = for<'py> fn(&Bound<'py, PyAny>) -> PyResult<Bound<'py, PyAny>>;

/// Project collections in header order, with the record each item becomes.
const SECTIONS: [(&str, Option<RecordConversion>); 7] = [
    ("sources", Some(compiled_source_record)),
    ("seeds", Some(compiled_seed_record)),
    ("functions", Some(compiled_function_record)),
    ("audits", Some(compiled_audit_record)),
    ("sql_tests", None),
    ("public_enums", None),
    ("public_constants", None),
];

/// What a model's declared columns are written as.
enum Declared {
    Absent,
    Native(Vec<PayloadColumn>),
    Python,
}

/// Writes payload parts in order and remembers where each one lies.
struct PayloadWriter<W: Write> {
    out: W,
    offset: u64,
}

/// Write `project`'s facts and `config` to `out`: magic, parts, JSON header, header offset.
pub(crate) fn write_payload<W: Write>(
    out: W,
    project: &Bound<'_, PyAny>,
    config: &Bound<'_, PyAny>,
) -> PyResult<W> {
    let py: Python<'_> = project.py();
    let mut writer: PayloadWriter<W> = PayloadWriter { out, offset: 0 };
    writer.write(CUSTOM_HOST_PAYLOAD_MAGIC)?;
    let mut models: Vec<Value> = Vec::new();
    for entry in project_models(project)? {
        models.push(writer.model(py, &entry)?);
    }
    let mut sections: Map<String, Value> = Map::new();
    for (name, conversion) in SECTIONS {
        let value: Bound<'_, PyAny> = project_attribute(project, name)?;
        let value: Bound<'_, PyAny> = match conversion {
            Some(record) => PyTuple::new(
                py,
                value
                    .try_iter()?
                    .map(|item| record(&item?))
                    .collect::<PyResult<Vec<_>>>()?,
            )?
            .into_any(),
            None => value,
        };
        sections.insert(name.to_owned(), writer.pickle(&value)?);
    }
    let header: Value = json!({
        "target": writer.scalar(&native_field(project, "effective_target_name")?)?,
        "config": writer.pickle(config)?,
        "sections": sections,
        "models": models,
    });
    let header_offset: u64 = writer.offset;
    let text: Vec<u8> = serde_json::to_vec(&header)
        .map_err(|error| pyo3::exceptions::PyValueError::new_err(error.to_string()))?;
    writer.write(&text)?;
    writer.write(&header_offset.to_le_bytes())?;
    Ok(writer.out)
}

impl<W: Write> PayloadWriter<W> {
    fn write(&mut self, bytes: &[u8]) -> PyResult<()> {
        self.out.write_all(bytes)?;
        self.offset += bytes.len() as u64;
        Ok(())
    }

    /// `bytes` as one part tagged `tag`.
    fn part(&mut self, tag: &str, bytes: &[u8]) -> PyResult<Value> {
        let start: u64 = self.offset;
        self.write(bytes)?;
        Ok(json!([tag, start, bytes.len()]))
    }

    fn pickle(&mut self, value: &Bound<'_, PyAny>) -> PyResult<Value> {
        let bytes: Bound<'_, PyBytes> = pickled(value)?;
        self.part(PICKLE_TAG, bytes.as_bytes())
    }

    /// `None`, a `str`, or a tuple of `str` inline; any other value pickled.
    fn scalar(&mut self, value: &Bound<'_, PyAny>) -> PyResult<Value> {
        if value.is_none() {
            return Ok(Value::Null);
        }
        if let Some(text) = exact_text(value)? {
            return Ok(Value::String(text));
        }
        if value.is_exact_instance_of::<PyTuple>() {
            let items: &Bound<'_, PyTuple> = value.downcast::<PyTuple>()?;
            let mut texts: Vec<Value> = Vec::with_capacity(items.len());
            for item in items.iter() {
                let Some(text) = exact_text(&item)? else {
                    return self.pickle(value);
                };
                texts.push(Value::String(text));
            }
            return Ok(json!([TEXTS_TAG, texts]));
        }
        self.pickle(value)
    }

    /// A `str` as a text part; any other value pickled.
    fn text(&mut self, value: &Bound<'_, PyAny>) -> PyResult<Value> {
        if let Some(text) = exact_text(value)? {
            return self.part(TEXT_TAG, text.as_bytes());
        }
        self.pickle(value)
    }

    /// A concrete `pathlib.Path` as its text; any other value pickled.
    fn path(&mut self, path: &Bound<'_, PyAny>) -> PyResult<Value> {
        match plain_path_text(path)? {
            Some(text) => Ok(Value::String(text)),
            None => self.pickle(path),
        }
    }

    /// An empty tuple as `null`; any other value pickled.
    fn declarations(&mut self, value: &Bound<'_, PyAny>) -> PyResult<Value> {
        if value.is_exact_instance_of::<PyTuple>() && value.downcast::<PyTuple>()?.is_empty() {
            return Ok(Value::Null);
        }
        self.pickle(value)
    }

    fn model(&mut self, py: Python<'_>, entry: &ModelEntry<'_>) -> PyResult<Value> {
        let mut model: Map<String, Value> = Map::new();
        model.insert("name".to_owned(), Value::String(entry.name.clone()));
        model.insert("path".to_owned(), self.path(&entry.path)?);
        model.insert("key".to_owned(), json!(entry.key));
        model.insert("deps".to_owned(), json!(entry.deps));
        let mut config: Map<String, Value> = Map::new();
        if let Some(values) = &entry.config_values {
            for name in CONFIG_KEYS {
                if let Some(value) = values.get_item(name)? {
                    config.insert(name.to_owned(), self.scalar(&value)?);
                }
            }
        }
        model.insert("config".to_owned(), Value::Object(config));
        let facts: Map<String, Value> = match entry.model.downcast::<NativeCompiledModel>() {
            Ok(view) => self.native_model(py, view.get())?,
            Err(_) => self.record_model(&entry.model)?,
        };
        model.extend(facts);
        Ok(Value::Object(model))
    }

    /// A native view's facts, read from the compiled project without Python records.
    fn native_model(
        &mut self,
        py: Python<'_>,
        view: &NativeCompiledModel,
    ) -> PyResult<Map<String, Value>> {
        let mut model: Map<String, Value> = Map::new();
        let (authored, query): (Arc<str>, Arc<str>) = view.read(py, |record, _| {
            Ok((
                record.host.model_file.get().contents_text().clone(),
                record.query_sql.clone(),
            ))
        })?;
        model.insert(
            "authored".to_owned(),
            self.part(TEXT_TAG, authored.as_bytes())?,
        );
        model.insert("query".to_owned(), self.part(TEXT_TAG, query.as_bytes())?);
        let place: String = view.query_place(py)?;
        model.insert("place".to_owned(), self.part(TEXT_TAG, place.as_bytes())?);
        let inferred: Value = match view.inferred_rows(py)? {
            Some(rows) => self.part(INFERRED_TAG, &json_bytes(&rows)?)?,
            None => Value::Null,
        };
        model.insert("inferred".to_owned(), inferred);
        let declared: Value = match native_declared(py, view)? {
            Declared::Absent => Value::Null,
            Declared::Native(columns) => self.part(COLUMNS_TAG, &json_bytes(&columns)?)?,
            Declared::Python => match view.declared_columns(py)? {
                Some(columns) => self.pickle(columns.bind(py).as_any())?,
                None => Value::Null,
            },
        };
        model.insert("declared".to_owned(), declared);
        let enums: Bound<'_, PyAny> = view.enum_declarations(py)?.into_bound(py).into_any();
        model.insert("enums".to_owned(), self.declarations(&enums)?);
        let columns: Bound<'_, PyAny> = view.enum_columns(py)?.into_bound(py).into_any();
        model.insert("enum_types".to_owned(), enum_type_names(&columns)?);
        let constants: Bound<'_, PyAny> = view.constant_declarations(py)?.into_bound(py).into_any();
        model.insert("constants".to_owned(), self.declarations(&constants)?);
        Ok(model)
    }

    /// A model record's facts, read through its fields.
    fn record_model(&mut self, record: &Bound<'_, PyAny>) -> PyResult<Map<String, Value>> {
        let mut model: Map<String, Value> = Map::new();
        let authored: Value = self.text(&native_field(record, "authored_sql")?)?;
        model.insert("authored".to_owned(), authored);
        let query: Value = self.text(&native_field(record, "query_sql")?)?;
        model.insert("query".to_owned(), query);
        model.insert("place".to_owned(), Value::Null);
        let inferred: Bound<'_, PyAny> = native_field(record, "inferred_columns")?;
        let inferred: Value = if inferred.is_none() {
            Value::Null
        } else {
            self.pickle(&inferred)?
        };
        model.insert("inferred".to_owned(), inferred);
        let entry: Bound<'_, PyAny> = native_field(record, "schema_entry")?;
        let declared: Value = if entry.is_none() {
            Value::Null
        } else {
            self.pickle(&native_field(&entry, "columns")?)?
        };
        model.insert("declared".to_owned(), declared);
        let enums: Value = self.declarations(&native_field(record, "enum_declarations")?)?;
        model.insert("enums".to_owned(), enums);
        model.insert(
            "enum_types".to_owned(),
            enum_type_names(&enum_columns(record)?)?,
        );
        let constants: Value =
            self.declarations(&native_field(record, "constant_declarations")?)?;
        model.insert("constants".to_owned(), constants);
        Ok(model)
    }
}

/// `[column, enum name]` for each enum-typed column, in column order.
fn enum_type_names(columns: &Bound<'_, PyAny>) -> PyResult<Value> {
    let mut pairs: Vec<Value> = Vec::new();
    for (column, declaration) in columns.downcast::<PyDict>()?.iter() {
        let column: String = column.extract()?;
        let name: String = native_field(&declaration, "name")?.extract()?;
        pairs.push(json!([column, name]));
    }
    Ok(Value::Array(pairs))
}

/// The view's declared columns when every one was parsed natively at a plain path.
fn native_declared(py: Python<'_>, view: &NativeCompiledModel) -> PyResult<Declared> {
    view.read(py, |record, py| {
        let Some(schema) = &record.schema else {
            return Ok(Declared::Absent);
        };
        let mut columns: Vec<PayloadColumn> = Vec::with_capacity(schema.host.columns.specs.len());
        for column in &schema.host.columns.specs {
            let Some(spec) = column.native_spec() else {
                return Ok(Declared::Python);
            };
            let location: Option<(String, LineColumnSpan)> = match &spec.location {
                Some((file, span)) => match plain_path_text(&file.get().path_objects(py)?.1)? {
                    Some(text) => Some((text, *span)),
                    None => return Ok(Declared::Python),
                },
                None => None,
            };
            columns.push(PayloadColumn::new(&spec.column, location));
        }
        Ok(Declared::Native(columns))
    })
}

/// The text of a concrete `pathlib.Path`, which `Path(text)` rebuilds equal, or `None`.
fn plain_path_text(path: &Bound<'_, PyAny>) -> PyResult<Option<String>> {
    let class = path.get_type();
    let module: String = class.module()?.to_str()?.to_owned();
    let qualname: String = class.qualname()?.to_str()?.to_owned();
    if module.split('.').next() != Some(PATHLIB_MODULE)
        || !PATH_CLASSES.contains(&qualname.as_str())
    {
        return Ok(None);
    }
    if let Ok(text) = path.str()?.to_str() {
        return Ok(Some(text.to_owned()));
    }
    Ok(None)
}

/// The text of an exact `str` that UTF-8 can carry, or `None`.
fn exact_text(value: &Bound<'_, PyAny>) -> PyResult<Option<String>> {
    if !value.is_exact_instance_of::<PyString>() {
        return Ok(None);
    }
    if let Ok(text) = value.downcast::<PyString>()?.to_str() {
        return Ok(Some(text.to_owned()));
    }
    Ok(None)
}

fn json_bytes<T: serde::Serialize>(value: &T) -> PyResult<Vec<u8>> {
    serde_json::to_vec(value)
        .map_err(|error| pyo3::exceptions::PyValueError::new_err(error.to_string()))
}

/// The payload a custom-rule host decodes for `project` and `config`.
#[pyfunction]
#[pyo3(signature = (project, config))]
pub(crate) fn custom_host_payload<'py>(
    project: &Bound<'py, PyAny>,
    config: &Bound<'py, PyAny>,
) -> PyResult<Bound<'py, PyBytes>> {
    let bytes: Vec<u8> = write_payload(Vec::new(), project, config)?;
    Ok(PyBytes::new(project.py(), &bytes))
}
