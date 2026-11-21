//! Codes the snapshot's worker reports on its own, and how provider consumers run in snapshot nodes.

use crate::project_snapshot::models::RunsIn;

/// The code a node fails with when a provider it uses is missing secrets, as `sqb build` reports it.
pub(crate) const MISSING_SECRET_CODE: &str = "D012";

/// Snapshot node kinds a provider consumer can run inside.
pub(crate) const MODEL_NODE_KIND: &str = "model";
pub(crate) const SOURCE_NODE_KIND: &str = "source";

/// The setting-row field the published providers document leaves out: secret matching reads it, the UI does not.
pub(crate) const SNAPSHOT_HIDDEN_SETTING: &str = "alias";

/// Discovered collections whose items inject providers, the kind each is published as, and where it runs.
pub(crate) const CONSUMERS: [(&str, &str, RunsIn); 8] = [
    ("loader", "loader_functions", RunsIn::SourceLoader),
    ("task", "task_functions", RunsIn::Itself),
    ("asset", "asset_functions", RunsIn::Itself),
    ("check", "check_functions", RunsIn::Itself),
    ("hook", "hook_functions", RunsIn::ModelHooks),
    (
        "materialization",
        "materialization_files",
        RunsIn::ModelMaterialization,
    ),
    ("event_exporter", "event_exporters", RunsIn::Itself),
    ("sink", "command_output_sinks", RunsIn::Itself),
];
