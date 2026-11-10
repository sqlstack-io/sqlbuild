//! Plain-data request and outcomes for fast column lineage.

/// The SQLBuild resource kinds a `__ref`, `__source` or `__seed` call names.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum LineageResourceType {
    Model,
    Source,
    Seed,
}

/// A semantic column use projected onto a project resource and authored SQL.
#[derive(Debug)]
pub struct DirectSemanticUse {
    pub model: String,
    pub context: String,
    pub expression: String,
    pub resource_type: LineageResourceType,
    pub resource_name: String,
    pub column: String,
    pub confidence: String,
    pub line: Option<u64>,
    pub offset: Option<u64>,
}

impl LineageResourceType {
    /// Python's `CompiledResourceType` value.
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Model => "model",
            Self::Source => "source",
            Self::Seed => "seed",
        }
    }

    /// Parse Python's `CompiledResourceType` value for the kinds lineage reads.
    pub fn from_value(value: &str) -> Option<Self> {
        match value {
            "model" => Some(Self::Model),
            "source" => Some(Self::Source),
            "seed" => Some(Self::Seed),
            _ => None,
        }
    }
}

/// Python's `ColumnTransformKind` values that fast lineage produces.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LineageTransformKind {
    Direct,
    Cast,
    Expression,
    Aggregation,
    Star,
    Constant,
}

impl LineageTransformKind {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Direct => "direct",
            Self::Cast => "cast",
            Self::Expression => "expression",
            Self::Aggregation => "aggregation",
            Self::Star => "star",
            Self::Constant => "constant",
        }
    }
}

/// Python's `ColumnLineageConfidence` values that fast lineage produces.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LineageConfidence {
    High,
    Medium,
    Unknown,
}

impl LineageConfidence {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::High => "high",
            Self::Medium => "medium",
            Self::Unknown => "unknown",
        }
    }
}

/// One resource's known columns, in the order Python's schema mapping lists them.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LineageSchemaResource {
    pub resource_type: LineageResourceType,
    pub name: String,
    /// Inferred then declared column names; repeats keep their first position.
    pub columns: Vec<String>,
}

/// The work one model needs: star expansion over compact facts, or a parse of its SQL.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FastLineageModel {
    /// A model with compact facts and an unresolved root star.
    StarExpansion {
        query_sql: String,
        existing_columns: Vec<String>,
    },
    /// A model without compact facts.
    Parse {
        query_sql: String,
        inferred_columns: Vec<String>,
        /// `@@@name` defaults a native parser reads in place of each placeholder.
        placeholders: Vec<(String, String)>,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FastLineageRequest {
    /// The adapter's analysis dialect; `None` parses as generic SQL.
    pub dialect: Option<String>,
    pub schema: Vec<LineageSchemaResource>,
    pub models: Vec<FastLineageModel>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LineageSource {
    pub resource_type: LineageResourceType,
    pub resource_name: String,
    pub column_name: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LineageColumn {
    pub output_column: String,
    pub transform_kind: LineageTransformKind,
    pub confidence: LineageConfidence,
    pub upstream_columns: Vec<LineageSource>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FastLineageOutcome {
    /// Star columns to append to the model's compact facts.
    StarColumns(Vec<LineageColumn>),
    /// Lineage built from the parsed SQL.
    Built {
        columns: Vec<LineageColumn>,
        has_star: bool,
    },
    /// Python records no lineage for this model.
    Omitted,
    /// The SQL did not parse; Python logs this message and records no lineage.
    Unparsed(String),
    /// Polyglot does not know the dialect name, so Python's parse raises `ValueError`.
    UnknownDialect(String),
}

/// A lineage outcome flattened into its status, columns, star flag and detail.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LineageOutcomeParts<C> {
    pub status: &'static str,
    pub columns: Vec<C>,
    pub has_star: bool,
    pub detail: Option<String>,
}

impl<C> LineageOutcomeParts<C> {
    fn new(status: &'static str, columns: Vec<C>, has_star: bool, detail: Option<String>) -> Self {
        Self {
            status,
            columns,
            has_star,
            detail,
        }
    }
}

impl FastLineageOutcome {
    /// The status, columns and star flag; detail is the parse error or the unknown dialect.
    pub fn into_parts(self) -> LineageOutcomeParts<LineageColumn> {
        match self {
            Self::StarColumns(columns) => LineageOutcomeParts::new("star", columns, true, None),
            Self::Built { columns, has_star } => {
                LineageOutcomeParts::new("built", columns, has_star, None)
            }
            Self::Omitted => LineageOutcomeParts::new("omitted", Vec::new(), false, None),
            Self::Unparsed(message) => {
                LineageOutcomeParts::new("unparsed", Vec::new(), false, Some(message))
            }
            Self::UnknownDialect(name) => {
                LineageOutcomeParts::new("unknown_dialect", Vec::new(), false, Some(name))
            }
        }
    }
}

/// Python's `InferredNullability` values that rich lineage produces.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LineageNullability {
    NonNull,
    Nullable,
    Unknown,
}

impl LineageNullability {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::NonNull => "non_null",
            Self::Nullable => "nullable",
            Self::Unknown => "unknown",
        }
    }
}

/// One resource's typed columns, as Python's schema mapping builds them.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RichSchemaResource {
    pub resource_type: LineageResourceType,
    pub name: String,
    /// `columns[name] = type or "UNKNOWN"`: a repeat keeps its position and takes the later type.
    pub assigned: Vec<(String, Option<String>)>,
    /// `columns.setdefault(name, type or "UNKNOWN")`, applied after `assigned`.
    pub defaulted: Vec<(String, Option<String>)>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RichLineageRequest {
    /// The `PolyglotAnalysisDialect` value Python resolved from the adapter's dialect.
    pub dialect: String,
    pub schema: Vec<RichSchemaResource>,
    /// Each requested model's compiled query SQL, in project order.
    pub models: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RichLineageColumn {
    pub column: LineageColumn,
    pub nullability: LineageNullability,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RichLineageOutcome {
    Built {
        columns: Vec<RichLineageColumn>,
        has_star: bool,
    },
    /// Polyglot rejected the query; Python logs this message and records no lineage.
    Skipped(String),
}

impl RichLineageOutcome {
    /// The status, columns and star flag; detail is the polyglot error.
    pub fn into_parts(self) -> LineageOutcomeParts<RichLineageColumn> {
        match self {
            Self::Built { columns, has_star } => {
                LineageOutcomeParts::new("built", columns, has_star, None)
            }
            Self::Skipped(message) => {
                LineageOutcomeParts::new("skipped", Vec::new(), false, Some(message))
            }
        }
    }
}

/// What a directory whose `readdir` fails mid-listing does to the fingerprint, by Python version.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InterruptedListingPolicy {
    /// Python 3.12: `Path.walk` lets the `OSError` escape `rglob`, so the result is `None`.
    Uncacheable,
    /// Python 3.13 and 3.14: `glob` skips the directory's entries.
    Skip,
}

/// The relation lineage cache key Python's `relation_lineage_fingerprint` computes.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RelationFingerprint {
    /// The hex SHA-256 digest of the authored inputs.
    Digest(String),
    /// Python returns `None`: dynamic context, malformed environment markers or unreadable input.
    Uncacheable,
    /// Discovery's walk failed internally: a native compiler failure, not a Python `None`.
    Failed(String),
}

impl RelationFingerprint {
    /// The digest, `None` where Python returns `None`, or the internal failure.
    pub fn into_digest(self) -> Result<Option<String>, String> {
        match self {
            Self::Digest(digest) => Ok(Some(digest)),
            Self::Uncacheable => Ok(None),
            Self::Failed(message) => Err(message),
        }
    }
}
