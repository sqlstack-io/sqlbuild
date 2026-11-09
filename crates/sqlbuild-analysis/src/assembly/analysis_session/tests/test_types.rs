use crate::assembly::analysis_session::models::{ContractProof, PivotOutcome};

/// A model as `(name, sql, source names, model refs)`.
pub(crate) type ModelSpec = (
    &'static str,
    &'static str,
    &'static [&'static str],
    &'static [&'static str],
);

pub(crate) struct SessionTestCase {
    pub(crate) description: &'static str,
    pub(crate) models: &'static [ModelSpec],
    /// `step_lines` of each step, in order.
    pub(crate) expected_steps: &'static [&'static [&'static str]],
    /// `described` of each model's outcome, in request order.
    pub(crate) expected_outcomes: &'static [&'static [&'static str]],
}

/// A session over the orders request, run with or without rich type inference.
pub(crate) struct StarExpansionTestCase {
    pub(crate) description: &'static str,
    pub(crate) rich_type_inference: bool,
    pub(crate) models: &'static [ModelSpec],
    /// `step_lines` of each step, in order.
    pub(crate) expected_steps: &'static [&'static [&'static str]],
}

pub(crate) struct SessionFactsTestCase {
    pub(crate) description: &'static str,
    pub(crate) models: &'static [ModelSpec],
    /// `fact_lines` of each model, in request order.
    pub(crate) expected_facts: &'static [&'static str],
}

pub(crate) struct UnscheduledTestCase {
    pub(crate) description: &'static str,
    pub(crate) models: &'static [ModelSpec],
    pub(crate) expected_error: Option<&'static str>,
}

pub(crate) struct SessionFailureTestCase {
    pub(crate) description: &'static str,
    pub(crate) dialect: &'static str,
    pub(crate) models: &'static [ModelSpec],
    pub(crate) expected_error: Option<&'static str>,
}

pub(crate) struct ExpressionShapeTestCase {
    pub(crate) description: &'static str,
    pub(crate) dialect: &'static str,
    pub(crate) case_sensitive_shapes: bool,
    pub(crate) expression: &'static str,
    pub(crate) expected_shape: &'static str,
}

pub(crate) struct DictTestCase {
    pub(crate) description: &'static str,
    pub(crate) pairs: &'static [(&'static str, &'static str)],
    pub(crate) expected_dict: &'static [(&'static str, &'static str)],
}

pub(crate) struct WavesTestCase {
    pub(crate) description: &'static str,
    pub(crate) producers: &'static [&'static [usize]],
    pub(crate) expected_waves: Option<&'static [&'static [usize]]>,
}

pub(crate) struct PivotTestCase {
    pub(crate) description: &'static str,
    pub(crate) dialect: &'static str,
    pub(crate) sql: &'static str,
    /// `(pivot column, value column, aggregate)` of one `amounts` family, or none declared.
    pub(crate) family: Option<(&'static str, &'static str, &'static str)>,
    pub(crate) expected_outcome: PivotOutcome,
}

/// Types, nullabilities, direct CTE outputs and filtered non-null outputs, in Python's order.
pub(crate) type RecoveredFacts = (
    &'static [(&'static str, &'static str)],
    &'static [(&'static str, &'static str)],
    &'static [&'static str],
    &'static [&'static str],
);

pub(crate) struct CteRecoveryTestCase {
    pub(crate) description: &'static str,
    pub(crate) sql: &'static str,
    /// None where the recovery defers to Python.
    pub(crate) expected_facts: Option<RecoveredFacts>,
}

pub(crate) struct LegacyAnalysisTestCase {
    pub(crate) description: &'static str,
    pub(crate) sql: &'static str,
    /// The outcome, columns and lineage rows; None where the analysis defers to Python.
    pub(crate) expected_lines: Option<&'static [&'static str]>,
}

pub(crate) struct CacheReuseTestCase {
    pub(crate) description: &'static str,
    pub(crate) models: &'static [ModelSpec],
    /// `(hits, misses, stored)` of the cold run, then of the warm run.
    pub(crate) expected_stats: [(usize, usize, usize); 2],
}

pub(crate) struct CacheEditTestCase {
    pub(crate) description: &'static str,
    pub(crate) models: &'static [ModelSpec],
    pub(crate) edited: &'static [ModelSpec],
    /// Which models the run after the edit takes from the cache, in request order.
    pub(crate) expected_hits: &'static [bool],
}

pub(crate) struct CacheKeyTestCase {
    pub(crate) description: &'static str,
    /// Changes one input of the request the key must cover.
    pub(crate) change: fn(&mut crate::assembly::analysis_session::models::SessionRequest),
    pub(crate) expected_key_changed: bool,
}

pub(crate) struct RelationDigestTestCase {
    pub(crate) description: &'static str,
    /// Records one relation fact in the started session.
    pub(crate) change: fn(&mut crate::assembly::analysis_session::models::AnalysisSession),
    /// The relation whose digest, or the model whose key, is compared.
    pub(crate) subject: &'static str,
    pub(crate) expected_changed: bool,
}

pub(crate) struct DamagedCacheTestCase {
    pub(crate) description: &'static str,
    pub(crate) models: &'static [ModelSpec],
    /// `(hits, misses, stored)` of the run over the damaged store.
    pub(crate) expected_stats: (usize, usize, usize),
}

/// One run of a session with an analysis cache attached.
pub(crate) struct CachedRun {
    /// `step_lines` of each step, then `described` of each model's outcome.
    pub(crate) lines: (Vec<Vec<String>>, Vec<Vec<String>>),
    pub(crate) store: sqlbuild_cache::store::models::NativeStore,
    pub(crate) stats: crate::assembly::analysis_session::models::AnalysisCacheStats,
    pub(crate) keys: Vec<Option<sqlbuild_cache::digest::types::ContentDigest>>,
    /// Whether each model's outcome came from the cache.
    pub(crate) hits: Vec<bool>,
    /// The catalog's schema additions and analysis names, then the sorted fact models.
    pub(crate) catalog: (
        crate::assembly::analysis_session::types::Shapes,
        Vec<String>,
        Vec<String>,
    ),
}

pub(crate) struct KeptAnalysisTestCase {
    pub(crate) description: &'static str,
    pub(crate) dialect: &'static str,
    pub(crate) sql: &'static str,
    pub(crate) family: Option<(&'static str, &'static str, &'static str)>,
    pub(crate) expected_outcome: PivotOutcome,
    /// `name type nullability` of each model's published columns: the pivot, then a plain one.
    pub(crate) expected_columns: [&'static [&'static str]; 2],
}

/// Final `(name, type)` columns and the forced star.
pub(crate) type ColumnView<'a> = (Option<Vec<(&'a str, Option<&'a str>)>>, bool);

/// The expected column view, or the internal failure.
pub(crate) type ExpectedModelColumns = Result<ColumnView<'static>, &'static str>;

pub(crate) struct ModelColumnsTestCase {
    pub(crate) description: &'static str,
    pub(crate) analysis_enabled: bool,
    pub(crate) succeeded: bool,
    pub(crate) session_columns: Option<&'static [(&'static str, &'static str)]>,
    pub(crate) session_proof: Option<(bool, &'static [(&'static str, &'static str)])>,
    pub(crate) fallback: Option<Option<&'static [(&'static str, &'static str)]>>,
    pub(crate) standalone_proof: Option<(bool, &'static [(&'static str, &'static str)])>,
    pub(crate) poisoned: &'static [&'static str],
    pub(crate) expected_columns: ExpectedModelColumns,
}

pub(crate) struct ReferenceFactsTestCase {
    pub(crate) description: &'static str,
    pub(crate) rows: &'static [(&'static str, &'static str)],
    pub(crate) expected_lineage: &'static [(&'static str, &'static str, &'static str)],
    pub(crate) expected_required: &'static [&'static str],
}

pub(crate) struct NamesSetOperationTestCase {
    pub(crate) description: &'static str,
    pub(crate) sql: &'static str,
    pub(crate) expected_set_operation: bool,
}

pub(crate) struct KeywordFoldingTestCase {
    pub(crate) description: &'static str,
    pub(crate) keyword: &'static str,
    pub(crate) expected_non_ascii_folds: Vec<char>,
}

pub(crate) struct NativeDialectTestCase {
    pub(crate) description: &'static str,
    pub(crate) dialect: Option<&'static str>,
    pub(crate) expected_dialect: Option<&'static str>,
}

pub(crate) struct ContractProofJsonTestCase {
    pub(crate) description: &'static str,
    pub(crate) proof: ContractProof,
    pub(crate) expected_json: &'static str,
}

/// One model of a session analysed alone against the whole session's relation facts.
pub(crate) struct DeltaSessionTestCase {
    pub(crate) description: &'static str,
    pub(crate) model_index: usize,
    pub(crate) expected_whole_session_outcome: bool,
}

/// A model query holding typed markers, and the described line of its column `v`.
pub(crate) struct TypedMarkerTestCase {
    pub(crate) description: &'static str,
    pub(crate) sql: &'static str,
    pub(crate) expected_column: &'static str,
}

pub(crate) struct TypedMarkerCacheTestCase {
    pub(crate) description: &'static str,
    pub(crate) sql: &'static str,
    pub(crate) expected_hits: &'static [bool],
}

/// A model calling a function by its bare name, with the project functions the compile declares.
pub(crate) struct BareFunctionTestCase {
    pub(crate) description: &'static str,
    pub(crate) known_functions: &'static [&'static str],
    pub(crate) sql: &'static str,
    pub(crate) expected_last_line: &'static str,
}
