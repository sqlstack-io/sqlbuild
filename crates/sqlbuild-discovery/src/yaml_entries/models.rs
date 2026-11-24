//! Parsed YAML declaration entries whose leaves are the caller's own authored values.

use sqlbuild_model_config::header_metadata::models::ParsedAudit;

/// One declared column of a seed or schema entry.
#[derive(Clone, Debug, PartialEq)]
pub struct ParsedYamlColumn<N> {
    pub name: N,
    pub column_type: Option<N>,
    pub nullable: Option<N>,
    pub description: Option<N>,
    /// The authored mapping, or `None` for Python's empty default.
    pub meta: Option<N>,
    pub audits: Vec<ParsedAudit<N>>,
}

/// `na_values`: one list for every column, or a list per named column.
#[derive(Clone, Debug, PartialEq)]
pub enum ParsedNaValues<N> {
    List(Vec<N>),
    PerColumn(Vec<(N, Vec<N>)>),
}

/// A seed's `csv_settings`; each field holds the authored value or `None` when unset.
#[derive(Clone, Debug, PartialEq)]
pub struct ParsedCsvSettings<N> {
    pub delimiter: Option<N>,
    pub quotechar: Option<N>,
    pub doublequote: Option<N>,
    pub escapechar: Option<N>,
    pub skipinitialspace: Option<N>,
    pub lineterminator: Option<N>,
    pub encoding: Option<N>,
    pub na_values: Option<ParsedNaValues<N>>,
    pub keep_default_na: Option<N>,
}

/// One seed declaration of a `sql/seeds/**/*.yml` file.
#[derive(Clone, Debug, PartialEq)]
pub struct ParsedSeedEntry<N> {
    pub name: N,
    pub description: Option<N>,
    pub database: Option<N>,
    pub schema: Option<N>,
    pub meta: Option<N>,
    pub tags: Vec<N>,
    /// The settings, or `None` when the entry declares none.
    pub csv_settings: Option<ParsedCsvSettings<N>>,
    pub columns: Vec<ParsedYamlColumn<N>>,
    pub audits: Vec<ParsedAudit<N>>,
}

/// A source freshness age policy; at least one bound is set.
#[derive(Clone, Debug, PartialEq)]
pub struct ParsedAgePolicy<N> {
    pub warn_after: Option<N>,
    pub error_after: Option<N>,
}

/// A source's freshness config, its strategy and value kind resolved as Python stores them.
#[derive(Clone, Debug, PartialEq)]
pub struct ParsedFreshness<N> {
    /// `adapter`, `column` or `sql`.
    pub strategy: &'static str,
    /// `timestamp`, `integer` or `string`.
    pub value_kind: Option<&'static str>,
    pub column: Option<N>,
    pub query: Option<N>,
    pub filter: Option<N>,
    pub lag_tolerance: Option<N>,
    pub age_policy: Option<ParsedAgePolicy<N>>,
}

/// One source declaration of a `sources/*.yml` file.
#[derive(Clone, Debug, PartialEq)]
pub struct ParsedSourceEntry<N> {
    pub name: N,
    pub database: Option<N>,
    pub schema: Option<N>,
    pub table: Option<N>,
    /// Whether reads of the source add `FINAL`, so replacing engines return one row per key.
    pub read_final: bool,
    pub loader: Option<String>,
    pub managed: bool,
    pub freshness: Option<ParsedFreshness<N>>,
    /// `append`, `delete_insert`, `merge` or `table`.
    pub write_strategy: Option<&'static str>,
    pub load_batch_size: Option<N>,
    pub cursor_column: Option<N>,
    pub unique_key: Vec<String>,
    pub expression: Option<N>,
    pub description: Option<N>,
    /// The authored flag, or `True` implied by typed columns.
    pub type_enforcement: Option<bool>,
    pub contract: Option<N>,
    pub meta: Option<N>,
    pub columns: Vec<ParsedYamlColumn<N>>,
    pub audits: Vec<ParsedAudit<N>>,
}

impl<N> ParsedSourceEntry<N> {
    /// A source entry with only its name set, as Python's `SourceEntry` defaults.
    pub fn named(name: N) -> Self {
        Self {
            name,
            database: None,
            schema: None,
            table: None,
            read_final: false,
            loader: None,
            managed: false,
            freshness: None,
            write_strategy: None,
            load_batch_size: None,
            cursor_column: None,
            unique_key: Vec::new(),
            expression: None,
            description: None,
            type_enforcement: None,
            contract: None,
            meta: None,
            columns: Vec::new(),
            audits: Vec::new(),
        }
    }
}
