//! Whole-project compile reuse on the shared native store, driven by the Python host's facts.

use crate::compile_outputs::models::ReuseRulesPy;
use crate::compile_outputs::models::SettingsRow;
use crate::compile_outputs::types::ArtifactRow;
use std::collections::BTreeMap;
use std::ffi::{OsStr, OsString};
use std::path::{Path, PathBuf};

use pyo3::exceptions::PyValueError;
use sqlbuild_cache::digest::main::bytes_digest::bytes_digest;
use sqlbuild_cache::digest::main::hex_digest::hex_digest;
use sqlbuild_cache::project_reuse::constants::{NATIVE_DIGEST_PREFIX, REUSE_STORE_SUFFIX};
use sqlbuild_cache::project_reuse::models::{RecordedArtifact, ReuseRules, SettingsInputs};
use sqlbuild_cache::project_snapshot::main::path_text::path_text;
use sqlbuild_cache::project_snapshot::models::SnapshotRules;

pub(crate) type ReplayRow = (String, Vec<String>, i64);
pub(crate) type LaunchIdentityRow = (String, BTreeMap<String, String>, Vec<String>);

impl From<ReuseRulesPy> for ReuseRules {
    fn from(rules: ReuseRulesPy) -> Self {
        Self {
            snapshot: SnapshotRules {
                excluded: rules.excluded,
                excluded_root: rules.excluded_root,
                presence_suffixes: rules.presence_suffixes,
                output_files: rules.output_files,
            },
            tracked_prefixes: rules.tracked_prefixes,
            untracked_names: rules.untracked_names,
            disabling_variables: rules.disabling_variables,
            config_filenames: rules.config_filenames,
            racy_window_ns: rules.racy_window_ns,
            max_stored_entries: rules.max_stored_entries,
            max_entry_bytes: rules.max_entry_bytes,
            missing_environment_value: rules.missing_environment_value,
            missing_file_digest: rules.missing_file_digest,
            missing_path_mtime_ns: rules.missing_path_mtime_ns,
            project_root_path_mtime_ns: rules.project_root_path_mtime_ns,
            compiled_root: rules.compiled_root,
            retired_directories: rules.retired_directories,
        }
    }
}

pub(crate) fn poisoned<T>(_: T) -> pyo3::PyErr {
    PyValueError::new_err("compile reuse attempt is poisoned")
}

pub(crate) fn settings_inputs(row: SettingsRow) -> SettingsInputs {
    let SettingsRow {
        case_sensitive,
        names,
        prefixes,
        env_files,
        secrets_dirs,
    } = row;
    SettingsInputs {
        case_sensitive,
        names,
        prefixes,
        env_files,
        secrets_dirs,
    }
}

pub(crate) fn recorded_artifact(artifact: ArtifactRow) -> RecordedArtifact {
    match artifact {
        (Some(digest), _, _) => RecordedArtifact::Written { digest },
        (None, size, mtime_ns) => RecordedArtifact::Kept {
            size: size.unwrap_or_default(),
            mtime_ns: mtime_ns.unwrap_or_default(),
        },
    }
}

pub(crate) fn native_digest(contents: &[u8]) -> String {
    format!(
        "{NATIVE_DIGEST_PREFIX}{}",
        hex_digest(&bytes_digest(contents))
    )
}

/// The one store file kept for the selected target.
pub(crate) fn store_slot(directory: &Path, selected_target: Option<&OsStr>) -> PathBuf {
    let label: String = selected_target.map_or_else(String::new, |target| {
        format!("target:{}", path_text(target))
    });
    directory.join(format!(
        "{}.{REUSE_STORE_SUFFIX}",
        hex_digest(&bytes_digest(label.as_bytes()))
    ))
}

/// Host strings that may carry undecodable bytes, as exact text.
pub(crate) fn texts(values: &[OsString]) -> Vec<String> {
    values.iter().map(|value| path_text(value)).collect()
}
