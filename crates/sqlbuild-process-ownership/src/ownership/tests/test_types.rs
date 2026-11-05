/// `holder` and `marker` index the case's two claims: 0 is the recovered claim, 1 another one.
pub(super) struct RecoveryTestCase {
    pub(super) description: &'static str,
    pub(super) holder: Option<usize>,
    pub(super) marker: usize,
    pub(super) lifetime_seconds: u32,
    pub(super) force: bool,
    pub(super) expected_exited_before_recovery: bool,
    pub(super) expected_recovered_exited: bool,
    pub(super) expected_stopped: bool,
}

#[cfg(windows)]
pub(super) struct JobAccessTestCase {
    pub(super) description: &'static str,
    pub(super) access: u32,
    pub(super) expected_own_access: Result<(), u32>,
    pub(super) expected_foreign_access: Result<(), u32>,
}
