//! The named assumptions local test conversions relied on, with how many tests relied on each.

use std::collections::BTreeMap;

use crate::cli_text::models::CliStyle;
use crate::test_command::constants::{STRICT_ADVICE, STRICT_CONFIRMED};

/// Each assumption with its test count, and how to refuse them; empty when none were relied on.
#[must_use]
pub fn render_test_assumptions(
    assumptions: &[Vec<String>],
    style: CliStyle,
    strict: bool,
) -> String {
    let mut counts: BTreeMap<&str, usize> = BTreeMap::new();
    for relied_on in assumptions {
        for assumption in relied_on {
            *counts.entry(assumption.as_str()).or_default() += 1;
        }
    }
    if counts.is_empty() {
        return String::new();
    }
    let width: usize = counts.keys().map(|name| name.len()).max().unwrap_or(0);
    let mut lines: Vec<String> = vec![
        String::new(),
        style.warning_strong(&format!(
            "Assumptions relied on ({}):",
            if strict {
                STRICT_CONFIRMED
            } else {
                STRICT_ADVICE
            }
        )),
    ];
    for (name, tests) in counts {
        let noun: &str = if tests == 1 { "test" } else { "tests" };
        lines.push(format!("  {name:<width$}  {tests} {noun}"));
    }
    format!("{}\n", lines.join("\n"))
}
