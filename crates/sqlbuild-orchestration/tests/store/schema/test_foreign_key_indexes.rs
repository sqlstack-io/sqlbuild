//! Every foreign key is indexed on its referencing columns, so deleting a parent stays bounded.

use std::collections::{BTreeMap, BTreeSet};

use crate::store::main::begin::begin;
use crate::store::main::commit::commit;
use crate::store::main::migrate_store::migrate_store;
use crate::store::main::query::query;
use crate::store::models::TransactionMode;
use crate::store::tests::schema::helpers::text;
use crate::store::tests::schema::test_types::ForeignKeyIndexTestCase;
use crate::store::tests::store_helpers::{close_test_store, open_test_store, run};
use crate::store::tests::store_test_types::Backend;

const FOREIGN_KEYS: &str = "SELECT m.name AS table_name, f.id AS key_id, f.\"from\" AS column_name \
    FROM sqlite_master m JOIN pragma_foreign_key_list(m.name) f WHERE m.type = 'table'";
const INDEX_COLUMNS: &str = "SELECT m.name AS table_name, l.name AS index_name, \
    l.\"unique\" AS is_unique, i.seqno AS position, COALESCE(i.name, '') AS column_name \
    FROM sqlite_master m JOIN pragma_index_list(m.name) l JOIN pragma_index_info(l.name) i \
    WHERE m.type = 'table' ORDER BY m.name, l.name, i.seqno";

#[test]
fn given_control_store_schema_when_listing_foreign_keys_then_each_is_indexed() {
    let test_cases = [ForeignKeyIndexTestCase {
        description: "SQLite schema",
        backend: Backend::Sqlite,
        expected_uncovered: Vec::new(),
    }];
    for test_case in test_cases {
        run(async {
            let Some(fixture) = open_test_store(test_case.backend).await else {
                return;
            };
            migrate_store(&fixture.store).await.expect("migrate store");
            let transaction = begin(&fixture.store, TransactionMode::Read)
                .await
                .expect("begin read");
            let (transaction, keys) = query(transaction, FOREIGN_KEYS, vec![])
                .await
                .expect("list foreign keys");
            let (transaction, columns) = query(transaction, INDEX_COLUMNS, vec![])
                .await
                .expect("list index columns");
            commit(transaction).await.expect("finish read");

            let mut foreign_keys: BTreeMap<(String, i64), BTreeSet<String>> = BTreeMap::new();
            for row in &keys {
                foreign_keys
                    .entry((
                        text(row, "table_name"),
                        row["key_id"].as_i64().expect("key id"),
                    ))
                    .or_default()
                    .insert(text(row, "column_name"));
            }
            let mut indexes: BTreeMap<(String, String), (bool, Vec<String>)> = BTreeMap::new();
            for row in &columns {
                let entry = indexes
                    .entry((text(row, "table_name"), text(row, "index_name")))
                    .or_insert((row["is_unique"].as_i64() == Some(1), Vec::new()));
                entry.1.push(text(row, "column_name"));
            }
            let uncovered: Vec<(String, BTreeSet<String>)> = foreign_keys
                .into_iter()
                .filter(|((table, _), referencing)| {
                    !indexes.iter().any(|((index_table, _), (unique, indexed))| {
                        index_table == table
                            && (indexed.len() >= referencing.len()
                                && indexed[..referencing.len()]
                                    .iter()
                                    .all(|column| referencing.contains(column))
                                || *unique
                                    && indexed.iter().all(|column| referencing.contains(column)))
                    })
                })
                .map(|((table, _), referencing)| (table, referencing))
                .collect();
            assert_eq!(
                uncovered, test_case.expected_uncovered,
                "{}: foreign keys without an index on their referencing columns",
                test_case.description
            );
            close_test_store(fixture).await;
        });
    }
}
