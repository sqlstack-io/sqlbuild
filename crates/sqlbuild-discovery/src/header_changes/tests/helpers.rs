use std::collections::BTreeSet;

use crate::header_changes::main::header_change::header_change;
use crate::header_changes::main::header_key_digests::header_key_digests;
use crate::header_changes::models::HeaderChange;
use crate::header_changes::tests::test_types::HeaderChangeTestCase;
use crate::model_files::main::model_file_options::model_file_options;
use crate::model_files::main::parse_model_text::parse_model_text;

const RELATIVE_PATH: &str = "sql/models/orders.sql";
const UPSTREAM: [(&str, &str); 1] = [("model", "customers")];

/// The change of a recorded model after its file is rewritten, or deleted when none is given.
pub(super) fn edited_change(test_case: &HeaderChangeTestCase) -> HeaderChange {
    let project = tempfile::tempdir().expect("project directory");
    let path = project.path().join(RELATIVE_PATH);
    std::fs::create_dir_all(path.parent().expect("models directory")).expect("models directory");
    let stored = parse_model_text(
        &path.to_string_lossy(),
        test_case.stored_contents.to_owned(),
        &model_file_options((false, false), None),
    )
    .expect("stored model parses");
    let keys = header_key_digests(&stored.header_values);
    let upstream: Vec<(String, String)> = UPSTREAM
        .iter()
        .map(|(kind, name)| ((*kind).to_owned(), (*name).to_owned()))
        .collect();
    let _ = test_case
        .edited_contents
        .map(|contents| std::fs::write(&path, contents).expect("edited model"));
    header_change(
        project.path(),
        RELATIVE_PATH,
        (test_case.recorded.then_some(&keys), &upstream),
    )
}

/// The set of changed keys a header change names.
pub(super) fn keys(names: &[&str]) -> BTreeSet<String> {
    names.iter().map(|name| (*name).to_owned()).collect()
}
