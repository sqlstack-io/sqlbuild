//! Independent cleanup also expires samples from removed locations and namespaces.

use std::time::Duration;

use crate::store::errors::ControlError;
use crate::store::models::ControlStore;
use crate::warehouse_metadata::_helpers::statistics::prune_statistics;
use crate::warehouse_metadata::constants::MAX_HISTORY_RECORDS;
use crate::warehouse_metadata::models::StatisticsRetention;

pub async fn run_statistics_retention(
    store: &ControlStore,
    retention: StatisticsRetention,
) -> Result<(), ControlError> {
    let mut interval = tokio::time::interval(Duration::from_secs(60));
    interval.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
    loop {
        interval.tick().await;
        loop {
            match prune_statistics(store, retention).await {
                Ok(MAX_HISTORY_RECORDS) => tokio::task::yield_now().await,
                Ok(_) => break,
                Err(ControlError::Database(error)) if ControlError::transient_database(&error) => {
                    break;
                }
                Err(error) => return Err(error),
            }
        }
    }
}
