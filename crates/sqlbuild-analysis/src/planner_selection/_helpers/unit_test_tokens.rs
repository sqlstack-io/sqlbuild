//! Classify selector groups, tokens and intersection parts as unit-test or resource selectors.

use std::collections::BTreeSet;

use sqlbuild_core::text::main::glob_match::glob_match;

use crate::graph::errors::SelectorError;
use crate::graph::main::parse_selector::parse_selector;
use crate::graph::main::selector_name_help::selector_name_help;
use crate::graph::models::ParsedSelector;
use crate::planner_selection::constants::{
    INTERSECTION_SEPARATOR, NAME_KIND, PATTERN_CHARACTERS, TEST_KIND, TOKEN_SEPARATOR,
    UNIT_TEST_SELECTOR_CODE, UNKNOWN_SELECTOR_CODE,
};
use crate::planner_selection::main::unit_test_selector_rejection::unit_test_selector_rejection;
use crate::planner_selection::models::UnitTestSelectorNames;

/// The unit tests a selector names and whether it also selects resources.
type Classified = (BTreeSet<String>, bool);

/// The unit tests the groups select and the groups left with their unit-test tokens removed.
pub(crate) fn split_groups(
    groups: &[String],
    names: &UnitTestSelectorNames,
    accepts_unit_tests: bool,
) -> Result<(BTreeSet<String>, Vec<String>), SelectorError> {
    let mut tests: BTreeSet<String> = BTreeSet::new();
    let mut kept_groups: Vec<String> = Vec::new();
    for group in groups {
        let tokens: Vec<&str> = group
            .split(char::is_whitespace)
            .filter(|token| !token.is_empty())
            .collect();
        let mut kept_tokens: Vec<&str> = Vec::new();
        for token in &tokens {
            let (matched, keep): Classified = split_token(token, names, accepts_unit_tests)?;
            tests.extend(matched);
            if keep {
                kept_tokens.push(token);
            }
        }
        if kept_tokens.len() == tokens.len() {
            kept_groups.push(group.clone());
        } else if !kept_tokens.is_empty() {
            kept_groups.push(kept_tokens.join(TOKEN_SEPARATOR));
        }
    }
    Ok((tests, kept_groups))
}

fn split_token(
    token: &str,
    names: &UnitTestSelectorNames,
    accepts_unit_tests: bool,
) -> Result<Classified, SelectorError> {
    let parts: Vec<&str> = token.split(INTERSECTION_SEPARATOR).collect();
    let mut classified: Vec<Classified> = Vec::with_capacity(parts.len());
    for part in &parts {
        classified.push(classify(part, names, accepts_unit_tests)?);
    }
    let selects_tests: bool = classified
        .iter()
        .any(|(matched, keep)| !matched.is_empty() && !keep);
    if parts.len() > 1 && selects_tests {
        for part in &parts {
            parse_selector(part)?;
        }
        return Err(SelectorError {
            code: UNIT_TEST_SELECTOR_CODE,
            message: format!(
                "selector '{token}' intersects a unit test with ','; unit tests can only be selected whole, by name"
            ),
            help: None,
        });
    }
    if parts.len() > 1 {
        return Ok((BTreeSet::new(), true));
    }
    Ok(classified
        .into_iter()
        .next()
        .unwrap_or((BTreeSet::new(), true)))
}

fn classify(
    part: &str,
    names: &UnitTestSelectorNames,
    accepts_unit_tests: bool,
) -> Result<Classified, SelectorError> {
    let Ok(ParsedSelector::Kind {
        kind,
        value,
        upstream,
        downstream,
    }) = parse_selector(part)
    else {
        return Ok((BTreeSet::new(), true));
    };
    if kind != NAME_KIND && kind != TEST_KIND {
        return Ok((BTreeSet::new(), true));
    }
    let matched_tests: BTreeSet<String> = matching(&value, &names.tests);
    if kind == NAME_KIND && !matching(&value, &names.resources).is_empty() {
        return Ok((BTreeSet::new(), true));
    }
    if matched_tests.is_empty() {
        if kind == TEST_KIND || accepts_unit_tests {
            return Err(unknown(&kind, &value, names));
        }
        return Ok((BTreeSet::new(), true));
    }
    if !accepts_unit_tests {
        return Err(unit_test_selector_rejection(part));
    }
    if upstream || downstream {
        return Err(SelectorError {
            code: UNIT_TEST_SELECTOR_CODE,
            message: format!(
                "selector '{part}' expands a unit test with '+'; unit tests have no lineage, so select them by name without '+'"
            ),
            help: None,
        });
    }
    Ok((matched_tests, false))
}

fn unknown(kind: &str, value: &str, names: &UnitTestSelectorNames) -> SelectorError {
    let label: &str = if is_pattern(value) { "pattern" } else { "name" };
    let (subject, candidates): (&str, Vec<&str>) = if kind == NAME_KIND {
        (
            "selector",
            names
                .tests
                .union(&names.resources)
                .map(String::as_str)
                .collect(),
        )
    } else {
        (
            "unit test",
            names.tests.iter().map(String::as_str).collect(),
        )
    };
    SelectorError {
        code: UNKNOWN_SELECTOR_CODE,
        message: format!("unknown {subject} {label} '{value}'"),
        help: selector_name_help(value, candidates),
    }
}

/// The names `value` selects: itself, else every `value[<item>]` of a `for_each` asset, else its glob matches.
fn matching(value: &str, names: &BTreeSet<String>) -> BTreeSet<String> {
    if names.contains(value) {
        return BTreeSet::from([value.to_owned()]);
    }
    if !is_pattern(value) {
        let prefix: String = format!("{value}[");
        return names
            .iter()
            .filter(|name| name.starts_with(&prefix) && name.ends_with(']'))
            .cloned()
            .collect();
    }
    names
        .iter()
        .filter(|name| glob_match(name, value))
        .cloned()
        .collect()
}

fn is_pattern(value: &str) -> bool {
    value.contains(PATTERN_CHARACTERS)
}
