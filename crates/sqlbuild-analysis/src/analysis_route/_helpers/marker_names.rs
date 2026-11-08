//! One analysis name per distinct relation marker call, so repeated calls read as one relation.

use std::sync::LazyLock;

use regex::{Captures, Regex};

use crate::relation_markers::constants::MARKER_NAME_PREFIX;
use crate::relation_markers::types::MarkerCall;

static MARKER_NAME: LazyLock<Result<Regex, String>> = LazyLock::new(|| {
    Regex::new(&format!(r"\b{MARKER_NAME_PREFIX}(\d+)\b")).map_err(|error| error.to_string())
});

/// `named` with each marker name renamed to its call's first index, padded to keep positions.
pub(crate) fn first_call_names(named: &str, calls: &[MarkerCall]) -> Result<String, String> {
    let pattern: &Regex = MARKER_NAME.as_ref().map_err(Clone::clone)?;
    Ok(pattern
        .replace_all(named, |captures: &Captures<'_>| {
            let written: &str = captures.get(0).map_or("", |found| found.as_str());
            let first: Option<usize> = match captures.get(1).map(|index| index.as_str().parse()) {
                Some(Ok(index)) => first_index(calls, index),
                Some(Err(_)) | None => None,
            };
            match first {
                Some(first) => {
                    let name: String = format!("{MARKER_NAME_PREFIX}{first}");
                    format!("{name:<width$}", width = written.len())
                }
                None => written.to_owned(),
            }
        })
        .into_owned())
}

/// The calls whose index is their call's first, in order, with that index.
pub(crate) fn distinct_calls(calls: &[MarkerCall]) -> Vec<(usize, &MarkerCall)> {
    let mut distinct: Vec<(usize, &MarkerCall)> = Vec::with_capacity(calls.len());
    for (index, call) in calls.iter().enumerate() {
        if first_index(calls, index) == Some(index) {
            distinct.push((index, call));
        }
    }
    distinct
}

/// The first index of the call at `index`, which is `index` unless an earlier call is the same.
fn first_index(calls: &[MarkerCall], index: usize) -> Option<usize> {
    let call: &MarkerCall = calls.get(index)?;
    calls.iter().position(|earlier| earlier == call)
}
