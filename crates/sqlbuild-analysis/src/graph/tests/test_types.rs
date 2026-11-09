use crate::graph::models::ParsedSelector;
use crate::graph::tests::helpers::ResourceSpec;

pub(super) struct SelectorTestCase {
    pub(super) description: &'static str,
    pub(super) select: &'static [&'static str],
    pub(super) exclude: &'static [&'static str],
    pub(super) expected_keys: &'static [(&'static str, &'static str)],
}

pub(super) struct SelectorErrorTestCase {
    pub(super) description: &'static str,
    pub(super) select: &'static [&'static str],
    pub(super) exclude: &'static [&'static str],
    pub(super) expected_error: SelectorFailure,
}

/// `(code, message, help)` of the `PlannerInputError` Python raises.
pub(super) type SelectorFailure = (&'static str, &'static str, Option<&'static str>);

pub(super) struct CloseMatchTestCase {
    pub(super) description: &'static str,
    pub(super) word: &'static str,
    pub(super) candidates: &'static [&'static str],
    pub(super) cutoff: f64,
    pub(super) expected_matches: &'static [&'static str],
}

pub(super) struct ParseSelectorTestCase {
    pub(super) description: &'static str,
    pub(super) raw: &'static str,
    pub(super) expected_selector: ParsedSelector,
}

pub(super) struct ParseSelectorErrorTestCase {
    pub(super) description: &'static str,
    pub(super) raw: &'static str,
    pub(super) expected_code: &'static str,
}

pub(super) struct SelectedNamesTestCase {
    pub(super) description: &'static str,
    pub(super) project: &'static [ResourceSpec],
    pub(super) select: &'static [&'static str],
    pub(super) exclude: &'static [&'static str],
    pub(super) expected_names: &'static [&'static str],
}

pub(super) struct ProjectSelectorErrorTestCase {
    pub(super) description: &'static str,
    pub(super) project: &'static [ResourceSpec],
    pub(super) select: &'static [&'static str],
    pub(super) expected_error: SelectorFailure,
}

pub(super) struct JobSelectorTestCase {
    pub(super) description: &'static str,
    pub(super) select: &'static [&'static str],
    pub(super) exclude: &'static [&'static str],
    pub(super) expected_names: &'static [&'static str],
}

pub(super) struct JobSelectorErrorTestCase {
    pub(super) description: &'static str,
    pub(super) attached: bool,
    pub(super) select: &'static [&'static str],
    pub(super) expected_error: SelectorFailure,
}
