//! Real people, sessions and published custom roles for role approval contracts.

use serde_json::{Value, json};
use uuid::Uuid;

use crate::authorization::main::{
    assign_builtin_role::assign_builtin_role, authorize_session::authorize_session,
    bootstrap_user::bootstrap_user,
};
use crate::authorization::main::{
    decide_role_approval::decide_role_approval, request_role_activation::request_role_activation,
};
use crate::credentials::main::{
    identities::create_principal::create_principal, sessions::login_password::login_password,
    sessions::set_password::set_password,
};
use crate::credentials::models::{
    Action, AdministratorAuthentication, AuditContext, BuiltinRole, DefaultRole, IssuedSession,
    PasswordAuthority, PasswordLogin, PermissionScope, PrincipalKind, RoleAssignmentRequest,
    RoleKey,
};
use crate::credentials::models::{
    RoleActivation, RoleActivationRequest, RoleApproval, RoleDecision,
};
use crate::credentials::tests::location_helpers::create_location;
use crate::credentials::tests::test_types::RoleFixture;
use crate::runs::tests::helpers::snapshot;
use crate::snapshots::main::publish_snapshot::publish_snapshot;
use crate::store::errors::ControlError;
use crate::store::main::{begin::begin, commit::commit, query::query};
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

pub(crate) async fn assign(
    store: &ControlStore,
    fixture: (&AuditContext, &IssuedSession),
    request: RoleAssignmentRequest,
) {
    assert!(
        assign_builtin_role(
            store,
            (fixture.0, credentials(fixture.1)),
            &request,
            &AdministratorAuthentication::Password
        )
        .await
        .expect("role assignment")
    );
}

/// One grant over the `tag:orders` selector, resolved to these snapshot nodes.
pub(crate) fn orders_grant(permissions: Value, resolved_nodes: &[&str]) -> Value {
    json!({"nodes": ["tag:orders"], "permissions": permissions, "resolved_nodes": {"tag:orders": resolved_nodes}})
}

/// Ad-hoc and SQL permissions over one selector, resolved to these snapshot nodes.
pub(crate) fn developer_grants(selector: &str, resolved_nodes: &[&str]) -> Value {
    json!([{"nodes": [selector], "permissions": ["run.adhoc", "view.sql"], "resolved_nodes": {selector: resolved_nodes}}])
}

/// Publish one generation whose `roles` document gives the support role these grants.
pub(crate) async fn publish_support_role(
    store: &ControlStore,
    location_id: Uuid,
    (generation, grants): (i64, Value),
) {
    let mut input = snapshot(location_id);
    input.generation = generation;
    input.documents.insert(
        "roles".into(),
        json!({SUPPORT_ROLE: {"description": "Answer order questions", "grants": grants}}),
    );
    publish_snapshot(store, &input)
        .await
        .expect("publish roles");
}

/// Administrators by count, then a member holding the published, still unapproved support role.
pub(crate) async fn role_fixture(store: &ControlStore, administrators: usize) -> RoleFixture {
    let context = AuditContext {
        actor: "local-control-store-owner".into(),
        granting_role: "control-store-owner".into(),
        request_id: Uuid::now_v7(),
        ip_address: Some("127.0.0.1".parse().expect("local peer")),
    };
    let first = bootstrap_user(store, &context, "orders-admin")
        .await
        .expect("bootstrap");
    let mut sessions = vec![signed_in(store, &context, (first, "orders-admin")).await];
    for index in 1..administrators {
        let name = format!("fulfillment-admin-{index}");
        let principal = create_principal(store, &context, (&name, PrincipalKind::Person))
            .await
            .expect("administrator");
        let session = signed_in(store, &context, (principal, &name)).await;
        assign(
            store,
            (&context, &sessions[0]),
            RoleAssignmentRequest {
                principal_id: principal,
                role: BuiltinRole::Admin.into(),
                location_id: None,
                target: None,
                revoke: false,
            },
        )
        .await;
        sessions.push(session);
    }
    let member = create_principal(store, &context, ("support-person", PrincipalKind::Person))
        .await
        .expect("member");
    let member_session = signed_in(store, &context, (member, "support-person")).await;
    let location_id = create_location(store, "orders", "orders-admin")
        .await
        .expect("location");
    publish_support_role(
        store,
        location_id,
        (1, json!([orders_grant(json!(["view"]), &["orders"])])),
    )
    .await;
    assign(
        store,
        (&context, &sessions[0]),
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
        administrators: sessions,
        member: member_session,
        member_id: member,
    }
}

/// The role that grants the member this action on these nodes, or the denial reason.
pub(crate) async fn authorize_member(
    store: &ControlStore,
    fixture: &RoleFixture,
    (action, nodes): (Action, &[&str]),
) -> Result<String, String> {
    let scope = PermissionScope {
        action,
        location_id: Some(fixture.location_id),
        target: Some("development".into()),
        nodes: nodes.iter().map(|node| (*node).into()).collect(),
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
    decision
        .map(|(_, context)| context.granting_role)
        .map_err(|error| {
            let reason = error.to_string();
            let reason = reason.trim_start_matches("control-plane operation denied: ");
            reason
                .split("; decision ")
                .next()
                .unwrap_or_default()
                .to_owned()
        })
}

/// The role that grants the member this location-scoped action, or `None` when denied.
pub(crate) async fn granting_role(
    store: &ControlStore,
    fixture: &RoleFixture,
    action: Action,
) -> Option<String> {
    authorize_member(store, fixture, (action, &[])).await.ok()
}

/// Role approval audit facts in commit order: action and recorded outcome.
pub(crate) async fn role_audit(store: &ControlStore) -> Vec<(String, Value)> {
    let transaction = begin(store, TransactionMode::Read).await.expect("read");
    let (transaction, rows) = query(
        transaction,
        "SELECT action, CAST(after_json AS TEXT) AS after FROM sqb_security_audit \
         WHERE action IN ('role.activation_requested','role.approved','role.rejected') ORDER BY storage_id",
        vec![],
    )
    .await
    .expect("role audit");
    commit(transaction).await.expect("commit");
    rows.iter()
        .map(|row| {
            (
                row["action"].as_str().unwrap_or_default().into(),
                serde_json::from_str(row["after"].as_str().unwrap_or("null")).expect("after json"),
            )
        })
        .collect()
}

pub(crate) async fn try_request(
    store: &ControlStore,
    fixture: &RoleFixture,
    requester: usize,
) -> Result<RoleActivation, ControlError> {
    request_role_activation(
        store,
        (
            &fixture.context,
            credentials(&fixture.administrators[requester]),
        ),
        &RoleActivationRequest {
            location_id: fixture.location_id,
            target: "development".into(),
            role_key: SUPPORT_ROLE.into(),
            reason: "Support answers order questions".into(),
        },
    )
    .await
}

pub(crate) async fn request(
    store: &ControlStore,
    fixture: &RoleFixture,
    requester: usize,
) -> RoleActivation {
    try_request(store, fixture, requester)
        .await
        .expect("activation request")
}

/// Request and approve the support role's current definition through a second administrator.
pub(crate) async fn activate(store: &ControlStore, fixture: &RoleFixture) {
    let activation = request(store, fixture, 0).await;
    assert!(
        approve(store, fixture, (1, &activation, false))
            .await
            .expect("approve")
    );
}

pub(crate) async fn approve(
    store: &ControlStore,
    fixture: &RoleFixture,
    (approver, activation, confirm): (usize, &RoleActivation, bool),
) -> Result<bool, ControlError> {
    decide_role_approval(
        store,
        (
            &fixture.context,
            credentials(&fixture.administrators[approver]),
        ),
        &RoleApproval {
            request_id: activation.request_id,
            decision: RoleDecision::Approved,
            reason: "Reviewed the support grants".into(),
            confirm_self_approval: confirm,
        },
    )
    .await
}
