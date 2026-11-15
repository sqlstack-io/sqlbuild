//! The providers section of `sqb debug`.

use crate::debug_report::models::{DebugLine, DebugProvider, DebugStatus};

/// How many providers have every secret set, then the missing secrets, or with `verbose` every provider's secrets.
#[must_use]
pub fn debug_provider_lines(providers: &[DebugProvider], verbose: bool) -> Vec<DebugLine> {
    if providers.is_empty() {
        return vec![DebugLine::checked(
            "providers",
            "none discovered",
            (DebugStatus::Ok, None),
        )];
    }
    let ready: usize = providers
        .iter()
        .filter(|provider| has_every_secret(provider))
        .count();
    let all_ready: bool = ready == providers.len();
    let mut lines: Vec<DebugLine> = vec![DebugLine::checked(
        "providers",
        &format!("{ready} of {} ready", providers.len()),
        if all_ready {
            (DebugStatus::Ok, Some("secrets set"))
        } else {
            (DebugStatus::Warn, Some("missing secrets"))
        },
    )];
    for provider in providers {
        if verbose && provider.secrets.is_empty() {
            lines.push(DebugLine::checked(
                &provider.name,
                "(no secrets)",
                (DebugStatus::Ok, Some("ok")),
            ));
        }
        for (variable, set) in &provider.secrets {
            if *set && !verbose {
                continue;
            }
            lines.push(DebugLine::checked(
                &provider.name,
                variable,
                if *set {
                    (DebugStatus::Ok, Some("set"))
                } else {
                    (DebugStatus::Warn, Some("missing"))
                },
            ));
        }
    }
    if !verbose && !all_ready {
        lines.push(DebugLine::checked(
            "more",
            "sqb debug --verbose lists every provider",
            (DebugStatus::Skip, None),
        ));
    }
    lines
}

fn has_every_secret(provider: &DebugProvider) -> bool {
    provider.secrets.iter().all(|(_, set)| *set)
}
