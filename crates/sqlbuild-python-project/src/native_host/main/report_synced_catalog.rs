//! Write what a plan from synced state found when it checked the catalog.

use pyo3::prelude::{Bound, PyAny, PyAnyMethods, PyResult};

use crate::native_host::_helpers::synced_catalog::write_stderr;
use crate::native_host::constants::SYNCED_CATALOG_ATTRIBUTE;

/// Write what the synced state checks found, once the plan is made.
pub fn report_synced_catalog(adapter: &Bound<'_, PyAny>) -> PyResult<()> {
    if !adapter.hasattr(SYNCED_CATALOG_ATTRIBUTE)? {
        return Ok(());
    }
    let catalog: Bound<'_, PyAny> = adapter.getattr(SYNCED_CATALOG_ATTRIBUTE)?;
    if catalog.is_none() {
        return Ok(());
    }
    let report: Option<String> = catalog.call_method0("report")?.extract()?;
    match report {
        Some(line) => write_stderr(adapter.py(), &format!("{line}\n")),
        None => Ok(()),
    }
}
