use std::path::PathBuf;
use std::process::Child;

pub(crate) struct CommandTestCase {
    pub(crate) description: &'static str,
    pub(crate) arguments: &'static [&'static str],
    pub(crate) expected_success: bool,
    pub(crate) expected_stdout_prefix: &'static str,
    pub(crate) expected_stderr: &'static str,
}

pub(crate) struct TriggerCursorTestCase {
    pub(crate) description: &'static str,
    pub(crate) change: &'static str,
    pub(crate) expected_success: bool,
}

pub(crate) struct OpenApiTestCase {
    pub(crate) description: &'static str,
    pub(crate) expected_path: &'static str,
}

pub(crate) struct ServiceTestCase {
    pub(crate) description: &'static str,
    pub(crate) arguments: &'static [&'static str],
    pub(crate) expected_leaders: i64,
    pub(crate) expected_heartbeats: i64,
}

pub(crate) struct TestControl {
    pub(crate) child: Child,
    pub(crate) directory: tempfile::TempDir,
    pub(crate) stderr: PathBuf,
}

impl Drop for TestControl {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}
