//! Both launchers hand off the same scoped invocation and exclude privileged credentials.

use sqlbuild_core::command_output::constants::INVOCATION_CONTEXT_ENV;
use sqlbuild_orchestration::runs::models::RunClaim;

use crate::workers::models::LauncherScope;

pub(in crate::workers) fn arguments(scope: &LauncherScope<'_>, claim: &RunClaim) -> Vec<String> {
    [
        "location".into(),
        "execute".into(),
        "--server".into(),
        scope.server_url.to_owned(),
        "--location-id".into(),
        scope.location_id.to_string(),
        "--target".into(),
        scope.target.to_owned(),
        "--run-id".into(),
        claim.request.run_id.to_string(),
        "--claim-id".into(),
        claim.claim_id.to_string(),
        "--project-dir".into(),
    ]
    .into()
}

pub(in crate::workers) fn excluded(name: &str, control_variable: &str) -> bool {
    name == control_variable
        || matches!(
            name,
            "SQLBUILD_CONTROL_DATABASE_URL" | "SQLBUILD_LOCATION_TOKEN" | "SQLBUILD_RUN_TOKEN"
        )
        || name == INVOCATION_CONTEXT_ENV
}

pub(in crate::workers) fn environment(
    claim: &RunClaim,
    bearer: &str,
) -> [(&'static str, String); 2] {
    [
        ("SQLBUILD_RUN_TOKEN", bearer.into()),
        (INVOCATION_CONTEXT_ENV, serde_json::json!({
            "run_id": claim.request.run_id,
            "integration": {"name": "sqlbuild", "run_id": claim.request.run_id, "job_name": claim.request.job_key},
        }).to_string()),
    ]
}
