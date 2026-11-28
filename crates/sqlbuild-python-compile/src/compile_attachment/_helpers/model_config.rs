//! The native config builder, validator and inherited storage policies of one compile.

use pyo3::prelude::{Bound, PyAny, PyAnyMethods, PyResult, Python};
use pyo3::pyfunction;
use pyo3::types::{PyDict, PyDictMethods, PyTuple};

use crate::compile_attachment::_helpers::storage_policies::inherited_storage;
use crate::input_loops::main::table_backed::table_backed;
use sqlbuild_python_base::records::main::construct::new_record::new_record;
use sqlbuild_python_base::records::main::construct::new_record_with::new_record_with;
use sqlbuild_python_base::records::main::construct::record_class::record_class;
use sqlbuild_python_base::records::main::fields::field::field as native_field;
use sqlbuild_python_base::records::types::RecordId;
use sqlbuild_python_front::model_config::main::model_nodes::unicode_environment::unicode_environment;
use sqlbuild_python_front::model_config::models::NativeModelConfigBuilder;

/// Capture the project inputs every model's native config build and validation read.
#[pyfunction]
#[pyo3(signature = (*, inputs, validator))]
pub(crate) fn native_model_config_session<'py>(
    inputs: &Bound<'py, PyAny>,
    validator: &Bound<'py, PyAny>,
) -> PyResult<Bound<'py, PyAny>> {
    let py: Python<'py> = inputs.py();
    let project_config: Bound<'py, PyAny> = native_field(inputs, "project_config")?;
    let target: Bound<'py, PyAny> = native_field(inputs, "target_config")?;
    let declared: Bound<'py, PyAny> = native_field(&project_config, "materialization_defaults")?;
    let materialization_defaults: Bound<'py, PyAny> = if declared.is_truthy()? {
        declared
    } else {
        new_record(py, RecordId::MaterializationDefaultsConfig, (), None)?
    };
    let hook_entries: Bound<'py, PyTuple> = PyTuple::new(
        py,
        [
            RecordId::SqlHookEntry,
            RecordId::NamedSqlHookEntry,
            RecordId::PythonHookEntry,
        ]
        .iter()
        .map(|class| record_class(py, *class))
        .collect::<PyResult<Vec<_>>>()?,
    )?;
    let target_namespace: Option<(Bound<'py, PyAny>, Bound<'py, PyAny>)> = if target.is_none() {
        None
    } else {
        Some((
            native_field(&target, "database")?,
            native_field(&target, "schema")?,
        ))
    };
    let builder: Bound<'py, PyAny> = Bound::new(
        py,
        NativeModelConfigBuilder::new(
            (
                native_field(&project_config, "defaults")?,
                native_field(&project_config, "path_defaults")?.cast_into()?,
                hook_entries,
            ),
            (
                native_field(inputs, "effective_vars")?.cast_into()?,
                unicode_environment(py)?,
            ),
            (
                (
                    native_field(inputs, "effective_target_name")?,
                    native_field(inputs, "run_id")?,
                ),
                target_namespace,
            ),
        )?,
    )?
    .into_any();
    let inherited: Bound<'py, PyDict> = PyDict::new(py);
    let mut materializations: Vec<Bound<'py, PyAny>> = vec![py.None().into_bound(py)];
    materializations.extend(table_backed(py)?);
    for materialized in materializations {
        let storage: Bound<'py, PyTuple> =
            inherited_storage(&materialized, &materialization_defaults, &target)?;
        inherited.set_item(&materialized, storage)?;
    }
    new_record_with(
        py,
        RecordId::NativeModelConfigSession,
        &[
            (
                "path_defaults",
                native_field(&project_config, "path_defaults")?,
            ),
            ("builder", builder),
            ("validator", validator.clone()),
            ("inherited_storage", inherited.into_any()),
        ],
    )
}
