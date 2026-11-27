//! `[testing.local] allow` and `tests/overrides`: refused outside Snowflake projects, checked by SQLQuack in them.

use std::path::PathBuf;

use pyo3::prelude::{Bound, PyAny, PyAnyMethods, PyResult, Python};
use pyo3::types::{PyDict, PyDictMethods};
use sqlbuild_model_config::model_validation::constants::SNOWFLAKE_ADAPTER;
use sqlbuild_python_base::records::main::construct::new_error::new_error;
use sqlbuild_python_base::records::main::fields::field::field as native_field;
use sqlbuild_python_base::records::types::ErrorId;

use crate::compile_driver::constants::LOCAL_OVERRIDES_DIR;
use crate::compile_driver::models::ProjectConfiguration;
use crate::contracts::main::resolve_effective_adapter_name::resolve_effective_adapter_name;
use crate::native_host::main::local_test_inputs_problem::local_test_inputs_problem;

/// Refuse local test allowances outside a Snowflake project, and what SQLQuack can't use in one.
pub(crate) fn reject_invalid_local_test_inputs(
    config: &ProjectConfiguration<'_, '_>,
) -> PyResult<()> {
    let py: Python<'_> = config.project_dir.py();
    let allow: Bound<'_, PyAny> = native_field(config.project_config, "local_test_allow")?;
    let allowing: bool = !allow.cast::<PyDict>()?.is_empty();
    let adapter: String =
        resolve_effective_adapter_name(config.project_config, config.local_config)?.extract()?;
    if adapter != SNOWFLAKE_ADAPTER {
        return match allowing {
            true => Err(new_error(
                py,
                ErrorId::ProjectConfig,
                &format!(
                    "[testing.local] allow in sqlbuild_project.toml allows SQLQuack known \
                     differences when local tests convert Snowflake SQL, and this is a {adapter} \
                     project"
                ),
                (None, Some("remove [testing.local] allow")),
            )),
            false => Ok(()),
        };
    }
    let project_dir: PathBuf = config.project_dir.extract()?;
    if !allowing && !project_dir.join(LOCAL_OVERRIDES_DIR).is_dir() {
        return Ok(());
    }
    match local_test_inputs_problem(config.project_dir, &allow)? {
        Some(message) => Err(new_error(
            py,
            ErrorId::ProjectConfig,
            &message,
            (None, None),
        )),
        None => Ok(()),
    }
}
