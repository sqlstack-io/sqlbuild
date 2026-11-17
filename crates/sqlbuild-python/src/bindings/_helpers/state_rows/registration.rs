//! Register the native warehouse state row functions.

use pyo3::prelude::{Bound, PyModule, PyResult};

use sqlbuild_python_front::state_rows::main::event_rows::register_events::register_events;
use sqlbuild_python_front::state_rows::main::event_rows::register_freshness::register_freshness;
use sqlbuild_python_front::state_rows::main::migrations::register_migration_io::register_migration_io;
use sqlbuild_python_front::state_rows::main::migrations::register_migration_projections::register_migration_projections;
use sqlbuild_python_front::state_rows::main::migrations::register_migration_rows::register_migration_rows;
use sqlbuild_python_front::state_rows::main::tables::register_state_tables::register_state_tables;

pub(crate) fn register(module: &Bound<'_, PyModule>) -> PyResult<()> {
    register_events(module)?;
    register_freshness(module)?;
    register_migration_rows(module)?;
    register_migration_io(module)?;
    register_migration_projections(module)?;
    register_state_tables(module)?;
    Ok(())
}
