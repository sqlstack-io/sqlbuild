//! Why `--changed` runs a node, as one line for verbose output.

use sqlbuild_state::consumption::models::{ForcedRun, RunReason};

/// Why `--changed` runs a node, as one line for verbose output.
pub fn run_reason_text(reason: &RunReason) -> String {
    match reason {
        RunReason::Forced(ForcedRun::PlannerChange(kind)) => format!("planner change ({kind})"),
        RunReason::Forced(ForcedRun::FullRefresh) => "full refresh".to_owned(),
        RunReason::Forced(ForcedRun::RelationMissing) => "relation missing".to_owned(),
        RunReason::Forced(ForcedRun::CursorInterval) => "cursor interval requested".to_owned(),
        RunReason::Forced(ForcedRun::InputsNotEnumerable) => {
            "inputs cannot be enumerated".to_owned()
        }
        RunReason::NoRecord => "no completed record".to_owned(),
        RunReason::Unfinished => "last execution did not finish".to_owned(),
        RunReason::AmbiguousOrder => "record order is ambiguous".to_owned(),
        RunReason::DefinitionChanged => "definition changed".to_owned(),
        RunReason::InputsChanged => "inputs changed".to_owned(),
        RunReason::InputChanged(input) => format!("input {input} changed"),
        RunReason::UpstreamDefinitionChanged(input) => {
            format!("upstream {input} was rebuilt with a definition change")
        }
        RunReason::InputUnknown(input) => format!("input {input} has no known version"),
        RunReason::UpstreamRuns(key) => format!("upstream {} {} runs", key.kind, key.name),
    }
}
