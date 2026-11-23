use crate::tree::main::rglob::rglob;
use crate::tree::models::ProjectTree;
use crate::tree::tests::test_types::GlobTestCase;
use std::fs;
use std::path::Path;

fn write_file(project: &Path, relative_path: &str) {
    let path = project.join(relative_path);
    fs::create_dir_all(path.parent().expect("parent directory")).expect("directories");
    fs::write(path, "MODEL ();\nSELECT 1").expect("file");
}

#[cfg(unix)]
fn link(project: &Path, (link, target): (&str, &str)) {
    std::os::unix::fs::symlink(project.join(target), project.join(link)).expect("symlink");
}

/// Build the case's project and glob `models/` for its suffix.
pub(super) fn globbed_paths(test_case: &GlobTestCase) -> Vec<String> {
    let project = tempfile::tempdir().expect("temporary project");
    test_case
        .files
        .iter()
        .for_each(|relative_path| write_file(project.path(), relative_path));
    test_case.directories.iter().for_each(|relative_path| {
        fs::create_dir_all(project.path().join(relative_path)).expect("directory")
    });
    #[cfg(unix)]
    test_case
        .links
        .iter()
        .for_each(|pair| link(project.path(), *pair));
    let tree = ProjectTree::new(project.path());
    rglob(&tree, "models", |entry| {
        entry.name.ends_with(test_case.suffix)
    })
    .expect("walk")
}

/// Walk the case's raw-named files: displayed paths, read failures and each resolved file's text.
#[cfg(unix)]
pub(super) fn undecodable_walk(
    test_case: &crate::tree::tests::test_types::UndecodableWalkTestCase,
) -> (Vec<String>, Vec<Option<String>>, Vec<String>) {
    use std::os::unix::ffi::OsStrExt;
    let project = tempfile::tempdir().expect("temporary project");
    for (index, relative_path) in test_case.files.iter().enumerate() {
        let path = project
            .path()
            .join(std::ffi::OsStr::from_bytes(relative_path));
        fs::create_dir_all(path.parent().expect("parent directory")).expect("directories");
        fs::write(path, format!("file {index}")).expect("file");
    }
    let tree = ProjectTree::new(project.path());
    let root = crate::models::ProjectRoot {
        directory: project.path().to_path_buf(),
        display_prefix: "project/".to_owned(),
    };
    let paths: Vec<String> =
        rglob(&tree, "models", |entry| entry.name.ends_with(".sql")).expect("walk");
    let failures: Vec<Option<String>> = paths
        .iter()
        .map(|path| {
            crate::_helpers::reading::undecodable_path_failure(&root, &tree, path)
                .map(|failure| failure.message)
        })
        .collect();
    let contents: Vec<String> = paths
        .iter()
        .map(|path| fs::read_to_string(tree.absolute(path)).expect("resolved file"))
        .collect();
    let shown: Vec<String> = paths
        .iter()
        .map(|path| crate::tree::main::display_text::display_text(path).into_owned())
        .collect();
    (shown, failures, contents)
}

/// Walk `models/` through a junction `models/linked` to a directory holding `orders.sql`.
#[cfg(windows)]
pub(super) fn junction_walk() -> Vec<String> {
    let project = tempfile::tempdir().expect("temporary project");
    let target = project.path().join("elsewhere");
    fs::create_dir_all(&target).expect("target directory");
    fs::write(target.join("orders.sql"), "MODEL ();\nSELECT 1").expect("file");
    fs::create_dir_all(project.path().join("models")).expect("models directory");
    junction::create(&target, project.path().join("models").join("linked")).expect("junction");
    rglob(&ProjectTree::new(project.path()), "models", |entry| {
        entry.name.ends_with(".sql")
    })
    .expect("walk")
}

/// Python's code points for a segment whose raw name was listed on Windows or elsewhere.
pub(super) fn segment_points(segment: &str, windows: bool) -> Vec<u32> {
    crate::tree::_helpers::raw_names::segment_code_points_on(segment, windows)
}

/// Owned copies of borrowed texts.
pub(super) fn owned<'a>(texts: impl Iterator<Item = &'a str>) -> Vec<String> {
    texts.map(str::to_owned).collect()
}

/// Walk files named by UTF-16 units: displayed paths, whether each fails when read, and its text.
#[cfg(windows)]
pub(super) fn wide_name_walk(
    test_case: &crate::tree::tests::test_types::WideNameWalkTestCase,
) -> (Vec<String>, Vec<bool>, Vec<String>) {
    use std::os::windows::ffi::OsStringExt;
    let project = tempfile::tempdir().expect("temporary project");
    let models = project.path().join("models");
    fs::create_dir_all(&models).expect("models directory");
    for (index, name) in test_case.names.iter().enumerate() {
        fs::write(
            models.join(std::ffi::OsString::from_wide(name)),
            format!("file {index}"),
        )
        .expect("file");
    }
    let tree = ProjectTree::new(project.path());
    let root = crate::models::ProjectRoot {
        directory: project.path().to_path_buf(),
        display_prefix: String::new(),
    };
    let paths: Vec<String> =
        rglob(&tree, "models", |entry| entry.name.ends_with(".sql")).expect("walk");
    let failures: Vec<bool> = paths
        .iter()
        .map(|path| {
            crate::_helpers::reading::undecodable_path_failure(&root, &tree, path).is_some()
        })
        .collect();
    let contents: Vec<String> = paths
        .iter()
        .map(|path| fs::read_to_string(tree.absolute(path)).expect("resolved file"))
        .collect();
    let shown: Vec<String> = paths
        .iter()
        .map(|path| crate::tree::main::display_text::display_text(path).into_owned())
        .collect();
    (shown, failures, contents)
}
