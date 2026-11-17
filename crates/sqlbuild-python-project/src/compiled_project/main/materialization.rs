//! The `materialized` config value when it is text, `table` when it is absent.

use crate::compiled_project::_helpers::project::MATERIALIZED_KEY;
use crate::compiled_project::_helpers::project::TABLE_MATERIALIZATION;
use pyo3::prelude::{Bound, PyResult};
use pyo3::types::{PyDict, PyDictMethods};
use sqlbuild_python_base::boundary::main::text_item::text_item;

/// The `materialized` config value when it is text, `table` when it is absent.
pub fn materialization(values: &Bound<'_, PyDict>) -> PyResult<Option<String>> {
    if values.get_item(MATERIALIZED_KEY)?.is_none() {
        return Ok(Some(TABLE_MATERIALIZATION.to_owned()));
    }
    text_item(values, MATERIALIZED_KEY)
}
