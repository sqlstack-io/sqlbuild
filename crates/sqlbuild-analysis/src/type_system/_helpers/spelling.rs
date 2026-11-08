//! Each dialect's spelling of a type tree: the canonical text `same_type` compares.

use serde_json::Value;

/// A dialect whose type spellings SQLBuild knows.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum SpellingDialect {
    Generic,
    Snowflake,
    BigQuery,
    DuckDb,
    Databricks,
    Postgres,
    Tsql,
    Sqlite,
    ClickHouse,
}

impl SpellingDialect {
    /// The dialect a name refers to, in any letter case; None for a dialect SQLBuild lacks.
    pub(crate) fn named(name: &str) -> Option<Self> {
        Some(match name.to_ascii_lowercase().as_str() {
            "" | "generic" => Self::Generic,
            "snowflake" => Self::Snowflake,
            "bigquery" => Self::BigQuery,
            "duckdb" | "motherduck" => Self::DuckDb,
            "databricks" => Self::Databricks,
            "postgres" | "postgresql" => Self::Postgres,
            "tsql" | "mssql" | "sqlserver" => Self::Tsql,
            "sqlite" => Self::Sqlite,
            "clickhouse" => Self::ClickHouse,
            _ => return None,
        })
    }
}

/// `tree` written as `dialect` spells it.
pub(crate) fn spell(tree: &Value, dialect: SpellingDialect) -> String {
    let mut text: String = String::new();
    Speller {
        dialect,
        text: &mut text,
    }
    .data_type(tree, false);
    text
}

struct Speller<'a> {
    dialect: SpellingDialect,
    text: &'a mut String,
}

impl Speller<'_> {
    fn write(&mut self, text: &str) {
        self.text.push_str(text);
    }

    /// `name`, then `(length)` when there is one.
    fn sized(&mut self, name: &str, length: Option<u64>) {
        self.write(name);
        if let Some(length) = length {
            self.write(&format!("({length})"));
        }
    }

    /// `name`, then `(precision)` or `(precision, scale)` when there is a precision.
    fn scaled(&mut self, name: &str, precision: Option<u64>, scale: Option<u64>) {
        self.write(name);
        if let Some(precision) = precision {
            match scale {
                Some(scale) => self.write(&format!("({precision}, {scale})")),
                None => self.write(&format!("({precision})")),
            }
        }
    }

    /// A ClickHouse scalar, nullable except as a map key.
    fn clickhouse(&mut self, name: &str, map_key: bool) {
        if map_key {
            self.write(name);
        } else {
            self.write(&format!("Nullable({name})"));
        }
    }

    fn data_type(&mut self, tree: &Value, map_key: bool) {
        let tag: &str = tree.get("data_type").and_then(Value::as_str).unwrap_or("");
        if self.number_type(tree, tag, map_key)
            || self.text_type(tree, tag, map_key)
            || self.time_type(tree, tag)
        {
            return;
        }
        self.other_type(tree, tag);
    }

    /// A boolean or numeric type; false for any other tag.
    fn number_type(&mut self, tree: &Value, tag: &str, map_key: bool) -> bool {
        use SpellingDialect::{
            BigQuery, ClickHouse, Databricks, Postgres, Snowflake, Sqlite, Tsql,
        };
        let dialect: SpellingDialect = self.dialect;
        let number = |key: &str| tree.get(key).and_then(Value::as_u64);
        let flag = |key: &str| tree.get(key).and_then(Value::as_bool).unwrap_or(false);
        let length: Option<u64> = number("length");
        let precision: Option<u64> = number("precision");
        let scale: Option<u64> = number("scale");
        match tag {
            "boolean" => match dialect {
                Tsql => self.write("BIT"),
                ClickHouse => self.write("Bool"),
                _ => self.write("BOOLEAN"),
            },
            "tiny_int" => match dialect {
                Postgres => self.sized("SMALLINT", length),
                ClickHouse => self.clickhouse("Int8", map_key),
                _ => self.sized("TINYINT", length),
            },
            "small_int" => match dialect {
                Sqlite => self.write("INTEGER"),
                BigQuery => self.write("INT64"),
                ClickHouse => self.clickhouse("Int16", map_key),
                _ => self.sized("SMALLINT", length),
            },
            "int" => match dialect {
                BigQuery => self.write("INT64"),
                ClickHouse => self.clickhouse("Int32", map_key),
                Tsql | Sqlite => self.sized("INTEGER", length),
                _ => self.sized("INT", length),
            },
            "big_int" => match dialect {
                BigQuery => self.write("INT64"),
                ClickHouse => self.clickhouse("Int64", map_key),
                _ => self.sized("BIGINT", length),
            },
            "int128" => match dialect {
                ClickHouse => self.write("Int128"),
                _ => self.write("INT128"),
            },
            "uint8" | "uint16" | "uint32" | "uint64" | "uint128" => {
                let (canonical, clickhouse) = match tag {
                    "uint8" => ("UTINYINT", "UInt8"),
                    "uint16" => ("USMALLINT", "UInt16"),
                    "uint32" => ("UINTEGER", "UInt32"),
                    "uint64" => ("UBIGINT", "UInt64"),
                    _ => ("UINT128", "UInt128"),
                };
                self.write(if dialect == ClickHouse {
                    clickhouse
                } else {
                    canonical
                });
            }
            "float" if dialect == ClickHouse => {
                self.clickhouse("Float32", map_key);
                self.scaled("", precision, scale);
            }
            "double" if dialect == ClickHouse => {
                self.clickhouse("Float64", map_key);
                self.scaled("", precision, scale);
            }
            "float" => {
                let name: &str = if flag("real_spelling")
                    && !matches!(dialect, Databricks | Snowflake | BigQuery)
                {
                    "REAL"
                } else {
                    match dialect {
                        Postgres => "REAL",
                        BigQuery => "FLOAT64",
                        _ => "FLOAT",
                    }
                };
                if dialect == Databricks {
                    self.write(name);
                } else {
                    self.scaled(name, precision, scale);
                }
            }
            "double" => {
                let name: &str = match dialect {
                    Tsql => "FLOAT",
                    BigQuery => "FLOAT64",
                    Sqlite => "REAL",
                    Postgres => "DOUBLE PRECISION",
                    _ => "DOUBLE",
                };
                self.scaled(name, precision, scale);
            }
            "decimal" => match dialect {
                ClickHouse => self.scaled("Decimal", precision, scale),
                BigQuery => self.scaled("NUMERIC", precision, scale),
                _ => self.scaled("DECIMAL", precision, scale),
            },
            _ => return false,
        }
        true
    }

    /// A text, binary, JSON or UUID type; false for any other tag.
    fn text_type(&mut self, tree: &Value, tag: &str, map_key: bool) -> bool {
        use SpellingDialect::{
            BigQuery, ClickHouse, Databricks, DuckDb, Postgres, Snowflake, Sqlite, Tsql,
        };
        let dialect: SpellingDialect = self.dialect;
        let number = |key: &str| tree.get(key).and_then(Value::as_u64);
        let length: Option<u64> = number("length");
        match tag {
            "text_with_length" if dialect == ClickHouse => self.write("Nullable(String)"),
            "char" | "var_char" | "string" | "text" if dialect == ClickHouse => {
                self.write("String");
            }
            "char" => match dialect {
                DuckDb | Sqlite => self.write("TEXT"),
                Databricks if length.is_none() => self.write("STRING"),
                _ => self.sized("CHAR", length),
            },
            "var_char" => match dialect {
                DuckDb | Sqlite => self.sized("TEXT", length),
                Databricks if length.is_none() => self.write("STRING"),
                _ => self.sized("VARCHAR", length),
            },
            "text" => match dialect {
                Tsql => self.write("VARCHAR(MAX)"),
                BigQuery | Databricks => self.write("STRING"),
                Snowflake => self.write("VARCHAR"),
                _ => self.write("TEXT"),
            },
            "text_with_length" => match dialect {
                Databricks | Snowflake | Tsql => self.sized("VARCHAR", length),
                BigQuery => self.sized("STRING", length),
                _ => self.sized("TEXT", length),
            },
            "string" => match dialect {
                Postgres if length.is_none() => self.write("TEXT"),
                Postgres => self.sized("VARCHAR", length),
                Tsql if length.is_none() => self.write("VARCHAR(MAX)"),
                Tsql => self.sized("VARCHAR", length),
                DuckDb => self.sized("TEXT", length),
                _ => self.sized("STRING", length),
            },
            "binary" | "var_binary" if dialect == BigQuery => self.sized("BYTES", length),
            "binary" => match dialect {
                Postgres => self.sized("BYTEA", length),
                DuckDb | Sqlite => self.sized("BLOB", length),
                _ => self.sized("BINARY", length),
            },
            "var_binary" => match dialect {
                Postgres => self.sized("BYTEA", length),
                DuckDb | Sqlite => self.sized("BLOB", length),
                Databricks => self.sized("BINARY", length),
                ClickHouse => self.clickhouse("String", map_key),
                _ => self.sized("VARBINARY", length),
            },
            "blob" => match dialect {
                Postgres => self.write("BYTEA"),
                Tsql | DuckDb => self.write("VARBINARY"),
                BigQuery => self.write("BYTES"),
                Databricks => self.write("BINARY"),
                ClickHouse => self.write("Nullable(String)"),
                _ => self.write("BLOB"),
            },
            "bit" => match dialect {
                Databricks | Snowflake | BigQuery | ClickHouse => self.write("BOOLEAN"),
                _ => self.sized("BIT", length),
            },
            "var_bit" => self.sized("VARBIT", length),
            "json_b" if dialect == Postgres => self.write("JSONB"),
            "json" | "json_b" => match dialect {
                Tsql => self.write("NVARCHAR(MAX)"),
                Snowflake => self.write("VARIANT"),
                _ => self.write("JSON"),
            },
            "uuid" => match dialect {
                Tsql => self.write("UNIQUEIDENTIFIER"),
                BigQuery | Databricks => self.write("STRING"),
                _ => self.write("UUID"),
            },
            _ => return false,
        }
        true
    }

    /// A date, time, timestamp or interval type; false for any other tag.
    fn time_type(&mut self, tree: &Value, tag: &str) -> bool {
        use SpellingDialect::{BigQuery, ClickHouse, Databricks, DuckDb, Postgres, Tsql};
        let dialect: SpellingDialect = self.dialect;
        let number = |key: &str| tree.get(key).and_then(Value::as_u64);
        let flag = |key: &str| tree.get(key).and_then(Value::as_bool).unwrap_or(false);
        let precision: Option<u64> = number("precision");
        match tag {
            "date" => self.write("DATE"),
            "time" if flag("timezone") => match dialect {
                DuckDb => self.write("TIMETZ"),
                Postgres => self.sized("TIMETZ", precision),
                _ => {
                    self.sized("TIME", precision);
                    self.write(" WITH TIME ZONE");
                }
            },
            "time" => match dialect {
                Databricks => self.write("TIMESTAMP"),
                _ => self.sized("TIME", precision),
            },
            "timestamp" => {
                let timezone: bool = flag("timezone");
                match dialect {
                    ClickHouse => self.sized("DateTime", precision),
                    Tsql if timezone => self.sized("DATETIMEOFFSET", precision),
                    Tsql => self.sized("DATETIME2", precision),
                    BigQuery if timezone => self.write("TIMESTAMP"),
                    BigQuery => self.write("DATETIME"),
                    DuckDb if timezone => self.write("TIMESTAMPTZ"),
                    _ if timezone => self.sized("TIMESTAMPTZ", precision),
                    _ => self.sized("TIMESTAMP", precision),
                }
            }
            "interval" => {
                self.write("INTERVAL");
                for (key, prefix) in [("unit", " "), ("to", " TO ")] {
                    if let Some(unit) = tree.get(key).and_then(Value::as_str) {
                        self.write(prefix);
                        self.write(&unit.to_ascii_uppercase());
                    }
                }
            }
            _ => return false,
        }
        true
    }

    /// A nested, custom or other type.
    fn other_type(&mut self, tree: &Value, tag: &str) {
        use SpellingDialect::{ClickHouse, DuckDb, Generic, Postgres, Snowflake, Tsql};
        let dialect: SpellingDialect = self.dialect;
        let number = |key: &str| tree.get(key).and_then(Value::as_u64);
        match tag {
            "array" => {
                let element: &Value = tree.get("element_type").unwrap_or(&Value::Null);
                match dialect {
                    Postgres | DuckDb => {
                        self.data_type(element, false);
                        match number("dimension") {
                            Some(dimension) => self.write(&format!("[{dimension}]")),
                            None => self.write("[]"),
                        }
                    }
                    Snowflake | ClickHouse => {
                        self.write(if dialect == ClickHouse {
                            "Array("
                        } else {
                            "ARRAY("
                        });
                        self.data_type(element, false);
                        self.write(")");
                    }
                    Tsql => self.write("NVARCHAR(MAX)"),
                    _ => {
                        self.write("ARRAY<");
                        self.data_type(element, false);
                        self.write(">");
                    }
                }
            }
            "list" => {
                self.data_type(tree.get("element_type").unwrap_or(&Value::Null), false);
                self.write(" LIST");
            }
            "map" => {
                let key: &Value = tree.get("key_type").unwrap_or(&Value::Null);
                let value: &Value = tree.get("value_type").unwrap_or(&Value::Null);
                let (open, close) = match dialect {
                    Snowflake | DuckDb => ("MAP(", ")"),
                    ClickHouse => ("Map(", ")"),
                    _ => ("MAP<", ">"),
                };
                self.write(open);
                self.data_type(key, dialect == ClickHouse);
                self.write(", ");
                self.data_type(value, false);
                self.write(close);
            }
            "struct" => self.struct_type(tree),
            "object" => self.field_list("OBJECT(", tree),
            "union" => self.field_list("UNION(", tree),
            "vector" => {
                self.write("VECTOR");
                let element: Option<&Value> = tree.get("element_type").filter(|v| !v.is_null());
                let dimension: Option<u64> = number("dimension");
                if element.is_some() || dimension.is_some() {
                    self.write("(");
                    if let Some(element) = element {
                        self.data_type(element, false);
                        if dimension.is_some() {
                            self.write(", ");
                        }
                    }
                    if let Some(dimension) = dimension {
                        self.write(&dimension.to_string());
                    }
                    self.write(")");
                }
            }
            "enum" => {
                let values: Vec<String> = tree
                    .get("values")
                    .and_then(Value::as_array)
                    .map_or(&[][..], Vec::as_slice)
                    .iter()
                    .map(|value| format!("'{}'", value.as_str().unwrap_or("").replace('\'', "''")))
                    .collect();
                self.write(&format!("ENUM({})", values.join(", ")));
            }
            "custom" => {
                let name: &str = tree.get("name").and_then(Value::as_str).unwrap_or("");
                self.write(&custom_name(name, dialect));
            }
            _ if dialect == Generic => self.write(&tag.to_ascii_uppercase()),
            _ => self.write(&tag.to_ascii_uppercase().replace('_', "")),
        }
    }

    /// An `OBJECT` or `UNION` of named fields, opened by `open`.
    fn field_list(&mut self, open: &str, tree: &Value) {
        self.write(open);
        let fields: &[Value] = tree
            .get("fields")
            .and_then(Value::as_array)
            .map_or(&[], Vec::as_slice);
        for (index, field) in fields.iter().enumerate() {
            if index > 0 {
                self.write(", ");
            }
            self.write(field.get(0).and_then(Value::as_str).unwrap_or(""));
            self.write(" ");
            self.data_type(field.get(1).unwrap_or(&Value::Null), false);
        }
        self.write(")");
    }

    fn struct_type(&mut self, tree: &Value) {
        use SpellingDialect::{BigQuery, ClickHouse, Databricks, DuckDb, Snowflake};
        let dialect: SpellingDialect = self.dialect;
        let nested: bool = tree.get("nested").and_then(Value::as_bool).unwrap_or(false);
        let (open, close, separator): (&str, &str, &str) = match dialect {
            Snowflake => ("OBJECT(", ")", " "),
            DuckDb => ("STRUCT(", ")", " "),
            ClickHouse => ("Tuple(", ")", " "),
            Databricks | BigQuery => (
                "STRUCT<",
                ">",
                if dialect == Databricks { ": " } else { " " },
            ),
            _ if nested => ("STRUCT(", ")", " "),
            _ => ("STRUCT<", ">", " "),
        };
        self.write(open);
        let fields: &[Value] = tree
            .get("fields")
            .and_then(Value::as_array)
            .map_or(&[], Vec::as_slice);
        for (index, field) in fields.iter().enumerate() {
            if index > 0 {
                self.write(", ");
            }
            let name: &str = field.get("name").and_then(Value::as_str).unwrap_or("");
            if !name.is_empty() {
                self.write(name);
                self.write(separator);
            }
            self.data_type(field.get("data_type").unwrap_or(&Value::Null), false);
        }
        self.write(close);
    }
}

/// A type name no dialect gives its own shape, as `dialect` spells it.
fn custom_name(name: &str, dialect: SpellingDialect) -> String {
    let (base, arguments): (&str, &str) = name.find('(').map_or((name, ""), |at| name.split_at(at));
    let base_upper: String = base.trim().to_ascii_uppercase();
    match (dialect, base_upper.as_str()) {
        (SpellingDialect::Snowflake, "TIMESTAMP_NTZ" | "TIMESTAMP_LTZ" | "TIMESTAMP_TZ") => {
            format!("{}{arguments}", base_upper.replace('_', ""))
        }
        (SpellingDialect::Tsql, "VARIANT") => "SQL_VARIANT".to_owned(),
        (
            SpellingDialect::BigQuery | SpellingDialect::ClickHouse | SpellingDialect::Snowflake,
            _,
        ) => name.to_owned(),
        (_, "INT64") if arguments.is_empty() => "BIGINT".to_owned(),
        (_, "FLOAT64") if arguments.is_empty() => "DOUBLE".to_owned(),
        _ => name.to_owned(),
    }
}
