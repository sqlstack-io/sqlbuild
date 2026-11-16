use crate::cli_text::models::{CliStyle, PreBuildWorkText};
use crate::plan_display::models::{
    ModelLabelFacts, ParsedBound, PlanPageSection, TimestampGrainFacts,
};

pub(super) struct LabelTestCase {
    pub(super) description: &'static str,
    pub(super) entry: ModelLabelFacts,
    pub(super) expected_label: &'static str,
    pub(super) expected_resource_type: &'static str,
    pub(super) expected_annotation: &'static str,
}

pub(super) struct BoundTestCase {
    pub(super) description: &'static str,
    pub(super) value: &'static str,
    pub(super) typing: (Option<&'static str>, Option<&'static str>),
    pub(super) parsed: Option<ParsedBound>,
    pub(super) expected_text: &'static str,
}

pub(super) struct NamesTestCase {
    pub(super) description: &'static str,
    pub(super) names: Vec<String>,
    pub(super) expected_no_rows: &'static str,
    pub(super) expected_waiting: &'static str,
}

pub(super) struct BatchSizeTestCase {
    pub(super) description: &'static str,
    pub(super) batch_size: &'static str,
    pub(super) grain: &'static str,
    pub(super) expected_size: Result<&'static str, &'static str>,
}

pub(super) struct TimestampGrainTestCase {
    pub(super) description: &'static str,
    pub(super) facts: TimestampGrainFacts,
    pub(super) expected_grain: Result<Option<&'static str>, &'static str>,
}

pub(super) struct PreBuildWorkTestCase {
    pub(super) description: &'static str,
    pub(super) work: PreBuildWorkText,
    pub(super) expected_lines: Vec<&'static str>,
}

pub(super) struct QueryDiffTestCase {
    pub(super) description: &'static str,
    pub(super) style: CliStyle,
    pub(super) expected_lines: Vec<&'static str>,
}

/// A plan document and what its summary carries: changed model names, built names, page counts and kept runs.
pub(super) struct SummaryTestCase {
    pub(super) description: &'static str,
    pub(super) document: fn() -> serde_json::Value,
    pub(super) expected_models: &'static [&'static str],
    pub(super) expected_built: &'static [&'static str],
    pub(super) expected_pages: serde_json::Value,
    pub(super) expected_runs: serde_json::Value,
}

/// A page read of a plan section and the row names, section size and runs it returns.
pub(super) struct PageTestCase {
    pub(super) description: &'static str,
    pub(super) document: fn() -> serde_json::Value,
    pub(super) section: PlanPageSection,
    pub(super) window: (usize, usize),
    pub(super) expected_rows: serde_json::Value,
    pub(super) expected_total: usize,
    pub(super) expected_runs: serde_json::Value,
}
