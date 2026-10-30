use std::path::PathBuf;
use std::process::Child;

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
