use crate::graph::constants::{INTERSECTION_SEPARATOR, JOB_KIND, PATH_KIND, TAG_KIND};
use crate::graph::models::ParsedSelector;

/// The atoms of `selection` that name one node: bare or typed names, without `+`, globs, tags, paths or jobs.
#[must_use]
pub fn named_selector_atoms(selection: &str) -> Vec<String> {
    selection
        .split(char::is_whitespace)
        .flat_map(|token| token.split(INTERSECTION_SEPARATOR))
        .filter(|atom| names_one_node(atom))
        .map(str::to_owned)
        .collect()
}

fn names_one_node(atom: &str) -> bool {
    match crate::graph::_helpers::selectors::parse(atom) {
        Ok(ParsedSelector::Kind {
            kind,
            value,
            upstream: false,
            downstream: false,
        }) => {
            ![TAG_KIND, PATH_KIND, JOB_KIND].contains(&kind.as_str())
                && !crate::graph::_helpers::selectors::is_pattern(&value)
        }
        _ => false,
    }
}
