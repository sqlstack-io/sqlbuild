//! Project-wide checks of maintained models on the effective adapter.

use pyo3::prelude::{Bound, PyAny, PyAnyMethods, PyResult, Python};

use sqlbuild_analysis::assembly::maintained_checks::constants::MAINTAINED_NEVER_REFRESHED_CODE;
use sqlbuild_analysis::assembly::maintained_checks::main::never_refreshed_maintained::never_refreshed_maintained;
use sqlbuild_analysis::assembly::maintained_checks::models::MaintainedModel;
use sqlbuild_python_base::boundary::main::optional_text::optional_text;
use sqlbuild_python_base::records::main::construct::enum_member::enum_member;
use sqlbuild_python_base::records::main::fields::field::field as native_field;
use sqlbuild_python_base::records::types::RecordId;
use sqlbuild_python_front::model_config::main::config::config_text::config_text;
use sqlbuild_python_front::model_config::main::config::config_value::config_value;
use sqlbuild_python_front::model_config::main::config::model_values::model_values;
use sqlbuild_python_front::model_config::types::ConfigValues;

use crate::compile_driver::_helpers::setup::targets::planner_error;

const SNOWFLAKE_ADAPTER: &str = "snowflake";
const MAINTAINED_MATERIALIZATION: &str = "maintained";
const REFRESH_KEY: &str = "refresh";

/// Raise when a Snowflake maintained model has no refresh and no maintained model reads it.
pub(crate) fn maintained_refresh(
    adapter_name: &Bound<'_, PyAny>,
    project: &Bound<'_, PyAny>,
) -> PyResult<()> {
    if optional_text(adapter_name)?.as_deref() != Some(SNOWFLAKE_ADAPTER) {
        return Ok(());
    }
    let py: Python<'_> = project.py();
    let model_type: Bound<'_, PyAny> = enum_member(py, RecordId::CompiledResourceType, "MODEL")?;
    let mut maintained: Vec<MaintainedModel> = Vec::new();
    for model in native_field(project, "models")?.try_iter()? {
        let model: Bound<'_, PyAny> = model?;
        let values: ConfigValues<'_> = model_values(&model)?;
        if config_text(py, &values, "materialized")?.as_deref() != Some(MAINTAINED_MATERIALIZATION)
        {
            continue;
        }
        let mut reads: Vec<String> = Vec::new();
        for key in native_field(&model, "deps")?.try_iter()? {
            let key: Bound<'_, PyAny> = key?;
            if native_field(&key, "resource_type")?.eq(&model_type)? {
                reads.push(native_field(&key, "name")?.extract()?);
            }
        }
        maintained.push(MaintainedModel {
            name: native_field(&model, "name")?.extract()?,
            has_refresh: !config_value(py, &values, REFRESH_KEY)?.is_none(),
            reads,
        });
    }
    let Some(name) = never_refreshed_maintained(&maintained) else {
        return Ok(());
    };
    Err(planner_error(
        py,
        format!(
            "maintained model '{name}' has no refresh and no maintained model reads it, so \
             Snowflake would never refresh it; add refresh (lag 1h)"
        ),
        MAINTAINED_NEVER_REFRESHED_CODE,
        Some(format!(
            "set how stale '{name}' may get in its MODEL header:\n            refresh (lag 1h),"
        )),
    ))
}
