//! Recall record inputs and the rows they become.

use crate::schema::models::Table;

/// One stored value.
#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize, utoipa::ToSchema)]
#[serde(tag = "kind", content = "value", rename_all = "snake_case")]
pub enum RecallValue {
    Null,
    Integer(i64),
    Real(f64),
    Text(String),
    Blob(Vec<u8>),
}

impl RecallValue {
    /// Human-readable conflict-key text, before dialect-specific timestamp normalization.
    #[must_use]
    pub fn key_text(&self) -> String {
        match self {
            Self::Text(text) => text.clone(),
            Self::Integer(number) => number.to_string(),
            Self::Real(number) => number.to_string(),
            Self::Blob(_) => "<blob>".to_owned(),
            Self::Null => String::new(),
        }
    }
}

/// One row bound for a Recall table: named values, without `ingested_at`, which the store sets.
#[derive(Clone, Debug, PartialEq)]
pub struct RecallRow {
    pub table: Table,
    pub values: Vec<(&'static str, RecallValue)>,
}

impl RecallRow {
    /// Validate schema-typed values and rebuild the receiver-owned envelope and content digest.
    pub fn from_values(
        envelope: &RowEnvelope,
        table: Table,
        values: &std::collections::BTreeMap<String, RecallValue>,
    ) -> Result<Self, crate::records::errors::RecordError> {
        crate::records::_helpers::record_row::record_row(envelope, table, values)
    }

    /// The value of `column`, if the row has it.
    #[must_use]
    pub fn value(&self, column: &str) -> Option<&RecallValue> {
        self.values
            .iter()
            .find(|(name, _)| *name == column)
            .map(|(_, value)| value)
    }

    /// The row's content digest.
    #[must_use]
    pub fn content_hash(&self) -> Option<&str> {
        match self.value("content_hash") {
            Some(RecallValue::Text(text)) => Some(text),
            _ => None,
        }
    }
}

/// What every row of one invocation carries.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RowEnvelope {
    pub invocation_id: String,
    pub invocation_started_at: String,
    pub producer_version: String,
}

/// A process output stream.
#[derive(
    Clone, Copy, Debug, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize, utoipa::ToSchema,
)]
#[serde(rename_all = "snake_case")]
pub enum OutputStream {
    Stdout,
    Stderr,
}

impl OutputStream {
    #[must_use]
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Stdout => "stdout",
            Self::Stderr => "stderr",
        }
    }

    /// The stream named `name`.
    #[must_use]
    pub fn from_name(name: &str) -> Option<Self> {
        match name {
            "stdout" => Some(Self::Stdout),
            "stderr" => Some(Self::Stderr),
            _ => None,
        }
    }
}

/// One exact-byte output chunk.
#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize, utoipa::ToSchema)]
#[serde(deny_unknown_fields)]
pub struct OutputChunk {
    /// Per invocation, from 0, shared by both streams.
    pub sequence: u64,
    pub stream: OutputStream,
    pub occurred_at: String,
    pub data: Vec<u8>,
    /// Bytes of the same stream before this chunk.
    pub byte_offset: u64,
    /// Output records the queue dropped just before this one.
    pub dropped_before: u64,
    /// On the empty chunk standing for output past the stream's limit: the bytes and sequences it replaces.
    pub omitted: Option<OmittedOutput>,
}

/// Output of one stream that was not recorded because it exceeded `max_output_bytes`.
#[derive(
    Clone,
    Copy,
    Debug,
    Default,
    PartialEq,
    Eq,
    serde::Serialize,
    serde::Deserialize,
    utoipa::ToSchema,
)]
#[serde(deny_unknown_fields)]
pub struct OmittedOutput {
    pub bytes: u64,
    /// Sequences this gap accounts for, its own included.
    pub sequences: u64,
}

/// The context of one invocation, written once when it starts.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct InvocationContext {
    pub command: Option<String>,
    pub subcommand: Option<String>,
    /// The effective selectors: positional, `--select` and the contents of every selector file, at most [`MAX_RECORDED_SELECTION_BYTES`](crate::records::constants::MAX_RECORDED_SELECTION_BYTES).
    pub selection: Vec<String>,
    /// How many effective selectors there were, recorded ones and truncated ones alike.
    pub selection_count: Option<u64>,
    /// Selectors were left out of `selection` to keep it within its bound.
    pub selection_truncated: bool,
    /// The selector files as passed, in order.
    pub selector_files: Vec<String>,
    pub exclusion: Vec<String>,
    pub project_name: Option<String>,
    pub project_dir_hash: Option<String>,
    pub target: Option<String>,
    pub adapter: Option<String>,
    pub destination_database: Option<String>,
    pub destination_schema: Option<String>,
    pub destination_fingerprint: Option<String>,
    pub sqlbuild_version: Option<String>,
    pub git_revision: Option<String>,
    pub git_dirty: Option<bool>,
    pub host_token: Option<String>,
    pub pid: Option<u32>,
    /// JSON object text, at most 16 KiB.
    pub external_context: Option<String>,
    pub recall_filter_active: bool,
    pub argv_digest: Option<String>,
    /// `integration`, `ci` or `local`: where the invocation came from.
    pub origin: Option<String>,
    /// What a diagnostic command checked, from its flags; never its raw arguments.
    pub command_mode: Option<String>,
}

/// The correlation identifiers of a fact.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Correlations {
    pub run_id: Option<String>,
    pub resource_id: Option<String>,
    pub resource_attempt_id: Option<String>,
    pub operation_id: Option<String>,
    pub statement_id: Option<String>,
}

/// One structured diagnostic.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DiagnosticInput {
    pub sequence: u64,
    pub occurred_at: String,
    pub severity: String,
    pub logger: String,
    pub source: String,
    pub message: String,
    /// JSON object text.
    pub fields: String,
    pub log_stream_id: Option<String>,
    pub correlations: Correlations,
}

/// One captured artifact and the reference that names it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ArtifactInput {
    /// `sha256:<hex>` of `content`.
    pub digest: String,
    pub kind: String,
    pub media_type: String,
    pub content: Vec<u8>,
    pub seen_at: String,
    pub ref_id: String,
    pub role: String,
    pub statement_id: Option<String>,
    pub resource_id: Option<String>,
}

/// Delivery accounting of one stream.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct DeliveryCounts {
    pub accepted: u64,
    pub delivered: u64,
    pub filtered: u64,
    pub dropped: u64,
    pub failed: u64,
    pub conflicts: u64,
}
