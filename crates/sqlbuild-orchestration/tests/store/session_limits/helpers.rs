//! Disposable PostgreSQL stores whose sessions run past their limits.

use std::time::Duration;

use crate::health::main::control_cause::control_cause;
use crate::health::models::ServerHealth;
use crate::platform::main::health::report_session_cut_offs::report_session_cut_offs;
use crate::store::main::migrate_store::migrate_store;
use crate::store::main::{begin::begin, commit::commit, query::query};
use crate::store::models::TransactionMode;
use crate::store::tests::session_limits::test_types::CutOffReport;
use crate::store::tests::store_helpers::{close_test_store, open_test_store};
use crate::store::tests::store_test_types::Backend;

/// The statement and idle-in-transaction limits a pooled session holds inside a `mode` transaction.
pub(super) async fn session_limits(mode: TransactionMode) -> Option<(String, String)> {
    let test_store = open_test_store(Backend::Postgres).await?;
    let transaction = begin(&test_store.store, mode).await.expect("begin");
    let (transaction, statement) = query(transaction, "SHOW statement_timeout", Vec::new())
        .await
        .expect("statement limit");
    let (transaction, idle) = query(
        transaction,
        "SHOW idle_in_transaction_session_timeout",
        Vec::new(),
    )
    .await
    .expect("idle limit");
    commit(transaction).await.expect("close");
    close_test_store(test_store).await;
    let setting = |rows: &[crate::store::types::Row], name: &str| {
        rows[0][name].as_str().unwrap_or_default().to_owned()
    };
    Some((
        setting(&statement, "statement_timeout"),
        setting(&idle, "idle_in_transaction_session_timeout"),
    ))
}

/// Run `session`'s statements, lowering a limit, pausing, then running one PostgreSQL cuts off, and report the cut-off as the health probe does.
pub(super) async fn cut_off_session(
    (limit, pause_ms, statement): (&str, u64, &str),
) -> Option<CutOffReport> {
    let test_store = open_test_store(Backend::Postgres).await?;
    let store = &test_store.store;
    migrate_store(store).await.expect("migrate");
    let health = ServerHealth::new("test", false);
    health.attach_store(store);
    let transaction = begin(store, TransactionMode::Read).await.expect("begin");
    let (transaction, _) = query(transaction, limit, Vec::new())
        .await
        .expect("lower the limit");
    tokio::time::sleep(Duration::from_millis(pause_ms)).await;
    let failed = query(transaction, statement, Vec::new())
        .await
        .expect_err("the control database cuts the session off");
    let cause = control_cause(&failed, (&Default::default(), false)).map(|cause| cause.case);
    let reported = report_session_cut_offs(store, &health)
        .await
        .expect("report the cut-off session");
    let health_case = health.control_cause().map(|cause| cause.case);
    let transaction = begin(store, TransactionMode::Read)
        .await
        .expect("read events");
    let (transaction, rows) = query(
        transaction,
        "SELECT payload FROM recall_events WHERE event_type='platform_control_session_cut_off'",
        Vec::new(),
    )
    .await
    .expect("platform events");
    commit(transaction).await.expect("close");
    close_test_store(test_store).await;
    Some(CutOffReport {
        cause,
        reported,
        health: health_case,
        events: rows
            .iter()
            .filter_map(|row| row["payload"]["platform"]["idle"].as_bool())
            .collect(),
    })
}
