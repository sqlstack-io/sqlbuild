//! Real people, sessions and published custom roles for custom role contracts.

use serde_json::{Value, json};
use uuid::Uuid;

use crate::authorization::main::{
    assign_builtin_role::assign_builtin_role, authorize_session::authorize_session,
    bootstrap_user::bootstrap_user,
};
use crate::credentials::main::{
    identities::create_principal::create_principal, sessions::login_password::login_password,
    sessions::set_password::set_password,
};
use crate::credentials::models::{
    Action, AuditContext, DefaultRole, IssuedSession, PasswordAuthority, PasswordLogin,
    PermissionScope, RoleAssignmentRequest, RoleKey,
};
use crate::credentials::tests::location_helpers::create_location;
use crate::credentials::tests::test_types::RoleFixture;
use crate::runs::tests::helpers::snapshot;
use crate::snapshots::main::publish_snapshot::publish_snapshot;
use crate::store::errors::ControlError;
use crate::store::main::begin::begin;
use crate::store::models::{ControlStore, TransactionMode};

pub(crate) const SUPPORT_ROLE: &str = "orders_support";

async fn signed_in(
    store: &ControlStore,
    context: &AuditContext,
    (principal, name): (Uuid, &str),
) -> IssuedSession {
    set_password(
        store,
        context,
        PasswordAuthority::Operator(principal),
        "orders-password-2026",
    )
    .await
    .expect("password");
    let PasswordLogin::Authenticated(session) =
        login_password(store, context, (name, "orders-password-2026"))
            .await
            .expect("login")
    else {
        panic!("valid login");
    };
    session
}

pub(crate) fn credentials(session: &IssuedSession) -> (&str, &str) {
    (session.bearer.as_str(), session.csrf.as_str())
}

async fn assign(
    store: &ControlStore,
    fixture: (&AuditContext, &IssuedSession),
    request: RoleAssignmentRequest,
) {
    assert!(
        assign_builtin_role(store, (fixture.0, credentials(fixture.1)), &request)
            .await
            .expect("role assignment")
    );
}

/// Publish one generation whose `roles` document grants the support role these actions.
pub(crate) async fn publish_support_role(
    store: &ControlStore,
    location_id: Uuid,
    (generation, actions): (i64, Value),
) {
    let mut input = snapshot(location_id);
    input.generation = generation;
    input.documents.insert(
        "roles".into(),
        json!({SUPPORT_ROLE: {"description": "Answer order questions", "actions": actions}}),
    );
    publish_snapshot(store, &input)
        .await
        .expect("publish roles");
}

/// The first administrator, then a member holding the published support role.
pub(crate) async fn role_fixture(store: &ControlStore) -> RoleFixture {
    let context = AuditContext {
        actor: "local-control-store-owner".into(),
        granting_role: "control-store-owner".into(),
        request_id: Uuid::now_v7(),
        ip_address: Some("127.0.0.1".parse().expect("local peer")),
    };
    let first = bootstrap_user(store, &context, "orders-admin")
        .await
        .expect("bootstrap");
    let administrator = signed_in(store, &context, (first, "orders-admin")).await;
    let member = create_principal(store, &context, "support-person")
        .await
        .expect("member");
    let member_session = signed_in(store, &context, (member, "support-person")).await;
    let location_id = create_location(store, "orders", "orders-admin")
        .await
        .expect("location");
    publish_support_role(store, location_id, (1, json!(["view"]))).await;
    assign(
        store,
        (&context, &administrator),
        RoleAssignmentRequest {
            principal_id: member,
            role: RoleKey::Custom(SUPPORT_ROLE.into()),
            location_id: Some(location_id),
            target: None,
            revoke: false,
        },
    )
    .await;
    RoleFixture {
        context,
        location_id,
        administrator,
        member: member_session,
        member_id: member,
    }
}

/// The role that grants the member this action in the fixture location, or `None` when denied.
pub(crate) async fn granting_role(
    store: &ControlStore,
    fixture: &RoleFixture,
    action: Action,
) -> Option<String> {
    let scope = PermissionScope {
        action,
        location_id: Some(fixture.location_id),
        target: Some("development".into()),
        nodes: Default::default(),
    };
    let decision = authorize_session(
        begin(store, TransactionMode::Write).await.expect("begin"),
        (&fixture.context, credentials(&fixture.member)),
        &scope,
        DefaultRole::None,
    )
    .await;
    assert!(
        decision
            .as_ref()
            .err()
            .is_none_or(|error| matches!(error, ControlError::Denied { .. })),
        "authorization either grants or denies"
    );
    decision.ok().map(|(_, context)| context.granting_role)
}
