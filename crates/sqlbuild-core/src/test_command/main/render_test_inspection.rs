//! `sqb test --inspect`: the resolved test plan without executing it.

use crate::run_on::main::run_on_inspection_line::run_on_inspection_line;
use crate::test_command::constants::ERROR_SEVERITY;
use crate::test_command::models::{TestInspectionEntry, TestPlanWarning};

fn names(label: &str, values: &[String]) -> String {
    let shown: String = if values.is_empty() {
        "none".to_owned()
    } else {
        values.join(", ")
    };
    format!("  {label}: {shown}\n")
}

/// Each test's fixture boundaries and model chain, warnings, the outcome line, and the exit code.
#[must_use]
pub fn render_test_inspection(
    entries: &[TestInspectionEntry],
    warnings: &[TestPlanWarning],
) -> (String, i32) {
    let mut text: String = "\nResolved test plan\n\n".to_owned();
    for entry in entries {
        text.push_str(&format!("{}\n", entry.label));
        if let Some(decision) = &entry.run_on {
            text.push_str(&run_on_inspection_line(decision));
        }
        for (label, values) in [
            ("mocked sources", &entry.mocked_sources),
            ("mocked refs", &entry.mocked_refs),
            ("mocked seeds", &entry.mocked_seeds),
            ("mocked dbt refs", &entry.mocked_dbt_refs),
            ("mocked table functions", &entry.mocked_table_functions),
            ("real models", &entry.real_models),
            ("expected models", &entry.expected_models),
        ] {
            text.push_str(&names(label, values));
        }
        for mock in &entry.mocked_refs {
            text.push_str(&format!(
                "  boundary: {mock} is replaced by __ref__{mock}\n"
            ));
        }
        for reason in &entry.selection_reasons {
            text.push_str(&format!("  selected: {reason}\n"));
        }
        text.push('\n');
    }
    let errors: usize = warnings
        .iter()
        .filter(|warning| warning.severity == ERROR_SEVERITY)
        .count();
    for warning in warnings {
        text.push_str(&format!("  {}: {}\n", warning.severity, warning.message));
    }
    text.push_str(&format!(
        "Test plan inspection {}: {} selected, {errors} errors.\n",
        if errors > 0 { "failed" } else { "complete" },
        entries.len()
    ));
    (text, i32::from(errors > 0))
}
