//! Compile-owned native catalog, mapping state and boundary requests.

use crate::semantic_validation::types::{PreparedCompactAnalysis, ProbeKey, Relations};
use polyglot_sql::validation::SchemaTable;
use polyglot_sql::{DialectType, SchemaValidationOptions};
use rayon::ThreadPool;
use std::collections::HashMap;
use std::sync::atomic::AtomicUsize;
use std::sync::{Arc, Mutex};

/// One schema-validation diagnostic with its cleaned-SQL position.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DiagnosticRow {
    pub code: String,
    pub message: String,
    pub line: Option<usize>,
    pub column: Option<usize>,
    pub start: Option<usize>,
    pub end: Option<usize>,
    pub severity: String,
}

/// One macro-expansion edit: the authored char span and the span it expanded to.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Expansion {
    pub source_start: usize,
    pub source_end: usize,
    pub output_start: usize,
    pub output_end: usize,
}

#[derive(Default, Debug)]
pub struct Columns(pub Vec<(String, Option<String>)>);

#[derive(Debug)]
pub struct CatalogInput {
    pub dialect: String,
    pub quoted_ignore_case: bool,
    pub known_functions: Vec<String>,
    pub known_types: Vec<String>,
    pub relations: Relations,
}

#[derive(Debug)]
pub struct NormalizationInput {
    pub sql: String,
    pub dialect: String,
    pub stubs: HashMap<String, String>,
    pub placeholders: HashMap<String, String>,
}

/// Normalized analysis SQL with, per normalized character, the input character it came from.
#[derive(Debug)]
pub struct MappedNormalization {
    pub sql: String,
    pub offsets: Vec<usize>,
}

#[derive(Debug)]
pub struct PositionInput {
    pub authored: String,
    pub query: String,
    pub expanded: String,
    pub cleaned: String,
    pub passes: Vec<Vec<Expansion>>,
}

/// Memoized call-site function-name probes, owned by the catalog that validates a compile.
/// How polyglot's unknown-function findings are reported: a model's are partial-semantics notes
/// (D42); resource SQL (SQL tests, hooks, audits) keeps them as B101 errors.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum UnknownFunctions {
    Notes,
    Errors,
}

#[derive(Debug, Default)]
pub(crate) struct FunctionProbes {
    pub(crate) suggestions: Mutex<HashMap<ProbeKey, Option<String>>>,
}

pub struct ProjectCatalog {
    pub(crate) dialect: DialectType,
    pub(crate) options: SchemaValidationOptions,
    pub(crate) quoted_ignore_case: bool,
    pub(super) tables: HashMap<String, SchemaTable>,
    pub(super) overrides: Vec<HashMap<String, SchemaTable>>,
    pub(super) analysis_tables: HashMap<String, SchemaTable>,
    pub(crate) function_probes: Arc<FunctionProbes>,
    /// One analysis pool per compile catalog, built on first use and shared by its views.
    pub(super) analysis_pool: Arc<Mutex<Option<Arc<ThreadPool>>>>,
    /// Analysis batches running on the pool; preparation only borrows the pool while none run.
    pub(super) running_analyses: Arc<AtomicUsize>,
}

/// One resolved compact analysis batch that runs without borrowing its project catalog.
pub struct CompactAnalysisJob {
    pub(super) analysis: Mutex<Option<PreparedCompactAnalysis>>,
    pub(super) catalog: ProjectCatalog,
}

#[derive(Debug)]
pub struct BindingPositions {
    pub(super) authored: String,
    pub(super) query_start: Option<usize>,
    pub(super) lines: Vec<usize>,
    pub(super) cleaned_lines: Vec<usize>,
    pub(super) offsets: Vec<usize>,
    pub(super) passes: Vec<Vec<Expansion>>,
    pub(super) words: std::sync::OnceLock<HashMap<String, (usize, bool)>>,
}

impl std::fmt::Debug for ProjectCatalog {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("ProjectCatalog")
            .field("dialect", &self.dialect)
            .field("quoted_ignore_case", &self.quoted_ignore_case)
            .field("relation_count", &self.tables.len())
            .finish_non_exhaustive()
    }
}
