//! Compare the held delta result with the full compile's; every difference found.

use crate::compile_delta::_helpers::verify::ACTUAL;
use crate::compile_delta::_helpers::verify::Actual;
use crate::compile_delta::_helpers::verify::EXPECTED;
use crate::compile_delta::_helpers::verify::locked;
use crate::compile_delta::_helpers::verify::tree_differences;
use crate::compile_delta::_helpers::verify::without_timings;

/// Compare the held delta result with the full compile's; every difference found.
pub fn differences(raised: Option<String>) -> Vec<String> {
    let Some(expected) = locked(&EXPECTED).take() else {
        return Vec::new();
    };
    let actual: Actual = locked(&ACTUAL).take().unwrap_or_default();
    let mut found: Vec<String> = Vec::new();
    if expected.raised != raised {
        found.push(format!(
            "raised error differs: delta {:?}, full {:?}",
            expected.raised, raised
        ));
    }
    if expected.raised.is_some() || raised.is_some() {
        return found;
    }
    if expected.exit_code != actual.exit_code {
        found.push(format!(
            "exit code differs: delta {:?}, full {:?}",
            expected.exit_code, actual.exit_code
        ));
    }
    let delta_stdout: Option<String> = expected.stdout.as_deref().map(without_timings);
    let full_stdout: Option<String> = actual.stdout.as_deref().map(without_timings);
    if delta_stdout != full_stdout {
        let (delta, full) = (
            delta_stdout.unwrap_or_default(),
            full_stdout.unwrap_or_default(),
        );
        let common: usize = delta
            .bytes()
            .zip(full.bytes())
            .take_while(|(left, right)| left == right)
            .count();
        let start: usize = common.saturating_sub(200);
        let excerpt = |text: &str| -> String {
            text.get(start..(common + 300).min(text.len()))
                .unwrap_or_default()
                .to_owned()
        };
        found.push(format!(
            "report differs at byte {common}:\n--- delta\n{}\n--- full\n{}",
            excerpt(&delta),
            excerpt(&full)
        ));
    }
    if expected.artifacts_written != actual.artifacts_written {
        found.push(format!(
            "artifact writing differs: delta {}, full {}",
            expected.artifacts_written, actual.artifacts_written
        ));
    }
    found.extend(tree_differences(
        (&expected.failed_root, &expected.failed),
        &actual.written,
    ));
    if actual.artifacts_written {
        found.extend(tree_differences(
            (&expected.compiled_root, &expected.compiled),
            &actual.written,
        ));
    }
    found.truncate(50);
    found
}
