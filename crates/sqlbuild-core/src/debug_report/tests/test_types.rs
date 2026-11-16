use crate::debug_report::models::{
    ConnectionCheck, ConnectionValue, DebugFailure, DebugProvider, DebugReport,
};

pub(super) struct FormatTestCase {
    pub(super) description: &'static str,
    pub(super) report: DebugReport,
    pub(super) expected_text: &'static str,
    pub(super) expected_json: &'static str,
    pub(super) expected_color_fragments: &'static [&'static str],
}

pub(super) struct ConnectionLinesTestCase {
    pub(super) description: &'static str,
    pub(super) settings: Vec<(String, ConnectionValue)>,
    pub(super) check: ConnectionCheck,
    pub(super) expected_lines: Vec<(&'static str, &'static str, Option<&'static str>)>,
}

pub(super) struct DebugFailureTestCase {
    pub(super) description: &'static str,
    pub(super) report: DebugReport,
    pub(super) expected_failure: Option<DebugFailure>,
}

pub(super) struct ProviderLinesTestCase {
    pub(super) description: &'static str,
    pub(super) providers: Vec<DebugProvider>,
    pub(super) verbose: bool,
    pub(super) expected_lines: Vec<(&'static str, &'static str, Option<&'static str>)>,
}
