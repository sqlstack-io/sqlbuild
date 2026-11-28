use crate::sql_scan::models::Unclosed;
use crate::sql_scan::models::{LexicalSyntax, QuotePolicy};

pub(crate) struct MatchingParenPolicyTestCase {
    pub(crate) description: &'static str,
    pub(crate) sql: &'static str,
    pub(crate) policy: QuotePolicy,
    pub(crate) expected_close: Result<usize, Unclosed>,
}

pub(crate) struct NonCodeEndTestCase {
    pub(crate) description: &'static str,
    pub(crate) sql: &'static str,
    pub(crate) policy: QuotePolicy,
    pub(crate) expected_end: Result<Option<usize>, Unclosed>,
}

pub(crate) struct DispatchTestCase {
    pub(crate) description: String,
    pub(crate) sql: Vec<u8>,
    pub(crate) index: usize,
    pub(crate) policy: QuotePolicy,
    pub(crate) expected_end: Result<Option<usize>, Unclosed>,
}

pub(crate) struct DispatchInputTestCase {
    pub(crate) description: &'static str,
    pub(crate) sql: &'static [u8],
    pub(crate) expected_cases: fn(&[u8]) -> Vec<DispatchTestCase>,
}

pub(crate) struct DialectNonCodeEndTestCase {
    pub(crate) description: &'static str,
    pub(crate) sql: &'static str,
    pub(crate) index: usize,
    pub(crate) syntax: LexicalSyntax,
    pub(crate) expected_end: Result<Option<usize>, Unclosed>,
}

pub(crate) struct DialectFirstNonCodeRangeTestCase {
    pub(crate) description: &'static str,
    pub(crate) sql: &'static str,
    pub(crate) syntax: crate::sql_scan::models::LexicalSyntax,
    pub(crate) expected_first_range: Option<(usize, usize)>,
}

pub(crate) struct DialectNonCodeRangesTestCase {
    pub(crate) description: &'static str,
    pub(crate) sql: &'static str,
    pub(crate) syntax: crate::sql_scan::models::LexicalSyntax,
    pub(crate) expected_marker_protected: bool,
}

pub(crate) struct LexicalRulesTestCase {
    pub(crate) description: &'static str,
    pub(crate) syntax: LexicalSyntax,
    pub(crate) sql: &'static str,
    pub(crate) expected_key: &'static str,
    pub(crate) expected_reads_differently: bool,
}

pub(crate) struct PrefixCheckTestCase {
    pub(crate) description: &'static str,
    pub(crate) declared: &'static [&'static str],
    pub(crate) expected_unsupported: &'static [&'static str],
}

pub(crate) struct BoundaryTestCase {
    pub(crate) description: &'static str,
    /// `{}` marks each site.
    pub(crate) template: &'static str,
    pub(crate) values: &'static [&'static str],
    pub(crate) dialect: &'static str,
    pub(crate) expected_sql: &'static str,
}

pub(crate) struct GeneratedBoundaryTestCase {
    pub(crate) description: &'static str,
    pub(crate) syntax: LexicalSyntax,
    /// Splices that gained or lost code, or changed although they read right.
    pub(crate) expected_failures: &'static [&'static str],
}

pub(crate) struct CandidateSkipTestCase {
    pub(crate) description: &'static str,
    pub(crate) syntax: LexicalSyntax,
    pub(crate) expected_ranges: fn(&str, &LexicalSyntax) -> Vec<(usize, usize)>,
}
