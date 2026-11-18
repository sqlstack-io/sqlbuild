//! Preserve adapter overrides when describing migration execution.

use crate::planner_phases::main::inputs::runtime_dialect::runtime_dialect;
use sqlbuild_host::entries::models::AdapterHost;
use sqlbuild_python_base::native_host::types::Syntax;

use crate::planner_migrations::_helpers::column::column_objects::replace;
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
    let dialect = runtime_dialect(runtime)?;
    let promotion = record_class(runtime.py(), RecordId::MigrationPromotion)?;
    if dialect
        .syntax("supports_transactional_ddl")?
        .call0()?
        .is_truthy()?
    {
        return native_field(&promotion, "TRANSACTIONAL_RENAME");
    }
    if !decision.eq("migrate")? && dialect.syntax("adapter_name")?.eq("snowflake")? {
        return native_field(&promotion, "SWAP");
    }
    native_field(&promotion, "RENAME")
}

/// The migration entry with the adapter's staging, transfer and promotion choices.
pub(crate) fn migration_with_execution<'py>(
    runtime: &Bound<'py, PyAny>,
    model: &Bound<'py, PyAny>,
    entry: &Bound<'py, PyAny>,
) -> PyResult<Bound<'py, PyAny>> {
    let py = runtime.py();
    let dialect = runtime_dialect(runtime)?;
    let transient = if dialect.syntax("adapter_name")?.eq("snowflake")? {
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
        &dialect.syntax("render_migration_stage")?,
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
