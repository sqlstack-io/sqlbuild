//! Discovered project inputs with every `for_each` asset expanded from the process's discovery source.

use std::sync::PoisonError;

use pyo3::prelude::{Bound, PyAny, PyResult, Python};
use pyo3::types::PyTuple;
use sqlbuild_orchestration::discovered_sets::types::DiscoveredSets;
use sqlbuild_python_base::native_host::main::within_identity_pass::within_identity_pass;
use sqlbuild_python_base::records::main::fields::replaced::replaced;

use crate::discovered_sets::_helpers::item_records::expanded_assets;
use crate::discovered_sets::_helpers::live_sets::resolved_sets;
use crate::discovered_sets::_helpers::state::{SOURCE, USED};
use crate::discovered_sets::models::DiscoverySource;

/// `inputs` with each `for_each` asset as one asset per item of its set, or as its placeholder.
pub fn with_discovered_sets<'py>(inputs: &Bound<'py, PyAny>) -> PyResult<Bound<'py, PyAny>> {
    let py: Python<'py> = inputs.py();
    let source: DiscoverySource = SOURCE
        .lock()
        .unwrap_or_else(PoisonError::into_inner)
        .clone()
        .unwrap_or_default();
    let sets: DiscoveredSets = resolved_sets(inputs, &source)?;
    let assets: Vec<Bound<'py, PyAny>> =
        within_identity_pass(py, || expanded_assets(inputs, &sets))?;
    *USED.lock().unwrap_or_else(PoisonError::into_inner) = Some(sets);
    replaced(
        inputs,
        &[("asset_functions", PyTuple::new(py, assets)?.into_any())],
    )
}
