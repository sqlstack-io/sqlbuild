use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use crate::project_reuse::_helpers::entry::write_entry;
use crate::project_reuse::_helpers::sections::{decode_sections, encode_sections};
use crate::project_reuse::models::{SettingsInputs, StoredInputs, StoredOutput, StoredProjectFile};
use crate::project_reuse::types::ImageSections;
use crate::project_snapshot::models::PathStamp;

pub(crate) fn stamp(relative_path: &str, kind: &'static str, link: Option<&str>) -> PathStamp {
    PathStamp {
        relative_path: relative_path.to_owned(),
        kind,
        size: 42,
        mtime_ns: -7,
        ctime_ns: 1_700_000_000_000_000_000,
        inode: 9,
        link: link.map(str::to_owned),
    }
}

pub(crate) fn stored_inputs() -> StoredInputs {
    StoredInputs {
        invocation_digest: "b3:orders".to_owned(),
        runtime: BTreeMap::from([("python".to_owned(), "3.12".to_owned())]),
        search_path: vec![("site".to_owned(), -2)],
        environment_names: vec!["SQLBUILD_REGION".to_owned()],
        environment_digest: "b3:env".to_owned(),
        modules: vec![("/site/orders.py".to_owned(), 5, 10)],
        project_files: vec![
            StoredProjectFile {
                stamp: stamp("sql/models/orders.sql", "f", None),
                digest: Some("b3:model".to_owned()),
                racy: true,
            },
            StoredProjectFile {
                stamp: stamp("seeds", "dl", Some("../shared")),
                digest: None,
                racy: false,
            },
        ],
        target_files: vec![stamp("/project/target/compiled/orders.sql", "f", None)],
        target_tree: true,
        settings_inputs: vec![SettingsInputs {
            case_sensitive: true,
            names: vec!["region".to_owned()],
            prefixes: vec!["orders_".to_owned()],
            env_files: vec![".env".to_owned()],
            secrets_dirs: vec!["/secrets".to_owned()],
        }],
        settings_digest: "b3:settings".to_owned(),
    }
}

pub(crate) fn stored_output() -> StoredOutput {
    StoredOutput {
        stderr_lines: vec!["Compiled 1 model".to_owned()],
        exit_code: 1,
        timings_span: Some((3, 9)),
        stdout: "{\"command\": \"compile\"}\n".to_owned(),
    }
}

/// Store `slots` compiles in `directory`, each older than the next, keeping `max_entries`.
pub(crate) fn store_aged_slots(directory: &Path, slots: u64, max_entries: usize) {
    for slot in 0..slots {
        let path = directory.join(format!("slot{slot}.store"));
        write_entry(
            &path,
            (&stored_inputs(), &stored_output(), None),
            max_entries,
        )
        .expect("stored");
        let stamp = std::time::SystemTime::UNIX_EPOCH + std::time::Duration::from_secs(slot + 1);
        std::fs::File::options()
            .write(true)
            .open(&path)
            .and_then(|file| file.set_modified(stamp))
            .expect("aged slot");
    }
}

/// Leave output files with no slot in `directory`, each modified `age` seconds ago.
pub(crate) fn leave_orphaned_outputs(directory: &Path, outputs: &[(&str, u64)]) {
    std::fs::create_dir_all(directory).expect("store folder");
    for (name, age) in outputs {
        let path = directory.join(name);
        std::fs::write(&path, b"output").expect("orphaned output");
        let modified = std::time::SystemTime::now() - std::time::Duration::from_secs(*age);
        std::fs::File::options()
            .write(true)
            .open(&path)
            .and_then(|file| file.set_modified(modified))
            .expect("aged output");
    }
}

/// The slot file names left in `directory`, sorted.
pub(crate) fn slot_names(directory: &Path) -> Vec<String> {
    let mut names: Vec<String> = std::fs::read_dir(directory)
        .expect("slots")
        .map(|entry| {
            entry
                .expect("slot")
                .file_name()
                .to_string_lossy()
                .into_owned()
        })
        .collect();
    names.sort();
    names
}

/// The output file stored beside the slot file at `path`.
pub(crate) fn output_file(path: &Path) -> std::path::PathBuf {
    path.with_extension("output")
}

/// Keep only the first `length` bytes of the file.
pub(crate) fn truncate(path: &Path, length: usize) {
    let bytes = std::fs::read(path).expect("file bytes");
    std::fs::write(path, &bytes[..length]).expect("truncated file");
}

/// Swap in the intact output of a compile that printed something else.
pub(crate) fn replace_output(path: &Path) {
    let other = path.with_file_name("other.store");
    let output = StoredOutput {
        stdout: "{\"command\": \"build\"}\n".to_owned(),
        ..stored_output()
    };
    write_entry(&other, (&stored_inputs(), &output, None), usize::MAX).expect("other compile");
    std::fs::rename(output_file(&other), output_file(path)).expect("swapped output");
}

/// A slot file as the previous single-file native store format wrote it.
pub(crate) fn previous_format_slot() -> Vec<u8> {
    let mut bytes: Vec<u8> = b"SQBSTORE".to_vec();
    bytes.extend_from_slice(&1_u32.to_le_bytes());
    bytes.extend_from_slice(&[0_u8; 32]);
    bytes.extend_from_slice(b"project-reuse");
    bytes
}

/// Append a section named `name` to the slot file, as a later release might.
pub(crate) fn add_slot_section(path: &Path, name: &str, bytes: &[u8]) {
    let stored = std::fs::read(path).expect("slot bytes");
    let file = decode_sections(&stored, "project-reuse-slot").expect("slot sections");
    let mut sections: Vec<(&str, &[u8])> = file
        .sections
        .iter()
        .map(|(name, bytes)| (name.as_str(), *bytes))
        .collect();
    sections.push((name, bytes));
    let (encoded, _) = encode_sections("project-reuse-slot", &sections);
    std::fs::write(path, encoded).expect("extended slot");
}

/// The bytes of the slot file's section named `name`.
pub(crate) fn slot_section(path: &Path, name: &str) -> Option<Vec<u8>> {
    let stored = std::fs::read(path).expect("slot bytes");
    let file = decode_sections(&stored, "project-reuse-slot").expect("slot sections");
    file.section(name).map(<[u8]>::to_vec)
}

/// Flip one byte near the end of the file.
pub(crate) fn damage(path: &Path) {
    let mut bytes = std::fs::read(path).expect("slot bytes");
    let last = bytes.len() - 1;
    bytes[last] ^= 0xff;
    std::fs::write(path, bytes).expect("damaged slot");
}

/// A small project image of two sections.
pub(crate) fn image() -> ImageSections {
    vec![
        ("facts".to_owned(), b"{\"models\": [\"orders\"]}".to_vec()),
        ("rules".to_owned(), b"{}".to_vec()),
    ]
}

/// The image file beside the slot at `path`.
pub(crate) fn image_file(path: &Path) -> PathBuf {
    path.with_extension("image")
}
