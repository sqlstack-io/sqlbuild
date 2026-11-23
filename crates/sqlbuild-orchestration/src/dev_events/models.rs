//! The events a `sqb dev` location child sends its parent, one JSON line each.

use serde::{Deserialize, Serialize};

use crate::platform::models::MissingProviderSetting;

/// One step of publishing a location generation.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DevPhase {
    Discover,
    Compile,
    Publish,
}

/// Where a step stands.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DevStepState {
    Started,
    Finished,
}

/// A file position an error points at.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct SourcePosition {
    pub path: String,
    pub line: u32,
    pub column: u32,
}

/// A provider whose unset secrets stop the runs that use it: where each is declared and what needs it, never a value.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct UnsetProviderSecrets {
    pub provider: String,
    /// The provider's file, relative to the project directory.
    pub path: String,
    pub settings: Vec<MissingProviderSetting>,
    /// The node keys that use the provider, then those that read them.
    pub needed_by: Vec<String>,
}

/// What happened in the location child.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "event", rename_all = "snake_case")]
pub enum DevEvent {
    /// A publication step started or finished; `detail` summarises a finished step.
    Step {
        phase: DevPhase,
        state: DevStepState,
        detail: Option<String>,
    },
    /// Project inputs changed; `paths` lists the first few, `more` counts the rest.
    Changed { paths: Vec<String>, more: usize },
    /// The interpreter restarted because Python sources changed.
    Restarted { reason: String },
    /// A generation was published; `record` is the publication record `sqb location serve` prints.
    Published {
        generation: i64,
        snapshot_id: String,
        record: serde_json::Value,
    },
    /// A generation failed; the last good generation keeps serving.
    Failed {
        generation: i64,
        message: String,
        position: Option<SourcePosition>,
        /// The error's code, such as `D001`, when it has one.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        code: Option<String>,
        /// How to fix it, when the error says.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        help: Option<String>,
    },
    /// Something the location could not do, such as a warehouse sync that stopped; it keeps serving.
    Warning { title: String, detail: String },
    /// The providers the latest provider-secrets health shows missing secrets the target doesn't allow to miss; empty once none is.
    MissingSecrets {
        providers: Vec<UnsetProviderSecrets>,
        /// The env file the location loads, relative to the project; `None` when it loads none.
        env_file: Option<String>,
    },
    /// The discovered sets a publication used, summarised for the ready card.
    Discovered { summary: String },
}

/// What `sqb dev` asks its location child to do, one JSON line each on the child's stdin.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "control", rename_all = "snake_case")]
pub enum DevControl {
    /// The foreground warehouse connection opened, or was skipped or failed.
    Warehouse { connected: bool },
    /// Compile and publish a new generation even when no input changed.
    Recompile,
}
