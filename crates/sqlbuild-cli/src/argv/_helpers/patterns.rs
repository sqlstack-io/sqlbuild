//! `argparse`'s nargs regular expressions over the `A`/`O`/`-` argument-string pattern.

use std::collections::HashSet;

use crate::argv::models::{ActionSpec, Nargs};

/// One greedy quantified character class: its characters and repetition bounds.
#[derive(Clone, Copy)]
struct Element {
    characters: &'static str,
    minimum: usize,
    maximum: usize,
}

const UNBOUNDED: usize = usize::MAX;

const fn element(characters: &'static str, minimum: usize, maximum: usize) -> Element {
    Element {
        characters,
        minimum,
        maximum,
    }
}

/// `_get_nargs_pattern`: the capture group of one action, as greedy elements.
fn nargs_pattern(action: &ActionSpec) -> Vec<Element> {
    let optional: bool = !action.option_strings.is_empty();
    match (action.nargs, optional) {
        (Nargs::Single, true) => vec![element("A", 1, 1)],
        (Nargs::Single, false) => vec![
            element("-", 0, UNBOUNDED),
            element("A", 1, 1),
            element("-", 0, UNBOUNDED),
        ],
        (Nargs::Optional, true) => vec![element("A", 0, 1)],
        (Nargs::Optional, false) => vec![
            element("-", 0, UNBOUNDED),
            element("A", 0, 1),
            element("-", 0, UNBOUNDED),
        ],
        (Nargs::ZeroOrMore, true) => vec![element("A", 0, UNBOUNDED)],
        (Nargs::ZeroOrMore, false) => {
            vec![element("-", 0, UNBOUNDED), element("A-", 0, UNBOUNDED)]
        }
        (Nargs::OneOrMore, true) => vec![element("A", 1, UNBOUNDED)],
        (Nargs::OneOrMore, false) => vec![
            element("-", 0, UNBOUNDED),
            element("A", 1, 1),
            element("A-", 0, UNBOUNDED),
        ],
        (Nargs::Parser, true) => vec![element("A", 1, 1), element("AO", 0, UNBOUNDED)],
        (Nargs::Parser, false) => vec![
            element("-", 0, UNBOUNDED),
            element("A", 1, 1),
            element("-AO", 0, UNBOUNDED),
        ],
        (Nargs::Zero | Nargs::Attached, true) => Vec::new(),
        (Nargs::Attached, false) => vec![element("-", 0, UNBOUNDED)],
        (Nargs::Zero, false) => vec![element("-", 0, UNBOUNDED)],
    }
}

/// `re.match` of the actions' concatenated groups at the start of `pattern`: each group's length.
pub(crate) fn match_groups(actions: &[&ActionSpec], pattern: &[u8]) -> Option<Vec<usize>> {
    let mut elements: Vec<(usize, Element)> = Vec::new();
    for (group, action) in actions.iter().enumerate() {
        elements.extend(nargs_pattern(action).into_iter().map(|item| (group, item)));
    }
    let mut matcher: Matcher<'_> = Matcher {
        counts: vec![0; elements.len()],
        elements,
        pattern,
        failed: HashSet::new(),
    };
    if !matcher.backtrack(0, 0) {
        return None;
    }
    let mut lengths: Vec<usize> = vec![0; actions.len()];
    for ((group, _), count) in matcher.elements.iter().zip(matcher.counts) {
        lengths[*group] += count;
    }
    Some(lengths)
}

/// A greedy backtracking match of quantified elements, remembering positions known to fail.
struct Matcher<'a> {
    elements: Vec<(usize, Element)>,
    pattern: &'a [u8],
    counts: Vec<usize>,
    failed: HashSet<(usize, usize)>,
}

impl Matcher<'_> {
    /// Python's greedy order: each element takes as many characters as it can first.
    fn backtrack(&mut self, index: usize, position: usize) -> bool {
        let Some((_, item)) = self.elements.get(index).copied() else {
            return true;
        };
        if self.failed.contains(&(index, position)) {
            return false;
        }
        let available: usize = self.pattern[position..]
            .iter()
            .take_while(|character| item.characters.as_bytes().contains(character))
            .count()
            .min(item.maximum);
        if available >= item.minimum {
            for count in (item.minimum..=available).rev() {
                self.counts[index] = count;
                if self.backtrack(index + 1, position + count) {
                    return true;
                }
            }
        }
        self.failed.insert((index, position));
        false
    }
}
