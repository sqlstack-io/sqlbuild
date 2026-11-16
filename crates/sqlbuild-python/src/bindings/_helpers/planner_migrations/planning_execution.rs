//! Preserve adapter overrides when describing migration execution.

use sqlbuild_host::entries::models::AdapterHost;

use crate::bindings::_helpers::planner_migrations::column_objects::replace;
use crate::bindings::_helpers::planner_phases::runtime::runtime_field;
use pyo3::prelude::{Bound, PyAny, PyAnyMethods, PyResult};
use pyo3::types::PyBool;
use sqlbuild_python_base::boundary::main::call_with::call_with;
use sqlbuild_python_base::native_host::main::host_entry::host_entry;
use sqlbuild_python_base::records::main::construct::record_class::record_class;
use sqlbuild_python_base::records::main::fields::field::field as native_field;
use sqlbuild_python_base::records::types::RecordId;

fn label<'py>(location: &Bound<'py, PyAny>) -> PyResult<Bound<'py, PyAny>> {
    let qualified = native_field(location, "qualified_name")?;
    if qualified.is_truthy()? {
        Ok(qualified)
    } else {
        native_field(location, "name")
    }
}

fn migration_promotion<'py>(
    runtime: &Bound<'py, PyAny>,
    decision: &Bound<'py, PyAny>,
) -> PyResult<Bound<'py, PyAny>> {
    let adapter = runtime_field(runtime, |runtime| &runtime.adapter)?;
    let promotion = record_class(runtime.py(), RecordId::MigrationPromotion)?;
    if adapter
        .call_method0("supports_transactional_ddl")?
        .is_truthy()?
    {
        return native_field(&promotion, "TRANSACTIONAL_RENAME");
    }
    if !decision.eq("migrate")? && native_field(&adapter, "adapter_name")?.eq("snowflake")? {
        return native_field(&promotion, "SWAP");
    }
    native_field(&promotion, "RENAME")
}

/// The migration entry with the adapter's staging, transfer and promotion choices.
pub(super) fn migration_with_execution<'py>(
    runtime: &Bound<'py, PyAny>,
    model: &Bound<'py, PyAny>,
    entry: &Bound<'py, PyAny>,
) -> PyResult<Bound<'py, PyAny>> {
    let py = runtime.py();
    let adapter = runtime_field(runtime, |runtime| &runtime.adapter)?;
    let transient = if native_field(&adapter, "adapter_name")?.eq("snowflake")? {
        PyBool::new(
            py,
            native_field(
                &native_field(&native_field(model, "config")?, "table_type")?,
                "value",
            )?
            .eq("transient")?,
        )
        .to_owned()
        .into_any()
    } else {
        py.None().into_bound(py)
    };
    let stage = call_with(
        &native_field(&adapter, "render_migration_stage")?,
        &[
            ("origin", label(&native_field(entry, "origin")?)?),
            ("stage", label(&native_field(entry, "destination")?)?),
            (
                "origin_is_transient",
                native_field(entry, "origin_is_transient")?,
            ),
            ("stage_is_transient", transient.clone()),
        ],
    )?;
    let transfer = native_field(&stage, "transfer")?;
    let fallback = if native_field(&stage, "fallback_statements")?.is_truthy()? {
        host_entry(py, AdapterHost::MigrationTransfer)?.getattr("COPY")?
    } else {
        py.None().into_bound(py)
    };
    replace(
        entry,
        &[
            ("stage_is_transient", transient),
            ("transfer", transfer),
            ("transfer_fallback", fallback),
            (
                "promotion",
                migration_promotion(runtime, &native_field(entry, "decision")?)?,
            ),
        ],
    )
}
