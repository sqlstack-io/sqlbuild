//! People, requests and counts the plan contract builds on.

use uuid::Uuid;

use crate::authorization::main::assign_builtin_role::assign_builtin_role;
use crate::credentials::main::identities::create_principal::create_principal;
use crate::credentials::main::sessions::set_password::set_password;
use crate::credentials::models::{
    AuditContext, BuiltinRole, PasswordAuthority, RoleAssignmentRequest,
};
use crate::platform::main::report_location_health::report_location_health;
use crate::platform::models::{
    LocationHealth, LocationHealthReport, LocationWorkPoll, PlanRequest,
};
use crate::runs::main::request_person_run::request_person_run;
use crate::runs::models::PersonRunRequest;
use crate::runs::tests::test_types::ExactKeyPlanner;
use crate::store::errors::ControlError;
use crate::store::main::{begin::begin, commit::commit, query::query};
use crate::store::models::{ControlStore, TransactionMode};

pub(crate) fn plan_request(tokens: &[&str]) -> PlanRequest {
    PlanRequest {
        request_id: Uuid::now_v7(),
        command: tokens.iter().map(|token| (*token).to_owned()).collect(),
    }
}

/// Request a plan on the `development` target as `person`.
pub(crate) async fn ask(
    store: &ControlStore,
    (context, person): (&AuditContext, Uuid),
    location: Uuid,
    request: &PlanRequest,
) -> Result<bool, ControlError> {
    request_person_run(
        store,
        (context, person),
        PersonRunRequest::Plan(&ExactKeyPlanner, (location, "development", request)),
    )
    .await
}

/// A principal with a password and, when given, a built-in role on the location's `development` target.
pub(crate) async fn person(
    store: &ControlStore,
    (context, admin): (&AuditContext, Uuid),
    (name, location): (&str, Uuid),
    roles: &[BuiltinRole],
) -> Uuid {
    let person = create_principal(store, context, name)
        .await
        .expect("person");
    set_password(
        store,
        context,
        PasswordAuthority::Operator(person),
        "orders-password-2026",
    )
    .await
    .expect("password");
    for role in roles {
        assign_builtin_role(
            store,
            (context, admin),
            &RoleAssignmentRequest {
                principal_id: person,
                role: (*role).into(),
                location_id: Some(location),
                target: Some("development".into()),
                revoke: false,
            },
        )
        .await
        .expect("assign");
    }
    person
}

pub(crate) async fn register_instance(store: &ControlStore, location: Uuid) -> LocationWorkPoll {
    let poll = LocationWorkPoll {
        session_id: Uuid::now_v7(),
        target: "development".into(),
    };
    report_location_health(
        store,
        location,
        &LocationHealthReport {
            event_id: Uuid::now_v7(),
            session_id: poll.session_id,
            target: poll.target.clone(),
            health: LocationHealth::Heartbeat {},
        },
    )
    .await
    .expect("register instance");
    poll
}

/// Run and run-node rows together; a plan must add none.
pub(crate) async fn run_rows(store: &ControlStore) -> i64 {
    let transaction = begin(store, TransactionMode::Read).await.expect("begin");
    let (transaction, rows) = query(
        transaction,
        "SELECT (SELECT COUNT(*) FROM sqb_runs) + (SELECT COUNT(*) FROM sqb_run_nodes) AS n",
        Vec::new(),
    )
    .await
    .expect("count");
    commit(transaction).await.expect("commit");
    rows[0]["n"].as_i64().expect("count")
}
