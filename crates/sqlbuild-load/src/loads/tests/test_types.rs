pub(super) struct CheckLoadTestCase {
    pub(super) description: &'static str,
    pub(super) settings: &'static str,
    pub(super) sources: &'static [(&'static str, &'static [(&'static str, &'static str)])],
    pub(super) expected_result: Result<&'static str, (&'static str, &'static [&'static str])>,
}

pub(super) struct YamlLineTestCase {
    pub(super) description: &'static str,
    pub(super) path: &'static [&'static str],
    pub(super) expected_line: Option<usize>,
}

pub(super) struct LoadMessageTestCase {
    pub(super) description: &'static str,
    pub(super) message: &'static str,
    pub(super) expected_message: &'static str,
}
