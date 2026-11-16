//! Parse one selector token into Python's `ParsedSelector` or `PathSelector`.

use crate::planner_selection::_helpers::selectors::parsed_selector;
use crate::planner_selection::models::Carrier;
use pyo3::prelude::{Bound, PyAny, PyResult};
use pyo3::types::PyString;
use sqlbuild_analysis::graph::main::parse_selector::parse_selector;
use sqlbuild_analysis::graph::models::ParsedSelector;

/// Parse one selector token into Python's `ParsedSelector` or `PathSelector`.
pub fn parse_project_selector<'py>(raw: &Bound<'py, PyString>) -> PyResult<Bound<'py, PyAny>> {
    let carrier: Carrier<'py> = Carrier::single(raw)?;
    let parsed: ParsedSelector =
        parse_selector(&carrier.send(raw)?).map_err(|error| carrier.error(error))?;
    parsed_selector(&carrier, parsed)
}
