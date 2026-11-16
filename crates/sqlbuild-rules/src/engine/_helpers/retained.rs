//! Retained model rows in the native record format, without their SQL text.

use std::collections::BTreeMap;
use std::str::FromStr;

use serde_json::{Map, Number, Value};
use sqlbuild_cache::store::errors::StoreDecodeError;
use sqlbuild_cache::store::models::{RecordReader, RecordWriter};

use crate::models::{
    Column, Declaration, DeclarationMember, DynamicColumnFamily, EvaluateRequest, Model, Reference,
    RetainedRows,
};

const NULL: u64 = 0;
const FALSE: u64 = 1;
const TRUE: u64 = 2;
const NUMBER: u64 = 3;
const STRING: u64 = 4;
const ARRAY: u64 = 5;
const OBJECT: u64 = 6;
const ABSENT: u64 = 0;
const PRESENT: u64 = 1;
/// Items reserved up front per decoded list at most, whatever count a damaged row claims.
const MAX_RESERVED_ITEMS: usize = 1 << 12;

/// Writes model rows as length-prefixed records.
#[derive(Default)]
struct RowWriter {
    record: RecordWriter,
}

impl RowWriter {
    fn flag(&mut self, value: bool) {
        self.record.put_u64(u64::from(value));
    }

    fn count(&mut self, value: usize) {
        self.record.put_u64(value as u64);
    }

    fn text(&mut self, value: &str) {
        self.record.put_str(value);
    }

    fn optional_text(&mut self, value: Option<&str>) {
        match value {
            Some(text) => {
                self.record.put_u64(PRESENT);
                self.text(text);
            }
            None => self.record.put_u64(ABSENT),
        }
    }

    fn texts(&mut self, values: &[String]) {
        self.count(values.len());
        for value in values {
            self.text(value);
        }
    }

    fn value(&mut self, value: &Value) {
        match value {
            Value::Null => self.record.put_u64(NULL),
            Value::Bool(flag) => self.record.put_u64(if *flag { TRUE } else { FALSE }),
            Value::Number(number) => {
                self.record.put_u64(NUMBER);
                self.text(&number.to_string());
            }
            Value::String(text) => {
                self.record.put_u64(STRING);
                self.text(text);
            }
            Value::Array(items) => {
                self.record.put_u64(ARRAY);
                self.count(items.len());
                for item in items {
                    self.value(item);
                }
            }
            Value::Object(entries) => {
                self.record.put_u64(OBJECT);
                self.count(entries.len());
                for (key, item) in entries {
                    self.text(key);
                    self.value(item);
                }
            }
        }
    }

    fn optional_value(&mut self, value: Option<&Value>) {
        match value {
            Some(value) => {
                self.record.put_u64(PRESENT);
                self.value(value);
            }
            None => self.record.put_u64(ABSENT),
        }
    }

    fn declarations(&mut self, declarations: &[Declaration]) {
        self.count(declarations.len());
        for declaration in declarations {
            let Declaration {
                name,
                relative_path,
                members,
                value,
                value_type,
                render_as,
            } = declaration;
            self.text(name);
            self.text(relative_path);
            self.count(members.len());
            for DeclarationMember { name, value } in members {
                self.text(name);
                self.value(value);
            }
            self.optional_value(value.as_ref());
            self.optional_text(value_type.as_deref());
            self.optional_text(render_as.as_deref());
        }
    }

    /// One model's row; its SQL text, digest and derived counts are left out.
    fn model(&mut self, model: &Model) {
        let Model {
            sql_analysis_disabled,
            name,
            relative_path,
            query_sql: _,
            authored_sql: _,
            query_line_offset: _,
            query_column_offset: _,
            authored_query: _,
            typed_calls: _,
            config,
            authored_config_keys,
            logical_schema,
            references,
            columns,
            dynamic_columns,
            dynamic_columns_proven,
            bare_dynamic_pivot,
            enum_columns,
            enum_declarations,
            constant_declarations,
            declared_audit_count,
            targeting_test_count,
            empty_input_only_test_count: _,
            payload_digest: _,
            stripped: _,
            json_grain,
        } = model;
        self.flag(*sql_analysis_disabled);
        self.text(name);
        self.text(relative_path);
        self.count(config.len());
        for (key, value) in config {
            self.text(key);
            self.value(value);
        }
        self.texts(authored_config_keys);
        self.optional_text(logical_schema.as_deref());
        self.count(references.len());
        for Reference {
            ref_kind,
            ref_name,
            ref_package,
        } in references
        {
            self.text(ref_kind);
            self.text(ref_name);
            self.optional_text(ref_package.as_deref());
        }
        self.count(columns.len());
        for Column {
            name,
            data_type,
            nullable,
            audit_count,
            type_proven,
        } in columns
        {
            self.text(name);
            self.text(data_type);
            self.record.put_u64(match nullable {
                None => NULL,
                Some(false) => FALSE,
                Some(true) => TRUE,
            });
            self.record.put_u64(u64::from(*audit_count));
            self.flag(*type_proven);
        }
        self.count(dynamic_columns.len());
        for DynamicColumnFamily {
            name,
            pivot_column,
            value_column,
            aggregate,
            data_type,
            name_pattern,
            type_proven,
        } in dynamic_columns
        {
            for text in [name, pivot_column, value_column, aggregate, data_type] {
                self.text(text);
            }
            self.optional_text(name_pattern.as_deref());
            self.flag(*type_proven);
        }
        self.flag(*dynamic_columns_proven);
        self.flag(*bare_dynamic_pivot);
        self.texts(enum_columns);
        self.declarations(enum_declarations);
        self.declarations(constant_declarations);
        self.record.put_u64(u64::from(*declared_audit_count));
        self.record.put_u64(u64::from(*targeting_test_count));
        let grain: Option<Value> = json_grain
            .as_ref()
            .map(|grain| serde_json::to_value(grain).unwrap_or(Value::Null));
        self.optional_value(grain.as_ref());
    }
}

/// Reads the rows a [`RowWriter`] wrote.
struct RowReader<'a> {
    record: RecordReader<'a>,
}

type Read<T> = Result<T, StoreDecodeError>;

impl RowReader<'_> {
    fn count(&mut self) -> Read<usize> {
        usize::try_from(self.record.u64()?).map_err(|_| StoreDecodeError)
    }

    fn small(&mut self) -> Read<u32> {
        u32::try_from(self.record.u64()?).map_err(|_| StoreDecodeError)
    }

    fn flag(&mut self) -> Read<bool> {
        match self.record.u64()? {
            0 => Ok(false),
            1 => Ok(true),
            _ => Err(StoreDecodeError),
        }
    }

    fn text(&mut self) -> Read<String> {
        self.record.string()
    }

    fn present(&mut self) -> Read<bool> {
        match self.record.u64()? {
            ABSENT => Ok(false),
            PRESENT => Ok(true),
            _ => Err(StoreDecodeError),
        }
    }

    fn optional_text(&mut self) -> Read<Option<String>> {
        Ok(if self.present()? {
            Some(self.text()?)
        } else {
            None
        })
    }

    fn list<T>(&mut self, mut item: impl FnMut(&mut Self) -> Read<T>) -> Read<Vec<T>> {
        let count: usize = self.count()?;
        let mut items: Vec<T> = Vec::with_capacity(count.min(MAX_RESERVED_ITEMS));
        for _ in 0..count {
            items.push(item(self)?);
        }
        Ok(items)
    }

    fn value(&mut self) -> Read<Value> {
        Ok(match self.record.u64()? {
            NULL => Value::Null,
            FALSE => Value::Bool(false),
            TRUE => Value::Bool(true),
            NUMBER => Value::Number(Number::from_str(&self.text()?).map_err(|_| StoreDecodeError)?),
            STRING => Value::String(self.text()?),
            ARRAY => Value::Array(self.list(Self::value)?),
            OBJECT => {
                let count: usize = self.count()?;
                let mut entries: Map<String, Value> = Map::new();
                for _ in 0..count {
                    let key: String = self.text()?;
                    entries.insert(key, self.value()?);
                }
                Value::Object(entries)
            }
            _ => return Err(StoreDecodeError),
        })
    }

    fn optional_value(&mut self) -> Read<Option<Value>> {
        Ok(if self.present()? {
            Some(self.value()?)
        } else {
            None
        })
    }

    fn declaration(&mut self) -> Read<Declaration> {
        Ok(Declaration {
            name: self.text()?,
            relative_path: self.text()?,
            members: self.list(|reader| {
                Ok(DeclarationMember {
                    name: reader.text()?,
                    value: reader.value()?,
                })
            })?,
            value: self.optional_value()?,
            value_type: self.optional_text()?,
            render_as: self.optional_text()?,
        })
    }

    fn column(&mut self) -> Read<Column> {
        Ok(Column {
            name: self.text()?,
            data_type: self.text()?,
            nullable: match self.record.u64()? {
                NULL => None,
                FALSE => Some(false),
                TRUE => Some(true),
                _ => return Err(StoreDecodeError),
            },
            audit_count: self.small()?,
            type_proven: self.flag()?,
        })
    }

    fn dynamic_column(&mut self) -> Read<DynamicColumnFamily> {
        Ok(DynamicColumnFamily {
            name: self.text()?,
            pivot_column: self.text()?,
            value_column: self.text()?,
            aggregate: self.text()?,
            data_type: self.text()?,
            name_pattern: self.optional_text()?,
            type_proven: self.flag()?,
        })
    }

    fn model(&mut self) -> Read<Model> {
        Ok(Model {
            sql_analysis_disabled: self.flag()?,
            name: self.text()?,
            relative_path: self.text()?,
            query_sql: String::new(),
            authored_sql: String::new(),
            query_line_offset: 0,
            query_column_offset: 0,
            authored_query: None,
            typed_calls: Vec::new(),
            config: {
                let count: usize = self.count()?;
                let mut config: BTreeMap<String, Value> = BTreeMap::new();
                for _ in 0..count {
                    let key: String = self.text()?;
                    config.insert(key, self.value()?);
                }
                config
            },
            authored_config_keys: self.list(Self::text)?,
            logical_schema: self.optional_text()?,
            references: self.list(|reader| {
                Ok(Reference {
                    ref_kind: reader.text()?,
                    ref_name: reader.text()?,
                    ref_package: reader.optional_text()?,
                })
            })?,
            columns: self.list(Self::column)?,
            dynamic_columns: self.list(Self::dynamic_column)?,
            dynamic_columns_proven: self.flag()?,
            bare_dynamic_pivot: self.flag()?,
            enum_columns: self.list(Self::text)?,
            enum_declarations: self.list(Self::declaration)?,
            constant_declarations: self.list(Self::declaration)?,
            declared_audit_count: self.small()?,
            targeting_test_count: self.small()?,
            empty_input_only_test_count: 0,
            payload_digest: None,
            stripped: false,
            json_grain: self
                .optional_value()?
                .map(serde_json::from_value)
                .transpose()
                .map_err(|_| StoreDecodeError)?,
        })
    }
}

/// Every model's row without its SQL text, in model order.
pub(crate) fn encoded_rows(models: &[Model]) -> Vec<u8> {
    let mut writer: RowWriter = RowWriter::default();
    writer.count(models.len());
    for model in models {
        writer.model(model);
    }
    writer.record.into_bytes()
}

/// The request `retained` holds, each model without its SQL text, digest or derived counts.
pub(crate) fn retained_request(retained: &RetainedRows) -> Read<EvaluateRequest> {
    let mut request: EvaluateRequest =
        serde_json::from_slice(&retained.request_json).map_err(|_| StoreDecodeError)?;
    let mut reader: RowReader<'_> = RowReader {
        record: RecordReader::new(&retained.rows),
    };
    request.models = reader.list(RowReader::model)?;
    reader.record.finish()?;
    if request.models.len() != retained.digests.len() {
        return Err(StoreDecodeError);
    }
    Ok(request)
}
