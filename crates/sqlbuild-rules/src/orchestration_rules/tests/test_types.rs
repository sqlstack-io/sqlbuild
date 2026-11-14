pub(super) struct NamedSelectorLimitTestCase {
    pub(super) description: &'static str,
    pub(super) files: Vec<(&'static str, String)>,
    pub(super) config: serde_json::Value,
    pub(super) expected_findings: Vec<(&'static str, u64, u64, &'static str)>,
}
