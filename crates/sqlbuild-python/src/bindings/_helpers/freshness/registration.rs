//! Register the native `sqb freshness` command, observation, planning and state functions.

use pyo3::prelude::{Bound, PyModule, PyResult};

use crate::bindings::_helpers::freshness::exception_types;
use crate::bindings::_helpers::freshness::freshness_observe;
use crate::bindings::_helpers::freshness::observation;
use crate::bindings::_helpers::freshness::planning;
use crate::bindings::_helpers::freshness::publication;
use crate::bindings::_helpers::freshness::state_io;

pub(crate) fn register(module: &Bound<'_, PyModule>) -> PyResult<()> {
    exception_types::register(module)?;
    freshness_observe::register(module)?;
    observation::register(module)?;
    planning::register(module)?;
    publication::register(module)?;
    state_io::register(module)
}
