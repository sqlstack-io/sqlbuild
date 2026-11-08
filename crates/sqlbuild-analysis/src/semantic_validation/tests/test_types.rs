pub(crate) struct SemanticValidationTestCase {
    pub(crate) description: &'static str,
    pub(crate) request: &'static str,
    pub(crate) expected_complete: bool,
}

pub(crate) struct SemanticValidationBatchTestCase {
    pub(crate) description: &'static str,
    pub(crate) request: &'static str,
    pub(crate) expected_validity: [bool; 2],
}

pub(crate) struct UnsupportedFunctionTestCase {
    pub(crate) description: &'static str,
    pub(crate) dialect: polyglot_sql::DialectType,
    pub(crate) sql: &'static str,
    pub(crate) expected_calls: &'static [(&'static str, &'static str)],
}

pub(crate) struct LocatedFunctionDiagnosticTestCase {
    pub(crate) description: &'static str,
    pub(crate) sql: &'static str,
    pub(crate) expected_message: &'static str,
    pub(crate) expected_location: (Option<usize>, Option<usize>),
    pub(crate) expected_span: (Option<usize>, Option<usize>),
}

pub(crate) struct UniqueWordOffsetTestCase {
    pub(crate) description: &'static str,
    pub(crate) authored: &'static str,
    pub(crate) identifier: &'static str,
    pub(crate) expected_offset: Option<usize>,
}

pub(crate) struct UniqueWordScalingTestCase {
    pub(crate) description: &'static str,
    pub(crate) identifiers: usize,
    pub(crate) expected_max_duration: std::time::Duration,
}

pub(crate) struct TypeLocationTestCase {
    pub(crate) description: &'static str,
    pub(crate) sql: &'static str,
    pub(crate) dialect: &'static str,
    pub(crate) findings: &'static [(&'static str, &'static str)],
    pub(crate) expected_starts: &'static [&'static str],
}

pub(crate) struct NormalizationBatchTestCase {
    pub(crate) description: &'static str,
    pub(crate) dialect: &'static str,
    pub(crate) requests: &'static [(&'static str, &'static [(&'static str, &'static str)])],
    pub(crate) expected_failures: &'static [usize],
}

pub(crate) struct NativeStubTestCase {
    pub(crate) description: &'static str,
    pub(crate) dialect: &'static str,
    pub(crate) sql: &'static str,
    pub(crate) expected_sql: &'static str,
}

pub(crate) struct TruthTestScanTestCase {
    pub(crate) description: &'static str,
    pub(crate) sql: &'static str,
    pub(crate) expected_tests: &'static [(usize, usize, bool, bool)],
}

pub(crate) struct TruthTestDialectTestCase {
    pub(crate) description: &'static str,
    pub(crate) dialect: polyglot_sql::DialectType,
    pub(crate) expected_rejected: bool,
}

pub(crate) struct LocatedTruthTestDiagnosticTestCase {
    pub(crate) description: &'static str,
    pub(crate) dialect: polyglot_sql::DialectType,
    pub(crate) sql: &'static str,
    pub(crate) expected_message: &'static str,
    pub(crate) expected_location: (Option<usize>, Option<usize>),
    pub(crate) expected_span: (Option<usize>, Option<usize>),
}

pub(crate) struct TemporalCastScanTestCase {
    pub(crate) description: &'static str,
    pub(crate) sql: &'static str,
    pub(crate) expected_casts: &'static [(usize, usize, &'static str)],
}

pub(crate) struct TemporalCastDialectTestCase {
    pub(crate) description: &'static str,
    pub(crate) dialect: polyglot_sql::DialectType,
    pub(crate) expected_warnings: usize,
}
