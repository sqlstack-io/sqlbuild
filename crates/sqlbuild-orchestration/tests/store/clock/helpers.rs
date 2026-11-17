//! A settable database clock for one test task, so scheduling contracts step time instead of waiting.

use std::cell::Cell;
use std::future::Future;

use chrono::{DateTime, Utc};

tokio::task_local! {
    static DATABASE_NOW: Cell<DateTime<Utc>>;
}

/// Every transaction `future` begins reads the database clock from this task instead of the store.
pub(crate) async fn with_database_clock<T>(
    start: DateTime<Utc>,
    future: impl Future<Output = T>,
) -> T {
    DATABASE_NOW.scope(Cell::new(start), future).await
}

pub(crate) fn set_database_clock(now: DateTime<Utc>) {
    DATABASE_NOW.with(|clock| clock.set(now));
}

pub(crate) fn database_clock() -> Option<DateTime<Utc>> {
    DATABASE_NOW.try_with(Cell::get).ok()
}
