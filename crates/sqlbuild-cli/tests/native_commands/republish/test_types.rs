//! Test cases for `sqb location republish`.

/// One invocation against an `http://` network origin, and what the client did with the credential.
pub(crate) struct RepublishCliTestCase {
    pub(crate) description: &'static str,
    pub(crate) key: Option<&'static str>,
    pub(crate) states: &'static [&'static str],
    pub(crate) arguments: &'static [&'static str],
    pub(crate) expected_exit: i32,
    pub(crate) expected_methods: &'static [&'static str],
    pub(crate) expected_warnings: usize,
    pub(crate) expected_stderr: &'static str,
}
