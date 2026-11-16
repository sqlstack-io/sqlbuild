//! Whether the top-level `[connection]` is the implicit default target: no named connections or targets.

use pyo3::prelude::{Bound, PyAny, PyAnyMethods, PyResult};
use sqlbuild_python_base::records::main::fields::field::field as native_field;

/// Whether the top-level `[connection]` is the implicit default target: no named connections or targets.
pub fn connection_is_default_target(discovered: &Bound<'_, PyAny>) -> PyResult<bool> {
    for config in ["project_config", "local_config"] {
        let config: Bound<'_, PyAny> = native_field(discovered, config)?;
        for field in ["connections", "targets"] {
            if native_field(&config, field)?.is_truthy()? {
                return Ok(false);
            }
        }
    }
    Ok(true)
}
