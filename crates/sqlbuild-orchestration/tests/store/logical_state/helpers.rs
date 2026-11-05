//! Hash complete logical state without printing credential or evidence rows on a failure.

use std::collections::BTreeMap;

use sqlbuild_recall::records::main::sha256_hex::sha256_hex;

use crate::store::main::{begin::begin, commit::commit, query::query};
use crate::store::models::{ControlStore, TransactionMode};

pub(crate) async fn logical_state(store: &ControlStore) -> BTreeMap<String, String> {
    let transaction = begin(store, TransactionMode::Read)
        .await
        .expect("state cut");
    let sql = [
        "SELECT name FROM sqlite_master WHERE type='table' ORDER BY name",
        "SELECT table_name AS name FROM information_schema.tables WHERE table_schema=current_schema() AND table_type='BASE TABLE' ORDER BY table_name",
    ][usize::from(matches!(store, ControlStore::Postgres(..)))];
    let (mut transaction, tables) = query(transaction, sql, vec![]).await.expect("tables");
    let mut result = BTreeMap::new();
    for table in tables {
        let name = table["name"].as_str().expect("table name");
        let rows;
        (transaction, rows) = query(
            transaction,
            &format!("SELECT * FROM \"{}\"", name.replace('"', "\"\"")),
            vec![],
        )
        .await
        .expect("logical rows");
        let mut rows: Vec<String> = rows
            .iter()
            .map(|row| serde_json::to_string(row).expect("canonical row"))
            .collect();
        rows.sort();
        result.insert(
            name.to_owned(),
            sha256_hex(&serde_json::to_vec(&rows).expect("rows")),
        );
    }
    commit(transaction).await.expect("finish state cut");
    result
}
