//! Natively parsed header columns as custom-rule host payloads carry them.

use std::path::PathBuf;

use pyo3::prelude::{Bound, PyAny, PyResult, Python};
use pyo3::types::PyTuple;
use pyo3::{IntoPyObject, pyfunction};
use serde::{Deserialize, Serialize};
use sqlbuild_discovery::model_files::models::{NativeAudit, NativeColumn};
use sqlbuild_discovery::models::LineColumnSpan;
use sqlbuild_model_config::header_metadata::models::{ParsedThresholds, ThresholdBound};
use sqlbuild_sqltext::compiler::models::AuthoredValue;

use crate::rules::_helpers::custom::custom_hosts::rules_error;
use sqlbuild_python_front::model_config::main::headers::native_column_record::native_column_record;

/// One native header column with the text of the path its location names.
#[derive(Debug, Serialize, Deserialize)]
pub(crate) struct PayloadColumn {
    name: String,
    column_type: Option<String>,
    nullable: Option<bool>,
    description: Option<String>,
    audits: Vec<PayloadAudit>,
    migrate_from: Option<String>,
    location: Option<(String, [usize; 4])>,
}

#[derive(Debug, Serialize, Deserialize)]
struct PayloadAudit {
    definition_name: String,
    arguments: Vec<(String, PayloadValue)>,
    name: Option<String>,
    description: Option<String>,
    severity: Option<String>,
    run_scope: Option<String>,
    always_run: Option<bool>,
    thresholds: Option<(Option<PayloadBound>, Option<PayloadBound>)>,
    minimum_samples: Option<PayloadValue>,
    evidence_limit: Option<PayloadValue>,
    meta: Option<PayloadValue>,
}

/// A threshold bound with its limits as `f64` bits, so every value survives exactly.
#[derive(Debug, Serialize, Deserialize)]
enum PayloadBound {
    Below(u64),
    Above(u64),
    Outside(u64, u64),
}

#[derive(Debug, Serialize, Deserialize)]
enum PayloadValue {
    Null,
    Boolean(bool),
    BareWord(String),
    String(String),
    List(Vec<PayloadValue>),
    Map(Vec<(String, PayloadValue)>),
    Set(Vec<PayloadValue>),
    Tuple(Vec<PayloadValue>),
    TypedConstant(Vec<(String, PayloadValue)>),
    InlineSqlHook(String),
    NamedSqlHook(String, Vec<(String, PayloadValue)>),
    PythonHook(String, Vec<(String, PayloadValue)>),
}

impl PayloadColumn {
    /// `column` with its location naming `path`.
    pub(crate) fn new(column: &NativeColumn, location: Option<(String, LineColumnSpan)>) -> Self {
        Self {
            name: column.name.clone(),
            column_type: column.column_type.clone(),
            nullable: column.nullable,
            description: column.description.clone(),
            audits: column.audits.iter().map(PayloadAudit::new).collect(),
            migrate_from: column.migrate_from.clone(),
            location: location.map(|(path, span)| {
                (
                    path,
                    [span.line, span.column, span.end_line, span.end_column],
                )
            }),
        }
    }

    fn column(self) -> (NativeColumn, Option<(String, LineColumnSpan)>) {
        let location: Option<(String, LineColumnSpan)> =
            self.location
                .map(|(path, [line, column, end_line, end_column])| {
                    (
                        path,
                        LineColumnSpan {
                            line,
                            column,
                            end_line,
                            end_column,
                        },
                    )
                });
        let column: NativeColumn = NativeColumn {
            name: self.name,
            column_type: self.column_type,
            nullable: self.nullable,
            description: self.description,
            audits: self.audits.into_iter().map(PayloadAudit::audit).collect(),
            migrate_from: self.migrate_from,
        };
        (column, location)
    }
}

impl PayloadAudit {
    fn new(audit: &NativeAudit) -> Self {
        Self {
            definition_name: audit.definition_name.clone(),
            arguments: pairs(&audit.arguments),
            name: audit.name.clone(),
            description: audit.description.clone(),
            severity: audit.severity.clone(),
            run_scope: audit.run_scope.clone(),
            always_run: audit.always_run,
            thresholds: audit.thresholds.map(|thresholds| {
                (
                    thresholds.warn.map(PayloadBound::new),
                    thresholds.error.map(PayloadBound::new),
                )
            }),
            minimum_samples: audit.minimum_samples.as_ref().map(PayloadValue::new),
            evidence_limit: audit.evidence_limit.as_ref().map(PayloadValue::new),
            meta: audit.meta.as_ref().map(PayloadValue::new),
        }
    }

    fn audit(self) -> NativeAudit {
        NativeAudit {
            definition_name: self.definition_name,
            arguments: authored_pairs(self.arguments),
            name: self.name,
            description: self.description,
            severity: self.severity,
            run_scope: self.run_scope,
            always_run: self.always_run,
            thresholds: self.thresholds.map(|(warn, error)| ParsedThresholds {
                warn: warn.map(PayloadBound::bound),
                error: error.map(PayloadBound::bound),
            }),
            minimum_samples: self.minimum_samples.map(PayloadValue::authored),
            evidence_limit: self.evidence_limit.map(PayloadValue::authored),
            meta: self.meta.map(PayloadValue::authored),
        }
    }
}

impl PayloadBound {
    fn new(bound: ThresholdBound) -> Self {
        match bound {
            ThresholdBound::Below(limit) => Self::Below(limit.to_bits()),
            ThresholdBound::Above(limit) => Self::Above(limit.to_bits()),
            ThresholdBound::Outside(lower, upper) => {
                Self::Outside(lower.to_bits(), upper.to_bits())
            }
        }
    }

    fn bound(self) -> ThresholdBound {
        match self {
            Self::Below(limit) => ThresholdBound::Below(f64::from_bits(limit)),
            Self::Above(limit) => ThresholdBound::Above(f64::from_bits(limit)),
            Self::Outside(lower, upper) => {
                ThresholdBound::Outside(f64::from_bits(lower), f64::from_bits(upper))
            }
        }
    }
}

impl PayloadValue {
    fn new(value: &AuthoredValue) -> Self {
        match value {
            AuthoredValue::Null => Self::Null,
            AuthoredValue::Boolean(flag) => Self::Boolean(*flag),
            AuthoredValue::BareWord(text) => Self::BareWord(text.clone()),
            AuthoredValue::String(text) => Self::String(text.clone()),
            AuthoredValue::List(items) => Self::List(items.iter().map(Self::new).collect()),
            AuthoredValue::Map(entries) => Self::Map(pairs(entries)),
            AuthoredValue::Set(items) => Self::Set(items.iter().map(Self::new).collect()),
            AuthoredValue::Tuple(items) => Self::Tuple(items.iter().map(Self::new).collect()),
            AuthoredValue::TypedConstant(entries) => Self::TypedConstant(pairs(entries)),
            AuthoredValue::InlineSqlHook(text) => Self::InlineSqlHook(text.clone()),
            AuthoredValue::NamedSqlHook(name, entries) => {
                Self::NamedSqlHook(name.clone(), pairs(entries))
            }
            AuthoredValue::PythonHook(name, entries) => {
                Self::PythonHook(name.clone(), pairs(entries))
            }
        }
    }

    fn authored(self) -> AuthoredValue {
        match self {
            Self::Null => AuthoredValue::Null,
            Self::Boolean(flag) => AuthoredValue::Boolean(flag),
            Self::BareWord(text) => AuthoredValue::BareWord(text),
            Self::String(text) => AuthoredValue::String(text),
            Self::List(items) => {
                AuthoredValue::List(items.into_iter().map(Self::authored).collect())
            }
            Self::Map(entries) => AuthoredValue::Map(authored_pairs(entries)),
            Self::Set(items) => AuthoredValue::Set(items.into_iter().map(Self::authored).collect()),
            Self::Tuple(items) => {
                AuthoredValue::Tuple(items.into_iter().map(Self::authored).collect())
            }
            Self::TypedConstant(entries) => AuthoredValue::TypedConstant(authored_pairs(entries)),
            Self::InlineSqlHook(text) => AuthoredValue::InlineSqlHook(text),
            Self::NamedSqlHook(name, entries) => {
                AuthoredValue::NamedSqlHook(name, authored_pairs(entries))
            }
            Self::PythonHook(name, entries) => {
                AuthoredValue::PythonHook(name, authored_pairs(entries))
            }
        }
    }
}

fn pairs(entries: &[(String, AuthoredValue)]) -> Vec<(String, PayloadValue)> {
    entries
        .iter()
        .map(|(key, value)| (key.clone(), PayloadValue::new(value)))
        .collect()
}

fn authored_pairs(entries: Vec<(String, PayloadValue)>) -> Vec<(String, AuthoredValue)> {
    entries
        .into_iter()
        .map(|(key, value)| (key, value.authored()))
        .collect()
}

/// The `SchemaColumn`s of one model's payload columns, built as the compiling process built them.
#[pyfunction]
pub(crate) fn custom_host_declared_columns<'py>(
    py: Python<'py>,
    data: &[u8],
) -> PyResult<Bound<'py, PyTuple>> {
    let columns: Vec<PayloadColumn> =
        serde_json::from_slice(data).map_err(|error| rules_error(py, error.to_string()))?;
    let mut records: Vec<Bound<'py, PyAny>> = Vec::with_capacity(columns.len());
    for column in columns {
        let (column, location) = column.column();
        let location: Option<(Bound<'py, PyAny>, LineColumnSpan)> = match location {
            Some((path, span)) => Some((PathBuf::from(path).into_pyobject(py)?.into_any(), span)),
            None => None,
        };
        records.push(native_column_record(py, &column, location)?);
    }
    PyTuple::new(py, records)
}
