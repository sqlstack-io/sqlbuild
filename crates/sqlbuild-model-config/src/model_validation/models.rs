//! What the model validators read besides the effective config values.

use std::collections::HashSet;

use crate::errors::ConfigError;

/// Project-wide facts every model's validation reads.
#[derive(Clone, Debug)]
pub struct ProjectValidationFacts {
    /// Discovered custom materialization names.
    pub custom_materializations: HashSet<String>,
    /// Whether the project enables concurrent microbatches.
    pub microbatch_concurrency: bool,
    /// Discovered model names.
    pub models: HashSet<String>,
    /// Discovered seed names.
    pub seeds: HashSet<String>,
    /// Discovered source names.
    pub sources: HashSet<String>,
    /// Discovered SQL function names, scalar and table.
    pub functions: HashSet<String>,
    /// Discovered table function names.
    pub table_functions: HashSet<String>,
    /// The effective adapter name, when the project names one.
    pub adapter: Option<String>,
}

/// One extracted SQL reference: its `SqlReferenceKind` value and name.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ModelReference {
    pub kind: String,
    pub name: String,
    /// For a dbt reference, whether the external resolver rejects it.
    pub externally_rejected: bool,
}

/// One model's facts beyond its config values.
#[derive(Clone, Debug)]
pub struct ModelValidationFacts<'a> {
    pub model_name: &'a str,
    /// The model file's project-relative path, as reference errors name it.
    pub relative_path: &'a str,
    /// Merged references in extraction order.
    pub references: &'a [ModelReference],
    /// Column names of the model's declared schema, when one applies.
    pub declared_columns: Option<&'a [String]>,
    /// The expanded query SQL.
    pub query_sql: &'a str,
    /// Whether the resolved time-travel retention is unmanaged.
    pub retention_unmanaged: bool,
    /// Whether the resolved table type was declared by any layer.
    pub table_type_declared: bool,
}

/// Why validation stops at the first problem.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ValidationStop {
    /// The first config error.
    Error(ConfigError),
    /// The reference at this index names a dbt model the external resolver rejected.
    External(usize),
}

impl ValidationStop {
    /// The config error, or the index of the externally rejected reference.
    pub fn into_error(self) -> Result<ConfigError, usize> {
        match self {
            Self::Error(error) => Ok(error),
            Self::External(index) => Err(index),
        }
    }
}

/// The header's `time_travel_retention`, applied over the inherited policy.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RetentionOverride {
    /// Absent or `inherit`: keep the inherited policy.
    Inherit,
    /// `disabled`: retention is unmanaged.
    Unmanaged,
    /// A whole-day duration, or `0d`.
    Days(u64),
}

/// The header's `table_type`, applied over the inherited type.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TableTypeOverride {
    /// Absent or `inherit`: keep the inherited type.
    Inherit,
    Permanent,
    Transient,
}

/// One header column's name and the old column it renames, if any.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct ColumnMigration {
    pub name: String,
    pub migrate_from: Option<String>,
}
