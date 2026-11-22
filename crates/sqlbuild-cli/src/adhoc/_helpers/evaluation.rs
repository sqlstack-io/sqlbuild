//! A selection evaluated as `sqb build` resolves it, with each refusal traced to the token that causes it.

use std::collections::{BTreeMap, BTreeSet};

use sqlbuild_analysis::graph::main::parse_selector::parse_selector;
use sqlbuild_analysis::graph::models::ParsedSelector;
use sqlbuild_core::text::main::close_matches::close_matches;
use sqlbuild_orchestration::runs::models::{
    SelectionField, SelectionLocation, SelectionProblem, SelectionRefusal, SelectionVocabulary,
    VocabularyEntry,
};

use crate::adhoc::_helpers::planner::resolve_with_tests;
use crate::adhoc::_helpers::publication::{IndexedPublication, is_sql_test, name_of};
use crate::adhoc::constants::{
    BUILD_VERB, FUNCTION_KIND_ALIAS, FUNCTION_KINDS, GLOB_CHARACTERS, GLOB_SELECTOR, NAME_SELECTOR,
    PROJECT_FOLDER, SUGGESTION_CUTOFF, SUGGESTION_LIMIT,
};
use crate::adhoc::models::Refusal;

/// The sorted keys `sqb build --select <select> --exclude <exclude>` selects, or why and where it refuses them.
pub(crate) fn evaluate(
    publication: &IndexedPublication,
    (select, exclude): (&str, &str),
) -> Result<Vec<String>, Box<SelectionProblem>> {
    built(publication, &tokens(select), &tokens(exclude))
        .map(|keys| keys.into_iter().collect())
        .map_err(|refusal| Box::new(problem(publication, (select, exclude), refusal)))
}

/// Tags, folders, jobs, sources, seeds and kinds, each with the number of nodes it selects.
pub(crate) fn vocabulary(publication: &IndexedPublication) -> SelectionVocabulary {
    let mut tags: BTreeMap<String, u64> = BTreeMap::new();
    for (tag, keys) in &publication.graph.tags {
        *tags.entry(tag.clone()).or_default() += keys.len() as u64;
    }
    for (tag, names) in &publication.python.graph.tags {
        *tags.entry(tag.clone()).or_default() += names.len() as u64;
    }
    let mut paths: BTreeMap<String, u64> = BTreeMap::new();
    let mut kinds: BTreeMap<String, u64> = BTreeMap::new();
    let mut sources: BTreeMap<String, u64> = BTreeMap::new();
    let mut seeds: BTreeMap<String, u64> = BTreeMap::new();
    for resource in publication
        .resources
        .iter()
        .filter(|resource| !is_sql_test(resource))
    {
        let (kind, name) = resource.key.split_once(':').unwrap_or_default();
        *kinds.entry(kind.to_owned()).or_default() += 1;
        match kind {
            "source" => *sources.entry(name.to_owned()).or_default() += 1,
            "seed" => *seeds.entry(name.to_owned()).or_default() += 1,
            _ => {}
        }
        for folder in resource.folder.iter().flat_map(|folder| folders(folder)) {
            *paths.entry(folder).or_default() += 1;
        }
    }
    let functions: u64 = FUNCTION_KINDS
        .iter()
        .filter_map(|kind| kinds.get(*kind))
        .sum();
    if functions > 0 {
        kinds.insert(FUNCTION_KIND_ALIAS.to_owned(), functions);
    }
    let jobs: BTreeMap<String, u64> = publication
        .jobs
        .selections
        .iter()
        .map(|(name, keys)| (name.clone(), keys.len() as u64))
        .collect();
    SelectionVocabulary {
        snapshot_id: None,
        tags: entries(tags),
        paths: entries(paths),
        jobs: entries(jobs),
        sources: entries(sources),
        seeds: entries(seeds),
        kinds: entries(kinds),
    }
}

fn tokens(text: &str) -> Vec<String> {
    text.split_whitespace().map(str::to_owned).collect()
}

/// The selection a plain `sqb build` resolves, Python nodes and unit tests included.
fn built(
    publication: &IndexedPublication,
    select: &[String],
    exclude: &[String],
) -> Result<BTreeSet<String>, Refusal> {
    resolve_with_tests((BUILD_VERB, true), publication, select, exclude)
}

fn problem(
    publication: &IndexedPublication,
    texts: (&str, &str),
    refusal: Refusal,
) -> SelectionProblem {
    let location: Option<SelectionLocation> = locate(publication, texts, &refusal);
    let suggestions: Vec<String> = location
        .as_ref()
        .map(|location| suggestions(publication, &location.token))
        .unwrap_or_default();
    let refused: Option<SelectionRefusal> = location
        .as_ref()
        .and_then(|location| refusal_of(publication, &location.token));
    let (code, message, help) = match refusal {
        Refusal::Selector(error) => (Some(error.code.to_owned()), error.message, error.help),
        Refusal::Selection(message) => (None, message, None),
    };
    SelectionProblem {
        code,
        message,
        help,
        location,
        suggestions,
        refused,
    }
}

/// The first token that, selected alone, is refused exactly as the whole selection was; none for a refusal of the whole.
fn locate(
    publication: &IndexedPublication,
    (select, exclude): (&str, &str),
    refusal: &Refusal,
) -> Option<SelectionLocation> {
    if matches!(refusal, Refusal::Selection(_)) {
        return None;
    }
    for (field, text) in [
        (SelectionField::Select, select),
        (SelectionField::Exclude, exclude),
    ] {
        for (start, end, token) in spans(text) {
            let alone: Option<Refusal> = built(publication, &[token.to_owned()], &[]).err();
            if alone.as_ref() == Some(refusal) {
                return Some(SelectionLocation {
                    field,
                    token: token.to_owned(),
                    start: utf16_offset(text, start),
                    end: utf16_offset(text, end),
                });
            }
        }
    }
    None
}

/// Each whitespace-separated token with its byte range.
fn spans(text: &str) -> Vec<(usize, usize, &str)> {
    let mut spans: Vec<(usize, usize, &str)> = Vec::new();
    let mut start: Option<usize> = None;
    for (index, character) in text.char_indices().chain([(text.len(), ' ')]) {
        match (start, character.is_whitespace()) {
            (None, false) => start = Some(index),
            (Some(begin), true) => {
                spans.push((begin, index, &text[begin..index]));
                start = None;
            }
            _ => {}
        }
    }
    spans
}

fn utf16_offset(text: &str, byte: usize) -> u32 {
    u32::try_from(text[..byte].encode_utf16().count()).unwrap_or(u32::MAX)
}

/// The first comma part of `token` that is refused on its own, with its position.
fn refused_part<'token>(
    publication: &IndexedPublication,
    token: &'token str,
) -> Option<(usize, &'token str)> {
    token
        .split(',')
        .enumerate()
        .find(|(_, part)| built(publication, &[(*part).to_owned()], &[]).is_err())
}

/// The refused part's text, position, selector type and value; a path-between part names no single value.
fn refusal_of(publication: &IndexedPublication, token: &str) -> Option<SelectionRefusal> {
    let (index, part) = refused_part(publication, token)?;
    let Ok(ParsedSelector::Kind { kind, value, .. }) = parse_selector(part) else {
        return None;
    };
    let selector_type: String = if kind == NAME_SELECTOR && value.contains(GLOB_CHARACTERS) {
        GLOB_SELECTOR.to_owned()
    } else {
        kind
    };
    Some(SelectionRefusal {
        part: part.to_owned(),
        index: u32::try_from(index).unwrap_or(u32::MAX),
        selector_type,
        value,
    })
}

/// `token` with its first refused comma part's value replaced by the closest values of that selector type.
fn suggestions(publication: &IndexedPublication, token: &str) -> Vec<String> {
    let Some((index, part)) = refused_part(publication, token) else {
        return Vec::new();
    };
    let Ok(ParsedSelector::Kind {
        kind,
        value,
        upstream,
        downstream,
    }) = parse_selector(part)
    else {
        return Vec::new();
    };
    let parts: Vec<&str> = token.split(',').collect();
    let typed: bool = part.trim_matches('+').contains(':');
    let candidates: Vec<String> = candidates(publication, &kind);
    let candidates: Vec<&str> = candidates.iter().map(String::as_str).collect();
    let marked = |replacement: &str| -> String {
        format!(
            "{}{}{replacement}{}",
            if upstream { "+" } else { "" },
            if typed {
                format!("{kind}:")
            } else {
                String::new()
            },
            if downstream { "+" } else { "" },
        )
    };
    let mut replacements: Vec<String> = Vec::new();
    for replacement in close_matches(&value, &candidates, SUGGESTION_LIMIT, SUGGESTION_CUTOFF) {
        let part: String = marked(&replacement);
        let mut rebuilt: Vec<&str> = parts.clone();
        rebuilt[index] = &part;
        replacements.push(rebuilt.join(","));
    }
    replacements
}

/// The values a selector of `kind` can name in this publication.
fn candidates(publication: &IndexedPublication, kind: &str) -> Vec<String> {
    let vocabulary: SelectionVocabulary = publication
        .vocabulary
        .get_or_init(|| vocabulary(publication))
        .clone();
    let values = |entries: Vec<VocabularyEntry>| -> Vec<String> {
        entries.into_iter().map(|entry| entry.value).collect()
    };
    match kind {
        "tag" => values(vocabulary.tags),
        "path" => values(vocabulary.paths),
        "kind" => values(vocabulary.kinds),
        "job" => values(vocabulary.jobs),
        "test" => publication.names.tests.iter().cloned().collect(),
        "name" => publication
            .names
            .resources
            .iter()
            .chain(&publication.names.tests)
            .cloned()
            .collect(),
        _ => publication
            .resources
            .iter()
            .filter(|resource| kind_of(&resource.key) == kind)
            .map(|resource| name_of(&resource.key).to_owned())
            .collect(),
    }
}

/// The kind part of a `kind:name` key.
fn kind_of(key: &str) -> &str {
    key.split_once(':').map_or("", |(kind, _)| kind)
}

/// `models/staging/orders` and every folder above it.
fn folders(folder: &str) -> Vec<String> {
    if folder.is_empty() || folder == PROJECT_FOLDER {
        return Vec::new();
    }
    let parts: Vec<&str> = folder.split('/').collect();
    (1..=parts.len())
        .map(|depth| parts[..depth].join("/"))
        .collect()
}

fn entries(counts: BTreeMap<String, u64>) -> Vec<VocabularyEntry> {
    counts
        .into_iter()
        .map(|(value, count)| VocabularyEntry { value, count })
        .collect()
}
