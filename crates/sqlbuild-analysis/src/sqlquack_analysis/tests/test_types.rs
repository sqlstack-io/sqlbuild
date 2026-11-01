pub(crate) struct CompileLineageTestCase {
    pub(crate) description: &'static str,
    pub(crate) dialect: &'static str,
    pub(crate) sql: &'static str,
    /// `output transform confidence [type:resource.column, ...]` lines, or `unread` / `no answer`.
    pub(crate) expected_lines: &'static [&'static str],
}

pub(crate) struct CompileTypesTestCase {
    pub(crate) description: &'static str,
    pub(crate) sql: &'static str,
    pub(crate) expected_lines: &'static [&'static str],
}
