//! `sqb admin` opens its store quietly, unless asked to say so or opening is slow.

use sqlbuild_orchestration::store::errors::ControlError;
use sqlbuild_orchestration::store::models::ControlStore;

use crate::argv::models::{Namespace, Stream};
use crate::native_commands::_helpers::command::decision::{
    control_database, control_database_variable, flag,
};
use crate::native_commands::_helpers::command::terminal::write_output;
use crate::native_commands::_helpers::location::execution::open_migrated_store;
use crate::native_commands::constants::{ADMIN_OPEN_NOTICE, OPENING_STORE, STORE_READY};
use crate::server_config::models::ServerSection;

/// Open and migrate the store, announcing it with `--verbose` or once it takes noticeably long.
pub(crate) async fn open_admin_store(namespace: &Namespace) -> Result<ControlStore, ControlError> {
    let url: String = control_database(
        namespace,
        &control_database_variable(namespace, &ServerSection::default()),
    )?;
    let opening = open_migrated_store(&url);
    tokio::pin!(opening);
    if !flag(namespace, "verbose") {
        tokio::select! {
            biased;
            opened = &mut opening => return opened,
            () = tokio::time::sleep(ADMIN_OPEN_NOTICE) => {}
        }
    }
    drop(write_output(&[(Stream::Stderr, OPENING_STORE)]));
    let store: ControlStore = opening.await?;
    drop(write_output(&[(Stream::Stderr, STORE_READY)]));
    Ok(store)
}
