//! Convert public migration models at the Python boundary for native presentation.

use crate::bindings::_helpers::plan_output::plan_text::display_limit;
use crate::bindings::_helpers::plan_output::values::{
    iso, items, optional_text, relation_name, text, texts,
};
use crate::bindings::_helpers::records::fields::field as native_field;
use pyo3::prelude::{Bound, PyAny, PyAnyMethods, PyModule, PyModuleMethods, PyResult};
use pyo3::{pyfunction, wrap_pyfunction};
use sqlbuild_core::cli_text::models::{
    CliStyle, ColumnMigrationText, MigrationDecisionText, MigrationText, MigrationTextReport,
    OldNameText, PreBuildWorkText, RetentionText, TableTypeText,
};
use sqlbuild_core::plan_display::main::{
    format_model_migrations, format_pre_build_work, old_name_rows,
};
use std::collections::BTreeMap;

const MANUAL_RENAME_HINT: &str = "matched by unchanged definition; sqb rename <old> <new> rewrites references and declares migrate_from so the move does not depend on discovery";

fn label(value: &Bound<'_, PyAny>) -> PyResult<Option<String>> {
    if value.is_none() {
        return Ok(None);
    }
    text(&native_field(value, "label")?).map(Some)
}

fn decision(entry: &Bound<'_, PyAny>) -> PyResult<MigrationDecisionText> {
    let value = native_field(entry, "decision")?;
    Ok(MigrationDecisionText {
        value: text(&value)?,
        label: text(&native_field(&value, "label")?)?,
        blocks_build: native_field(entry, "blocks_build")?.extract()?,
        origin_missing: native_field(entry, "origin_missing")?.extract()?,
        ..MigrationDecisionText::default()
    })
}

fn migration(entry: &Bound<'_, PyAny>, materialization: Option<String>) -> PyResult<MigrationText> {
    let mut decision = decision(entry)?;
    let value = native_field(entry, "decision")?;
    decision.decision_blocks_build = native_field(&value, "blocks_build")?.extract()?;
    decision.checks_compatibility = native_field(&value, "checks_compatibility")?.extract()?;
    Ok(MigrationText {
        model_name: text(&native_field(entry, "model_name")?)?,
        origin: text(&relation_name(&native_field(entry, "origin")?)?)?,
        destination: text(&relation_name(&native_field(entry, "destination")?)?)?,
        decision,
        compatibility: text(&native_field(
            &native_field(entry, "compatibility")?,
            "value",
        )?)?,
        discovery: text(&native_field(&native_field(entry, "discovery")?, "value")?)?,
        completed_at: optional_text(&iso(&native_field(entry, "completed_at")?)?)?,
        target_label: text(&native_field(entry, "target_label")?)?,
        transfer: label(&native_field(entry, "transfer")?)?,
        transfer_fallback: label(&native_field(entry, "transfer_fallback")?)?,
        storage_transition: optional_text(&native_field(entry, "storage_transition")?)?,
        promotion: label(&native_field(entry, "promotion")?)?,
        findings: texts(&native_field(entry, "compatibility_findings")?)?,
        materialization,
    })
}

fn old_name(entry: &Bound<'_, PyAny>) -> PyResult<OldNameText> {
    let expires = native_field(entry, "expires_at")?;
    let expires_date = if expires.is_none() {
        None
    } else {
        Some(expires.call_method1("strftime", ("%Y-%m-%d",))?.extract()?)
    };
    Ok(OldNameText {
        model_name: text(&native_field(entry, "model_name")?)?,
        origin: text(&relation_name(&native_field(entry, "origin")?)?)?,
        destination: text(&relation_name(&native_field(entry, "destination")?)?)?,
        action: text(&native_field(entry, "action")?)?,
        expires_date,
        column_aliases: native_field(entry, "column_aliases")?.extract()?,
        grants_supported: native_field(entry, "grants_supported")?.extract()?,
        grants_copied: optional_text(&native_field(entry, "grants_copied")?)?,
        reason: optional_text(&native_field(entry, "reason")?)?,
    })
}

fn column(entry: &Bound<'_, PyAny>) -> PyResult<ColumnMigrationText> {
    Ok(ColumnMigrationText {
        model_name: text(&native_field(entry, "model_name")?)?,
        origin_column: text(&native_field(entry, "origin_column")?)?,
        destination_column: text(&native_field(entry, "destination_column")?)?,
        decision: decision(entry)?,
        discovery: text(&native_field(&native_field(entry, "discovery")?, "value")?)?,
        completed_at: optional_text(&iso(&native_field(entry, "completed_at")?)?)?,
    })
}

/// Migration sections appended to `lines`, in planner order.
pub(crate) fn migration_lines(
    lines: Vec<String>,
    plan: &Bound<'_, PyAny>,
    display_options: &Bound<'_, PyAny>,
) -> PyResult<Vec<String>> {
    let mut materializations: BTreeMap<String, String> = BTreeMap::new();
    for entry in items(&native_field(plan, "model_entries")?)? {
        materializations.insert(
            text(&native_field(&entry, "name")?)?,
            text(&native_field(&entry, "materialization_type")?)?,
        );
    }
    let mut migrations: Vec<MigrationText> = Vec::new();
    for entry in items(&native_field(plan, "migration_entries")?)? {
        let name = text(&native_field(&entry, "model_name")?)?;
        migrations.push(migration(&entry, materializations.get(&name).cloned())?);
    }
    let report = MigrationTextReport {
        migrations,
        old_names: items(&native_field(plan, "old_name_view_entries")?)?
            .iter()
            .map(old_name)
            .collect::<PyResult<_>>()?,
        columns: items(&native_field(plan, "column_migration_entries")?)?
            .iter()
            .map(column)
            .collect::<PyResult<_>>()?,
        display: display_limit(display_options)?,
        manual_rename_hint: MANUAL_RENAME_HINT.to_owned(),
    };
    Ok(format_model_migrations::format_model_migrations(
        lines, &report,
    ))
}

fn days(value: &Bound<'_, PyAny>) -> PyResult<Option<i64>> {
    value.extract()
}

/// Table-type, retention and migration work that runs before model builds.
pub(crate) fn pre_build_lines(
    lines: Vec<String>,
    plan: &Bound<'_, PyAny>,
    display_options: &Bound<'_, PyAny>,
) -> PyResult<Vec<String>> {
    let mut table_types: Vec<TableTypeText> = Vec::new();
    for entry in items(&native_field(plan, "table_type_entries")?)? {
        table_types.push(TableTypeText {
            model_name: text(&native_field(&entry, "model_name")?)?,
            desired_type: text(&native_field(&entry, "desired_type")?)?,
            actual_type: optional_text(&native_field(&entry, "actual_type")?)?
                .filter(|actual| !actual.is_empty()),
            source: text(&native_field(&entry, "source")?)?,
            irreversible_warning: optional_text(&native_field(&entry, "irreversible_warning")?)?,
        });
    }
    let mut retention: Vec<RetentionText> = Vec::new();
    for entry in items(&native_field(plan, "retention_entries")?)? {
        let request = native_field(&entry, "request")?;
        let mut parts: Vec<String> = Vec::new();
        for field in ["database", "schema", "name"] {
            let part = native_field(&request, field)?;
            if part.is_truthy()? {
                parts.push(text(&part)?);
            }
        }
        retention.push(RetentionText {
            scope: parts.join("."),
            desired_days: native_field(&request, "desired_days")?.extract()?,
            actual_days: days(&native_field(&entry, "actual_days")?)?,
            effective_days: days(&native_field(&entry, "effective_days")?)?,
            source: text(&native_field(&entry, "source")?)?,
            direction: text(&native_field(&native_field(&entry, "direction")?, "value")?)?,
            phase: text(&native_field(&native_field(&entry, "phase")?, "value")?)?,
            irreversible_warning: optional_text(&native_field(&entry, "irreversible_warning")?)?,
        });
    }
    let work = PreBuildWorkText {
        table_types,
        retention,
        display: display_limit(display_options)?,
    };
    migration_lines(
        format_pre_build_work::format_pre_build_work(lines, &work),
        plan,
        display_options,
    )
}

/// Format retention, table-type, and migration work that runs before model builds.
#[pyfunction(name = "format_pre_build_work", signature = (*, lines, plan, display_options))]
fn format_pre_build(
    lines: Vec<String>,
    plan: &Bound<'_, PyAny>,
    display_options: &Bound<'_, PyAny>,
) -> PyResult<Vec<String>> {
    pre_build_lines(lines, plan, display_options)
}

#[pyfunction(name = "old_name_rows", signature = (*, entry, style))]
fn render_old_name(entry: &Bound<'_, PyAny>, style: &Bound<'_, PyAny>) -> PyResult<Vec<String>> {
    Ok(old_name_rows::old_name_rows(
        &old_name(entry)?,
        CliStyle {
            use_color: native_field(style, "use_color")?.extract()?,
        },
    ))
}

pub(crate) fn register(module: &Bound<'_, PyModule>) -> PyResult<()> {
    module.add_function(wrap_pyfunction!(format_pre_build, module)?)?;
    module.add_function(wrap_pyfunction!(render_old_name, module)?)
}
