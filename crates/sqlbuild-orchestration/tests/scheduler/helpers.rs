//! Contention and expiry exercise scheduler ownership against real databases, not fake clocks.

pub(crate) use crate::scheduler::main::_acquire_lease::acquire_lease;
pub(crate) use crate::scheduler::main::_evaluate_trigger::evaluate_trigger;
pub(crate) use crate::scheduler::main::_scheduler_cycle::scheduler_cycle;
use crate::scheduler::main::renew_lease::renew_lease;
use crate::scheduler::models::SchedulerLease;
use crate::store::errors::ControlError;
use crate::store::main::migrate_store::migrate_store;
use crate::store::models::ControlStore;
use futures_util::{StreamExt, stream};
use uuid::Uuid;

use crate::store::tests::lease_helpers::expire_lease;
use crate::store::tests::replica_helpers::second_store;
use crate::store::tests::store_helpers::{close_store, close_test_store, open_test_store};
use crate::store::tests::store_test_types::Backend;

/// Renew `lease` every five seconds, as a running scheduler does, until a renewal fails.
pub(crate) fn hold_lease(
    store: &ControlStore,
    lease: &SchedulerLease,
) -> tokio::task::JoinHandle<()> {
    let store = store.clone();
    let renewals = stream::try_unfold(lease.clone(), move |lease| {
        let store = store.clone();
        async move {
            tokio::time::sleep(std::time::Duration::from_secs(5)).await;
            let started = std::time::Instant::now();
            renew_lease(&store, &lease)
                .await
                .map(|renewed| Some(((), renewed)))
                .inspect_err(|error| {
                    println!(
                        "lease holder stopped: renewal failed after {:?} with {error}; the lease was due to expire at {}",
                        started.elapsed(),
                        lease.expires_at
                    );
                })
        }
    });
    tokio::spawn(renewals.for_each(|_| async {}))
}

pub(crate) async fn leadership_contract(backend: Backend) -> Option<i64> {
    let test_store = open_test_store(backend).await?;
    let store = &test_store.store;
    migrate_store(store)
        .await
        .expect("migrate leadership store");
    let second = second_store(store).await;
    let (first, other) = tokio::join!(
        acquire_lease(store, Uuid::now_v7()),
        acquire_lease(&second, Uuid::now_v7())
    );
    let first = first.expect("first contender completes");
    let other = other.expect("second contender completes");
    assert_ne!(
        first.is_some(),
        other.is_some(),
        "exactly one concurrent contender is elected"
    );
    let lease = first.or(other).expect("one elected holder");
    let renewed = renew_lease(&second, &lease)
        .await
        .expect("live epoch renews");
    assert_eq!(renewed.epoch, lease.epoch);
    assert!(renewed.expires_at >= lease.expires_at);
    assert!(
        acquire_lease(store, lease.holder)
            .await
            .expect("duplicate acquisition completes")
            .is_none()
    );
    let mut forged = lease.clone();
    forged.holder = Uuid::now_v7();
    assert!(matches!(
        renew_lease(store, &forged).await,
        Err(ControlError::Fenced)
    ));
    expire_lease(store).await;
    assert!(
        matches!(renew_lease(store, &lease).await, Err(ControlError::Fenced)),
        "even an uncontested expired holder must reacquire a new epoch"
    );
    let successor = acquire_lease(&second, Uuid::now_v7())
        .await
        .expect("acquire expired lease")
        .expect("successor elected");
    assert!(matches!(
        renew_lease(store, &lease).await,
        Err(ControlError::Fenced)
    ));
    renew_lease(store, &successor)
        .await
        .expect("successor owns the current fence");
    let epoch = successor.epoch;
    close_store(second).await;
    close_test_store(test_store).await;
    Some(epoch)
}

/// The operator a test's manual run requests come from.
pub(crate) fn operator() -> crate::credentials::models::AuditContext {
    crate::credentials::models::AuditContext {
        actor: "test-operator".into(),
        granting_role: "control-store-owner".into(),
        request_id: Uuid::now_v7(),
        ip_address: None,
    }
}

/// A manual request for one run of `job_key` on the development target.
pub(crate) fn manual_run(location_id: Uuid, job_key: &str) -> crate::runs::models::JobRunRequest {
    crate::runs::models::JobRunRequest {
        run_id: Uuid::now_v7(),
        location_id,
        target: "development".into(),
        job_key: job_key.into(),
        reason: "Build it now".into(),
        priority: 0,
        window: None,
        full_refresh: false,
        select: Vec::new(),
        exclude: Vec::new(),
    }
}

/// Write every published job's definition back with the concurrency source an older release recorded.
pub(crate) async fn store_old_job_definitions(store: &ControlStore, source: &str) {
    use crate::runs::tests::helpers::{json_column, rows};
    use crate::store::main::{begin::begin, commit::commit, query::query};
    use crate::store::models::{Parameter, TransactionMode};

    let jobs = rows(
        store,
        "SELECT job_key, definition_json FROM sqb_jobs",
        vec![],
    )
    .await;
    let mut transaction = begin(store, TransactionMode::Write).await.expect("begin");
    for job in jobs {
        let mut definition: serde_json::Value = json_column(&job["definition_json"]);
        definition["resolved_concurrency"]["source"] = serde_json::json!(source);
        (transaction, _) = query(
            transaction,
            "UPDATE sqb_jobs SET definition_json=$1 WHERE job_key=$2",
            vec![
                Parameter::Json(definition),
                Parameter::Text(job["job_key"].as_str().expect("job key").to_owned()),
            ],
        )
        .await
        .expect("store the old definition");
    }
    commit(transaction).await.expect("commit");
}

/// Point each snapshot's `jobs` and `groups` documents at old-form bodies under hashes of their own, as an older release stored them.
pub(crate) async fn store_old_documents(store: &ControlStore, source: &str) {
    use crate::runs::tests::helpers::rows;
    use crate::store::main::{
        begin::begin, commit::commit, compress::compress, decompress::decompress, query::query,
    };
    use crate::store::models::{Parameter, TransactionMode};

    let documents = rows(
        store,
        "SELECT b.document_hash, b.body FROM sqb_snapshot_documents d \
         JOIN sqb_documents b ON b.document_hash=d.document_hash WHERE d.kind IN ('jobs', 'groups')",
        vec![],
    )
    .await;
    let mut transaction = begin(store, TransactionMode::Write).await.expect("begin");
    for document in documents {
        let mut body: serde_json::Value =
            serde_json::from_slice(&decompress(&document["body"]).expect("body")).expect("JSON");
        for job in body.as_object_mut().expect("jobs or groups").values_mut() {
            job["resolved_concurrency"]["source"] = serde_json::json!(source);
        }
        let hash = document["document_hash"].as_str().expect("hash").to_owned();
        let old_hash = format!("{hash}-published-before-0004");
        (transaction, _) = query(
            transaction,
            "INSERT INTO sqb_documents (document_hash, body) VALUES ($1, $2)",
            vec![
                Parameter::Text(old_hash.clone()),
                Parameter::Bytes(
                    compress(&serde_json::to_vec(&body).expect("JSON")).expect("gzip"),
                ),
            ],
        )
        .await
        .expect("store the old document");
        (transaction, _) = query(
            transaction,
            "UPDATE sqb_snapshot_documents SET document_hash=$1 WHERE document_hash=$2",
            vec![Parameter::Text(old_hash), Parameter::Text(hash)],
        )
        .await
        .expect("point the snapshots at it");
    }
    commit(transaction).await.expect("commit");
}

/// A control-store result as a test compares it: `ok`, or the error as `Invalid("...")` and the like.
pub(crate) fn outcome<T>(result: Result<T, ControlError>) -> String {
    result.map_or_else(|error| format!("{error:?}"), |_| "ok".to_owned())
}

/// The scheduled orders snapshot with job `orders`, a group `nightly_group` of it and `triggers`, as a release before 0004 published it once its source is stored.
pub(crate) fn upgrade_snapshot(
    location_id: Uuid,
    triggers: serde_json::Value,
) -> crate::snapshots::models::SnapshotInput {
    let mut input = crate::scheduler::tests::contracts::ticks::helpers::snapshot(location_id);
    input.documents.insert(
        "jobs".into(),
        serde_json::json!({"orders": {"description": "Build orders", "selection": ["orders"],
            "resolved_concurrency": {"limit": 4}}}),
    );
    input.documents.insert(
        "groups".into(),
        serde_json::json!({"nightly_group": {"description": "Everything nightly",
            "selection": ["orders"], "jobs": {"orders": []}, "resolved_concurrency": {"limit": 4}}}),
    );
    input.documents.insert("triggers".into(), triggers);
    input
}
