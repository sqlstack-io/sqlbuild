//! `{name: identity.version_hash}` of the discovered Python hooks.

use pyo3::prelude::{Bound, PyAny, PyAnyMethods, PyResult, Python};
use pyo3::types::{PyDict, PyDictMethods};
use sqlbuild_host::entries::models::UserCodeHost;
use sqlbuild_python_base::native_host::main::host_entry::host_entry;
use sqlbuild_python_base::records::main::fields::field::field as native_field;
use sqlbuild_python_base::records::main::fields::mapping_items::mapping_items;

/// `{name: identity.version_hash}` of the discovered Python hooks.
pub fn hook_version_hashes<'py>(
    hook_functions: &Bound<'py, PyAny>,
) -> PyResult<Bound<'py, PyDict>> {
    let py: Python<'py> = hook_functions.py();
    let hashes: Bound<'py, PyDict> = PyDict::new(py);
    let identities: Bound<'py, PyAny> =
        host_entry(py, UserCodeHost::BuildHookIdentities)?.call1((hook_functions,))?;
    for item in mapping_items(&identities)?.try_iter()? {
        let (name, identity): (Bound<'py, PyAny>, Bound<'py, PyAny>) = item?.extract()?;
        hashes.set_item(name, native_field(&identity, "version_hash")?)?;
    }
    Ok(hashes)
}
