pub(super) struct SourceConnectionsTestCase {
    pub(super) description: &'static str,
    pub(super) toml: &'static str,
    pub(super) expected: Result<&'static [(&'static str, &'static str)], &'static str>,
}
