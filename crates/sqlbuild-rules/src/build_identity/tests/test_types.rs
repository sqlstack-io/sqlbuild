use crate::build_identity::errors::BuildIdentityError;

pub(crate) struct InstallTestCase {
    pub(crate) description: &'static str,
    pub(crate) identity: &'static str,
    pub(crate) expected_result: Result<(), BuildIdentityError>,
}

pub(crate) struct MissingIdentityTestCase {
    pub(crate) description: &'static str,
    pub(crate) fresh_process_test: &'static str,
    pub(crate) expected_failed: &'static str,
    pub(crate) expected_reason: &'static str,
}

pub(crate) struct FreshProcessTestCase {
    pub(crate) description: &'static str,
    pub(crate) expected_installed: bool,
}
