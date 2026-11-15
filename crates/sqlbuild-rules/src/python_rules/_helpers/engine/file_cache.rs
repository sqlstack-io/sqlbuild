//! When a file's cached Python findings still hold: same file, rules and ruleset, same project answers.

use std::collections::BTreeMap;

use fensu_native::rules::constants::{DEAD_DEFINITION_CODE, PUBLIC_MAIN_ENTRY_EXTERNAL_USE_CODE};

use crate::models::PythonSource;
use crate::python_rules::models::{CallInputs, PythonTarget};

/// The digest of every Python source by path, which project-wide rules read.
pub(crate) fn project_digest(sources: &[PythonSource]) -> String {
    let mut hasher = blake3::Hasher::new();
    for source in sources {
        hasher.update(source.relative_path.as_bytes());
        hasher.update(b"\0");
        hasher.update(source.digest.as_bytes());
        hasher.update(b"\0");
    }
    hasher.finalize().to_hex().to_string()
}

/// The identity a file's cached findings must carry: its bytes, its calls, and for project-wide rules the project.
pub(crate) fn file_identity(
    target: &PythonTarget,
    calls: &[(Vec<String>, std::collections::HashMap<String, u32>)],
    inputs: &CallInputs<'_>,
) -> String {
    let mut hasher = blake3::Hasher::new();
    hasher.update(inputs.fingerprint.as_bytes());
    hasher.update(b"\0");
    hasher.update(target.repository_path.as_bytes());
    hasher.update(b"\0");
    hasher.update(inputs.programs.sources[target.source].digest.as_bytes());
    for (codes, thresholds) in calls {
        hasher.update(b"\0call\0");
        hasher.update(codes.join(",").as_bytes());
        let sorted: BTreeMap<&String, &u32> = thresholds.iter().collect();
        for (name, value) in sorted {
            hasher.update(format!("\0{name}={value}").as_bytes());
        }
    }
    let reads_project: bool = calls
        .iter()
        .flat_map(|(codes, _)| codes)
        .any(|code| code == PUBLIC_MAIN_ENTRY_EXTERNAL_USE_CODE || code == DEAD_DEFINITION_CODE);
    if reads_project {
        hasher.update(b"\0project\0");
        hasher.update(inputs.project_digest.as_bytes());
    }
    hasher.finalize().to_hex().to_string()
}

/// The digest of project answers, in key order.
pub(crate) fn answers_digest(answers: &BTreeMap<String, Vec<String>>) -> String {
    let mut hasher = blake3::Hasher::new();
    for (key, values) in answers {
        hasher.update(key.as_bytes());
        for value in values {
            hasher.update(b"\x01");
            hasher.update(value.as_bytes());
        }
        hasher.update(b"\x02");
    }
    hasher.finalize().to_hex().to_string()
}
