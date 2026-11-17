//! Attach the built tree to its public record, rather than reconstructing it from that view.

use crate::model_config::_helpers::config::typed_config::TYPED_VALUES_SLOT;
use crate::model_config::models::TypedConfigValues;
use crate::model_config::types::BuiltConfigValues;
use pyo3::prelude::{Bound, PyAny, PyResult};
use sqlbuild_python_base::records::main::fields::store_cached::store_cached;
use sqlbuild_python_base::records::models::NativeRecord;
use std::sync::{Arc, Mutex};

/// Attach the built tree to its public record, rather than reconstructing it from that view.
pub fn retain_values(config: &Bound<'_, PyAny>, built: Option<&BuiltConfigValues>) -> PyResult<()> {
    if let Some(built) = built
        && let Ok(record) = config.cast::<NativeRecord>()
    {
        let typed = Bound::new(
            config.py(),
            TypedConfigValues {
                built: Some(Arc::clone(built)),
                read: Mutex::new(Vec::new()),
            },
        )?;
        store_cached(record, TYPED_VALUES_SLOT, &typed.into_any().unbind())?;
    }
    Ok(())
}
