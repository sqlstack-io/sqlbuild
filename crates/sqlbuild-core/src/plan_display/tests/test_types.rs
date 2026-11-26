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

/// A section and the names of all its rows, in plan order.
pub(super) struct SectionTestCase {
    pub(super) description: &'static str,
    pub(super) section: PlanPageSection,
    pub(super) expected_rows: &'static [&'static str],
}

/// A page read of a plan section: its window and name filter, and the row names and totals it returns.
pub(super) struct PageTestCase {
    pub(super) description: &'static str,
    pub(super) section: PlanPageSection,
    pub(super) window: (usize, usize),
    pub(super) name: Option<&'static str>,
    pub(super) expected_rows: &'static [&'static str],
    pub(super) expected_total: usize,
    pub(super) expected_section_total: usize,
}

/// A model page and what it brings beside its rows.
pub(super) struct ModelPageTestCase {
    pub(super) description: &'static str,
    pub(super) section: PlanPageSection,
    pub(super) expected_stats: Vec<serde_json::Value>,
    pub(super) expected_runs: &'static [&'static str],
    pub(super) expected_warnings: &'static [&'static str],
    pub(super) expected_starts: serde_json::Value,
}

/// A section of the summary and a note it counts.
pub(super) struct NoteTestCase {
    pub(super) description: &'static str,
    pub(super) section: &'static str,
    pub(super) expected_note: serde_json::Value,
}

/// A summary field and the value it carries in place of rows.
pub(super) struct SummaryFieldTestCase {
    pub(super) description: &'static str,
    pub(super) pointer: &'static str,
    pub(super) expected_value: serde_json::Value,
}

/// A diff and the counts plan pages report for it.
pub(super) struct DiffStatsTestCase {
    pub(super) description: &'static str,
    pub(super) diff: &'static [&'static str],
    pub(super) expected_stats: crate::plan_display::models::DiffStats,
}

/// A model of a document whose diffs were detached, and the diff text kept apart for it.
pub(super) struct DetachTestCase {
    pub(super) description: &'static str,
    pub(super) model: &'static str,
    pub(super) expected_detached: Option<&'static [&'static str]>,
}

/// A first build of so many models, and the most bytes its summary may take.
pub(super) struct SummarySizeTestCase {
    pub(super) description: &'static str,
    pub(super) models: usize,
    /// Every model runs under `--changed`, and each has its own distinct warning.
    pub(super) changed_with_warnings: bool,
    pub(super) expected_max_bytes: usize,
}

pub(super) struct ConfigChangeTestCase {
    pub(super) description: &'static str,
    pub(super) section: PlanPageSection,
    pub(super) model: &'static str,
    pub(super) expected_change: Option<&'static str>,
}

pub(super) struct PlanSummaryTestCase {
    pub(super) description: &'static str,
    pub(super) counts: crate::plan_display::models::PlanSummaryCounts,
    pub(super) expected_parts: &'static [&'static str],
}
