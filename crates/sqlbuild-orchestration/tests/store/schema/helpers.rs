//! Row readers and a killable writer process shared by the control-store schema contracts.

use std::io::{BufRead, BufReader};
use std::path::Path;
use std::process::{Command, Stdio};

use crate::credentials::tests::location_helpers::create_location;
use crate::store::main::begin::begin;
use crate::store::main::commit::commit;
use crate::store::main::migrate_store::migrate_store;
use crate::store::main::open_store::open_store;
use crate::store::main::query::query;
use crate::store::models::{Parameter, TransactionMode};
use crate::store::tests::store_helpers::run;
use crate::store::types::Row;

const CHILD_STORE_VARIABLE: &str = "SQLBUILD_TEST_CRASH_STORE";
const ACKNOWLEDGED: &str = "committed ";

pub(crate) fn text(row: &Row, key: &str) -> String {
    row[key].as_str().expect("text column").to_owned()
}

/// In a `kill_writer_after` process, commit one location at a time and acknowledge each, forever.
pub(crate) fn write_until_killed_when_spawned() {
    let Ok(path) = std::env::var(CHILD_STORE_VARIABLE) else {
        return;
    };
    run(async {
        let store = open_store(&path).await.expect("open store");
        migrate_store(&store).await.expect("migrate store");
        let mut index: u64 = 0;
        loop {
            create_location(&store, &format!("orders-{index}"), "test-admin")
                .await
                .expect("create location");
            println!("{ACKNOWLEDGED}{index}");
            index += 1;
        }
    });
}

/// Kill a writer running `test` after `commits` acknowledgements; return them and the WAL bytes.
pub(crate) fn kill_writer_after(test: &str, path: &Path, commits: usize) -> (usize, u64) {
    let name = format!(
        "{}::{test}",
        module_path!()
            .split_once("::")
            .expect("crate path")
            .1
            .rsplit_once("::")
            .expect("schema module")
            .0
    );
    let mut child = Command::new(std::env::current_exe().expect("test binary"))
        .args([name.as_str(), "--exact", "--nocapture", "--test-threads=1"])
        .env(CHILD_STORE_VARIABLE, path)
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .expect("start writer process");
    let acknowledged = BufReader::new(child.stdout.take().expect("writer output"))
        .lines()
        .map(|line| line.expect("writer line"))
        .filter(|line| line.starts_with(ACKNOWLEDGED))
        .take(commits)
        .count();
    child.kill().expect("kill writer");
    child.wait().expect("reap writer");
    let mut wal = path.as_os_str().to_owned();
    wal.push("-wal");
    let wal_bytes = std::fs::metadata(wal).map_or(0, |metadata| metadata.len());
    (acknowledged, wal_bytes)
}

/// The number of locations a reopened store holds.
pub(crate) fn surviving_locations(path: &Path) -> usize {
    run(async {
        let store = open_store(path.to_str().expect("UTF-8 path"))
            .await
            .expect("reopen store");
        let transaction = begin(&store, TransactionMode::Read)
            .await
            .expect("begin read");
        let (transaction, rows) = query(
            transaction,
            "SELECT COUNT(*) AS locations FROM sqb_locations",
            vec![],
        )
        .await
        .expect("count locations");
        commit(transaction).await.expect("finish read");
        usize::try_from(rows[0]["locations"].as_i64().expect("count")).expect("count")
    })
}

/// Commit `commits` rows of `bytes` each, one transaction at a time with none overlapping; return the WAL bytes.
pub(crate) fn wal_bytes_after_sequential_writes(path: &Path, commits: usize, bytes: usize) -> u64 {
    run(async {
        let store = open_store(path.to_str().expect("UTF-8 path"))
            .await
            .expect("open store");
        let transaction = begin(&store, TransactionMode::Write).await.expect("begin");
        let (transaction, _) = query(transaction, "CREATE TABLE wal_probe (payload TEXT)", vec![])
            .await
            .expect("create probe table");
        commit(transaction).await.expect("commit table");
        for index in 0..commits {
            let transaction = begin(&store, TransactionMode::Write).await.expect("begin");
            let (transaction, _) = query(
                transaction,
                "INSERT INTO wal_probe (payload) VALUES ($1)",
                vec![Parameter::Text(format!("{index:0>bytes$}"))],
            )
            .await
            .expect("write probe row");
            commit(transaction).await.expect("commit probe row");
        }
        let mut wal = path.as_os_str().to_owned();
        wal.push("-wal");
        std::fs::metadata(wal).map_or(0, |metadata| metadata.len())
    })
}
