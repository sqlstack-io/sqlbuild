//! Job settings that cannot apply together.

use crate::graph::errors::SelectorError;

/// Two named jobs give one node, or the whole build, different values of one setting.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct JobSettingConflict {
    pub message: String,
    pub help: &'static str,
}

/// Why the jobs a selection names cannot give their nodes settings.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum NodeJobSettingsError {
    Selector(SelectorError),
    Conflict(JobSettingConflict),
}
