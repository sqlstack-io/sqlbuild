use crate::models::RuleMetadata;
use crate::rules::models::BuiltinOptionKind;

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

pub(super) struct OptionsTestCase {
    pub(super) description: &'static str,
    pub(super) code: &'static str,
    pub(super) expected_options: Vec<(&'static str, BuiltinOptionKind)>,
}

pub(super) struct SourceKeyTestCase {
    pub(super) description: &'static str,
    pub(super) select: &'static [&'static str],
    pub(super) edited_source: &'static str,
    pub(super) expected_same_key: bool,
}

pub(super) struct PythonRunTestCase {
    pub(super) description: &'static str,
    pub(super) files: &'static [(&'static str, &'static str)],
    pub(super) config: serde_json::Value,
    pub(super) expected_faults: Vec<(&'static str, &'static str, u64, u64)>,
}

pub(super) struct PythonParseFailureTestCase {
    pub(super) description: &'static str,
    pub(super) files: &'static [(&'static str, &'static str)],
    pub(super) expected_error: &'static str,
}
