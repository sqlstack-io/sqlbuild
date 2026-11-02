pub(super) struct NormalizeTypeTestCase {
    pub(super) description: &'static str,
    pub(super) type_sql: &'static str,
    pub(super) dialect: &'static str,
    /// `name | family | precision | scale | length | parse error`, or Python's error.
    pub(super) expected_normalization: Result<&'static str, &'static str>,
}

pub(super) struct PythonIntTestCase {
    pub(super) description: &'static str,
    pub(super) text: &'static str,
    pub(super) expected_value: Option<i64>,
}

pub(super) struct SplitTypeTestCase {
    pub(super) description: &'static str,
    pub(super) type_sql: &'static str,
    pub(super) expected_split: (&'static str, Vec<&'static str>),
}

pub(super) struct BracketDepthTestCase {
    pub(super) description: &'static str,
    pub(super) depth: usize,
    pub(super) expected_native: bool,
}

pub(super) struct SameTypeTestCase {
    pub(super) description: &'static str,
    pub(super) dialect: &'static str,
    pub(super) types: (&'static str, &'static str),
    pub(super) expected_same: bool,
}
