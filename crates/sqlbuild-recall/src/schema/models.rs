//! The physical shape of Recall tables and the options their DDL is rendered with.

/// The SQL dialect a Recall store speaks.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Dialect {
    Sqlite,
    ClickHouse,
}

/// The logical type of one Recall column.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ColumnType {
    Text,
    Integer,
    Real,
    Blob,
    /// ISO-8601 UTC text with microseconds in SQLite, `DateTime64(6, 'UTC')` in ClickHouse.
    Timestamp,
    /// JSON text.
    Json,
    /// 0 or 1.
    Boolean,
}

impl ColumnType {
    /// The PostgreSQL type used by both DDL and typed query placeholders.
    #[must_use]
    pub const fn postgres_type(self) -> &'static str {
        match self {
            Self::Text => "TEXT",
            Self::Timestamp => "TIMESTAMPTZ",
            Self::Json => "JSONB",
            Self::Integer => "BIGINT",
            Self::Boolean => "BOOLEAN",
            Self::Real => "DOUBLE PRECISION",
            Self::Blob => "BYTEA",
        }
    }
}

/// One column: its name, type and whether it may be null.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ColumnSpec {
    pub name: &'static str,
    pub column_type: ColumnType,
    pub nullable: bool,
}

impl ColumnSpec {
    /// A column that is never null.
    #[must_use]
    pub const fn required(name: &'static str, column_type: ColumnType) -> Self {
        Self {
            name,
            column_type,
            nullable: false,
        }
    }

    /// A column that may be null.
    #[must_use]
    pub const fn optional(name: &'static str, column_type: ColumnType) -> Self {
        Self {
            name,
            column_type,
            nullable: true,
        }
    }
}

/// One Recall table.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct TableSpec {
    pub name: &'static str,
    /// Columns specific to this table; every table also has the common columns.
    pub columns: &'static [ColumnSpec],
    /// The deterministic identity of a row; a second row with this key is a duplicate.
    pub key: &'static [&'static str],
    /// SQLite secondary indexes: name suffix and columns.
    pub sqlite_indexes: &'static [(&'static str, &'static [&'static str])],
    /// ClickHouse engine without arguments: `ReplacingMergeTree` or `MergeTree`.
    pub clickhouse_engine: &'static str,
    /// ClickHouse `ORDER BY` columns.
    pub clickhouse_order_by: &'static [&'static str],
    /// Days ClickHouse keeps rows beyond retention; artifacts outlive their references.
    pub ttl_extra_days: u32,
    /// The `ReplacingMergeTree` version column: the newest copy survives merges.
    pub clickhouse_version: Option<&'static str>,
    /// The timestamp column partitions and TTL derive from.
    pub expiry_column: &'static str,
}

/// The tables Recall writes.
#[derive(
    Clone,
    Copy,
    Debug,
    PartialEq,
    Eq,
    Hash,
    PartialOrd,
    Ord,
    serde::Serialize,
    serde::Deserialize,
    utoipa::ToSchema,
)]
#[serde(rename_all = "snake_case")]
pub enum Table {
    Invocations,
    Output,
    Events,
    Diagnostics,
    Artifacts,
    ArtifactRefs,
    Heartbeats,
    Delivery,
    Meta,
    Conflicts,
}

/// What DDL is rendered with: database (ClickHouse), retention and cluster.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DdlOptions {
    pub database: Option<String>,
    pub retention_days: u32,
    pub cluster: Option<String>,
}

/// The statements SQLite maintenance runs, with their positional parameters.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SqliteRetentionSql {
    /// `?1` cutoff, `?2` now, `?3` limit: expired invocation IDs that are not running.
    pub expired_invocations: String,
    /// Per table, `?1` a JSON array of invocation IDs to delete.
    pub delete_invocations: Vec<(Table, String)>,
    /// `?1` cutoff: artifacts no remaining reference names.
    pub delete_orphan_artifacts: String,
    /// `?1` cutoff: maintenance claims older than retention.
    pub delete_old_claims: String,
}

/// What a Recall store holds: its tables with their column names, its views, and the recorded schema version.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct ObservedSchema {
    pub tables: std::collections::BTreeMap<Table, std::collections::BTreeSet<String>>,
    pub views: std::collections::BTreeSet<String>,
    /// The highest recorded schema version; `None` when none is recorded.
    pub version: Option<i64>,
}

/// How a store's tables and views compare with this build's, from their columns.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SchemaAge {
    /// A relation is missing or lacks a column of this build, and none has a column this build does not know.
    Older,
    /// Every table and view has exactly this build's columns.
    Current,
    /// A relation has a column this build does not know: a newer build created it.
    Newer,
    /// No view is visible, as to a user granted only the tables, so the store's age is unknown.
    Unseen,
}

/// What a writer knows about the store's schema version.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum StoreVersion {
    /// The highest recorded version, the age in seconds of the newest version row, and whether any view is visible to this user.
    Recorded {
        version: i64,
        age_seconds: i64,
        views_visible: bool,
    },
    /// No version row is left, as after retention expired it; the columns tell the store's age.
    Unrecorded(SchemaAge),
}

/// What a writer does to the views and the version row.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct VersionPlan {
    /// Replace every view with this build's definition.
    pub replace_views: bool,
    /// Run view statements at all: replacements, or creating missing views. A user who sees no view issues none, since they would fail on every start.
    pub view_ddl: bool,
    /// The version to append once the views are settled, if any.
    pub record: Option<i64>,
}
