//! A SQLBuild target's warehouse connection as the SQLLoad connection its loads write through.

use std::sync::Arc;

use pyo3::prelude::{Py, PyAny};
use sqlload::adapters::session::models::SessionConnection;
use sqlload::column_types::models::Warehouse;

use sqlbuild_load::warehouses::main::load_warehouse::load_warehouse;

use crate::sessions::models::PythonConnection;

/// The session connection for `adapter`'s warehouse, opening each session with `opener`; adapters SQLLoad can't load into fail naming the adapter.
pub fn session_connection(
    adapter: &str,
    database: Option<String>,
    (opener, description): (Py<PyAny>, String),
) -> Result<SessionConnection, String> {
    let (warehouse, motherduck): (Warehouse, bool) = load_warehouse(adapter)?;
    Ok(SessionConnection {
        warehouse,
        motherduck,
        database,
        caller: Arc::new(PythonConnection {
            opener,
            description,
        }),
    })
}
