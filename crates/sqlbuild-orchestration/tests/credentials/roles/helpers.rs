//! Real people, sessions and published custom roles for custom role contracts.

use serde_json::{Value, json};
use uuid::Uuid;

use crate::authorization::main::{
    assign_builtin_role::assign_builtin_role, authorize_person::authorize_person,
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
use crate::declarations::main::change_person_trigger::change_person_trigger;
use crate::declarations::main::list_person_trigger_changes::list_person_trigger_changes;
use crate::declarations::main::list_triggers::list_triggers;
use crate::declarations::models::{PersonTriggerChange, TriggerChange, TriggerChangeRequest};
use crate::runs::main::list_person_runs::list_person_runs;
use crate::runs::tests::helpers::snapshot;
use crate::scheduler::tests::contracts::ticks::helpers::calendar_text;
use crate::snapshots::main::publish_snapshot::publish_snapshot;
use crate::store::errors::ControlError;
use crate::store::main::{begin::begin, commit::commit, query::query};
use crate::store::models::{ControlStore, TransactionMode};
use crate::warehouse_metadata::main::read_person_catalog::read_person_catalog;
use crate::warehouse_metadata::main::read_person_statistics::read_person_statistics;
use crate::warehouse_metadata::models::{CatalogQuery, StatisticsQuery, StatisticsRetention};

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

pub(crate) fn credentials(session: &IssuedSession) -> Uuid {
    session.identity.principal_id
}

pub(crate) async fn assign(
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
    publish_support_role(
        store,
        location_id,
        (1, json!([orders_grant(json!(["view"]), &["orders"])])),
    )
    .await;
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
    let decision = authorize_person(
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
        .map_err(denial)
}

/// A denial's reason without its decision ID, or the whole message for any other error.
fn denial(error: ControlError) -> String {
    let reason = error.to_string();
    let reason = reason.trim_start_matches("control-plane operation denied: ");
    reason
        .split("; decision ")
        .next()
        .unwrap_or_default()
        .to_owned()
}

/// Publish support role grants with a job that writes only customers and its daily trigger.
pub(crate) async fn publish_customers_trigger(
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
    input.documents.insert(
        "jobs".into(),
        json!({"customers_job": {"description": "Build customers", "selection": ["customers"]}}),
    );
    input.documents.insert("triggers".into(), json!({"customers_daily": {"description": "Refresh customers",
        "job_key": "customers_job", "starts_at": calendar_text(0),
        "schedule": {"kind": "calendar", "times": [calendar_text(0)], "refresh_at": "2030-01-01T00:00:00Z"}}}));
    publish_snapshot(store, &input)
        .await
        .expect("publish trigger");
}

/// The member's override of the location's only trigger, or the denial reason.
pub(crate) async fn member_trigger_change(
    store: &ControlStore,
    fixture: &RoleFixture,
    change: TriggerChange,
) -> Result<bool, String> {
    let trigger_id = list_triggers(store, fixture.location_id, "development")
        .await
        .expect("triggers")[0]
        .trigger_id;
    change_person_trigger(
        store,
        (&fixture.context, credentials(&fixture.member)),
        &TriggerChangeRequest {
            event_id: Uuid::now_v7(),
            trigger_id,
            actor: String::new(),
            reason: "Hold customer refreshes".into(),
            change,
        },
    )
    .await
    .map_err(denial)
}

/// The administrator's override of the location's only trigger, with its reason.
pub(crate) async fn administrator_trigger_change(
    store: &ControlStore,
    fixture: &RoleFixture,
    (change, reason): (TriggerChange, &str),
) -> Uuid {
    let trigger_id = list_triggers(store, fixture.location_id, "development")
        .await
        .expect("triggers")[0]
        .trigger_id;
    assert!(
        change_person_trigger(
            store,
            (&fixture.context, credentials(&fixture.administrator)),
            &TriggerChangeRequest {
                event_id: Uuid::now_v7(),
                trigger_id,
                actor: String::new(),
                reason: reason.into(),
                change,
            },
        )
        .await
        .expect("administrator change")
    );
    trigger_id
}

/// The member's read of a trigger's on/off history: the changes, `None` for an unknown trigger, or the denial.
pub(crate) async fn member_trigger_changes(
    store: &ControlStore,
    fixture: &RoleFixture,
    trigger_id: Uuid,
) -> Result<Option<Vec<PersonTriggerChange>>, String> {
    list_person_trigger_changes(
        store,
        (&fixture.context, credentials(&fixture.member)),
        ((fixture.location_id, "development"), (trigger_id, 100)),
        DefaultRole::None,
    )
    .await
    .map_err(denial)
}

/// The member's catalog, statistics and run listings of the fixture location: allowed or the denial.
pub(crate) async fn member_listings(
    store: &ControlStore,
    fixture: &RoleFixture,
) -> Vec<Result<(), String>> {
    let member = (&fixture.context, credentials(&fixture.member));
    let scope = (fixture.location_id, "development");
    let catalog = CatalogQuery {
        database: None,
        schema: "main".into(),
        after_observed_at: None,
        after_event_id: None,
        limit: None,
    };
    let statistics = StatisticsQuery {
        database: None,
        schema: "main".into(),
        name: "orders".into(),
        after_observed_at: None,
        after_event_id: None,
        limit: None,
    };
    let retention = StatisticsRetention::days(30).expect("retention");
    vec![
        read_person_catalog(
            store,
            member,
            (scope.0, scope.1, &catalog),
            DefaultRole::None,
        )
        .await
        .map(|_| ())
        .map_err(denial),
        read_person_statistics(
            store,
            member,
            (scope.0, scope.1, &statistics),
            (DefaultRole::None, retention),
        )
        .await
        .map(|_| ())
        .map_err(denial),
        list_person_runs(store, member, (scope.0, scope.1, 10), DefaultRole::None)
            .await
            .map(|_| ())
            .map_err(denial),
    ]
}

/// The role that grants the member this location-scoped action, or `None` when denied.
pub(crate) async fn granting_role(
    store: &ControlStore,
    fixture: &RoleFixture,
    action: Action,
) -> Option<String> {
    authorize_member(store, fixture, (action, &[])).await.ok()
}

/// Custom role entries of the snapshot change feed, by publication generation.
pub(crate) async fn role_changes(store: &ControlStore) -> Vec<(i64, String)> {
    let transaction = begin(store, TransactionMode::Read).await.expect("read");
    let (transaction, rows) = query(
        transaction,
        "SELECT s.generation, c.change_kind FROM sqb_snapshot_changes c JOIN sqb_snapshots s ON s.snapshot_id=c.snapshot_id \
         WHERE c.subject_key='role:orders_support' ORDER BY s.generation, c.change_seq",
        vec![],
    )
    .await
    .expect("role changes");
    commit(transaction).await.expect("commit");
    rows.iter()
        .map(|row| {
            (
                row["generation"].as_i64().unwrap_or_default(),
                row["change_kind"].as_str().unwrap_or_default().into(),
            )
        })
        .collect()
}
