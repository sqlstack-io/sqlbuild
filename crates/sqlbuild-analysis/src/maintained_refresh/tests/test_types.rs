use crate::maintained_refresh::models::{
    LiveDynamicTable, MaintainedRead, MaintainedSettings, RefreshLag,
};

pub(super) struct TargetLagTestCase {
    pub(super) description: &'static str,
    pub(super) text: &'static str,
    pub(super) expected_lag: Option<RefreshLag>,
}

pub(super) struct RenderLagTestCase {
    pub(super) description: &'static str,
    pub(super) lag: RefreshLag,
    pub(super) expected_sql: &'static str,
}

pub(super) struct DriftTestCase {
    pub(super) description: &'static str,
    pub(super) desired: MaintainedSettings,
    pub(super) live: LiveDynamicTable,
    pub(super) expected_drift: bool,
}

pub(super) struct ReinitialiseTestCase {
    pub(super) description: &'static str,
    pub(super) reads: Vec<MaintainedRead>,
    pub(super) expected_readers: Vec<(&'static str, &'static str)>,
}
