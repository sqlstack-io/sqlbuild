use crate::models::RuleMetadata;

pub(super) struct CatalogueTestCase {
    pub(super) description: &'static str,
    pub(super) count: fn(&[RuleMetadata]) -> usize,
    pub(super) expected_count: usize,
}

pub(super) struct SelectionTestCase {
    pub(super) description: &'static str,
    pub(super) select: Vec<String>,
    pub(super) code: &'static str,
    pub(super) expected_selected: bool,
}

pub(super) struct CoreRuleTestCase {
    pub(super) description: &'static str,
    pub(super) source: &'static str,
    pub(super) fensu_code: &'static str,
    pub(super) expected_faults: Vec<(String, u32)>,
}
