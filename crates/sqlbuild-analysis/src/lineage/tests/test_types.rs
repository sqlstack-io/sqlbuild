use crate::lineage::models::InterruptedListingPolicy;
use sqlbuild_discovery::models::StageFailure;

pub(crate) struct SchemaFilterTestCase {
    pub(crate) description: &'static str,
    pub(crate) referenced: usize,
    pub(crate) expected_tables: usize,
}

pub(crate) struct SemanticUseTestCase {
    pub(crate) description: &'static str,
    pub(crate) query: &'static str,
    pub(crate) expected_count: usize,
    pub(crate) expected_line: u64,
    pub(crate) expected_offset: u64,
}

pub(crate) struct ReferenceTestCase {
    pub(crate) description: &'static str,
    pub(crate) sql: &'static str,
    pub(crate) expected_resources: &'static [(&'static str, &'static str, &'static str)],
    pub(crate) expected_normalized: &'static str,
}

pub(crate) struct ParsedLineageTestCase {
    pub(crate) description: &'static str,
    pub(crate) dialect: Option<&'static str>,
    pub(crate) sql: &'static str,
    pub(crate) inferred_columns: &'static [&'static str],
    pub(crate) expected_status: &'static str,
    /// One `output transform confidence [type:resource.column ...]` line per column.
    pub(crate) expected_lines: &'static [&'static str],
    pub(crate) expected_has_star: bool,
    pub(crate) expected_detail: Option<&'static str>,
}

pub(crate) struct StarExpansionTestCase {
    pub(crate) description: &'static str,
    pub(crate) sql: &'static str,
    pub(crate) existing_columns: &'static [&'static str],
    pub(crate) expected_lines: &'static [&'static str],
}

pub(crate) struct DeepUnionTestCase {
    pub(crate) description: &'static str,
    pub(crate) branches: usize,
    pub(crate) stack_bytes: usize,
    pub(crate) expected_status: &'static str,
    pub(crate) expected_lines: &'static [&'static str],
    pub(crate) expected_detail: Option<&'static str>,
}

pub(crate) struct RichLineageTestCase {
    pub(crate) description: &'static str,
    pub(crate) dialect: &'static str,
    pub(crate) sql: &'static str,
    pub(crate) expected_status: &'static str,
    /// One `output transform confidence nullability [type:resource.column ...]` line per column.
    pub(crate) expected_lines: &'static [&'static str],
    pub(crate) expected_has_star: bool,
    pub(crate) expected_detail: Option<&'static str>,
}

pub(crate) struct EnvironmentMarkerTestCase {
    pub(crate) description: &'static str,
    pub(crate) contents: &'static [u8],
    /// The names read, or `None` where Python refuses to cache the graph.
    pub(crate) expected_names: Option<&'static [&'static str]>,
}

pub(crate) struct InterruptedListingPolicyTestCase {
    pub(crate) description: &'static str,
    pub(crate) interrupted: bool,
    pub(crate) policy: InterruptedListingPolicy,
    pub(crate) expected_status: &'static str,
}

pub(crate) struct RichPanicTestCase {
    pub(crate) description: &'static str,
    pub(crate) models: &'static [&'static str],
    /// The answered models' SQL in order, or the request's error.
    pub(crate) expected_outcome: Result<&'static [&'static str], &'static str>,
}

pub(crate) struct UnavailableWalkTestCase {
    pub(crate) description: &'static str,
    pub(crate) failure: fn() -> StageFailure,
    pub(crate) expected_status: &'static str,
}

pub(crate) struct RichDialectTestCase {
    pub(crate) description: &'static str,
    pub(crate) names: &'static [&'static str],
    pub(crate) expected_known: bool,
}

pub(crate) struct SqlQuackLineageTestCase {
    pub(crate) description: &'static str,
    pub(crate) dialect: &'static str,
    /// Each model's name and compiled SQL, in project order; the last one is checked.
    pub(crate) models: &'static [(&'static str, &'static str)],
    /// One `output transform confidence nullability [type:resource.column ...]` line per column.
    pub(crate) expected_lines: &'static [&'static str],
    pub(crate) expected_has_star: bool,
}
