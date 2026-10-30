//! Contention and expiry exercise public APIs against the real databases, not fake clocks.

use sqlbuild_orchestration::scheduler::main::acquire_lease::acquire_lease;
use sqlbuild_orchestration::scheduler::main::renew_lease::renew_lease;
use sqlbuild_orchestration::store::errors::ControlError;
use sqlbuild_orchestration::store::main::migrate_store::migrate_store;
use uuid::Uuid;

use crate::lease_helpers::expire_lease;
use crate::replica_helpers::second_store;
use crate::store_helpers::{close_store, close_test_store, open_test_store};
use crate::store_test_types::Backend;

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
