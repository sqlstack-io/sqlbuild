pub(crate) struct CompileLineageTestCase {
    pub(crate) description: &'static str,
    pub(crate) dialect: &'static str,
    pub(crate) sql: &'static str,
    /// `Some(lines)` as `output transform confidence [type:resource.column, ...]`; `None` unread.
    pub(crate) expected_lines: Option<&'static [&'static str]>,
}
