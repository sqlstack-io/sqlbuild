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
            renew_lease(&store, &lease)
                .await
                .map(|renewed| Some(((), renewed)))
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
