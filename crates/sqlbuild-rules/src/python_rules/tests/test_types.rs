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

#[derive(Debug, PartialEq)]
pub(super) struct CachedRuns {
    pub(super) first_faults: Vec<(String, String)>,
    pub(super) second_faults: Vec<(String, String)>,
    pub(super) second_hits: u64,
    pub(super) second_misses: u64,
}

pub(super) struct PythonCacheTestCase {
    pub(super) description: &'static str,
    pub(super) files: &'static [(&'static str, &'static str)],
    pub(super) config: serde_json::Value,
    pub(super) changes: &'static [(&'static str, &'static str)],
    pub(super) changed_config: serde_json::Value,
    pub(super) expected_runs: CachedRuns,
}

pub(super) struct PythonScalingTestCase {
    pub(super) description: &'static str,
    pub(super) units: usize,
    pub(super) noisy_files: usize,
    pub(super) functions: usize,
    pub(super) config: serde_json::Value,
    pub(super) expected_annotation_faults: usize,
    pub(super) expected_max_duration: std::time::Duration,
}

pub(super) struct UnitScalingTestCase {
    pub(super) description: &'static str,
    pub(super) units: usize,
    pub(super) undescribed: usize,
    pub(super) config: serde_json::Value,
    pub(super) expected_codes: Vec<(&'static str, usize)>,
    pub(super) expected_max_duration: std::time::Duration,
    pub(super) growth: usize,
    pub(super) pairs: usize,
    pub(super) expected_max_ratio: f64,
}

pub(super) struct PythonGrowthTestCase {
    pub(super) description: &'static str,
    pub(super) units: usize,
    pub(super) noisy_files: usize,
    pub(super) functions: usize,
    pub(super) config: serde_json::Value,
    pub(super) growth: usize,
    pub(super) pairs: usize,
    pub(super) expected_unreachable_definitions: usize,
    pub(super) expected_max_ratio: f64,
}

pub(super) struct RelativeQueryTestCase {
    pub(super) description: &'static str,
    pub(super) project_dir: &'static str,
    pub(super) path: &'static str,
    pub(super) expected_path: &'static str,
}
