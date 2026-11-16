//! The old-name view history of every recorded move in a schema that holds both state tables.

use crate::state_rows::_helpers::migrations::migration_io::access;
use crate::state_rows::_helpers::migrations::migration_io::stored_names;
use crate::state_rows::constants::{MODEL_KIND, OLD_NAME_KIND};
use crate::state_rows::main::migrations::layout::layout;
use crate::state_rows::main::migrations::old_name_views::old_name_views;
use crate::state_rows::main::migrations::read_events::read_events;
use crate::state_rows::models::StateAccess;
use pyo3::prelude::{Bound, PyAny, PyResult};
use pyo3::pyfunction;
use pyo3::types::{PyDict, PyTuple};
use sqlbuild_python_base::boundary::models::Request;

/// The old-name view history of every recorded move in a schema that holds both state tables.
#[pyfunction(name = "read_old_name_view_histories")]
#[pyo3(signature = (**request))]
pub fn read_histories<'py>(request: Option<&Bound<'py, PyDict>>) -> PyResult<Bound<'py, PyTuple>> {
    let request = Request::new("read_old_name_view_histories", request)?;
    let access: StateAccess<'py> = access(&request)?;
    let location = || -> PyResult<(Bound<'py, PyAny>, Bound<'py, PyAny>)> {
        Ok((request.get("database")?, request.get("schema")?))
    };
    let stored: Option<Vec<String>> = stored_names(Some(request.get("stored_columns")?))?;
    let moves: Bound<'py, PyTuple> = read_events(&access, &layout(MODEL_KIND)?, location()?, None)?;
    let facts: Bound<'py, PyTuple> = read_events(
        &access,
        &layout(OLD_NAME_KIND)?,
        location()?,
        stored.as_deref(),
    )?;
    old_name_views(moves.as_any(), facts.as_any())
}
