pub(crate) struct FailureCauseTestCase {
    pub(crate) description: &'static str,
    pub(crate) report: fn(),
    pub(crate) expected_code: Option<&'static str>,
}

pub(crate) struct CacheKeyTestCase {
    pub(crate) description: &'static str,
    /// The environment variable values of the second lookup, by name.
    pub(crate) changed: &'static [(&'static str, &'static str)],
    pub(crate) expected_same_key: bool,
}
