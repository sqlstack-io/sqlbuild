//! Browser session fixtures and independent role-matrix expectations.

use crate::api::tests::test_types::TestApi;
use crate::credentials::models::BuiltinRole;

pub(crate) struct BrowserSession {
    pub(crate) api: TestApi,
    pub(crate) credential: (&'static str, String),
    pub(crate) csrf: String,
    pub(crate) forgery: (&'static str, &'static str),
}

#[derive(serde::Deserialize)]
pub(crate) struct SessionRoleCase {
    pub(crate) role: Option<BuiltinRole>,
    pub(crate) expected_metadata: u16,
    pub(crate) expected_connection: u16,
    pub(crate) expected_job: u16,
    /// A job run above priority 0 needs `run.prioritise`, which only Admin grants.
    pub(crate) expected_prioritised: u16,
    pub(crate) expected_sql: u16,
    pub(crate) expected_admin: u16,
}
