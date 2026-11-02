//! Independent database handles for multi-replica integration contracts.

use crate::store::main::open_store::open_store;
use crate::store::models::ControlStore;
use sqlx_core::connection::ConnectOptions;

pub(crate) async fn second_store(store: &ControlStore) -> ControlStore {
    open_store(&store_url(store))
        .await
        .expect("open independent control-store handle")
}

pub(crate) fn store_url(store: &ControlStore) -> String {
    let ControlStore::Postgres(pool) = store else {
        let ControlStore::Sqlite(path, _) = store else {
            unreachable!()
        };
        return path.to_str().expect("UTF-8 test path").to_owned();
    };
    let options = pool.connect_options();
    let mut url = options.to_url_lossy();
    // SQLx's lossy URL omits startup options, including the isolated schema search path.
    url.query_pairs_mut()
        .extend_pairs(options.get_options().map(|startup| ("options", startup)));
    url.to_string()
}
