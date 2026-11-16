//! A compiled model's authored cursor inputs resolved into filter and watermark roles.

use pyo3::prelude::{Bound, PyAny, PyAnyMethods, PyResult};
use pyo3::types::{PyDict, PyDictMethods, PyList, PyListMethods, PyString};

pub(crate) const CURSOR_INPUTS_KEY: &str = "cursor_inputs";
const FILTER_ROLE: &str = "filter";
const WATERMARK_ROLE: &str = "watermark";
pub(crate) const WATERMARK_STRATEGY: &str = "watermark";
pub(crate) const ROLLING_WINDOW_STRATEGY: &str = "rolling_window";

/// Watermark-strategy cursor inputs: each relation's column under the roles it declares.
pub(crate) fn role_blocks<'py>(
    authored: &Bound<'py, PyDict>,
    (filter, watermark): (&Bound<'py, PyDict>, &Bound<'py, PyDict>),
) -> PyResult<()> {
    for (relation, block) in authored.iter() {
        let (true, Ok(block)) = (
            relation.is_instance_of::<PyString>(),
            block.downcast::<PyDict>(),
        ) else {
            continue;
        };
        let column: Option<Bound<'py, PyAny>> = block.get_item("column")?;
        let roles: Option<Bound<'py, PyAny>> = block.get_item("roles")?;
        let (Some(column), Some(roles)) = (column, roles) else {
            continue;
        };
        let (true, Ok(roles)) = (
            column.is_instance_of::<PyString>(),
            roles.downcast::<PyList>(),
        ) else {
            continue;
        };
        if roles.contains(FILTER_ROLE)? {
            filter.set_item(&relation, &column)?;
        }
        if roles.contains(WATERMARK_ROLE)? {
            watermark.set_item(&relation, &column)?;
        }
    }
    Ok(())
}

/// The string-to-string entries of an authored mapping.
pub(crate) fn string_map<'py>(authored: &Bound<'py, PyDict>) -> PyResult<Bound<'py, PyDict>> {
    let result: Bound<'py, PyDict> = PyDict::new(authored.py());
    for (key, item) in authored.iter() {
        if key.is_instance_of::<PyString>() && item.is_instance_of::<PyString>() {
            result.set_item(key, item)?;
        }
    }
    Ok(result)
}
