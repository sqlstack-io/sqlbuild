pub(crate) struct MatchTestCase {
    pub(crate) description: &'static str,
    /// `(name, schema, database)` written in SQL.
    pub(crate) written: (&'static str, Option<&'static str>, Option<&'static str>),
    /// The matched `(kind, name, compatibility_for)`, or None.
    pub(crate) expected_match: Option<(&'static str, &'static str, Option<&'static str>)>,
}

pub(crate) struct HardCodedSqlTestCase {
    pub(crate) description: &'static str,
    /// Literal SQL a ClickHouse node runs, in line order.
    pub(crate) statements: &'static [&'static str],
    pub(crate) expected_p008_count: usize,
}
