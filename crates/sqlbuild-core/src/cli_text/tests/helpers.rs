use crate::cli_text::models::{
    DiagnosticText, DisplayLimit, LocationText, ModelPlanText, PlanTextReport,
};

/// A location in `sql/models/a.sql`, whose text has a CRLF line, a non-ASCII name and a vertical tab.
pub(super) fn location(line: i64, column: i64, end: (Option<i64>, Option<i64>)) -> LocationText {
    LocationText {
        path: "sql/models/a.sql".to_owned(),
        line,
        column,
        end_line: end.0,
        end_column: end.1,
        source: Some("SELECT 1\r\nFROM orders\u{e9} x\u{0b}third".to_owned()),
    }
}

/// A diagnostic with only its severity, code and message set.
pub(super) fn diagnostic(severity: &str, code: &str, message: &str) -> DiagnosticText {
    DiagnosticText {
        severity: severity.to_owned(),
        code: code.to_owned(),
        message: message.to_owned(),
        resource_type: None,
        resource_name: None,
        location: None,
        path: None,
        related: Vec::new(),
        notes: Vec::new(),
        affected_rules: Vec::new(),
        help: None,
        occurrences: 1,
    }
}

/// An empty uncolored plan report that shows every entry.
pub(super) fn plan_report(models: Vec<ModelPlanText>) -> PlanTextReport {
    PlanTextReport {
        use_color: false,
        include_header: true,
        full_refresh: false,
        display: DisplayLimit {
            max_entries: None,
            overflow_flag: "--verbose".to_owned(),
        },
        include_freshness_diagnostics: false,
        selection_diagnostics: None,
        models,
        functions: Vec::new(),
        seeds: Vec::new(),
        source_loads: Vec::new(),
        python_entries: Vec::new(),
        provider_usages: Vec::new(),
        warnings: Vec::new(),
        source_freshness: None,
        remaining_stale_models: Vec::new(),
        pruned_models: Vec::new(),
        pre_build_lines: Vec::new(),
        ignore_audit_failures: false,
        ignore_audit_failure_jobs: Vec::new(),
        job_warehouses: Vec::new(),
        audits: Vec::new(),
    }
}

/// A planned table model with a change reason and full backfill.
pub(super) fn table_model(name: &str, reason: &str) -> ModelPlanText {
    ModelPlanText {
        name: name.to_owned(),
        action: "create_table".to_owned(),
        reason: reason.to_owned(),
        materialization_label: "table".to_owned(),
        backfill_action: "full".to_owned(),
        ..ModelPlanText::default()
    }
}
