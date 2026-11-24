//! Which rows of a plan document belong to each section a summary counts and the rows endpoint pages.

use std::collections::{HashMap, HashSet};

use serde_json::Value;

use crate::plan_display::_helpers::diff_lines::diff_stats;
use crate::plan_display::_helpers::warning_groups::warning_groups;
use crate::plan_display::constants::{
    DIFF_STATS_FIELD, FIRST_RUN_REASON, IS_RENAME_FIELD, NO_CHANGE_REASON, ORIGIN_MODEL_FIELD,
    SKIP_ACTION,
};
use crate::plan_display::models::PlanPageSection;

/// Every section, in the order the plan reads, with its name in the API.
pub(crate) const SECTIONS: [(PlanPageSection, &str); 26] = [
    (PlanPageSection::PlanningError, "planning_error"),
    (PlanPageSection::FullRefresh, "full_refresh"),
    (PlanPageSection::QueryChanged, "query_changed"),
    (PlanPageSection::FunctionChanged, "function_changed"),
    (PlanPageSection::ConfigChanged, "config_changed"),
    (PlanPageSection::SchemaChanged, "schema_changed"),
    (PlanPageSection::FirstRun, "first_run"),
    (PlanPageSection::Renamed, "renamed"),
    (PlanPageSection::RoutineModels, "routine_models"),
    (PlanPageSection::Migrations, "migrations"),
    (PlanPageSection::ColumnMigrations, "column_migrations"),
    (
        PlanPageSection::TableTypeConversions,
        "table_type_conversions",
    ),
    (PlanPageSection::Retention, "retention"),
    (PlanPageSection::LoaderReloads, "loader_reloads"),
    (PlanPageSection::LoaderLoads, "loader_loads"),
    (PlanPageSection::SourceReloads, "source_reloads"),
    (PlanPageSection::SourceLoads, "source_loads"),
    (PlanPageSection::ChangedFunctions, "changed_functions"),
    (PlanPageSection::Functions, "functions"),
    (PlanPageSection::FirstRunSeeds, "first_run_seeds"),
    (PlanPageSection::ChangedSeeds, "changed_seeds"),
    (PlanPageSection::UnchangedSeeds, "unchanged_seeds"),
    (PlanPageSection::PythonNodes, "python_nodes"),
    (PlanPageSection::HeldBack, "held_back"),
    (PlanPageSection::Unchanged, "unchanged"),
    (PlanPageSection::Warnings, "warnings"),
];

/// A model's section by its plan reason; reasons without their own section build as routine models.
const MODEL_REASONS: [(&str, PlanPageSection); 8] = [
    ("planning_error", PlanPageSection::PlanningError),
    ("full_refresh", PlanPageSection::FullRefresh),
    ("query_changed", PlanPageSection::QueryChanged),
    ("function_changed", PlanPageSection::FunctionChanged),
    ("config_changed", PlanPageSection::ConfigChanged),
    ("schema_changed", PlanPageSection::SchemaChanged),
    ("first_run", PlanPageSection::FirstRun),
    ("renamed", PlanPageSection::Renamed),
];

/// One note a row carries: its kind and, for most kinds, a value, such as `run_reason` `no completed record`.
pub(crate) type Note = (&'static str, Option<String>);

/// A plan document read once for paging, with its `--changed` runs by node key.
pub(crate) struct PlanRows<'document> {
    pub(crate) document: &'document Value,
    pub(crate) runs: Option<HashMap<&'document str, &'document Value>>,
}

impl<'document> PlanRows<'document> {
    pub(crate) fn new(document: &'document Value) -> Self {
        let listed = document
            .get("changed")
            .and_then(|changed| changed.get("runs"))
            .and_then(Value::as_array);
        let runs = listed.map(|listed| {
            let mut runs: HashMap<&str, &Value> = HashMap::new();
            for run in listed {
                if let Some(node) = run_node(run) {
                    runs.insert(node, run);
                }
            }
            runs
        });
        Self { document, runs }
    }

    /// The migration steps: every migration-like row except renamed models' own migrations.
    pub(crate) fn migration_steps(&self) -> usize {
        let migrations = self
            .list("migrations")
            .into_iter()
            .filter(|migration| {
                migration.get(IS_RENAME_FIELD).and_then(Value::as_bool) != Some(true)
            })
            .count();
        migrations
            + self.list("column_migrations").len()
            + self.list("table_type_conversions").len()
            + self.list("retention").len()
    }

    /// The section's rows in plan order.
    pub(crate) fn rows(&self, section: PlanPageSection) -> Vec<&'document Value> {
        match section {
            PlanPageSection::Migrations => self.list("migrations"),
            PlanPageSection::ColumnMigrations => self.list("column_migrations"),
            PlanPageSection::TableTypeConversions => self.list("table_type_conversions"),
            PlanPageSection::Retention => self.list("retention"),
            PlanPageSection::LoaderReloads => self.loads("loader", true),
            PlanPageSection::LoaderLoads => self.loads("loader", false),
            PlanPageSection::SourceReloads => self.loads("source", true),
            PlanPageSection::SourceLoads => self.loads("source", false),
            PlanPageSection::ChangedFunctions => self.functions(true),
            PlanPageSection::Functions => self.functions(false),
            PlanPageSection::FirstRunSeeds
            | PlanPageSection::ChangedSeeds
            | PlanPageSection::UnchangedSeeds => self.seeds(section),
            PlanPageSection::PythonNodes => self.list("python_nodes"),
            PlanPageSection::HeldBack => self.changed_list("held_back"),
            PlanPageSection::Unchanged => self.changed_list("unchanged"),
            PlanPageSection::Warnings => warning_groups(&self.list("warnings"))
                .into_iter()
                .flat_map(|group| group.warnings)
                .collect(),
            model_section => self.models(model_section),
        }
    }

    fn list(&self, field: &str) -> Vec<&'document Value> {
        self.document
            .get(field)
            .and_then(Value::as_array)
            .map(|rows| rows.iter().collect())
            .unwrap_or_default()
    }

    /// The rows of the document's `field` list that `keep` accepts, in order.
    fn kept(&self, field: &str, keep: impl Fn(&Value) -> bool) -> Vec<&'document Value> {
        let mut rows: Vec<&'document Value> = self.list(field);
        rows.retain(|row| keep(row));
        rows
    }

    fn changed_list(&self, field: &str) -> Vec<&'document Value> {
        self.document
            .get("changed")
            .and_then(|changed| changed.get(field))
            .and_then(Value::as_array)
            .map(|rows| rows.iter().collect())
            .unwrap_or_default()
    }

    /// Whether `--changed` runs the node; every node runs without `--changed`.
    fn runs_node(&self, key: &str) -> bool {
        self.runs.as_ref().is_none_or(|runs| runs.contains_key(key))
    }

    fn models(&self, section: PlanPageSection) -> Vec<&'document Value> {
        self.kept("models", |model| {
            text(model, "action") != SKIP_ACTION
                && (section == PlanPageSection::PlanningError
                    || self.runs_node(&node_key("model", model)))
                && model_section(model) == section
        })
    }

    fn loads(&self, kind: &str, reload: bool) -> Vec<&'document Value> {
        self.kept("source_loads", |load| {
            let reloads = load.get("is_reload").and_then(Value::as_bool) == Some(true);
            text(load, "kind") == kind && reloads == reload
        })
    }

    fn functions(&self, changed: bool) -> Vec<&'document Value> {
        self.kept("functions", |function| {
            let reason = text(function, "reason");
            (!reason.is_empty() && reason != NO_CHANGE_REASON) == changed
        })
    }

    fn seeds(&self, section: PlanPageSection) -> Vec<&'document Value> {
        self.kept("seeds", |seed| {
            self.runs_node(&node_key("seed", seed)) && seed_section(seed) == section
        })
    }

    /// The `--changed` run of a model or seed row.
    pub(crate) fn run_of(&self, key: &str) -> Option<&'document Value> {
        self.runs.as_ref()?.get(key).copied()
    }

    /// The notes a row of `section` carries, for the summary's counts.
    pub(crate) fn notes(&self, section: PlanPageSection, row: &Value) -> Vec<Note> {
        let mut notes: Vec<Note> = Vec::new();
        let fields: &[(&'static str, &str)] = match section {
            PlanPageSection::Migrations => &[("decision", "decision")],
            PlanPageSection::ColumnMigrations => &[("decision", "decision")],
            PlanPageSection::TableTypeConversions => &[("desired_type", "desired_type")],
            PlanPageSection::Retention => &[("action", "action")],
            PlanPageSection::LoaderReloads
            | PlanPageSection::LoaderLoads
            | PlanPageSection::SourceReloads
            | PlanPageSection::SourceLoads => &[
                ("write_strategy", "write_strategy"),
                ("cursor_column", "cursor_column"),
            ],
            PlanPageSection::ChangedFunctions | PlanPageSection::Functions => {
                &[("language", "language"), ("reason", "reason")]
            }
            PlanPageSection::FirstRunSeeds
            | PlanPageSection::ChangedSeeds
            | PlanPageSection::UnchangedSeeds => &[("reason", "reason")],
            PlanPageSection::PythonNodes => &[
                ("kind", "kind"),
                ("identity_status", "identity_status"),
                ("phase", "phase"),
            ],
            PlanPageSection::Warnings => &[("severity", "severity"), ("code", "code")],
            PlanPageSection::HeldBack | PlanPageSection::Unchanged => &[],
            _ => &[
                ("materialization", "materialization_type"),
                ("action", "action"),
                ("cursor_column", "cursor_column"),
            ],
        };
        for (kind, field) in fields {
            if let Some(value) = row.get(*field).and_then(Value::as_str) {
                notes.push((kind, Some(value.to_owned())));
            }
        }
        match section {
            PlanPageSection::Migrations => {
                let completed = row.get("completed_at").is_some_and(|at| !at.is_null());
                notes.push(("completed", Some(completed.to_string())));
            }
            PlanPageSection::HeldBack => notes.extend(held_back_notes(row)),
            PlanPageSection::FirstRunSeeds
            | PlanPageSection::ChangedSeeds
            | PlanPageSection::UnchangedSeeds => {
                notes.extend(self.run_notes(&node_key("seed", row)));
            }
            section if model_rows(section) => {
                notes.extend(self.run_notes(&node_key("model", row)));
                notes.extend(model_notes(row));
            }
            _ => {}
        }
        notes
    }

    /// Why `--changed` runs the node: what it reads that changed, or its own reason.
    fn run_notes(&self, key: &str) -> Vec<Note> {
        let Some(run) = self.run_of(key) else {
            return Vec::new();
        };
        match run.get("downstream_of").and_then(Value::as_str) {
            Some(upstream) => vec![("reads", Some(key_name(upstream).to_owned()))],
            None => vec![("run_reason", Some(text(run, "reason").to_owned()))],
        }
    }
}

/// A model row's changed functions, and whether its query diff changes only whitespace.
fn model_notes(row: &Value) -> Vec<Note> {
    let mut notes: Vec<Note> = listed_notes(row, ("changed_functions", "changed_function"));
    if query_stats(row).is_some_and(|stats| stats.whitespace_only) {
        notes.push(("query_whitespace_only", None));
    }
    notes
}

/// One `note` per text in the row's `field` array.
fn listed_notes(row: &Value, (field, note): (&str, &'static str)) -> Vec<Note> {
    row.get(field)
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(Value::as_str)
        .map(|item| (note, Some(item.to_owned())))
        .collect()
}

/// What refreshes a held-back node: the jobs it waits for, or the held-back node it reads.
fn held_back_notes(row: &Value) -> Vec<Note> {
    let mut notes: Vec<Note> = listed_notes(row, ("refreshed_by", "refreshed_by"));
    if let Some(node) = row.get("reads").and_then(Value::as_str) {
        notes.push(("reads_held_back", Some(key_name(node).to_owned())));
    }
    notes
}

/// The query diff's counts: from its text, else the counts kept when the text was left out.
pub(crate) fn query_stats(row: &Value) -> Option<crate::plan_display::models::DiffStats> {
    match lines(row, "query_diff") {
        Some(diff) => Some(diff_stats(&diff)),
        None => stored_stats(row, "query"),
    }
}

/// A model row's diff counts by part, `query` and `config`, from text or kept counts.
pub(crate) fn row_diff_stats(row: &Value) -> serde_json::Map<String, Value> {
    let mut stats = serde_json::Map::new();
    for (part, field) in [("query", "query_diff"), ("config", "config_diff")] {
        let counted = match lines(row, field) {
            Some(diff) => Some(diff_stats(&diff)),
            None => stored_stats(row, part),
        };
        if let Some(counted) = counted {
            stats.insert(part.to_owned(), stats_json(counted));
        }
    }
    stats
}

/// The counts as the document and the API write them.
pub(crate) fn stats_json(stats: crate::plan_display::models::DiffStats) -> Value {
    serde_json::json!({
        "added": stats.added,
        "removed": stats.removed,
        "whitespace_only": stats.whitespace_only,
    })
}

fn stored_stats(row: &Value, part: &str) -> Option<crate::plan_display::models::DiffStats> {
    let stats = row.get(DIFF_STATS_FIELD)?.get(part)?;
    Some(crate::plan_display::models::DiffStats {
        added: usize::try_from(stats.get("added")?.as_u64()?).unwrap_or(usize::MAX),
        removed: usize::try_from(stats.get("removed")?.as_u64()?).unwrap_or(usize::MAX),
        whitespace_only: stats.get("whitespace_only")?.as_bool()?,
    })
}

/// A diff field's lines, when the row has them.
pub(crate) fn lines(row: &Value, field: &str) -> Option<Vec<String>> {
    let diff = row.get(field)?.as_array()?;
    Some(
        diff.iter()
            .filter_map(Value::as_str)
            .map(str::to_owned)
            .collect(),
    )
}

/// Whether the section's rows are planned models.
pub(crate) fn model_rows(section: PlanPageSection) -> bool {
    MODEL_REASONS.iter().any(|(_, model)| *model == section)
        || section == PlanPageSection::RoutineModels
}

/// A seed's section by its plan reason: a first run, no change of its own, or any other change.
fn seed_section(seed: &Value) -> PlanPageSection {
    match text(seed, "reason") {
        FIRST_RUN_REASON => PlanPageSection::FirstRunSeeds,
        "" | NO_CHANGE_REASON => PlanPageSection::UnchangedSeeds,
        _ => PlanPageSection::ChangedSeeds,
    }
}

fn model_section(model: &Value) -> PlanPageSection {
    let reason = text(model, "reason");
    MODEL_REASONS
        .iter()
        .find(|(name, _)| *name == reason)
        .map_or(PlanPageSection::RoutineModels, |(_, section)| *section)
}

/// The name a row is found by; renames match old names and retention changes their models.
pub(crate) fn row_name(section: PlanPageSection, row: &Value) -> String {
    match section {
        PlanPageSection::Renamed => {
            format!("{} {}", text(row, "name"), text(row, ORIGIN_MODEL_FIELD))
        }
        PlanPageSection::Migrations
        | PlanPageSection::ColumnMigrations
        | PlanPageSection::TableTypeConversions => text(row, "model").to_owned(),
        PlanPageSection::Retention => {
            let mut names: Vec<&str> = row
                .get("models")
                .and_then(Value::as_array)
                .into_iter()
                .flatten()
                .filter_map(Value::as_str)
                .collect();
            names.push(text(row, "name"));
            names.join(" ")
        }
        PlanPageSection::HeldBack => key_name(text(row, "node")).to_owned(),
        PlanPageSection::Unchanged => key_name(row.as_str().unwrap_or_default()).to_owned(),
        PlanPageSection::Warnings => {
            format!("{} {}", text(row, "model_name"), text(row, "message"))
        }
        _ => text(row, "name").to_owned(),
    }
}

/// The warnings about the named models: the warning names the model, or opens its message with it.
pub(crate) fn warnings_about<'document>(
    document: &'document Value,
    names: &HashSet<String>,
) -> Vec<&'document Value> {
    let mut found: Vec<&Value> = Vec::new();
    for warning in document
        .get("warnings")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
    {
        let named = warning
            .get("model_name")
            .and_then(Value::as_str)
            .or_else(|| {
                text(warning, "message")
                    .split_once(':')
                    .map(|(name, _)| name)
            });
        if named.is_some_and(|name| names.contains(name)) {
            found.push(warning);
        }
    }
    found
}

/// The node key of a model or seed row, such as `model:orders`.
pub(crate) fn node_key(kind: &str, row: &Value) -> String {
    format!("{kind}:{}", text(row, "name"))
}

/// The node key a `--changed` run names.
pub(crate) fn run_node(run: &Value) -> Option<&str> {
    run.get("node").and_then(Value::as_str)
}

/// A node key's name, such as `orders` for `model:orders`.
pub(crate) fn key_name(key: &str) -> &str {
    key.split_once(':').map_or(key, |(_, name)| name)
}

pub(crate) fn text<'value>(entry: &'value Value, field: &str) -> &'value str {
    entry.get(field).and_then(Value::as_str).unwrap_or_default()
}
