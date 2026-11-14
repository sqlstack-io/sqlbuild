//! Commits a killed process acknowledged survive, though closing a connection never checkpoints.

use crate::platform::_helpers::retention::wal_checkpoint;
use crate::store::main::open_store::open_store;
use crate::store::tests::schema::helpers::{
    kill_writer_after, surviving_locations, write_until_killed_when_spawned,
};
use crate::store::tests::schema::test_types::{CrashRecoveryTestCase, WalCheckpointTestCase};
use crate::store::tests::store_helpers::run;

#[test]
fn given_a_process_killed_between_commits_when_the_store_reopens_then_every_acknowledged_commit_survives()
 {
    write_until_killed_when_spawned();
    let test_cases = [
        CrashRecoveryTestCase {
            description: "killed after the first commits",
            expected_acknowledged: 5,
        },
        CrashRecoveryTestCase {
            description: "killed after many commits",
            expected_acknowledged: 200,
        },
    ];
    for test_case in test_cases {
        let directory = tempfile::tempdir().expect("crash store directory");
        let path = directory.path().join("control.sqlite3");

        let (acknowledged, wal_bytes) = kill_writer_after(
            "test_crash_recovery::given_a_process_killed_between_commits_when_the_store_reopens_then_every_acknowledged_commit_survives",
            &path,
            test_case.expected_acknowledged,
        );
        let survived = surviving_locations(&path);

        assert_eq!(
            acknowledged, test_case.expected_acknowledged,
            "{}: the writer was killed, not finished",
            test_case.description
        );
        assert!(
            wal_bytes > 0,
            "{}: the killed writer left its commits in the WAL",
            test_case.description
        );
        assert!(
            (test_case.expected_acknowledged..=test_case.expected_acknowledged + 1)
                .contains(&survived),
            "{}: {acknowledged} acknowledged, {survived} survived",
            test_case.description
        );
    }
}

#[test]
fn given_commits_left_in_the_wal_when_maintenance_checkpoints_then_the_wal_is_emptied_and_rows_remain()
 {
    write_until_killed_when_spawned();
    let test_cases = [WalCheckpointTestCase {
        description: "a killed writer's WAL",
        expected_acknowledged: 20,
        expected_wal_bytes: 0,
    }];
    for test_case in test_cases {
        let directory = tempfile::tempdir().expect("checkpoint store directory");
        let path = directory.path().join("control.sqlite3");
        let (acknowledged, left) = kill_writer_after(
            "test_crash_recovery::given_commits_left_in_the_wal_when_maintenance_checkpoints_then_the_wal_is_emptied_and_rows_remain",
            &path,
            test_case.expected_acknowledged,
        );

        let wal_bytes = run(async {
            let store = open_store(path.to_str().expect("UTF-8 path"))
                .await
                .expect("open store");
            wal_checkpoint(&store).await.expect("checkpoint");
            std::fs::metadata(directory.path().join("control.sqlite3-wal"))
                .expect("WAL file")
                .len()
        });
        let survived = surviving_locations(&path);

        assert_eq!(
            acknowledged, test_case.expected_acknowledged,
            "{}",
            test_case.description
        );
        assert!(left > 0, "{}: the WAL held commits", test_case.description);
        assert_eq!(
            wal_bytes, test_case.expected_wal_bytes,
            "{}",
            test_case.description
        );
        assert!(
            survived >= test_case.expected_acknowledged,
            "{}: {survived} survived",
            test_case.description
        );
    }
}
