//! Counts, warnings, failures and the skipped rollup of the build footer.

use crate::build_progress::_helpers::error_rows::failure_error_block;
use crate::build_progress::_helpers::text::pad;
use crate::build_progress::constants::{
    DELTA_SUFFIX, ERROR, FAILED, PASS, PYTHON_NODE_NAME_WIDTH, SKIPPED, SKIPPED_ROLLUP_CAP,
    SUCCESS, TIMEOUT_CODE, TIMEOUT_OUTCOME, WARN,
};
use crate::build_progress::models::{AuditResultText, BuildResultText, ErrorText, PythonNodeText};
use crate::cli_text::models::CliStyle;

/// Pass, warn, fail, timeout and skip counts of the footer.
#[derive(Default)]
pub(crate) struct FooterCounts {
    pub(crate) pass: i64,
    pub(crate) warn: i64,
    pub(crate) fail: i64,
    pub(crate) timeout: i64,
    pub(crate) skip: i64,
    /// Error-severity audit failures the build let through; not counted as failures.
    pub(crate) ignored: i64,
}

impl FooterCounts {
    fn status(&mut self, status: &str, warned: bool) {
        match status {
            SUCCESS if warned => self.warn += 1,
            SUCCESS => self.pass += 1,
            FAILED => self.fail += 1,
            SKIPPED => self.skip += 1,
            _ => {}
        }
    }
}

fn build_audits(result: &BuildResultText) -> impl Iterator<Item = &AuditResultText> {
    result
        .models
        .iter()
        .flat_map(|model| model.audits.iter())
        .chain(result.source_audits.iter())
        .chain(result.end_audits.iter())
}

pub(crate) fn count_results(result: &BuildResultText, python: &[PythonNodeText]) -> FooterCounts {
    let mut counts: FooterCounts = FooterCounts::default();
    for model in &result.models {
        if model.status == FAILED && model.error.code.as_deref() == Some(TIMEOUT_CODE) {
            counts.timeout += 1;
        } else {
            counts.status(&model.status, !model.warnings.is_empty());
        }
    }
    for audit in build_audits(result) {
        match audit.outcome.as_str() {
            PASS => counts.pass += 1,
            WARN => counts.warn += 1,
            ERROR if audit.ignored => counts.ignored += 1,
            ERROR => counts.fail += 1,
            SKIPPED => counts.skip += 1,
            _ => {}
        }
    }
    for seed in &result.seeds {
        counts.status(&seed.status, false);
    }
    for function in &result.functions {
        counts.status(&function.status, false);
        counts.warn += function.warnings.len() as i64;
    }
    for load in &result.loads {
        counts.status(&load.status, !load.warnings.is_empty());
    }
    for test in &result.tests {
        match test.outcome.as_str() {
            PASS => counts.pass += 1,
            TIMEOUT_OUTCOME => counts.timeout += 1,
            _ => counts.fail += 1,
        }
    }
    for node in python {
        counts.status(&node.status, !node.warnings.is_empty());
    }
    counts
}

/// One titled footer section that is shown only when it has entries.
struct Section {
    title: String,
    lines: Vec<String>,
}

impl Section {
    fn new(title: String) -> Self {
        Self {
            title,
            lines: Vec::new(),
        }
    }

    fn entry(&mut self, heading: String, body: impl IntoIterator<Item = String>) {
        self.lines.push(heading);
        self.lines.extend(body);
        self.lines.push(String::new());
    }

    fn failure(&mut self, heading: String, error: &ErrorText, style: CliStyle) {
        self.failure_with(heading, error, None, style);
    }

    fn failure_with(
        &mut self,
        heading: String,
        error: &ErrorText,
        extra: Option<String>,
        style: CliStyle,
    ) {
        let mut body: Vec<String> = Vec::new();
        if let Some(message) = &error.message {
            body.extend(failure_error_block(
                (error.code.as_deref(), message, error.help.as_deref()),
                style,
            ));
        }
        body.extend(extra);
        self.entry(heading, body);
    }

    fn finish(self) -> Vec<String> {
        if self.lines.is_empty() {
            return Vec::new();
        }
        let mut lines: Vec<String> = vec![String::new(), self.title, String::new()];
        lines.extend(self.lines);
        lines
    }
}

fn audit_warning(audit: &AuditResultText) -> String {
    let name: String = match audit.attached_column_name.as_deref() {
        Some(column) if !column.is_empty() => format!("{} ({column})", audit.audit_name),
        _ => audit.audit_name.clone(),
    };
    if audit.measurement {
        return format!("    audit {name} measured {}", audit.measured_value);
    }
    let row_label: &str = if audit.row_count == 1 { "row" } else { "rows" };
    format!("    audit {name} returned {} {row_label}", audit.row_count)
}

pub(crate) fn warning_details(
    result: &BuildResultText,
    python: &[PythonNodeText],
    style: CliStyle,
) -> Vec<String> {
    let mut section: Section = Section::new(style.warning_strong("Warnings:"));
    for model in &result.models {
        let mut warnings: Vec<String> = model
            .audits
            .iter()
            .filter(|audit| audit.outcome == WARN)
            .map(audit_warning)
            .collect();
        warnings.extend(
            model
                .warnings
                .iter()
                .map(|warning| format!("    {warning}")),
        );
        if !warnings.is_empty() {
            section.entry(format!("  {}", model.name), warnings);
        }
    }
    let labelled = result
        .functions
        .iter()
        .map(|function| {
            (
                format!("{}  ({})", function.name, function.kind),
                &function.warnings,
            )
        })
        .chain(result.loads.iter().map(|load| {
            (
                format!("{}  (loader {})", load.source_name, load.loader_name),
                &load.warnings,
            )
        }))
        .chain(
            python
                .iter()
                .map(|node| (format!("{}  ({})", node.name, node.kind), &node.warnings)),
        );
    for (label, messages) in labelled {
        if !messages.is_empty() {
            section.entry(
                format!("  {label}"),
                messages.iter().map(|message| format!("    {message}")),
            );
        }
    }
    section.finish()
}

fn inspection_relation_message(relation: &str) -> String {
    if relation.ends_with(DELTA_SUFFIX) {
        return format!("delta table kept for inspection: {relation}");
    }
    format!("staging table kept for inspection: {relation}")
}

fn message_error(message: Option<&String>) -> ErrorText {
    ErrorText {
        message: message.cloned(),
        ..ErrorText::default()
    }
}

/// Each ignored audit failure under the model, source or check it audits, marked ignored.
pub(crate) fn ignored_failure_details(result: &BuildResultText, style: CliStyle) -> Vec<String> {
    let mut section: Section = Section::new(style.warning_strong("Ignored audit failures:"));
    for model in &result.models {
        let ignored: Vec<String> = model
            .audits
            .iter()
            .filter(|audit| audit.ignored)
            .map(ignored_audit)
            .collect();
        if !ignored.is_empty() {
            section.entry(format!("  {}", model.name), ignored);
        }
    }
    for audit in result
        .source_audits
        .iter()
        .chain(result.end_audits.iter())
        .filter(|audit| audit.ignored)
    {
        let target: &str = audit
            .attached_target_name
            .as_deref()
            .unwrap_or(&audit.audit_name);
        section.entry(format!("  {target}"), [ignored_audit(audit)]);
    }
    section.finish()
}

/// An ignored audit failure: what it found, or the error that failed it, then that it was ignored.
fn ignored_audit(audit: &AuditResultText) -> String {
    match &audit.execution_error {
        Some(error) => format!("    audit {} failed: {error} (ignored)", audit.audit_name),
        None => format!("{} (ignored)", audit_warning(audit)),
    }
}

pub(crate) fn failure_details(result: &BuildResultText, style: CliStyle) -> Vec<String> {
    let mut section: Section = Section::new(style.error_strong("Failures:"));
    for load in result.loads.iter().filter(|load| load.status == FAILED) {
        section.failure(
            format!("  {}  ({})", load.source_name, load.resource_kind),
            &message_error(load.error_message.as_ref()),
            style,
        );
    }
    for seed in result.seeds.iter().filter(|seed| seed.status == FAILED) {
        section.failure(format!("  {}  (seed)", seed.name), &seed.error, style);
    }
    for model in result.models.iter().filter(|model| model.status == FAILED) {
        let phase: String = match model.failed_phase.as_deref() {
            Some(phase) if !phase.is_empty() => format!("  ({phase})"),
            _ => String::new(),
        };
        section.failure_with(
            format!("  {}{phase}", model.name),
            &model.error,
            model
                .staging_relation
                .as_deref()
                .map(|relation| format!("    {}", inspection_relation_message(relation))),
            style,
        );
    }
    for function in result
        .functions
        .iter()
        .filter(|function| function.status == FAILED)
    {
        section.failure(
            format!("  {}  ({})", function.name, function.kind),
            &function.error,
            style,
        );
    }
    for test in result.tests.iter().filter(|test| test.outcome != PASS) {
        section.failure(format!("  {}  (test)", test.label), &test.error, style);
    }
    for audit in result.source_audits.iter().chain(result.end_audits.iter()) {
        if audit.execution_error.is_none() {
            continue;
        }
        let target: String = audit
            .attached_target_name
            .as_deref()
            .map_or_else(String::new, |target| format!(" on {target}"));
        section.failure(
            format!("  {}{target}  (audit)", audit.audit_name),
            &message_error(audit.execution_error.as_ref()),
            style,
        );
    }
    section.finish()
}

pub(crate) fn python_failure_details(python: &[PythonNodeText], style: CliStyle) -> Vec<String> {
    let failed: Vec<&PythonNodeText> = python.iter().filter(|node| node.status == FAILED).collect();
    if failed.is_empty() {
        return Vec::new();
    }
    let mut lines: Vec<String> = vec![
        String::new(),
        style.error_strong("Python node failures:"),
        String::new(),
    ];
    for node in failed {
        let message: &str = match node.error_message.as_deref() {
            Some(message) if !message.is_empty() => message,
            _ => "Python node failed",
        };
        lines.push(format!(
            "  {}{} {}",
            pad(&node.kind, 10),
            pad(&node.name, PYTHON_NODE_NAME_WIDTH),
            style.error(message)
        ));
    }
    lines
}

/// A skipped audit's rollup entry: its name and why it did not run.
fn skipped_audit_label(audit: &AuditResultText) -> String {
    match &audit.skip_reason {
        Some(reason) => format!("{} (audit, {reason})", audit.audit_name),
        None => format!("{} (audit)", audit.audit_name),
    }
}

pub(crate) fn skipped_rollup(
    result: &BuildResultText,
    python: &[PythonNodeText],
    style: CliStyle,
) -> Vec<String> {
    let names: Vec<&str> = result
        .models
        .iter()
        .filter(|model| model.status == SKIPPED)
        .map(|model| model.name.as_str())
        .chain(
            result
                .seeds
                .iter()
                .filter(|seed| seed.status == SKIPPED)
                .map(|seed| seed.name.as_str()),
        )
        .chain(
            result
                .functions
                .iter()
                .filter(|function| function.status == SKIPPED)
                .map(|function| function.name.as_str()),
        )
        .chain(
            result
                .loads
                .iter()
                .filter(|load| load.status == SKIPPED)
                .map(|load| load.source_name.as_str()),
        )
        .chain(
            python
                .iter()
                .filter(|node| node.status == SKIPPED)
                .map(|node| node.name.as_str()),
        )
        .collect();
    let names: Vec<String> = names
        .into_iter()
        .map(str::to_owned)
        .chain(
            result
                .end_audits
                .iter()
                .filter(|audit| audit.outcome == SKIPPED)
                .map(skipped_audit_label),
        )
        .collect();
    if names.is_empty() {
        return Vec::new();
    }
    let visible: Vec<&str> = names
        .iter()
        .take(SKIPPED_ROLLUP_CAP)
        .map(String::as_str)
        .collect();
    let remaining: usize = names.len() - visible.len();
    let mut rendered: String = visible.join(", ");
    if remaining > 0 {
        rendered = format!("{rendered}, ... (+{remaining} more)");
    }
    vec![
        String::new(),
        style.muted(&format!("Skipped ({}): {rendered}", names.len())),
    ]
}
