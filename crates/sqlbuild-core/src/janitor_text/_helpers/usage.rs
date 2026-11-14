//! Wording of the janitor usage check: who reads a candidate and what happens to it.

use crate::cli_text::main::format_count_noun::format_count_noun;
use crate::janitor_text::_helpers::phases::noun;
use crate::janitor_text::constants::{READER_LIMIT, USAGE_NOT_CHECKED_PREFIX, WINDOW_UNITS};
use crate::janitor_text::models::{ArchiveCandidateText, JanitorPlanText, UsageCheckText};

/// The usage check when it ran and could read usage.
pub(crate) fn checked_usage(plan: &JanitorPlanText) -> Option<&UsageCheckText> {
    plan.usage_check.as_ref().filter(|check| check.checked)
}

/// Archive candidates with reads in the usage window.
pub(crate) fn in_use_candidates(plan: &JanitorPlanText) -> Vec<&ArchiveCandidateText> {
    plan.archive_candidates
        .iter()
        .filter(|candidate| candidate.usage.is_some())
        .collect()
}

/// `7 days` for a single-unit window such as `7d`, otherwise the window as written.
pub(crate) fn window_label(window: &str) -> String {
    let digits: &str = window.trim_end_matches(|character: char| !character.is_ascii_digit());
    let unit: &str = &window[digits.len()..];
    match (
        digits.parse::<i64>(),
        WINDOW_UNITS.iter().find(|(suffix, _)| *suffix == unit),
    ) {
        (Ok(count), Some((_, singular))) if !digits.is_empty() => noun_count(count, singular),
        _ => window.to_owned(),
    }
}

fn noun_count(count: i64, singular: &str) -> String {
    noun(usize::try_from(count).unwrap_or(usize::MAX), singular)
}

/// `alice, bob, carol +2 more`: the first names and how many are not shown.
fn readers(names: &[String]) -> String {
    let shown: usize = names.len().min(READER_LIMIT);
    let mut text: String = names[..shown].join(", ");
    if names.len() > shown {
        text.push_str(&format!(" +{} more", names.len() - shown));
    }
    text
}

/// `queried <date> by <readers> (<n> queries in <window>)`, plus the frozen flag for tables.
pub(crate) fn usage_detail(candidate: &ArchiveCandidateText, window: &str) -> Option<String> {
    let usage = candidate.usage.as_ref()?;
    let date: &str = usage.last_read_at.split(' ').next().unwrap_or_default();
    let via: String = match usage.via.as_deref() {
        None => String::new(),
        Some([]) => " through a view".to_owned(),
        Some(names) => format!(" via {}", readers(names)),
    };
    let mut text: String = format!(
        "queried {date} by {}{via} ({} in {})",
        readers(&usage.readers),
        format_count_noun(usage.query_count, "query", Some("queries")),
        window_label(window)
    );
    if candidate.is_table {
        match &candidate.changed_on {
            Some(changed_on) => {
                text.push_str(&format!("; frozen since {changed_on}, still in use"));
            }
            None => text.push_str("; frozen, still in use"),
        }
    }
    Some(text)
}

/// The notice lines when usage could not be checked, or what a check that ran left out.
pub(crate) fn usage_notices(plan: &JanitorPlanText) -> Vec<String> {
    let Some(check) = plan.usage_check.as_ref() else {
        return Vec::new();
    };
    if check.checked {
        return check.notices.clone();
    }
    vec![format!(
        "{USAGE_NOT_CHECKED_PREFIX}{}",
        check.reason.as_deref().unwrap_or("unknown reason")
    )]
}

/// The prompt line stating how many archived relations are in use, or which are kept.
pub(crate) fn usage_prompt_line(plan: &JanitorPlanText) -> Option<String> {
    let check: &UsageCheckText = checked_usage(plan)?;
    if check.keep_in_use && !plan.kept_in_use.is_empty() {
        return Some(format!(
            "{} in use {} kept.",
            noun(plan.kept_in_use.len(), "relation"),
            if plan.kept_in_use.len() == 1 {
                "is"
            } else {
                "are"
            }
        ));
    }
    if plan.archive_candidates.is_empty() {
        return None;
    }
    let in_use: usize = in_use_candidates(plan).len();
    let archived: usize = plan.archive_candidates.len();
    let in_use_text: String = match (archived, in_use) {
        (1, 0) => "it is not in use".to_owned(),
        (1, _) => "it is in use".to_owned(),
        (_, 0) => "none of them are in use".to_owned(),
        (_, 1) => "1 of them is in use".to_owned(),
        _ => format!("{in_use} of them are in use"),
    };
    Some(format!(
        "{} will be archived; {in_use_text}.",
        noun(archived, "relation")
    ))
}
