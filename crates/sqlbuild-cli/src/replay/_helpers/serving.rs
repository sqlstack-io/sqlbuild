//! The native reuse check against a launch record and the output a hit replays.

use std::path::{Path, PathBuf};
use std::time::Instant;

use sqlbuild_cache::project_reuse::main::check_reuse::check_reuse;
use sqlbuild_cache::project_reuse::main::replay_stdout::replay_stdout;
use sqlbuild_cache::project_reuse::models::{ReuseCheck, ReuseOutcome, ReuseRules, StoredOutput};
use sqlbuild_config::project::main::deprecated_setting_warnings::deprecated_setting_warnings;
use sqlbuild_config::project::main::project_config_paths::project_config_paths;

use crate::replay::_helpers::console::tenths_of_seconds;
use crate::replay::_helpers::records::{hit_timings, host_identity};
use crate::replay::models::{LaunchRecord, ServedInvocation};

fn elapsed_ms(started: Instant) -> i64 {
    i64::try_from(started.elapsed().as_millis()).unwrap_or(i64::MAX)
}

/// The stored output on a hit, or `None` when the slot was evicted or the check misses or bypasses.
pub(crate) fn stored_output(record: &LaunchRecord, rules: &ReuseRules) -> Option<StoredOutput> {
    let store_path = PathBuf::from(&record.store_path);
    if !store_path.is_file() {
        return None;
    }
    let attempt = check_reuse(ReuseCheck {
        project_dir: PathBuf::from(&record.project_dir),
        store_path,
        identity: host_identity(record),
        bypass_requested: false,
        json_output: record.json_output,
        rules,
    });
    if attempt.outcome != ReuseOutcome::Hit || attempt.replay_failed {
        return None;
    }
    attempt.replay
}

pub(crate) fn served(
    record: &LaunchRecord,
    output: &StoredOutput,
    started: Instant,
) -> Option<ServedInvocation> {
    let check_ms = elapsed_ms(started);
    let timings = if record.json_output {
        hit_timings(record, check_ms, elapsed_ms(started))
    } else {
        Vec::new()
    };
    let stdout = replay_stdout(output, &timings)?;
    let (prefix, suffix) = &record.hit_message;
    let mut stderr = format!("{prefix}{}{suffix}\n", tenths_of_seconds(check_ms));
    let warnings: Vec<String> =
        match deprecated_setting_warnings(&project_config_paths(Path::new(&record.project_dir))) {
            Ok(warnings) => warnings,
            Err(_) => return None,
        };
    for line in warnings.iter().chain(&output.stderr_lines) {
        stderr.push_str(line);
        stderr.push('\n');
    }
    Some(ServedInvocation {
        stdout,
        stderr,
        exit_code: output.exit_code,
    })
}
