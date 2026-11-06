/// One artifact batch written over a target where some files already exist.
pub(crate) struct WriteBatchTestCase {
    pub(crate) description: &'static str,
    pub(crate) file_count: usize,
    pub(crate) check_existing: bool,
    pub(crate) existing_identical: &'static [usize],
    pub(crate) existing_different: &'static [usize],
    pub(crate) expected_written: usize,
    pub(crate) expected_unchanged: usize,
}

/// One batch with existing non-UTF-8 files at several positions.
pub(crate) struct FirstFailureTestCase {
    pub(crate) description: &'static str,
    pub(crate) file_count: usize,
    pub(crate) failing: &'static [usize],
    pub(crate) expected_failing: usize,
}

/// One batch that queues the same path more than once.
pub(crate) struct RepeatedPathTestCase {
    pub(crate) description: &'static str,
    pub(crate) file_count: usize,
    pub(crate) repeated: usize,
    pub(crate) expected_contents: &'static [u8],
}

/// A change between two stale-file passes over a stamped compiled tree.
#[cfg(unix)]
pub(crate) struct StampedTreeTestCase {
    pub(crate) description: &'static str,
    /// Changes the target folder holding `compiled/` and `cache/` after the first pass.
    pub(crate) change: fn(&std::path::Path),
    /// The files the second pass manages, relative to the compiled folder.
    pub(crate) managed_after: &'static [&'static str],
    pub(crate) racy_window_ns: i64,
    /// Creates the `compiled/` folder under the target folder.
    pub(crate) layout: fn(&std::path::Path),
    pub(crate) expected_stamped: bool,
    pub(crate) expected_removed: usize,
    pub(crate) expected_files: &'static [&'static str],
}

/// A change made to the compiled tree after its stale-file walk started.
#[cfg(unix)]
pub(crate) struct WalkRaceTestCase {
    pub(crate) description: &'static str,
    pub(crate) change_during_walk: fn(&std::path::Path),
    pub(crate) expected_stamped: bool,
}

/// A change to a recorded SQL test artifact and whether, or why not, the record is kept.
pub(crate) struct ReusableRecordTestCase {
    pub(crate) description: &'static str,
    /// Changes the tests folder after the artifact `t.sql` was recorded.
    pub(crate) change: fn(&std::path::Path),
    pub(crate) identity: &'static str,
    pub(crate) expected_outcome: Result<(), crate::artifacts::models::SqlTestRecordMiss>,
}

/// A cache snapshot's externally visible encoding and recovered records.
pub(crate) struct SqlTestSnapshotWriteTestCase {
    pub(crate) description: &'static str,
    pub(crate) expected_text: &'static str,
    pub(crate) expected_record_count: usize,
}

/// A disposable on-disk snapshot, including absent and malformed files.
pub(crate) struct SqlTestSnapshotReadTestCase {
    pub(crate) description: &'static str,
    pub(crate) contents: &'static str,
    pub(crate) expected_records: &'static [(&'static str, u64)],
}
