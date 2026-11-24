//! Declaration-definition folders: browsing their children and listing what lies beneath.

use crate::scope_index::constants::GLOBAL_ROLE_PARENT;
use crate::scope_index::models::{
    DeclarationIdentity, DeclarationRecord, QualifiedIdentity, ScopeKind,
};
use crate::scope_report::_helpers::context::Context;
use crate::scope_report::_helpers::declarations::{declaration_report, declaration_text};
use crate::scope_report::_helpers::paths::{ancestry, normalized};
use crate::scope_report::_helpers::selection::count;
use crate::scope_report::_helpers::selection::{glob_matches, report_key};
use crate::scope_report::_helpers::targets::{
    diagnostic, query, resource_matches, shown, target_text,
};
use crate::scope_report::constants::{
    CURRENT_PATH, IDENTITY_SEPARATOR, INVALID_CURSOR, INVALID_PROSPECTIVE_PATH, LIST_SECTION,
    UNKNOWN_TARGET,
};
use crate::scope_report::models::{
    BrowseResult, DeclarationReport, DiagnosticFacts, FolderRequest, ListResult, QueryTarget,
    ReportDiagnostic, ReportFailure, ReportFilters, ReportSource, ScopeFolder, ScopeSection,
};
use sqlbuild_core::posix_path::main::posix_join::posix_join;
use sqlbuild_core::posix_path::main::posix_name::posix_name;
use sqlbuild_core::posix_path::main::posix_parent::posix_parent;
use sqlbuild_core::posix_path::main::posix_parts::posix_parts;
use std::collections::{BTreeMap, HashSet};

/// `_target_used_identities`: what the target uses, its diagnostics, and usage completeness.
pub(crate) fn target_used<'a>(
    context: &Context<'a>,
    target: Option<&QueryTarget>,
    prospective: bool,
) -> (HashSet<&'a DeclarationIdentity>, Vec<DiagnosticFacts>, bool) {
    if prospective {
        return (HashSet::new(), Vec::new(), false);
    }
    let runtime_usage: bool = context.completeness().runtime_usage;
    let Some(target) = target else {
        let used: HashSet<&'a DeclarationIdentity> = context
            .source
            .records
            .usages
            .iter()
            .map(|usage| &usage.declaration)
            .collect();
        return (used, Vec::new(), runtime_usage);
    };
    let shown: String = target_text(target);
    let matches: Vec<usize> = match query(context, target) {
        Ok(matched) => resource_matches(matched),
        Err(_) => {
            let message: String = format!("Browse/list target {shown} is not a known resource");
            return (
                HashSet::new(),
                vec![diagnostic(UNKNOWN_TARGET, message, None)],
                false,
            );
        }
    };
    let [resource] = matches.as_slice() else {
        let message: String =
            format!("Browse/list target {shown} does not resolve to exactly one resource");
        return (
            HashSet::new(),
            vec![diagnostic(UNKNOWN_TARGET, message, None)],
            false,
        );
    };
    let consumer: QualifiedIdentity =
        QualifiedIdentity::Resource(context.resource(*resource).identity.clone());
    let used: HashSet<&'a DeclarationIdentity> = context
        .usages_of_consumer(&consumer)
        .iter()
        .map(|usage| &context.usage(*usage).declaration)
        .collect();
    (used, Vec::new(), runtime_usage)
}

/// `_definition_folder`: globals under `global/<kind>s`, every other declaration in its folder.
pub(crate) fn definition_folder(record: &DeclarationRecord) -> String {
    if record.scope != ScopeKind::Global {
        return posix_parent(&record.path);
    }
    let role_depth: usize = if posix_parts(&record.path).first() == Some(&GLOBAL_ROLE_PARENT) {
        2
    } else {
        1
    };
    let source_root: String = posix_parts(&record.path)
        .into_iter()
        .take(role_depth)
        .collect::<Vec<&str>>()
        .join("/");
    let parent: String = posix_parent(&record.path);
    let relative: &str = parent
        .strip_prefix(source_root.as_str())
        .unwrap_or(&parent)
        .trim_matches('/');
    let base: String = format!("global/{}s", record.identity.kind.as_str());
    if relative.is_empty() || relative == CURRENT_PATH {
        base
    } else {
        format!("{base}/{relative}")
    }
}

/// The direct child of `parent_parts` holding `folder`, if `folder` lies strictly below it.
pub(crate) fn direct_child(folder: &str, parent_parts: &[&str]) -> Option<String> {
    let parts: Vec<&str> = posix_parts(folder);
    if parts.len() <= parent_parts.len() || parts[..parent_parts.len()] != *parent_parts {
        return None;
    }
    Some(posix_join(&parts[..=parent_parts.len()]))
}

/// The parts of a browsed folder; the project root has none.
pub(crate) fn folder_parts(folder: &str) -> Vec<&str> {
    if folder == CURRENT_PATH {
        Vec::new()
    } else {
        posix_parts(folder)
    }
}

/// `_folder_report`: counts beneath one child folder.
pub(crate) fn folder_report(
    context: &Context<'_>,
    path: String,
    records: &[usize],
    used: &HashSet<&DeclarationIdentity>,
) -> ScopeFolder {
    let prefix: Vec<&str> = posix_parts(&path);
    let mut children: HashSet<String> = HashSet::new();
    let mut kinds: BTreeMap<&str, i64> = BTreeMap::new();
    let mut used_count: i64 = 0;
    for position in records {
        let record: &DeclarationRecord = context.declaration(*position);
        let parts_folder: String = definition_folder(record);
        let parts: Vec<&str> = posix_parts(&parts_folder);
        if parts.len() > prefix.len() {
            children.insert(posix_join(&parts[..=prefix.len()]));
        }
        *kinds.entry(record.identity.kind.as_str()).or_default() += 1;
        used_count += i64::from(used.contains(&record.identity));
    }
    ScopeFolder {
        name: posix_name(&path).to_owned(),
        path,
        descendant_count: count(records.len()),
        child_count: count(children.len()),
        used_count,
        kind_counts: kinds
            .into_iter()
            .map(|(kind, total)| (kind.to_owned(), total))
            .collect(),
    }
}

/// The browse result of `request` over one lookup.
pub(crate) fn browsing(source: ReportSource<'_>, request: &FolderRequest) -> BrowseResult {
    let context: Context<'_> = Context::new(source);
    let Ok(folder) = normalized(&request.folder) else {
        let message: String = format!(
            "Folder {} is not a project-relative declaration folder",
            shown(&request.folder)
        );
        return BrowseResult {
            folder: request.folder.clone(),
            folders: Vec::new(),
            diagnostics: vec![ReportDiagnostic::Raised(diagnostic(
                INVALID_PROSPECTIVE_PATH,
                message,
                None,
            ))],
            complete: false,
        };
    };
    let (used, target_diagnostics, usage_complete) =
        target_used(&context, request.target.as_ref(), request.prospective);
    let parent: Vec<&str> = folder_parts(&folder);
    let mut children: BTreeMap<String, Vec<usize>> = BTreeMap::new();
    for position in context.canonical_declarations() {
        let defined: String = definition_folder(context.declaration(*position));
        if let Some(child) = direct_child(&defined, &parent) {
            children.entry(child).or_default().push(*position);
        }
    }
    let folders: Vec<ScopeFolder> = children
        .into_iter()
        .map(|(path, records)| folder_report(&context, path, &records, &used))
        .collect();
    let complete: bool =
        context.completeness().discovery && usage_complete && target_diagnostics.is_empty();
    BrowseResult {
        folder,
        folders,
        diagnostics: (0..source.lookup.diagnostics.len())
            .map(ReportDiagnostic::Index)
            .chain(target_diagnostics.into_iter().map(ReportDiagnostic::Raised))
            .collect(),
        complete,
    }
}

/// `ScopeReportFilters(globals="all")`.
fn default_filters() -> ReportFilters {
    ReportFilters {
        sections: vec![
            "available".to_owned(),
            "used".to_owned(),
            "relationship_scope".to_owned(),
        ],
        include_nearby: false,
        defined_under: None,
        kinds: Vec::new(),
        glob: None,
        used_only: false,
        dependency_depth: 0,
        sort: "identity".to_owned(),
        cursor: None,
        page_size: 100,
        nearby_depth: 1,
        globals: "all".to_owned(),
    }
}

/// The records beneath `under`; any path `normalize_path` rejects empties them.
fn defined_beneath(context: &Context<'_>, records: Vec<usize>, under: &str) -> Vec<usize> {
    let Ok(under) = normalized(under) else {
        return Vec::new();
    };
    let mut kept: Vec<usize> = Vec::new();
    for position in records {
        match ancestry(&context.declaration(position).path, &under) {
            Ok(true) => kept.push(position),
            Ok(false) => {}
            Err(_) => return Vec::new(),
        }
    }
    kept
}

/// The list result of `request`, or the path error Python raises.
pub(crate) fn listing(
    source: ReportSource<'_>,
    request: &FolderRequest,
) -> Result<ListResult, ReportFailure> {
    let context: Context<'_> = Context::new(source);
    let filters: ReportFilters = request.filters.clone().unwrap_or_else(default_filters);
    let Ok(folder) = normalized(&request.folder) else {
        let message: String = format!(
            "Folder {} is not a project-relative declaration folder",
            shown(&request.folder)
        );
        return Ok(ListResult {
            folder: request.folder.clone(),
            declarations: Vec::new(),
            section: ScopeSection {
                name: LIST_SECTION.to_owned(),
                total: 0,
                returned: 0,
                collapsed: false,
                collapsed_count: 0,
                truncated: false,
                complete: false,
                next_cursor: None,
                cursor: None,
                page_size: filters.page_size,
            },
            diagnostics: vec![ReportDiagnostic::Raised(diagnostic(
                INVALID_PROSPECTIVE_PATH,
                message,
                None,
            ))],
        });
    };
    let (used, target_diagnostics, usage_complete) =
        target_used(&context, request.target.as_ref(), request.prospective);
    let mut records: Vec<usize> = Vec::new();
    for position in context.canonical_declarations() {
        let defined: String = definition_folder(context.declaration(*position));
        if ancestry(&defined, &folder).map_err(ReportFailure::Path)? {
            records.push(*position);
        }
    }
    if !filters.kinds.is_empty() {
        records.retain(|position| {
            let kind: &str = context.declaration(*position).identity.kind.as_str();
            filters.kinds.iter().any(|selected| selected == kind)
        });
    }
    if filters.used_only {
        records.retain(|position| used.contains(&context.declaration(*position).identity));
    }
    if let Some(under) = &filters.defined_under {
        records = defined_beneath(&context, records, under);
    }
    if let Some(glob) = &filters.glob {
        records.retain(|position| glob_matches(context.declaration(*position), glob));
    }
    records.sort_by_cached_key(|position| report_key(&context, *position));
    let mut cursor_diagnostic: Option<DiagnosticFacts> = None;
    let mut cursor: Option<String> = filters.cursor.clone();
    if let Some(requested) = cursor.as_deref()
        && !requested.contains(IDENTITY_SEPARATOR)
    {
        let candidates: Vec<usize> = records
            .iter()
            .copied()
            .filter(|position| context.declaration(*position).identity.name == requested)
            .collect();
        cursor = match candidates.as_slice() {
            [only] => Some(declaration_text(context.declaration(*only))),
            _ => {
                let message: String = format!("Cursor {} is invalid", shown(requested));
                cursor_diagnostic = Some(diagnostic(INVALID_CURSOR, message, None));
                None
            }
        };
    }
    let mut start: usize = 0;
    if let Some(cursor) = cursor.as_deref() {
        let positions: Vec<usize> = records
            .iter()
            .enumerate()
            .filter(|(_, position)| declaration_text(context.declaration(**position)) == cursor)
            .map(|(index, _)| index)
            .collect();
        match positions.as_slice() {
            [index] => start = index + 1,
            _ => {
                let message: String = format!("Cursor {} is invalid", shown(cursor));
                cursor_diagnostic = Some(diagnostic(INVALID_CURSOR, message, None));
            }
        }
    }
    let size: usize = usize::try_from(filters.page_size.max(0)).unwrap_or(usize::MAX);
    let end: usize = start.saturating_add(size).min(records.len());
    let selected: &[usize] = records.get(start..end).unwrap_or_default();
    let reports: Vec<DeclarationReport> = selected
        .iter()
        .map(|position| declaration_report(&context, *position, None, None))
        .collect();
    let truncated: bool = start + selected.len() < records.len();
    let section: ScopeSection = ScopeSection {
        name: LIST_SECTION.to_owned(),
        total: count(records.len()),
        returned: count(reports.len()),
        collapsed: false,
        collapsed_count: 0,
        truncated,
        complete: context.completeness().discovery
            && usage_complete
            && target_diagnostics.is_empty(),
        next_cursor: reports
            .last()
            .filter(|_| truncated)
            .map(|report| report.identity.clone()),
        cursor,
        page_size: count(size),
    };
    Ok(ListResult {
        folder,
        declarations: reports,
        section,
        diagnostics: (0..source.lookup.diagnostics.len())
            .map(ReportDiagnostic::Index)
            .chain(target_diagnostics.into_iter().map(ReportDiagnostic::Raised))
            .chain(cursor_diagnostic.into_iter().map(ReportDiagnostic::Raised))
            .collect(),
    })
}
