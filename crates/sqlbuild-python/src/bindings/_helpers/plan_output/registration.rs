//! Register the text and JSON plan output and the cursor safety serializers.

use pyo3::prelude::{Bound, PyModule, PyResult};

use crate::bindings::_helpers::plan_output::display;
use crate::bindings::_helpers::plan_output::migrations;
use crate::bindings::_helpers::plan_output::plan_json;
use crate::bindings::_helpers::plan_output::plan_text;
use crate::bindings::_helpers::plan_output::safety;

pub(crate) fn register(module: &Bound<'_, PyModule>) -> PyResult<()> {
    display::register(module)?;
    migrations::register(module)?;
    plan_json::register(module)?;
    plan_text::register(module)?;
    safety::register(module)
}
