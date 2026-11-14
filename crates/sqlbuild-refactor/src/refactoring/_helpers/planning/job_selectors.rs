//! `select` and `exclude` in TOML job files, rewritten for renamed and moved models.

use std::collections::BTreeMap;
use std::path::Path;

use serde::Deserialize;
use sqlbuild_core::text::errors::TextDecodeError;
use toml::Spanned;

use crate::refactoring::_helpers::edits::text_edits::{line_column, text_edit};
use crate::refactoring::_helpers::files::paths::parent;
use crate::refactoring::_helpers::files::workspace::project_files_with_suffix;
use crate::refactoring::_helpers::scanning::chars::chars;
use crate::refactoring::constants::{
    JOB_FILES_DIRECTORY, MODEL_ROOT, PARTIAL_FOLDER_MOVE_REASON, TOML_SUFFIX,
    UNREADABLE_JOB_REASON, UNWRITABLE_SELECTOR_REASON,
};
use crate::refactoring::errors::RefactorError;
use crate::refactoring::models::{EditKind, ManualLocation, ModelFacts, TextEdit};

/// The selector fields of a job file; publication checks everything else.
#[derive(Deserialize)]
struct JobSelectors {
    select: Option<Spanned<String>>,
    exclude: Option<Spanned<String>>,
}

/// Selector edits by path, selectors to edit by hand or to check, and unreadable job files.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub(crate) struct SelectorParts {
    pub(crate) edits: Vec<(String, TextEdit)>,
    pub(crate) manual: Vec<ManualLocation>,
    pub(crate) notes: Vec<ManualLocation>,
    pub(crate) blocking: Vec<ManualLocation>,
}

/// What the refactoring changes, as job selectors see it.
struct Changes {
    /// Old model name to new, for each model whose name changes.
    names: BTreeMap<String, String>,
    /// Old folder to new, for each folder every model below it leaves together.
    folders: BTreeMap<String, String>,
    /// Folders some, but not all, of their models leave.
    partial: BTreeMap<String, Vec<String>>,
}

/// Rewrite renamed models and wholly moved folders in job selectors; a partly moved folder becomes a note.
pub(crate) fn job_selector_parts(
    project_dir: &Path,
    models: &[ModelFacts],
    renamed: &[(&ModelFacts, &str, &str)],
) -> Result<SelectorParts, RefactorError> {
    let mut parts = SelectorParts::default();
    let jobs_dir = project_dir.join(JOB_FILES_DIRECTORY);
    if !jobs_dir.is_dir() {
        return Ok(parts);
    }
    let changes = changes(models, renamed);
    for (relative, text) in project_files_with_suffix(&jobs_dir, TOML_SUFFIX)? {
        let path = format!("{JOB_FILES_DIRECTORY}/{relative}");
        let contents = match text {
            Ok(contents) => contents,
            Err(TextDecodeError { .. }) => {
                parts
                    .blocking
                    .push(location(path, None, UNREADABLE_JOB_REASON));
                continue;
            }
        };
        let Ok(job) = toml::from_str::<JobSelectors>(&contents) else {
            continue;
        };
        let text = chars(&contents);
        for field in [job.select, job.exclude].into_iter().flatten() {
            let span = field.span();
            let (start, end) = (
                contents[..span.start].chars().count(),
                contents[..span.end].chars().count(),
            );
            let (line, column) = line_column(&text, start);
            for (folder, moved) in &changes.partial {
                if selected_folders(field.get_ref()).any(|selected| selected == *folder) {
                    let reason = PARTIAL_FOLDER_MOVE_REASON
                        .replace("{folder}", folder)
                        .replace("{models}", &moved.join(", "));
                    parts
                        .notes
                        .push(location(path.clone(), Some((line, column)), &reason));
                }
            }
            let rewritten = rewrite(field.get_ref(), &changes);
            if rewritten == *field.get_ref() {
                continue;
            }
            match literal(&contents[span.start..span.end], &rewritten) {
                Some(replacement) => parts.edits.push((
                    path.clone(),
                    text_edit(&text, (start, end), replacement, EditKind::Selector),
                )),
                None => parts.manual.push(location(
                    path.clone(),
                    Some((line, column)),
                    &UNWRITABLE_SELECTOR_REASON.replace("{selector}", &rewritten),
                )),
            }
        }
    }
    Ok(parts)
}

fn location(path: String, at: Option<(usize, usize)>, reason: &str) -> ManualLocation {
    ManualLocation {
        path,
        line: at.map(|(line, _)| line),
        column: at.map(|(_, column)| column),
        reason: reason.to_owned(),
    }
}

fn changes(models: &[ModelFacts], renamed: &[(&ModelFacts, &str, &str)]) -> Changes {
    let names: BTreeMap<String, String> = renamed
        .iter()
        .filter(|(model, new_name, _)| model.name != *new_name)
        .map(|(model, new_name, _)| (model.name.clone(), (*new_name).to_owned()))
        .collect();
    let destinations: BTreeMap<&str, String> = renamed
        .iter()
        .map(|(model, _, destination)| (model.path.as_str(), parent(destination)))
        .collect();
    let mut folders: BTreeMap<String, String> = BTreeMap::new();
    let mut partial: BTreeMap<String, Vec<String>> = BTreeMap::new();
    let moved_from = renamed
        .iter()
        .map(|(model, _, destination)| (parent(&model.path), parent(destination)))
        .filter(|(from, to)| from != to)
        .flat_map(|(from, _)| ancestors(&from));
    for folder in moved_from {
        if folder == MODEL_ROOT || folders.contains_key(&folder) || partial.contains_key(&folder) {
            continue;
        }
        let below: Vec<&ModelFacts> = models
            .iter()
            .filter(|model| within(&parent(&model.path), &folder))
            .collect();
        let moved_to = |model: &ModelFacts| -> Option<String> {
            let to = destinations.get(model.path.as_str())?;
            let rest = &parent(&model.path)[folder.len()..];
            to.strip_suffix(rest).map(str::to_owned)
        };
        let targets: Vec<Option<String>> = below.iter().map(|model| moved_to(model)).collect();
        let whole: Option<String> = targets.first().cloned().flatten().filter(|to| {
            *to != folder
                && !within(to, &folder)
                && all_move_to(&targets, to)
                && !receives_other_models(models, &destinations, &folder, to)
        });
        match whole {
            Some(to) => {
                folders.insert(folder, to);
            }
            None => {
                let leaving: Vec<String> = below
                    .iter()
                    .filter(|model| {
                        destinations
                            .get(model.path.as_str())
                            .is_some_and(|to| !within(to, &folder))
                    })
                    .map(|model| model.name.clone())
                    .collect();
                if !leaving.is_empty() {
                    partial.insert(folder, leaving);
                }
            }
        }
    }
    Changes {
        names,
        folders,
        partial,
    }
}

/// Whether every model below a folder moves to `to`.
fn all_move_to(targets: &[Option<String>], to: &str) -> bool {
    targets.iter().all(|target| target.as_deref() == Some(to))
}

/// Whether a model from outside `folder` keeps or gets a place in `to`, so selecting `to` would widen to it.
fn receives_other_models(
    models: &[ModelFacts],
    destinations: &BTreeMap<&str, String>,
    folder: &str,
    to: &str,
) -> bool {
    models.iter().any(|model| {
        let lands: String = destinations
            .get(model.path.as_str())
            .cloned()
            .unwrap_or_else(|| parent(&model.path));
        within(&lands, to) && !within(&parent(&model.path), folder)
    })
}

/// `folder` and each folder above it, up to the model root.
fn ancestors(folder: &str) -> Vec<String> {
    let parts: Vec<&str> = folder.split('/').collect();
    (1..=parts.len())
        .map(|count| parts[..count].join("/"))
        .collect()
}

fn within(path: &str, folder: &str) -> bool {
    path == folder || path.starts_with(&format!("{folder}/"))
}

/// The folders a selector string names, normalized as path selectors match them.
fn selected_folders(selector: &str) -> impl Iterator<Item = String> + '_ {
    selector
        .split_whitespace()
        .flat_map(|token| token.split(','))
        .filter_map(selected_folder)
}

/// The normalized folder one selector part names, ignoring its graph operators.
fn selected_folder(part: &str) -> Option<String> {
    let (_, folder) = folder_of(part.trim_matches('+'))?;
    Some(folder)
}

/// The `path:` prefix (if any) and normalized folder of a folder selector part.
fn folder_of(core: &str) -> Option<(&str, String)> {
    let (prefix, value) = match core.split_once(':') {
        Some(("path", value)) => ("path:", value),
        Some(_) => return None,
        None if core.contains('/') && !core.contains('~') => ("", core),
        None => return None,
    };
    Some((
        prefix,
        value.replace('\\', "/").trim_matches('/').to_owned(),
    ))
}

/// `selector` with renamed model names and moved folders replaced, whitespace kept.
fn rewrite(selector: &str, changes: &Changes) -> String {
    let mut out = String::with_capacity(selector.len());
    let mut token = String::new();
    for character in selector.chars().chain(std::iter::once(' ')) {
        if character.is_whitespace() {
            if !token.is_empty() {
                let parts: Vec<String> = token
                    .split(',')
                    .map(|part| part_of(part, changes))
                    .collect();
                out.push_str(&parts.join(","));
                token.clear();
            }
            out.push(character);
        } else {
            token.push(character);
        }
    }
    out.pop();
    out
}

fn part_of(part: &str, changes: &Changes) -> String {
    let core = part.trim_matches('+');
    if core.is_empty() {
        return part.to_owned();
    }
    let lead = &part[..part.len() - part.trim_start_matches('+').len()];
    let tail = &part[part.trim_end_matches('+').len()..];
    let name = |value: &str| -> String {
        changes
            .names
            .get(value.trim())
            .map_or_else(|| value.to_owned(), |new| value.replace(value.trim(), new))
    };
    let core = if let Some((start, end)) = core.split_once('~') {
        format!("{}~{}", name(start), name(end))
    } else if let Some((prefix, folder)) = folder_of(core) {
        match changes.folders.get(&folder) {
            Some(to) => {
                let trailing = if core.ends_with('/') { "/" } else { "" };
                format!("{prefix}{to}{trailing}")
            }
            None => core.to_owned(),
        }
    } else if core.contains(':') {
        core.to_owned()
    } else {
        name(core)
    };
    format!("{lead}{core}{tail}")
}

/// `value` written as a TOML string in the style of `raw`, or `None` for multi-line strings.
fn literal(raw: &str, value: &str) -> Option<String> {
    if raw.starts_with("'''") || raw.starts_with("\"\"\"") {
        return None;
    }
    if raw.starts_with('\'') && !value.contains(['\'', '\n', '\r']) {
        return Some(format!("'{value}'"));
    }
    let mut escaped = String::with_capacity(value.len() + 2);
    escaped.push('"');
    for character in value.chars() {
        match character {
            '"' => escaped.push_str("\\\""),
            '\\' => escaped.push_str("\\\\"),
            '\n' => escaped.push_str("\\n"),
            '\t' => escaped.push_str("\\t"),
            '\r' => escaped.push_str("\\r"),
            control if control.is_control() => {
                escaped.push_str(&format!("\\u{:04X}", u32::from(control)));
            }
            other => escaped.push(other),
        }
    }
    escaped.push('"');
    Some(escaped)
}
