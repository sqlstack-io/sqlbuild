pub(super) struct CallDepthTestCase {
    pub(super) description: &'static str,
    pub(super) depth: usize,
    /// The guard error every parser raises, from its code, or `None` when each accepts the SQL.
    pub(super) expected_error: Option<String>,
}
