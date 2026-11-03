//! Full-state acceptance keeps protected live ownership and source facts non-vacuous.

use crate::runs::main::replay_projections::replay_run_projections;
use crate::store::errors::ControlError;
use crate::store::main::{begin::begin, commit::commit, query::query};
use crate::store::models::{ControlStore, TransactionMode};
use crate::store::tests::fault_helpers::{install_fault, remove_fault};
use crate::store::tests::fault_types::TransactionFault;
use crate::store::tests::logical_state::logical_state;

pub(crate) async fn rebuild_run_projections(store: &ControlStore) -> Result<(), ControlError> {
    let transaction = begin(store, TransactionMode::Write).await?;
    commit(replay_run_projections(transaction).await?).await
}

pub(crate) async fn run_replay_contract(store: &ControlStore) {
    let expected = logical_state(store).await;
    let mut transaction = begin(store, TransactionMode::Write)
        .await
        .expect("damage cut");
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../benchmarks/duckshop/edge_cases/orchestration/projection_replay.json");
    let fixture: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(path).expect("projection replay fixture"))
            .expect("projection replay fixture");
    for sql in fixture["run_damage"].as_array().expect("run damage") {
        (transaction, _) = query(transaction, sql.as_str().expect("damage SQL"), vec![])
            .await
            .expect("corrupt derived state");
    }
    commit(transaction).await.expect("damage commit");
    let damaged = logical_state(store).await;
    assert_ne!(damaged, expected);
    let fault = TransactionFault {
        description: "node repair fails after resetting run metadata",
        table: "sqb_run_nodes",
        operation: "INSERT",
        condition: "",
        sqlite_body: "SELECT RAISE(ABORT,'injected run replay failure')",
        postgres_body: "RAISE EXCEPTION 'injected run replay failure'",
    };
    install_fault(store, &fault).await;
    let error = rebuild_run_projections(store)
        .await
        .expect_err("injected failure");
    assert!(
        error.to_string().contains("injected run replay failure"),
        "{error}"
    );
    assert_eq!(logical_state(store).await, damaged);
    remove_fault(store, &fault).await;
    rebuild_run_projections(store)
        .await
        .expect("repair runs and nodes");
    assert_eq!(logical_state(store).await, expected);
    rebuild_run_projections(store)
        .await
        .expect("idempotent replay");
    assert_eq!(logical_state(store).await, expected);
}
