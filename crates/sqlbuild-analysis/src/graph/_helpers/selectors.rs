//! Python's planner selector grammar and resolution over the project graph.

use std::collections::BTreeSet;

use crate::assembly::project::types::ObjectKey;
use crate::graph::constants::{
    EMPTY_SELECTOR_CODE, EMPTY_VALUE_CODE, EXPANSION_MARKER, FOLDER_SEPARATOR,
    INTERSECTION_SEPARATOR, JOB_KIND, KIND_SEPARATOR, MISPLACED_MARKER_CODE, MISSING_NAME_CODE,
    MODEL_RESOURCE, MODEL_ROOT, MODEL_ROOT_PREFIX, NAME_KIND, PATH_KIND, PATH_ROOT_CODE,
    PATH_ROOT_ERROR, PATH_SELECTOR_CODE, PATH_SEPARATOR, PATTERN_CHARACTERS, PLANNER_DEFAULT_CODE,
    SEED_RESOURCE, SOURCE_RESOURCE, SQL_FILE_SUFFIX, SUGGESTION_CUTOFF, SUGGESTION_LIMIT,
    TABLE_FUNCTION_RESOURCE, TAG_KIND, TEST_KIND, UDF_RESOURCE, UNIT_TEST_CODE,
    UNIT_TEST_ONLY_TEST_AND_BUILD, UNKNOWN_KIND_CODE, UNKNOWN_NAME_CODE, UNKNOWN_PATH_CODE,
    UNKNOWN_TAG_CODE, UNMAPPED_KIND_CODE, UNMAPPED_KINDS,
};
use crate::graph::errors::SelectorError;
use crate::graph::models::{
    BuildResources, EdgeDirection as Direction, ParsedSelector, ProjectGraph,
};
use sqlbuild_core::text::main::close_matches::close_matches;
use sqlbuild_core::text::main::glob_match::glob_match;

pub(crate) type Keys = BTreeSet<ObjectKey>;
type Resolved = Result<Keys, SelectorError>;

fn error(code: &'static str, message: String) -> SelectorError {
    SelectorError {
        code,
        message,
        help: None,
    }
}

pub(crate) fn resolve(
    graph: &ProjectGraph,
    select: &[String],
    exclude: &[String],
) -> Result<Vec<ObjectKey>, SelectorError> {
    let all: Keys = graph.names.iter().map(|(_, key)| key.clone()).collect();
    if select.is_empty() && exclude.is_empty() {
        return Ok(all.into_iter().collect());
    }
    let selected: Keys = if select.is_empty() {
        all
    } else {
        tokens(graph, select)?
    };
    let excluded: Keys = tokens(graph, exclude)?;
    let scoped: Keys = selected.difference(&excluded).cloned().collect();
    Ok(build_resources(graph, &scoped, BuildResources::SELECTION)
        .into_iter()
        .collect())
}

/// Python's `expand_required_build_resources`: functions and seeds a selected scope needs.
pub(crate) fn build_resources(
    graph: &ProjectGraph,
    selected: &Keys,
    include: BuildResources,
) -> Keys {
    let mut expanded: Keys = selected.clone();
    for key in selected {
        for upstream in graph.closure(key, Direction::Upstream) {
            let wanted: bool = (include.upstream_functions && is_function(&upstream))
                || (include.upstream_seeds && upstream.0 == SEED_RESOURCE);
            if wanted {
                expanded.insert(upstream);
            }
        }
    }
    if include.downstream_functions {
        for key in selected.iter().filter(|key| key.0 == MODEL_RESOURCE) {
            expanded.extend(
                graph
                    .edges(key, Direction::Downstream)
                    .iter()
                    .filter(|key| is_function(key))
                    .cloned(),
            );
        }
    }
    expanded
}

fn is_function(key: &ObjectKey) -> bool {
    key.0 == UDF_RESOURCE || key.0 == TABLE_FUNCTION_RESOURCE
}

/// The union of whitespace-separated tokens across `selectors`, without build expansion.
pub(crate) fn tokens(graph: &ProjectGraph, selectors: &[String]) -> Resolved {
    let mut resolved: Keys = Keys::new();
    for selector in selectors {
        for token in selector
            .split(char::is_whitespace)
            .filter(|token| !token.is_empty())
        {
            resolved.extend(intersection(graph, token)?);
        }
    }
    Ok(resolved)
}

fn intersection(graph: &ProjectGraph, token: &str) -> Resolved {
    let mut parts = token.split(INTERSECTION_SEPARATOR);
    let mut result: Keys = single(graph, parts.next().unwrap_or_default())?;
    for part in parts {
        let next: Keys = single(graph, part)?;
        result = result.intersection(&next).cloned().collect();
    }
    Ok(result)
}

fn single(graph: &ProjectGraph, raw: &str) -> Resolved {
    match parse(raw)? {
        ParsedSelector::Path {
            start,
            end,
            upstream,
            downstream,
        } => path(graph, (&start, &end), (upstream, downstream)),
        ParsedSelector::Kind {
            kind,
            value,
            upstream,
            downstream,
        } => {
            let keys: Keys = matched(graph, &kind, &value)?;
            let mut result: Keys = keys.clone();
            for key in &keys {
                if upstream {
                    result.extend(graph.closure(key, Direction::Upstream));
                }
                if downstream {
                    result.extend(graph.closure(key, Direction::Downstream));
                }
            }
            Ok(result)
        }
    }
}

fn path(graph: &ProjectGraph, names: (&str, &str), expand: (bool, bool)) -> Resolved {
    let start: ObjectKey = named(graph, names.0)?;
    let end: ObjectKey = named(graph, names.1)?;
    let Some(mut result) = graph.path_nodes(&start, &end) else {
        return Err(error(
            PLANNER_DEFAULT_CODE,
            format!(
                "'{}:{}' is not downstream of '{}:{}'",
                end.0, end.1, start.0, start.1
            ),
        ));
    };
    if expand.0 {
        result.extend(graph.closure(&start, Direction::Upstream));
    }
    if expand.1 {
        result.extend(graph.closure(&end, Direction::Downstream));
    }
    Ok(result)
}

fn named(graph: &ProjectGraph, name: &str) -> Result<ObjectKey, SelectorError> {
    graph
        .key_named(name)
        .cloned()
        .ok_or_else(|| error(UNKNOWN_NAME_CODE, format!("unknown selector name '{name}'")))
}

pub(crate) fn parse(raw: &str) -> Result<ParsedSelector, SelectorError> {
    let stripped: &str = raw.trim_matches(char::is_whitespace);
    if stripped.is_empty() {
        return Err(error(EMPTY_SELECTOR_CODE, "empty selector".to_owned()));
    }
    let upstream: bool = stripped.starts_with(EXPANSION_MARKER);
    let downstream: bool = stripped.ends_with(EXPANSION_MARKER);
    let core: &str = stripped
        .trim_start_matches(EXPANSION_MARKER)
        .trim_end_matches(EXPANSION_MARKER);
    if core.is_empty() {
        return Err(error(
            MISSING_NAME_CODE,
            format!("selector '{stripped}' has no name after removing '+' markers"),
        ));
    }
    if core.contains(EXPANSION_MARKER) {
        return Err(error(
            MISPLACED_MARKER_CODE,
            format!("selector '{stripped}' contains '+' in an unsupported position"),
        ));
    }
    if let Some((start, end)) = core.split_once(PATH_SEPARATOR) {
        let (start, end) = (
            start.trim_matches(char::is_whitespace),
            end.trim_matches(char::is_whitespace),
        );
        if start.is_empty() || end.is_empty() {
            return Err(error(
                PATH_SELECTOR_CODE,
                format!("path selector '{stripped}' requires names on both sides of '~'"),
            ));
        }
        return Ok(ParsedSelector::Path {
            start: start.to_owned(),
            end: end.to_owned(),
            upstream,
            downstream,
        });
    }
    let (kind, value): (&str, &str) = match core.split_once(KIND_SEPARATOR) {
        Some((prefix, value)) => {
            if !is_selector_kind(prefix) {
                return Err(error(
                    UNKNOWN_KIND_CODE,
                    format!("unknown selector type '{prefix}' in '{stripped}'"),
                ));
            }
            if value.is_empty() {
                return Err(error(
                    EMPTY_VALUE_CODE,
                    format!("selector '{stripped}' has empty value after ':'"),
                ));
            }
            (prefix, value)
        }
        None if core.contains(FOLDER_SEPARATOR) => (PATH_KIND, core.trim_matches(FOLDER_SEPARATOR)),
        None => (NAME_KIND, core),
    };
    Ok(ParsedSelector::Kind {
        kind: kind.to_owned(),
        value: value.to_owned(),
        upstream,
        downstream,
    })
}

fn is_selector_kind(prefix: &str) -> bool {
    [
        SEED_RESOURCE,
        SOURCE_RESOURCE,
        TAG_KIND,
        PATH_KIND,
        TEST_KIND,
        JOB_KIND,
    ]
    .contains(&prefix)
        || UNMAPPED_KINDS.contains(&prefix)
}

/// Python's `match_selector_keys`: the keys one parsed selector names before `+` expansion.
pub(crate) fn matched(graph: &ProjectGraph, kind: &str, value: &str) -> Resolved {
    match kind {
        TAG_KIND => {
            let keys: Keys = graph.tagged(value);
            if keys.is_empty() {
                return Err(error(
                    UNKNOWN_TAG_CODE,
                    format!("no models found with tag '{value}'"),
                ));
            }
            Ok(keys)
        }
        PATH_KIND => folder(graph, value),
        JOB_KIND => job(graph, value),
        TEST_KIND => Err(error(
            UNIT_TEST_CODE,
            format!(
                "selector '{TEST_KIND}{KIND_SEPARATOR}{value}' selects a unit test; \
                 {UNIT_TEST_ONLY_TEST_AND_BUILD}"
            ),
        )),
        _ => {
            let keys: Keys = lookup(graph, kind, value)?;
            if !keys.is_empty() {
                return Ok(keys);
            }
            let pattern: bool = is_pattern(value);
            let names: Vec<&str> = graph
                .names
                .iter()
                .map(|(name, _)| name.as_str())
                .chain(
                    graph
                        .jobs
                        .iter()
                        .flat_map(|jobs| jobs.keys().map(String::as_str)),
                )
                .collect();
            Err(SelectorError {
                code: UNKNOWN_NAME_CODE,
                message: format!(
                    "unknown selector {} '{value}'",
                    if pattern { "pattern" } else { "name" }
                ),
                help: (!pattern).then(|| suggestion(value, names)).flatten(),
            })
        }
    }
}

pub(crate) fn suggestion(value: &str, mut names: Vec<&str>) -> Option<String> {
    names.sort_unstable();
    let matches: Vec<String> = close_matches(value, &names, SUGGESTION_LIMIT, SUGGESTION_CUTOFF);
    if matches.is_empty() {
        return None;
    }
    let quoted: Vec<String> = matches.iter().map(|name| format!("'{name}'")).collect();
    Some(format!("did you mean {}?", quoted.join(", ")))
}

fn lookup(graph: &ProjectGraph, kind: &str, value: &str) -> Resolved {
    let resource: Option<&str> = match kind {
        NAME_KIND => None,
        SEED_RESOURCE | SOURCE_RESOURCE => Some(kind),
        _ => {
            return Err(error(
                UNMAPPED_KIND_CODE,
                format!("selector type '{kind}' does not map to a resource type yet"),
            ));
        }
    };
    let kind_matches = |key: &ObjectKey| resource.is_none_or(|resource| key.0 == resource);
    if !is_pattern(value) {
        if resource.is_none() && graph.key_named(value).is_none() {
            if let Some(Some(keys)) = graph.jobs.as_ref().map(|jobs| jobs.get(value)) {
                return keys.clone();
            }
        }
        return Ok(graph
            .key_named(value)
            .filter(|key| kind_matches(key))
            .cloned()
            .into_iter()
            .collect());
    }
    Ok(graph
        .names
        .iter()
        .filter(|(name, key)| kind_matches(key) && glob_matches(value, name))
        .map(|(_, key)| key.clone())
        .collect())
}

/// The nodes the jobs and job groups `value` names or matches select.
fn job(graph: &ProjectGraph, value: &str) -> Resolved {
    let Some(jobs) = &graph.jobs else {
        return Err(error(
            UNMAPPED_KIND_CODE,
            format!(
                "selector '{JOB_KIND}{KIND_SEPARATOR}{value}' selects a job; jobs cannot be selected here"
            ),
        ));
    };
    let pattern: bool = is_pattern(value);
    let mut keys: Keys = Keys::new();
    let mut found: bool = false;
    for (name, selected) in jobs {
        if name == value || (pattern && glob_matches(value, name)) {
            found = true;
            keys.extend(selected.clone()?);
        }
    }
    if found {
        return Ok(keys);
    }
    Err(SelectorError {
        code: UNKNOWN_NAME_CODE,
        message: format!(
            "unknown job {} '{value}'",
            if pattern { "pattern" } else { "name" }
        ),
        help: (!pattern)
            .then(|| suggestion(value, jobs.keys().map(String::as_str).collect()))
            .flatten(),
    })
}

fn folder(graph: &ProjectGraph, value: &str) -> Resolved {
    let folder: String = value.replace('\\', "/").trim_matches('/').to_owned();
    let selector: &str = if folder == MODEL_ROOT {
        ""
    } else if let Some(rest) = folder.strip_prefix(MODEL_ROOT_PREFIX) {
        rest
    } else {
        return Err(error(PATH_ROOT_CODE, PATH_ROOT_ERROR.to_owned()));
    };
    let keys: Keys = graph
        .paths
        .iter()
        .filter(|(_, indexed)| {
            selector.is_empty()
                || indexed == selector
                || indexed.starts_with(&format!("{selector}/"))
        })
        .map(|(key, _)| key.clone())
        .collect();
    if keys.is_empty() {
        let hint: &str = if folder.ends_with(SQL_FILE_SUFFIX) {
            " Path selectors match folders; select a single model by its name."
        } else {
            ""
        };
        return Err(error(
            UNKNOWN_PATH_CODE,
            format!("no models found under path '{folder}'.{hint}"),
        ));
    }
    Ok(keys)
}

pub(crate) fn is_pattern(value: &str) -> bool {
    value.contains(PATTERN_CHARACTERS)
}

impl ProjectGraph {
    pub(crate) fn key_named(&self, name: &str) -> Option<&ObjectKey> {
        self.name_index
            .get(name)
            .map(|&position| &self.names[position].1)
    }

    fn tagged(&self, tag: &str) -> Keys {
        self.tags
            .iter()
            .find(|(name, _)| name == tag)
            .map(|(_, keys)| keys.iter().cloned().collect())
            .unwrap_or_default()
    }
}

/// Python's `fnmatch.fnmatchcase`: `*`, `?`, `[seq]` and `[!seq]` over characters.
fn glob_matches(pattern: &str, name: &str) -> bool {
    glob_match(name, pattern)
}
