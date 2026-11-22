//! Python's `attempt_reuse` and `write_compile_entry` over the native store.

use sqlbuild_core::process_environment::main::environment_var_os::environment_var_os;
use std::collections::{HashMap, HashSet};
use std::path::Path;
use std::time::{SystemTime, UNIX_EPOCH};

use crate::project_reuse::_helpers::entry::{
    read_output, read_slot, remove_entry, rewrite_inputs, write_entry,
};
use crate::project_reuse::_helpers::files::{
    carried_forward_digests, compare_project_files, needs_refresh, project_file_changes,
    restamped_paths, stored_project_files, unverified_paths, with_missing_digests,
};
use crate::project_reuse::_helpers::identity::{
    environment_digest, module_stamps, module_stamps_unchanged, search_path_stamps,
    settings_inputs_digest, tracked_environment_names,
};
use crate::project_reuse::_helpers::image::{image_path, mark_used};
use crate::project_reuse::_helpers::target::{target_files_unchanged, verified_target_files};
use crate::project_reuse::_helpers::timings::compile_timings_span;
use crate::project_reuse::errors::ReuseRecordError;
use crate::project_reuse::models::{
    CompileRecord, DeltaCandidate, RecordOutcome, RecordResult, ReuseAttempt, ReuseCheck,
    ReuseOutcome, ReuseRules, SettingsInputs, StoredInputs, StoredOutput,
};
use crate::project_snapshot::main::snapshot_project_files::snapshot_project_files;
use crate::project_snapshot::main::text_path::text_path;
use crate::project_snapshot::models::PathStamp;

const NO_STORED_COMPILE: &str = "no stored compile";
const REUSE_BYPASSED: &str = "compile reuse is off";
const OUTPUT_UNREADABLE: &str = "the stored output cannot be replayed";
const INVOCATION_CHANGED: &str = "the command invocation changed";
const RUNTIME_CHANGED: &str = "the Python runtime changed";
const SEARCH_PATH_CHANGED: &str = "a Python search path entry changed";
const ENVIRONMENT_NAMES_CHANGED: &str = "the set of tracked environment variables changed";
const ENVIRONMENT_CHANGED: &str = "a tracked environment variable changed";
const SETTINGS_CHANGED: &str = "provider settings inputs changed";
const MODULES_CHANGED: &str = "an imported Python module outside the project changed";
const TARGET_CHANGED: &str = "the compiled target changed";

fn now_ns() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |elapsed| {
            i64::try_from(elapsed.as_nanos()).unwrap_or(i64::MAX)
        })
}

fn bypassed(check: &ReuseCheck<'_>) -> bool {
    check.bypass_requested
        || check.rules.disabling_variables.iter().any(|(name, value)| {
            environment_var_os(name).is_some_and(|current| current == value.as_str())
        })
        || !check
            .rules
            .config_filenames
            .iter()
            .any(|name| check.project_dir.join(name).is_file())
}

fn snapshot(project_dir: &Path, rules: &ReuseRules) -> Vec<PathStamp> {
    snapshot_project_files(project_dir, &rules.snapshot)
}

pub(crate) fn check(check: ReuseCheck<'_>) -> ReuseAttempt {
    let rules: &ReuseRules = check.rules;
    let mut attempt: ReuseAttempt = ReuseAttempt {
        outcome: ReuseOutcome::Bypass,
        project_dir: check.project_dir.clone(),
        store_path: check.store_path.clone(),
        identity: check.identity.clone(),
        search_path: Vec::new(),
        snapshot: Vec::new(),
        snapshot_ns: 0,
        digests: HashMap::new(),
        restamped: HashSet::new(),
        replay: None,
        replay_failed: false,
        digested_files: 0,
        delta: DeltaCandidate::Unavailable(NO_STORED_COMPILE),
    };
    if bypassed(&check) {
        attempt.delta = DeltaCandidate::Unavailable(REUSE_BYPASSED);
        return attempt;
    }
    attempt.outcome = ReuseOutcome::Miss;
    attempt.snapshot_ns = now_ns();
    attempt.search_path =
        search_path_stamps(&check.identity.search_path, &check.project_dir, rules);
    attempt.snapshot = snapshot(&check.project_dir, rules);
    if let Some(slot) = read_slot(&check.store_path) {
        let inputs: &StoredInputs = &slot.inputs;
        let comparison: Comparison = checked(&attempt, inputs, rules);
        attempt.digested_files += comparison.digested_files;
        attempt.digests = comparison.digests;
        if comparison.unchanged
            && let Some(output) = read_output(&check.store_path, &slot)
        {
            if check.json_output && output.timings_span.is_none() {
                attempt.replay_failed = true;
                return attempt;
            }
            attempt.outcome = ReuseOutcome::Hit;
            mark_used(&image_path(&check.store_path));
            if needs_refresh(&inputs.project_files, &attempt.snapshot) {
                let refreshed: StoredInputs = StoredInputs {
                    project_files: stored_project_files(
                        &attempt.snapshot,
                        &attempt.digests,
                        attempt.snapshot_ns,
                        rules.racy_window_ns,
                    ),
                    ..inputs.clone()
                };
                rewrite_inputs(&check.store_path, &slot, &refreshed);
            }
            attempt.replay = Some(output);
            return attempt;
        }
        attempt.restamped = restamped_paths(&inputs.project_files, &attempt.snapshot);
        if comparison.unchanged {
            attempt.delta = DeltaCandidate::Unavailable(OUTPUT_UNREADABLE);
        } else if let Some(reason) = comparison.identity_change {
            attempt.delta = DeltaCandidate::Unavailable(reason);
        } else if !target_files_unchanged(
            &inputs.target_files,
            &attempt.project_dir,
            inputs.target_tree,
            rules,
        ) {
            attempt.delta = DeltaCandidate::Unavailable(TARGET_CHANGED);
        } else {
            let unverified: HashSet<String> =
                unverified_paths(&inputs.project_files, &attempt.snapshot);
            let known: usize = attempt.digests.len();
            attempt.digests = with_missing_digests(&attempt, &unverified, rules.racy_window_ns);
            attempt.digested_files += attempt.digests.len() - known;
            attempt.delta = DeltaCandidate::Files(project_file_changes(
                &inputs.project_files,
                &attempt.snapshot,
                (&attempt.digests, &unverified),
            ));
        }
    }
    let known: usize = attempt.digests.len();
    attempt.digests = with_missing_digests(&attempt, &HashSet::new(), rules.racy_window_ns);
    attempt.digested_files += attempt.digests.len() - known;
    attempt
}

/// Whether a stored compile still matches, with the digests still valid either way.
struct Comparison {
    unchanged: bool,
    /// The first identity component that differs, if any.
    identity_change: Option<&'static str>,
    digests: HashMap<String, String>,
    digested_files: usize,
}

/// Compare every stored identity, carrying forward digests still valid either way.
fn checked(attempt: &ReuseAttempt, inputs: &StoredInputs, rules: &ReuseRules) -> Comparison {
    let identity_change: Option<&'static str> = identity_change(attempt, inputs, rules);
    let (files_unchanged, verified, digested_files) = if identity_change.is_none() {
        compare_project_files(
            &attempt.project_dir,
            &inputs.project_files,
            &attempt.snapshot,
        )
    } else {
        (false, HashMap::new(), 0)
    };
    Comparison {
        identity_change,
        unchanged: files_unchanged
            && target_files_unchanged(
                &inputs.target_files,
                &attempt.project_dir,
                inputs.target_tree,
                rules,
            ),
        digests: carried_forward_digests(&inputs.project_files, &attempt.snapshot, verified),
        digested_files,
    }
}

/// The first stored identity component that no longer matches, in checking order.
fn identity_change(
    attempt: &ReuseAttempt,
    inputs: &StoredInputs,
    rules: &ReuseRules,
) -> Option<&'static str> {
    if inputs.invocation_digest != attempt.identity.invocation_digest {
        Some(INVOCATION_CHANGED)
    } else if inputs.runtime != attempt.identity.runtime {
        Some(RUNTIME_CHANGED)
    } else if inputs.search_path != attempt.search_path {
        Some(SEARCH_PATH_CHANGED)
    } else if tracked_environment_names(&inputs.environment_names, rules)
        != inputs.environment_names
    {
        Some(ENVIRONMENT_NAMES_CHANGED)
    } else if environment_digest(&inputs.environment_names, rules) != inputs.environment_digest {
        Some(ENVIRONMENT_CHANGED)
    } else if settings_inputs_digest(&inputs.settings_inputs, rules) != inputs.settings_digest {
        Some(SETTINGS_CHANGED)
    } else if !module_stamps_unchanged(&inputs.modules) {
        Some(MODULES_CHANGED)
    } else {
        None
    }
}

pub(crate) fn record(
    attempt: &ReuseAttempt,
    record: &CompileRecord,
    rules: &ReuseRules,
) -> Result<RecordResult, ReuseRecordError> {
    if attempt.outcome != ReuseOutcome::Miss {
        return Ok(RecordResult {
            outcome: RecordOutcome::NotAttempted,
            digested_files: 0,
        });
    }
    let result: Result<RecordResult, ReuseRecordError> = store(attempt, record, rules);
    if !matches!(
        result,
        Ok(RecordResult {
            outcome: RecordOutcome::Stored,
            ..
        })
    ) {
        remove_entry(&attempt.store_path);
    }
    remove_retired(&attempt.store_path, rules);
    result
}

fn store(
    attempt: &ReuseAttempt,
    record: &CompileRecord,
    rules: &ReuseRules,
) -> Result<RecordResult, ReuseRecordError> {
    let removed: RecordResult = RecordResult {
        outcome: RecordOutcome::Removed,
        digested_files: 0,
    };
    let (Some(stdout), Ok(settings_inputs)) = (&record.stdout, &record.settings) else {
        return Ok(removed);
    };
    if !record.compile_cache_enabled || record.read_run_id {
        return Ok(removed);
    }
    let project_dir: &Path = &attempt.project_dir;
    let digests: HashMap<String, String> =
        with_missing_digests(attempt, &attempt.restamped, rules.racy_window_ns);
    let digested_files: usize = digests.len() - attempt.digests.len();
    let unchanged_project: bool = same_snapshot(&snapshot(project_dir, rules), &attempt.snapshot);
    let target_files: Option<Vec<PathStamp>> = if unchanged_project {
        verified_target_files(project_dir, record, attempt.snapshot_ns, rules)
    } else {
        None
    };
    let Some(target_files) = target_files else {
        return Ok(RecordResult {
            outcome: RecordOutcome::Skipped,
            digested_files,
        });
    };
    let recorded: Recorded<'_> = Recorded {
        settings_inputs,
        digests: &digests,
        target_files,
    };
    let inputs: StoredInputs = stored_inputs(attempt, record, rules, recorded);
    let output: StoredOutput = StoredOutput {
        stderr_lines: record.stderr_lines.clone(),
        exit_code: record.exit_code,
        timings_span: if record.json_output {
            compile_timings_span(stdout)
        } else {
            None
        },
        stdout: stdout.clone(),
    };
    if stdout.len() > rules.max_entry_bytes {
        return Ok(RecordResult {
            outcome: RecordOutcome::Removed,
            digested_files,
        });
    }
    write_entry(
        &attempt.store_path,
        (&inputs, &output, record.image.as_ref()),
        rules.max_stored_entries,
    )?;
    Ok(RecordResult {
        outcome: RecordOutcome::Stored,
        digested_files,
    })
}

/// What recording measured for one stored compile.
struct Recorded<'a> {
    settings_inputs: &'a [SettingsInputs],
    digests: &'a HashMap<String, String>,
    target_files: Vec<PathStamp>,
}

fn stored_inputs(
    attempt: &ReuseAttempt,
    record: &CompileRecord,
    rules: &ReuseRules,
    recorded: Recorded<'_>,
) -> StoredInputs {
    let Recorded {
        settings_inputs,
        digests,
        target_files,
    } = recorded;
    let environment_names: Vec<String> =
        tracked_environment_names(&record.template_environment_names, rules);
    let covered: HashSet<String> = attempt
        .snapshot
        .iter()
        .map(|stamp| {
            attempt
                .project_dir
                .join(text_path(&stamp.relative_path))
                .to_string_lossy()
                .into_owned()
        })
        .collect();
    StoredInputs {
        invocation_digest: attempt.identity.invocation_digest.clone(),
        runtime: attempt.identity.runtime.clone(),
        search_path: attempt.search_path.clone(),
        environment_digest: environment_digest(&environment_names, rules),
        environment_names,
        modules: module_stamps(&record.module_paths, &covered),
        project_files: stored_project_files(
            &attempt.snapshot,
            digests,
            attempt.snapshot_ns,
            rules.racy_window_ns,
        ),
        target_files,
        target_tree: record.artifacts_written,
        settings_digest: settings_inputs_digest(settings_inputs, rules),
        settings_inputs: settings_inputs.to_vec(),
    }
}

/// Delete stored compiles older releases left in their own directories.
fn remove_retired(store_path: &Path, rules: &ReuseRules) {
    let Some(cache_directory) = store_path.parent().and_then(Path::parent) else {
        return;
    };
    for name in &rules.retired_directories {
        let _ = std::fs::remove_dir_all(cache_directory.join(name));
    }
}

fn same_snapshot(left: &[PathStamp], right: &[PathStamp]) -> bool {
    let index = |stamps: &[PathStamp]| -> HashMap<String, PathStamp> {
        stamps
            .iter()
            .map(|stamp| (stamp.relative_path.clone(), stamp.clone()))
            .collect()
    };
    left.len() == right.len() && index(left) == index(right)
}
