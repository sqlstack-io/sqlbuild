//! Repair and rollback against facts produced by real authenticated catalog HTTP journeys.

use crate::store::main::{begin::begin, commit::commit, query::query};
use crate::store::models::{ControlStore, TransactionMode};
use crate::store::tests::fault_helpers::{install_fault, remove_fault};
use crate::store::tests::fault_types::TransactionFault;
use crate::store::tests::logical_state::logical_state;
use crate::warehouse_metadata::_helpers::catalog::rebuild_catalog_projection;

pub(crate) async fn catalog_replay_contract(store: &ControlStore) {
    let expected = logical_state(store).await;
    let transaction = begin(store, TransactionMode::Write)
        .await
        .expect("damage cut");
    let (transaction, _) = query(transaction, "DELETE FROM sqb_warehouse_catalog", vec![])
        .await
        .expect("missing catalog");
    commit(transaction).await.expect("damage commit");
    let damaged = logical_state(store).await;
    let fault = TransactionFault {
        description: "catalog replay rollback after projection clear",
        table: "sqb_warehouse_catalog",
        operation: "INSERT",
        condition: "",
        sqlite_body: "SELECT RAISE(ABORT,'injected catalog replay failure')",
        postgres_body: "RAISE EXCEPTION 'injected catalog replay failure'",
    };
    install_fault(store, &fault).await;
    let error = rebuild_catalog_projection(store)
        .await
        .expect_err("injected fault");
    assert!(
        error
            .to_string()
            .contains("injected catalog replay failure"),
        "{error}"
    );
    assert_eq!(logical_state(store).await, damaged);
    remove_fault(store, &fault).await;
    rebuild_catalog_projection(store)
        .await
        .expect("repair catalog");
    assert_eq!(logical_state(store).await, expected);
    rebuild_catalog_projection(store)
        .await
        .expect("idempotent repair");
    assert_eq!(logical_state(store).await, expected);

    let transaction = begin(store, TransactionMode::Write)
        .await
        .expect("source fault");
    let (transaction, _) = query(
        transaction,
        "UPDATE recall_events SET event_schema_version=999 WHERE event_type='warehouse_catalog_changed'",
        vec![],
    )
    .await
    .expect("unsupported source");
    commit(transaction).await.expect("source fault commit");
    let unsupported = logical_state(store).await;
    assert!(
        rebuild_catalog_projection(store)
            .await
            .expect_err("unsupported schema")
            .to_string()
            .contains("unsupported catalog fact schema")
    );
    assert_eq!(logical_state(store).await, unsupported);
    let transaction = begin(store, TransactionMode::Write)
        .await
        .expect("restore source");
    let (transaction, _) = query(
        transaction,
        "UPDATE recall_events SET event_schema_version=1 WHERE event_type='warehouse_catalog_changed'",
        vec![],
    )
    .await
    .expect("restore schema");
    commit(transaction).await.expect("restored source commit");
    rebuild_catalog_projection(store)
        .await
        .expect("retry restored source");
    assert_eq!(logical_state(store).await, expected);
}
