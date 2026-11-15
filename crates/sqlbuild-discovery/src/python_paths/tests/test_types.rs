/// One project layout and the Python files outside documented roots.
pub(super) struct PythonPathsTestCase {
    pub(super) description: &'static str,
    pub(super) files: &'static [&'static str],
    pub(super) expected_unsupported: &'static [&'static str],
}

/// One project layout and the Python files the Python rules read.
pub(super) struct SupportedPathsTestCase {
    pub(super) description: &'static str,
    pub(super) files: &'static [&'static str],
    pub(super) expected_supported: &'static [&'static str],
}

/// One project layout and its Python test files.
pub(super) struct TestPathsTestCase {
    pub(super) description: &'static str,
    pub(super) files: &'static [&'static str],
    pub(super) expected_tests: &'static [&'static str],
}
