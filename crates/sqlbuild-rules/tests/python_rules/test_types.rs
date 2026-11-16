pub(crate) struct ReachabilityWorkTestCase {
    pub(crate) description: &'static str,
    pub(crate) modules: usize,
    pub(crate) growth: usize,
    pub(crate) config: serde_json::Value,
    pub(crate) expected_unreachable_definitions: usize,
    pub(crate) expected_max_ratio: f64,
}
