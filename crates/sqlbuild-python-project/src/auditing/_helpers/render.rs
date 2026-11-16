//! Audit SQL with its reference calls replaced by relation names or overrides.

use pyo3::prelude::{Bound, PyAny, PyAnyMethods, PyResult};

use sqlbuild_python_base::records::main::fields::field::field as native_field;
use sqlbuild_python_base::records::main::fields::mapping_get::mapping_get;

pub(crate) const REF_FUNCTION: &str = "__ref";
pub(crate) const SEED_FUNCTION: &str = "__seed";
pub(crate) const SOURCE_FUNCTION: &str = "__source";

/// `mapping.get(name)`, or `None` when absent or `None`.
pub(crate) fn lookup<'py>(
    mapping: &Bound<'py, PyAny>,
    name: &str,
) -> PyResult<Option<Bound<'py, PyAny>>> {
    let value: Bound<'py, PyAny> = mapping_get(mapping, name)?;
    Ok(Some(value).filter(|value| !value.is_none()))
}

/// The qualified name of the relation location `mapping` holds for `name`, if any.
pub(crate) fn qualified<'py>(
    mapping: &Bound<'py, PyAny>,
    name: &str,
) -> PyResult<Option<Bound<'py, PyAny>>> {
    let Some(location) = lookup(mapping, name)? else {
        return Ok(None);
    };
    let qualified: Bound<'py, PyAny> = native_field(&location, "qualified_name")?;
    Ok(Some(qualified).filter(|qualified| !qualified.is_none()))
}
