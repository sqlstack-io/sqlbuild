//! Per-model full-refresh resolution and full-rebuild refusals.

use pyo3::prelude::{Bound, PyAny, PyResult};
use sqlbuild_python_front::model_config::main::config::config_value::config_value;
use sqlbuild_python_front::model_config::main::config::model_values::model_values;

const FULL_REFRESH_KEY: &str = "full_refresh";

pub(crate) fn configured_full_refresh<'py>(
    model: &Bound<'py, PyAny>,
) -> PyResult<Bound<'py, PyAny>> {
    config_value(model.py(), &model_values(model)?, FULL_REFRESH_KEY)
}
