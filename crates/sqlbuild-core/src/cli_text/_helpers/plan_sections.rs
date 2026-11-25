//! The sections of the text plan: header, providers, loads, models, functions, seeds, warnings.

use crate::cli_text::_helpers::bounded_sections::{
    capped_names, name_column_width, name_value_line, overflow_line, section_header, strip_ansi,
    visible_count,
};
use crate::cli_text::_helpers::layout::tree_connector;
use crate::cli_text::constants::{
    ALTER_DYNAMIC_ACTION, BOUNDED_BACKFILL, CHANGED_IDENTITY, COLUMN_ADDED, COLUMN_REMOVED,
    COLUMN_TYPE_CHANGED, CONFIG_CHANGED_REASON, CREATE_DYNAMIC_ACTION, CREATE_VIEW_ACTION,
    CUSTOM_ACTION, DIFF_HEADER_MARKER, EFFECTIVE_BATCH_SIZE_TOKEN, ERROR_ACTION,
    ERROR_SEVERITY_VALUE, FIRST_RUN_REASON, FORWARD_BACKFILL, FULL_BACKFILL, FULL_REFRESH_REASON,
    INFO_SEVERITY_VALUE, NO_CHANGE_REASON, NORMAL_INCREMENTAL_REASON, QUERY_CHANGED_REASON,
    REASON_GROUPS, SCHEMA_CHANGED_REASON, SKIP_ACTION, SOURCE_FRESHNESS_UNKNOWN_WARNING_TITLE,
    STALE_INPUT_WARNING_TITLE, UNBOUNDED_RANGE,
};
use crate::cli_text::models::{
    CliStyle, CursorPlanText, DisplayLimit, FunctionPlanText, ModelPlanText, PlanTextReport,
    PlanWarningText, ProviderUsageText, PythonPlanText, SchemaFindingText, SourceFreshnessText,
    SourceLoadPlanText,
};

const STYLE: CliStyle = CliStyle { use_color: true };

/// The rows of one bounded section: blank line, `Title (count)` header, rows, overflow line.
fn section_lines<T>(
    title: &str,
    entries: &[T],
    display: &DisplayLimit,
    row: impl Fn(&T) -> Vec<String>,
) -> Vec<String> {
    let mut lines: Vec<String> = vec![
        String::new(),
        section_header(STYLE, &format!("{title} ({})", entries.len())),
    ];
    let visible: usize = visible_count(entries.len(), display);
    for entry in &entries[..visible] {
        lines.extend(row(entry));
    }
    lines.extend(overflow_line((entries.len(), visible), "  ", display));
    lines
}

/// Models whose action is not `skip`, and skipped models with a reason to show.
pub(crate) fn active_models(report: &PlanTextReport) -> Vec<&ModelPlanText> {
    report
        .models
        .iter()
        .filter(|model| model.action != SKIP_ACTION || model.skip_reason.is_some())
        .collect()
}

/// Whether the plan renders as a full refresh: every active model is one, or nothing is active.
pub(crate) fn full_refresh_layout(report: &PlanTextReport) -> bool {
    let active: Vec<&ModelPlanText> = active_models(report);
    (!active.is_empty()
        && active
            .iter()
            .all(|model| model.reason == FULL_REFRESH_REASON))
        || (report.full_refresh && active.is_empty())
}

/// `N selected`, with the full-refresh, source-load and Python-node parts.
pub(crate) fn ready_summary(report: &PlanTextReport, full_refresh: bool) -> String {
    let mut parts: Vec<String> = Vec::new();
    if full_refresh {
        parts.push("full refresh".to_owned());
    }
    let selected: usize = report.models.len() + report.seeds.len() + report.functions.len();
    parts.push(format!("{selected} selected"));
    let sources: usize = report.source_loads.len();
    if sources > 0 {
        let noun: &str = if sources == 1 { "source" } else { "sources" };
        let action: &str = if report.source_loads.iter().any(|load| load.is_reload) {
            "reload"
        } else {
            "load"
        };
        parts.push(format!("{sources} {noun} to {action}"));
    }
    let python: usize = report.python_entries.len();
    if python > 0 {
        let noun: &str = if python == 1 { "node" } else { "nodes" };
        parts.push(format!("{python} Python {noun}"));
    }
    let audits: usize = report.audits.len();
    if audits > 0 {
        let noun: &str = if audits == 1 { "audit" } else { "audits" };
        parts.push(format!("{audits} {noun}"));
    }
    let not_run: usize = report.audits_not_run.len();
    if not_run > 0 {
        let noun: &str = if not_run == 1 { "audit" } else { "audits" };
        parts.push(format!("{not_run} {noun} not run"));
    }
    parts.join(", ")
}

/// The name column every plan section aligns to.
pub(crate) fn plan_name_width(report: &PlanTextReport) -> usize {
    name_column_width(
        report
            .models
            .iter()
            .map(|model| model.name.as_str())
            .chain(
                report
                    .functions
                    .iter()
                    .map(|function| function.name.as_str()),
            )
            .chain(
                report
                    .python_entries
                    .iter()
                    .map(|entry| entry.name.as_str()),
            ),
    )
}

/// Each provider with its count of selected Python surfaces, or every surface when verbose.
pub(crate) fn provider_usage_lines(report: &PlanTextReport) -> Vec<String> {
    if report.provider_usages.is_empty() {
        return Vec::new();
    }
    let mut providers: Vec<(&str, Vec<&ProviderUsageText>)> = Vec::new();
    for usage in &report.provider_usages {
        if let Some((_, usages)) = providers
            .iter_mut()
            .find(|(name, _)| *name == usage.provider_name)
        {
            usages.push(usage);
        } else {
            providers.push((&usage.provider_name, vec![usage]));
        }
    }
    providers.sort_by(|left, right| left.0.cmp(right.0));
    let mut lines: Vec<String> = vec![String::new(), section_header(STYLE, "Providers")];
    if report.display.max_entries.is_some() {
        let width: usize = providers
            .iter()
            .map(|(name, _)| name.chars().count())
            .max()
            .unwrap_or(0);
        for (name, usages) in &providers {
            let surface: &str = if usages.len() == 1 {
                "surface"
            } else {
                "surfaces"
            };
            let padding: String = " ".repeat(width - name.chars().count());
            lines.push(format!(
                "  {}{padding}  {}",
                STYLE.object_name(name),
                STYLE.muted(&format!(
                    "used by {} selected Python {surface}",
                    usages.len()
                ))
            ));
        }
        return lines;
    }
    for (name, mut usages) in providers {
        lines.push(format!("  {}", STYLE.object_name(name)));
        usages.sort_by(|left, right| {
            (
                &left.consumer_kind,
                &left.consumer_name,
                &left.parameter_name,
            )
                .cmp(&(
                    &right.consumer_kind,
                    &right.consumer_name,
                    &right.parameter_name,
                ))
        });
        let visible: usize = visible_count(usages.len(), &report.display);
        for usage in &usages[..visible] {
            let annotation: String = usage
                .annotation_class_name
                .as_ref()
                .map_or_else(String::new, |class_name| format!(" ({class_name})"));
            lines.push(format!(
                "    {} {}{annotation}",
                STYLE.muted(&usage.consumer_kind),
                STYLE.object_name(&usage.consumer_name)
            ));
        }
        lines.extend(overflow_line(
            (usages.len(), visible),
            "    ",
            &report.display,
        ));
    }
    lines
}

/// The count of current models a direct plan pruned, listing them when verbose.
pub(crate) fn pruned_lines(report: &PlanTextReport) -> Vec<String> {
    let names: &[String] = &report.pruned_models;
    if names.is_empty() {
        return Vec::new();
    }
    let mut lines: Vec<String> = vec![
        String::new(),
        STYLE.muted(&format!(
            "Skipped current models ({} already up to date)",
            names.len()
        )),
    ];
    if report.display.max_entries.is_some() {
        return lines;
    }
    let width: usize = name_column_width(names.iter().map(String::as_str));
    for name in names {
        lines.push(name_value_line(name, "up to date", width, true));
    }
    lines
}

fn identity_diff_lines(entry: &PythonPlanText) -> Vec<String> {
    if entry.identity_status != CHANGED_IDENTITY
        || (entry.source_diff.is_empty() && entry.dependency_diff.is_empty())
    {
        return Vec::new();
    }
    let mut lines: Vec<String> = vec![STYLE.label("    python diff:")];
    if !entry.source_diff.is_empty() {
        lines.push(STYLE.label("      source diff:"));
        lines.extend(entry.source_diff.iter().map(|line| format!("  {line}")));
    }
    if !entry.dependency_diff.is_empty() {
        lines.push(STYLE.label("      dependency diff:"));
        for line in &entry.dependency_diff {
            let indented: String = format!("  {line}");
            if strip_ansi(&indented).contains(DIFF_HEADER_MARKER) {
                lines.push(STYLE.muted(&indented));
            } else {
                lines.push(indented);
            }
        }
    }
    lines
}

/// The Python entries of one run phase under `label`.
pub(crate) fn python_entry_lines(
    (report, phase, label): (&PlanTextReport, &str, &str),
    width: usize,
) -> Vec<String> {
    let entries: Vec<&PythonPlanText> = report
        .python_entries
        .iter()
        .filter(|entry| entry.phase == phase)
        .collect();
    if entries.is_empty() {
        return Vec::new();
    }
    section_lines(label, &entries, &report.display, |entry| {
        let mut lines: Vec<String> = vec![name_value_line(
            &entry.name,
            &format!("{} ({})", entry.kind, entry.identity_status),
            width,
            true,
        )];
        lines.extend(
            entry
                .wait_changes
                .iter()
                .map(|change| STYLE.muted(&format!("    {change}"))),
        );
        lines.extend(identity_diff_lines(entry));
        lines
    })
}

fn source_load_label(entry: &SourceLoadPlanText) -> String {
    let strategy: String = entry
        .write_strategy
        .clone()
        .unwrap_or_else(|| "self-managed".to_owned());
    let mut details: Vec<String> = Vec::new();
    if let Some(column) = &entry.cursor_column {
        details.push(format!("cursor: {column}"));
    }
    if !entry.unique_key.is_empty() {
        details.push(format!("unique_key: {}", entry.unique_key.join(", ")));
    }
    if details.is_empty() {
        return strategy;
    }
    format!("{strategy} ({})", details.join("; "))
}

/// The loaders, then the sources, to load or reload.
pub(crate) fn source_load_lines(report: &PlanTextReport, width: usize) -> Vec<String> {
    let mut lines: Vec<String> = Vec::new();
    for (kind, label) in [("loader", "Loaders"), ("source", "Sources")] {
        let entries: Vec<&SourceLoadPlanText> = report
            .source_loads
            .iter()
            .filter(|entry| entry.resource_kind == kind)
            .collect();
        if entries.is_empty() {
            continue;
        }
        let action: &str = if entries.iter().any(|entry| entry.is_reload) {
            "reload"
        } else {
            "load"
        };
        lines.extend(section_lines(
            &format!("{label} to {action}"),
            &entries,
            &report.display,
            |entry| {
                vec![name_value_line(
                    &entry.name,
                    &source_load_label(entry),
                    width,
                    true,
                )]
            },
        ));
    }
    lines
}

fn microbatch_lines(cursor: &CursorPlanText) -> Vec<String> {
    let mut lines: Vec<String> = Vec::new();
    if let Some(declared) = &cursor.declared_grain {
        let resolved: &str = cursor.effective_grain.as_deref().unwrap_or(declared);
        if resolved == declared {
            lines.push(format!("    grain: {declared}"));
        } else {
            lines.push(format!("    grain: {declared} -> {resolved} (effective)"));
        }
    }
    if let Some(declared) = &cursor.declared_batch_size {
        let mut text: String = declared.clone();
        if declared == EFFECTIVE_BATCH_SIZE_TOKEN {
            let resolved: &str = cursor.effective_batch_size.as_deref().unwrap_or("runtime");
            text = if cursor.effective_grain == cursor.declared_grain {
                resolved.to_owned()
            } else {
                format!("{declared} -> {resolved}")
            };
        }
        lines.push(format!("    batch size: {text}"));
    }
    if let (Some(count), Some(size)) = (cursor.planned_batch_count, &cursor.effective_batch_size) {
        lines.push(format!("    batches: {count} x {size}"));
    }
    if cursor.deferred {
        lines.push(format!(
            "    batches: counted at run time, {}",
            cursor.runtime_inputs_clause
        ));
    }
    lines.push(format!(
        "    batch concurrency: {}",
        cursor.batch_concurrency
    ));
    if let Some(policy) = &cursor.unaccounted_partition_policy {
        lines.push(format!("    unaccounted partition policy: {policy}"));
    }
    lines
}

fn cursor_lines(model: &ModelPlanText, show_range: bool) -> Vec<String> {
    let Some(cursor) = &model.cursor else {
        return Vec::new();
    };
    let mut lines: Vec<String> = vec![match &cursor.cursor_type {
        Some(cursor_type) => format!("    cursor: {} ({cursor_type})", cursor.column),
        None => format!("    cursor: {}", cursor.column),
    }];
    if cursor.microbatch {
        lines.push("    mode: microbatch".to_owned());
    }
    if cursor.requested_start.is_some() || cursor.requested_end.is_some() {
        lines.push(format!(
            "    requested: {} -> {}",
            cursor
                .requested_start
                .as_deref()
                .unwrap_or("earliest available"),
            cursor
                .requested_end
                .as_deref()
                .unwrap_or("latest available")
        ));
    }
    if let Some(window) = &cursor.empty_inputs_window {
        let line: String = format!("    window: {window}");
        if cursor.waiting_on_empty_inputs {
            lines.push(STYLE.warning(&line));
        } else {
            lines.push(line);
        }
    }
    if let (true, Some(range)) = (show_range, &cursor.range) {
        lines.push(format!("    range: {range}"));
    }
    if cursor.microbatch {
        lines.extend(microbatch_lines(cursor));
    }
    if cursor.runtime_owned {
        lines.push(format!(
            "    bounds: computed at run time, {}",
            cursor.runtime_inputs_clause
        ));
    } else if cursor.resolved {
        lines.push("    bounds: planner-resolved".to_owned());
    }
    lines
}

fn schema_change_suffix(model: &ModelPlanText) -> String {
    let has = |kind: &str| {
        model
            .schema_findings
            .iter()
            .any(|finding| finding.kind == kind)
    };
    let mut parts: Vec<&str> = Vec::new();
    if has(COLUMN_ADDED) {
        parts.push("add column");
    }
    if has(COLUMN_REMOVED) {
        parts.push("drop column");
    }
    if has(COLUMN_TYPE_CHANGED) {
        parts.push("type change");
    }
    parts.join(", ")
}

fn action_text(model: &ModelPlanText) -> String {
    if let Some(reason) = &model.skip_reason {
        return reason.clone();
    }
    if model.action == ERROR_ACTION {
        return ERROR_ACTION.to_owned();
    }
    if model.action == CREATE_VIEW_ACTION {
        return "recreate view".to_owned();
    }
    if model.action == CREATE_DYNAMIC_ACTION {
        return if model.reason == FIRST_RUN_REASON {
            "create".to_owned()
        } else {
            "recreate, full recompute".to_owned()
        };
    }
    if model.action == ALTER_DYNAMIC_ACTION {
        return "change refresh settings in place".to_owned();
    }
    if model.action == CUSTOM_ACTION {
        let materialization: &str = model
            .custom_materialization_name
            .as_deref()
            .unwrap_or("custom materialization");
        return format!("run {materialization}");
    }
    let suffix: String = schema_change_suffix(model);
    let base: String = match (&model.backfill_action, &model.backfill_duration) {
        (action, Some(duration)) if action == BOUNDED_BACKFILL => {
            format!("rebuild last {duration}")
        }
        (action, _) if action == FULL_BACKFILL && model.reason != FIRST_RUN_REASON => {
            "full rebuild".to_owned()
        }
        _ => return "continue forward".to_owned(),
    };
    if suffix.is_empty() {
        return base;
    }
    format!("{base}, {suffix}")
}

fn policy_lines(model: &ModelPlanText) -> Vec<String> {
    if model.backfill_action == FORWARD_BACKFILL
        || model.action == CREATE_DYNAMIC_ACTION
        || !(model.reason == QUERY_CHANGED_REASON || model.reason == SCHEMA_CHANGED_REASON)
    {
        return Vec::new();
    }
    let duration: &str = model.backfill_duration.as_deref().unwrap_or("full");
    let value: String = if model.backfill_action == BOUNDED_BACKFILL {
        format!("bounded-{duration}")
    } else {
        model.backfill_action.clone()
    };
    vec![format!("    policy: replay_on_change={value}")]
}

fn schema_finding_line(finding: &SchemaFindingText) -> String {
    let (symbol, label) = match finding.kind.as_str() {
        COLUMN_ADDED => ("+", "added"),
        COLUMN_REMOVED => ("-", "removed"),
        COLUMN_TYPE_CHANGED => ("~", "type changed"),
        other => ("?", other),
    };
    let expected: &str = finding.expected_type.as_deref().unwrap_or("None");
    let type_info: String = match finding.kind.as_str() {
        COLUMN_TYPE_CHANGED => format!(
            "  {expected} \u{2192} {}",
            finding.actual_type.as_deref().unwrap_or("None")
        ),
        COLUMN_ADDED
            if finding
                .expected_type
                .as_deref()
                .is_some_and(|kind| !kind.is_empty()) =>
        {
            format!("  {expected}")
        }
        _ => String::new(),
    };
    let line: String = format!(
        "      {symbol} {}{type_info}   ({label})",
        finding.column_name
    );
    match finding.kind.as_str() {
        COLUMN_ADDED => STYLE.success(&line),
        COLUMN_REMOVED => STYLE.error(&line),
        COLUMN_TYPE_CHANGED => STYLE.warning(&line),
        _ => line,
    }
}

fn schema_diff_lines(model: &ModelPlanText) -> Vec<String> {
    if model.schema_findings.is_empty() && model.rename_hints.is_empty() {
        return Vec::new();
    }
    let mut lines: Vec<String> = vec![STYLE.label("    schema diff:")];
    let mut hinted: Vec<String> = Vec::new();
    for finding in &model.schema_findings {
        lines.push(schema_finding_line(finding));
        if finding.kind != COLUMN_ADDED {
            continue;
        }
        for hint in &model.rename_hints {
            if hint.added_column.to_lowercase() == finding.column_name.to_lowercase() {
                lines.push(format!("        {}", STYLE.warning(&hint.message)));
                hinted.push(hint.added_column.to_lowercase());
            }
        }
    }
    for hint in &model.rename_hints {
        if !hinted.contains(&hint.added_column.to_lowercase()) {
            lines.push(format!(
                "      {}",
                STYLE.warning(&format!("{}: {}", hint.added_column, hint.message))
            ));
        }
    }
    lines
}

fn diff_lines(label: &str, diff: Option<&Vec<String>>) -> Vec<String> {
    let Some(diff) = diff else {
        return Vec::new();
    };
    let mut lines: Vec<String> = vec![STYLE.label(label)];
    lines.extend(diff.iter().cloned());
    lines
}

fn detail_entry_lines(model: &ModelPlanText, reason: &str, width: usize) -> Vec<String> {
    if reason == FIRST_RUN_REASON {
        let mut lines: Vec<String> = vec![name_value_line(
            &model.name,
            &model.materialization_label,
            width,
            true,
        )];
        lines.extend(cursor_lines(model, true));
        return lines;
    }
    let mut lines: Vec<String> = vec![name_value_line(
        &model.name,
        &STYLE.accent(&action_text(model)),
        width,
        false,
    )];
    lines.extend(cursor_lines(model, reason != FULL_REFRESH_REASON));
    lines.extend(policy_lines(model));
    if !model.changed_functions.is_empty() {
        lines.push(format!(
            "    cause: function {} changed",
            model.changed_functions.join(", ")
        ));
    }
    lines.extend(schema_diff_lines(model));
    lines.extend(diff_lines("    config diff:", model.config_diff.as_ref()));
    lines.extend(diff_lines("    query diff:", model.query_diff.as_ref()));
    lines
}

/// One section per change reason, in reason order, with each model's detail.
pub(crate) fn reason_group_lines(
    report: &PlanTextReport,
    active: &[&ModelPlanText],
    width: usize,
) -> Vec<String> {
    let mut lines: Vec<String> = Vec::new();
    for (reason, label) in REASON_GROUPS {
        let entries: Vec<&ModelPlanText> = active
            .iter()
            .copied()
            .filter(|model| model.reason == reason)
            .collect();
        if entries.is_empty() {
            continue;
        }
        lines.extend(section_lines(label, &entries, &report.display, |model| {
            detail_entry_lines(model, reason, width)
        }));
    }
    lines
}

/// Unchanged and normally incremental models.
pub(crate) fn routine_model_lines(
    report: &PlanTextReport,
    active: &[&ModelPlanText],
    width: usize,
) -> Vec<String> {
    let routine: Vec<&ModelPlanText> = active
        .iter()
        .copied()
        .filter(|model| {
            model.reason == NO_CHANGE_REASON || model.reason == NORMAL_INCREMENTAL_REASON
        })
        .collect();
    if routine.is_empty() {
        return Vec::new();
    }
    section_lines("Models", &routine, &report.display, |model| {
        let mut lines: Vec<String> = vec![name_value_line(
            &model.name,
            &model.materialization_label,
            width,
            true,
        )];
        lines.extend(cursor_lines(model, true));
        if let Some(upstream) = &model.reinitialised_by {
            lines.push(format!("    recomputes in full: {upstream} is rebuilt"));
        }
        if model.paused {
            lines.push("    paused: scheduled refreshes are suspended".to_owned());
        }
        lines
    })
}

fn function_lines(function: &FunctionPlanText, show_details: bool, width: usize) -> Vec<String> {
    let mut lines: Vec<String> = vec![name_value_line(&function.name, &function.kind, width, true)];
    if !show_details {
        return lines;
    }
    match function.reason.as_str() {
        FIRST_RUN_REASON => lines.push("    reason: first run".to_owned()),
        FULL_REFRESH_REASON => lines.push("    reason: full refresh".to_owned()),
        QUERY_CHANGED_REASON => {
            lines.extend(diff_lines("    query diff:", function.query_diff.as_ref()));
        }
        _ => {}
    }
    lines
}

/// The changed functions with their detail, or the unchanged ones.
pub(crate) fn functions_lines(report: &PlanTextReport, changed: bool, width: usize) -> Vec<String> {
    let entries: Vec<&FunctionPlanText> = report
        .functions
        .iter()
        .filter(|function| (function.reason != NO_CHANGE_REASON) == changed)
        .collect();
    if entries.is_empty() {
        return Vec::new();
    }
    let title: &str = if changed {
        "Changed functions"
    } else {
        "Functions"
    };
    section_lines(title, &entries, &report.display, |function| {
        function_lines(function, changed, width)
    })
}

/// The seeds with their load reason.
pub(crate) fn seed_lines(report: &PlanTextReport) -> Vec<String> {
    if report.seeds.is_empty() {
        return Vec::new();
    }
    section_lines("Seeds", &report.seeds, &report.display, |seed| {
        let label: &str = match seed.reason.as_str() {
            FIRST_RUN_REASON => "first_run",
            CONFIG_CHANGED_REASON => "seed_changed",
            NO_CHANGE_REASON => "current",
            _ => "",
        };
        if label.is_empty() {
            return vec![format!("  {}", seed.name)];
        }
        vec![format!("  {}  ({label})", seed.name)]
    })
}

/// `Audits (N)`: the audits selected by name, which run whether or not their targets build.
pub(crate) fn audit_lines(report: &PlanTextReport) -> Vec<String> {
    if report.audits.is_empty() {
        return Vec::new();
    }
    section_lines("Audits", &report.audits, &report.display, |audit| {
        vec![format!("  {audit}")]
    })
}

/// `Audits not run (N)`: end-of-build audits that read something the target doesn't have, each naming what.
pub(crate) fn not_run_audit_lines(report: &PlanTextReport) -> Vec<String> {
    if report.audits_not_run.is_empty() {
        return Vec::new();
    }
    section_lines(
        "Audits not run",
        &report.audits_not_run,
        &report.display,
        |(audit, missing)| {
            let verb: &str = if missing.len() == 1 {
                "which doesn't"
            } else {
                "which don't"
            };
            vec![format!(
                "  {audit}  reads {}, {verb} exist in the target",
                missing.join(", ")
            )]
        },
    )
}

/// `Full refresh (N)` with one count row per materialization label, most common first.
pub(crate) fn full_refresh_count_lines(active: &[&ModelPlanText]) -> Vec<String> {
    let mut counts: Vec<(&str, usize)> = Vec::new();
    for model in active {
        if let Some((_, count)) = counts
            .iter_mut()
            .find(|(label, _)| *label == model.materialization_label)
        {
            *count += 1;
        } else {
            counts.push((&model.materialization_label, 1));
        }
    }
    counts.sort_by_key(|(_, count)| std::cmp::Reverse(*count));
    let mut lines: Vec<String> = vec![section_header(
        STYLE,
        &format!("Full refresh ({})", active.len()),
    )];
    for (label, count) in counts {
        lines.push(format!("  {count:>3} {label}"));
    }
    let unbounded: usize = active
        .iter()
        .filter(|model| {
            model
                .cursor
                .as_ref()
                .and_then(|cursor| cursor.range.as_deref())
                .is_some_and(|range| range.ends_with(UNBOUNDED_RANGE))
        })
        .count();
    if unbounded > 0 {
        lines.push(format!("  {unbounded:>3} cursor range {UNBOUNDED_RANGE}"));
    }
    lines
}

fn diagnostic_section_lines(
    (entries, title, error): (&[&PlanWarningText], &str, bool),
    display: &DisplayLimit,
) -> Vec<String> {
    if entries.is_empty() {
        return Vec::new();
    }
    let line_style = |text: &str| {
        if error {
            STYLE.error(text)
        } else {
            STYLE.warning(text)
        }
    };
    let heading: String = format!("{title} ({})", entries.len());
    let mut lines: Vec<String> = vec![
        String::new(),
        if error {
            STYLE.error_strong(&heading)
        } else {
            STYLE.warning_strong(&heading)
        },
    ];
    let visible: usize = visible_count(entries.len(), display);
    for (index, warning) in entries[..visible].iter().enumerate() {
        let connector: String = tree_connector(STYLE, index + 1 == visible);
        let message_lines: Vec<&str> = warning.message.split('\n').collect();
        if let Some(model) = &warning.model_name {
            lines.push(format!("{connector} {}", STYLE.object_name(model)));
            for (line_index, line) in message_lines.iter().enumerate() {
                let child: String = tree_connector(STYLE, line_index + 1 == message_lines.len());
                lines.push(format!("    {child} {}", line_style(line)));
            }
        } else {
            lines.push(format!("{connector} {}", line_style(message_lines[0])));
            for line in &message_lines[1..] {
                lines.push(format!("    {}", line_style(line)));
            }
        }
    }
    lines.extend(overflow_line((entries.len(), visible), "  ", display));
    lines
}

/// The plan's errors, then its warnings, without the diagnostics other sections already show.
pub(crate) fn warning_lines(report: &PlanTextReport) -> Vec<String> {
    let include_stale: bool = report
        .selection_diagnostics
        .unwrap_or(report.include_freshness_diagnostics);
    let diagnostics: Vec<&PlanWarningText> = report
        .warnings
        .iter()
        .filter(|warning| {
            warning.severity != INFO_SEVERITY_VALUE
                && (include_stale || !warning.message.starts_with(STALE_INPUT_WARNING_TITLE))
                && !(report.include_freshness_diagnostics
                    && warning
                        .message
                        .starts_with(SOURCE_FRESHNESS_UNKNOWN_WARNING_TITLE))
        })
        .collect();
    let (errors, warnings): (Vec<&PlanWarningText>, Vec<&PlanWarningText>) = diagnostics
        .into_iter()
        .partition(|warning| warning.severity == ERROR_SEVERITY_VALUE);
    let mut lines: Vec<String> =
        diagnostic_section_lines((&errors, "Errors", true), &report.display);
    lines.extend(diagnostic_section_lines(
        (&warnings, "Warnings", false),
        &report.display,
    ));
    lines
}

fn freshness_count(label: &str, names: &[String], warn_nonzero: bool) -> String {
    let count: String = names.len().to_string();
    let styled: String = if warn_nonzero && !names.is_empty() {
        STYLE.warning(&count)
    } else if names.is_empty() {
        STYLE.muted(&count)
    } else {
        count
    };
    format!("  {} {styled}", STYLE.label(&format!("{label}:")))
}

fn freshness_set(label: &str, names: &[String], display: &DisplayLimit, warn: bool) -> String {
    let rendered: String = capped_names(names, display, |name| {
        if warn {
            STYLE.warning(name)
        } else {
            STYLE.object_name(name)
        }
    });
    format!("  {} {rendered}", STYLE.label(&format!("{label}:")))
}

/// The source freshness a direct plan observed, by outcome.
pub(crate) fn source_freshness_lines(report: &PlanTextReport) -> Vec<String> {
    let Some(freshness) = &report.source_freshness else {
        return Vec::new();
    };
    let SourceFreshnessText {
        observed,
        changed,
        unchanged,
        unknown,
        age_warnings,
        age_errors,
        stale_models,
        blocked_models,
    } = freshness;
    if observed.is_empty() && unknown.is_empty() {
        return Vec::new();
    }
    let display: &DisplayLimit = &report.display;
    let mut lines: Vec<String> = vec![String::new(), section_header(STYLE, "Source freshness")];
    for (label, names, warn) in [
        ("observed", observed, false),
        ("changed", changed, true),
        ("unchanged", unchanged, false),
        ("unknown", unknown, true),
    ] {
        lines.push(freshness_count(label, names, warn));
        if !names.is_empty() {
            lines.push(freshness_set(&format!("{label} set"), names, display, warn));
        }
    }
    for (label, names) in [
        ("age warnings", age_warnings),
        ("age errors", age_errors),
        ("source-stale models", stale_models),
        ("source-blocked models", blocked_models),
    ] {
        if !names.is_empty() {
            lines.push(freshness_set(label, names, display, true));
        }
    }
    lines
}

/// Stale models outside the selection, unless selection diagnostics are off.
pub(crate) fn remaining_stale_lines(report: &PlanTextReport) -> Vec<String> {
    if report.selection_diagnostics == Some(false) || report.remaining_stale_models.is_empty() {
        return Vec::new();
    }
    let names: &[String] = &report.remaining_stale_models;
    vec![
        String::new(),
        section_header(STYLE, "Remaining stale"),
        STYLE.muted(&format!("  models outside selection: {}", names.len())),
        STYLE.muted(&format!(
            "  model set: {}",
            capped_names(names, &report.display, |name| STYLE.muted(name))
        )),
    ]
}
