use crate::header_strings::errors::TripleQuoteError;
use crate::header_strings::models::HeaderQuote;

pub(crate) struct FoldTestCase {
    pub(crate) description: &'static str,
    pub(crate) written: &'static str,
    pub(crate) expected_folded: &'static str,
}

pub(crate) struct TripleEndTestCase {
    pub(crate) description: &'static str,
    pub(crate) text: &'static str,
    pub(crate) expected_end: Result<(usize, usize), TripleQuoteError>,
}

pub(crate) struct QuotedEndTestCase {
    pub(crate) description: &'static str,
    pub(crate) text: &'static str,
    pub(crate) expected_end: Option<usize>,
}

pub(crate) struct ProseTestCase {
    pub(crate) description: &'static str,
    pub(crate) text: &'static str,
    pub(crate) expected_prose: Result<(HeaderQuote, &'static str), &'static str>,
}

pub(crate) struct QuoteFailureTestCase {
    pub(crate) description: &'static str,
    pub(crate) error: &'static str,
    pub(crate) header: &'static str,
    pub(crate) expected_failure: Option<(usize, usize, &'static str)>,
}

pub(crate) struct ClosesInsideTestCase {
    pub(crate) description: &'static str,
    pub(crate) text: &'static str,
    pub(crate) end: usize,
    pub(crate) expected_inside: bool,
}

pub(crate) struct DescriptionEndTestCase {
    pub(crate) description: &'static str,
    pub(crate) text: &'static str,
    pub(crate) start: usize,
    pub(crate) expected_end: Option<usize>,
}

pub(crate) struct DescriptionKeysTestCase {
    pub(crate) description: &'static str,
    pub(crate) order: fn(Vec<usize>) -> Vec<usize>,
    pub(crate) expected_follows: fn(&[char], usize) -> bool,
}
