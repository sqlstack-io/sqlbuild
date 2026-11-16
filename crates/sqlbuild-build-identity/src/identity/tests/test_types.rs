pub(crate) struct BuildIdentityTestCase {
    pub(crate) description: &'static str,
    pub(crate) expected_hash_length: usize,
}

pub(crate) struct HashedWorkspaceFileTestCase {
    pub(crate) description: &'static str,
    pub(crate) workspace_file: &'static str,
    pub(crate) expected_hashed: bool,
}
