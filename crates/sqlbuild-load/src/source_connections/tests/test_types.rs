pub(super) struct SourceConnectionsTestCase {
    pub(super) description: &'static str,
    pub(super) toml: &'static str,
    pub(super) expected_result: Result<&'static [(&'static str, &'static str)], &'static str>,
}
