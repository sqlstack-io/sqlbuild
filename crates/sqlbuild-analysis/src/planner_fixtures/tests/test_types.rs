//! Fixture alignment regression inputs.

#[derive(Debug)]
pub(crate) struct PropagationTestCase {
    pub(crate) description: &'static str,
    pub(crate) upstream_type: Option<&'static str>,
    pub(crate) expected_type: Option<&'static str>,
}

#[derive(Debug)]
pub(crate) struct ExpectedTypesTestCase {
    pub(crate) description: &'static str,
    pub(crate) columns: &'static [(&'static str, &'static str, &'static str)],
    pub(crate) expected_types: &'static [(&'static str, &'static str)],
}

#[derive(Debug)]
pub(crate) struct SchemaTestCase {
    pub(crate) description: &'static str,
    pub(crate) authoritative: bool,
    pub(crate) supplied: &'static [&'static str],
    pub(crate) expected_analysis: bool,
    pub(crate) expected_diagnostic: Option<&'static str>,
}

#[derive(Debug)]
pub(crate) struct AlignmentTestCase {
    pub(crate) description: &'static str,
    pub(crate) supplied: &'static [&'static str],
    pub(crate) required: &'static [&'static str],
    pub(crate) star: bool,
    pub(crate) typed_nulls: bool,
    pub(crate) expected_completed: Option<Vec<usize>>,
    pub(crate) expected_diagnostics: &'static [&'static str],
}
#[derive(Debug)]
pub(super) struct FixtureReadCacheTestCase {
    pub description: &'static str,
    pub sql: &'static str,
    pub package: Option<&'static str>,
    pub argument_count: Option<i64>,
    pub dialect: Option<&'static str>,
    pub expected_misses: u64,
}

#[derive(Debug)]
pub(super) struct FixtureMarkerTestCase {
    pub description: &'static str,
    pub sql: &'static str,
    pub expected_sql: &'static str,
}

#[derive(Debug)]
pub(crate) struct FixtureLineTestCase {
    pub(crate) description: &'static str,
    pub(crate) contents: &'static str,
    pub(crate) cte_name: &'static str,
    pub(crate) expected_line: usize,
}

#[derive(Debug)]
pub(crate) struct CollectionMismatchTestCase {
    pub(crate) description: &'static str,
    pub(crate) expected_type: Option<&'static str>,
    pub(crate) inferred_type: Option<&'static str>,
    pub(crate) expected_mismatch: bool,
}

#[derive(Debug)]
pub(crate) struct RequiredColumnsTestCase {
    pub(crate) description: &'static str,
    pub(crate) reads: &'static [&'static str],
    pub(crate) star: bool,
    pub(crate) expected_required: &'static [&'static str],
    pub(crate) expected_completed: &'static [usize],
}

#[derive(Debug)]
pub(super) struct FixtureReadPrewarmTestCase {
    pub description: &'static str,
    pub sqls: &'static [&'static str],
    pub expected_misses: u64,
}
