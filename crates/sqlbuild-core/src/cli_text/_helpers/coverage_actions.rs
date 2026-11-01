//! The text and the action of one partial-check reason, shared by the notice and the strict report.

use crate::cli_text::constants::{
    DEFAULT_ACTION, INVALID_UPSTREAM, OPEN_INPUT, OPEN_SOURCE, UNKNOWN_FUNCTION,
};

/// The action that clears one partial-check reason.
pub(crate) fn action(reason: &str) -> String {
    if let Some(source) = reason.strip_prefix(OPEN_SOURCE) {
        return format!(
            "run `sqb contract generate --from <target> --select source:{source} --write` and \
             enforce the complete source contract"
        );
    }
    if reason.starts_with(UNKNOWN_FUNCTION) {
        return "declare it as a project function so its calls are checked, or correct its \
                spelling"
            .to_owned();
    }
    if reason.starts_with(INVALID_UPSTREAM) {
        return "fix the upstream error, then run `sqb compile --no-cache`".to_owned();
    }
    match reason {
        "some output types are unknown" => {
            "declare output types, then run `sqb compile --no-cache`"
        }
        "output shape could not be inferred" => {
            "declare the output schema; run `sqb compile --no-cache`"
        }
        "unresolved star over open inputs" => {
            "declare complete input contracts, then run `sqb compile --no-cache`"
        }
        "SQL analysis disabled" => "enable sql_analysis, then run `sqb compile --no-cache`",
        "type checking awaits dialect coercion support" => {
            "use a supported dialect for type checks; run `sqb compile --json` to inspect coverage"
        }
        _ => DEFAULT_ACTION,
    }
    .to_owned()
}

/// One reason as the reports word it.
pub(crate) fn reason_detail(reason: &str) -> String {
    reason
        .replace(OPEN_SOURCE, "reads open source ")
        .replace(OPEN_INPUT, "reads open input ")
        .replace(UNKNOWN_FUNCTION, "calls unknown function ")
}
