pub(crate) struct RedactTestCase {
    pub(crate) description: &'static str,
    pub(crate) text: &'static str,
    pub(crate) expected_text: &'static str,
}

pub(crate) struct FieldNameTestCase {
    pub(crate) description: &'static str,
    pub(crate) name: &'static str,
    pub(crate) expected_secret: bool,
}

pub(crate) struct NulPositionTestCase {
    pub(crate) description: &'static str,
    pub(crate) redact: fn(&str) -> String,
    /// The same redaction of a single reading.
    pub(crate) reading: fn(&str) -> String,
    pub(crate) shapes: &'static [&'static str],
    pub(crate) expected_exposed: usize,
}
