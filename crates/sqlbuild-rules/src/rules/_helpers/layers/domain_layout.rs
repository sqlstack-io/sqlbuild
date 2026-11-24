use crate::configuration::main::setting_help::setting_help;
use crate::models::{DeclarationKind, Fault, Model, RuleMetadata, ScopeDeclaration, ScopeKind};
use crate::rules::_helpers::dispatch::evaluation::path_fault;
use crate::rules::models::ProjectEvaluationRequest;
use std::collections::{BTreeMap, BTreeSet};

const DEFAULT_MAX_SUBDOMAIN_DEPTH: u32 = 1;
const DEFAULT_MIN_SHARED_PREFIXES: u32 = 2;
const DEFAULT_MAX_ROLE_CONTAINER_DEPTH: u32 = 1;
const DEFAULT_MAX_CONTAINER_FILES: u32 = 10;
const MIN_MODEL_PATH_COMPONENTS: usize = 3;
const MIN_BRANCH_OUTCOMES: usize = 2;
/// The kind folders a macro folder holds, each counted as its own container.
const MACRO_KIND_FOLDERS: [&str; 2] = ["python", "sql"];
/// The kind folder of SQL macro files, whose names are their file stems.
const SQL_MACRO_KIND_FOLDER: &str = "sql";
const GENERIC_BUCKET_NAMES: [&str; 8] = [
    "common", "helper", "helpers", "misc", "shared", "util", "utils", "macros",
];
const RESERVED_BUCKET_NAMES: [&str; 6] = [
    "macros",
    "_macros",
    "constants",
    "_constants",
    "enums",
    "_enums",
];

type TokenTerminal = (String, String);
type PrefixCandidate = (String, String, usize);

#[derive(Default)]
struct OwnerNode {
    direct_paths: Vec<String>,
    children: BTreeMap<String, OwnerNode>,
}

#[derive(Default)]
struct TokenNode {
    terminals: Vec<TokenTerminal>,
    children: BTreeMap<String, TokenNode>,
}

struct ResolvedModelPath {
    domain: String,
    level: String,
    owners: Vec<String>,
}

enum ModelPathResolution {
    Resolved(ResolvedModelPath),
    Missing,
    Ambiguous(Vec<String>),
}

struct ContainerEntry {
    path: String,
    buckets: Vec<String>,
    stem: String,
    kind: DeclarationKind,
}

struct OwnerInspection<'a> {
    evaluation: &'a ProjectEvaluationRequest<'a>,
    domain: &'a str,
    level: &'a str,
    min_prefixes: u32,
}

struct ContainerInspection<'a> {
    evaluation: &'a ProjectEvaluationRequest<'a>,
    root: &'a str,
    max_depth: u32,
    min_prefixes: u32,
}

impl OwnerNode {
    fn with_path(mut self, owners: &[String], path: &str) -> Self {
        if let Some((first, rest)) = owners.split_first() {
            let child = self.children.remove(first).unwrap_or_default();
            self.children
                .insert(first.clone(), child.with_path(rest, path));
        } else {
            self.direct_paths.push(path.to_owned());
        }
        self
    }
}

pub(crate) fn folder_layer_details(
    model_name: &str,
    relative_path: &str,
    layer: &str,
) -> Option<(String, String)> {
    let expected: &[&str] = match layer {
        "stg" | "stg_v" => &["staging"],
        "int_clean" => &["intermediate", "clean"],
        "int_enriched" => &["intermediate", "enriched"],
        "int_v" => &["intermediate"],
        _ => &["mart"],
    };
    let path: Vec<&str> = relative_path.split('/').collect();
    let parent = &path[..path.len().saturating_sub(1)];
    if parent
        .windows(expected.len())
        .any(|window| window == expected)
    {
        return None;
    }
    let layer_position = parent
        .iter()
        .position(|component| expected.contains(component))
        .unwrap_or(parent.len());
    let subjects = parent[layer_position..]
        .iter()
        .copied()
        .filter(|component| !expected.contains(component));
    let mut suggested: Vec<&str> = parent[..layer_position].to_vec();
    suggested.extend(expected.iter().copied());
    suggested.extend(subjects);
    suggested.extend(path.last().copied());
    let expected_path = expected.join("/");
    Some((
        format!("{layer} model {model_name:?} must keep layer folder {expected_path:?} contiguous"),
        format!(
            "Move the model to {:?}; preserve subject folders outside {expected_path:?}. Use rules.layout with SQBRPROJECT201-204 to enforce domain and subdomain ordering.",
            suggested.join("/"),
        ),
    ))
}

pub(crate) fn evaluate_project(evaluation: &ProjectEvaluationRequest<'_>) -> Vec<Fault> {
    let mut faults: Vec<Fault> = evaluate_models(evaluation);
    faults.extend(evaluate_containers(evaluation));
    faults
}

fn evaluate_models(evaluation: &ProjectEvaluationRequest<'_>) -> Vec<Fault> {
    let mut faults: Vec<Fault> = Vec::new();
    let levels: Vec<Vec<&str>> = evaluation
        .request
        .config
        .layout
        .levels
        .iter()
        .map(|level| level.split('/').collect())
        .collect();
    let domain_roots: Vec<Vec<&str>> = evaluation
        .request
        .config
        .layout
        .domain_roots
        .iter()
        .map(|root| root.split('/').collect())
        .collect();
    let max_depth = threshold(
        evaluation,
        "max_subdomain_depth",
        DEFAULT_MAX_SUBDOMAIN_DEPTH,
    );
    let min_prefixes = threshold(
        evaluation,
        "min_shared_owner_prefix_directories",
        DEFAULT_MIN_SHARED_PREFIXES,
    );
    let mut groups: BTreeMap<(String, String), OwnerNode> = BTreeMap::new();
    let mut domain_tree = OwnerNode::default();

    for model in &evaluation.request.models {
        let resolved = match resolve_model_path(model, &levels, &domain_roots) {
            ModelPathResolution::Resolved(resolved) => resolved,
            ModelPathResolution::Missing => {
                if let Some(rule) = evaluation.selected.get("SQBRPROJECT201") {
                    faults.push(path_fault(
                        rule,
                        &model.relative_path,
                        "model does not resolve beneath a configured domain root and level".into(),
                        format!(
                            "Move this model beneath one of the configured levels: {}.",
                            evaluation.request.config.layout.levels.join(", ")
                        ),
                    ));
                }
                continue;
            }
            ModelPathResolution::Ambiguous(candidates) => {
                if let Some(rule) = evaluation.selected.get("SQBRPROJECT201") {
                    faults.push(path_fault(
                        rule,
                        &model.relative_path,
                        format!(
                            "model has ambiguous domain-root and level candidates: {}",
                            candidates.join(", ")
                        ),
                        setting_help(
                            "List the intended normalized domain roots",
                            "rules.layout",
                            "domain_roots",
                            r#"["<domain root>"]"#,
                        ),
                    ));
                }
                continue;
            }
        };
        if let Some(rule) = evaluation.selected.get("SQBRPROJECT203")
            && resolved.owners.len() as u32 > max_depth
        {
            faults.push(path_fault(
                rule,
                &model.relative_path,
                format!(
                    "domain {:?} level {:?} has ownership depth {}; configured maximum is {}",
                    resolved.domain,
                    resolved.level,
                    resolved.owners.len(),
                    max_depth
                ),
                format!(
                    "Flatten this path beneath {}/{}, or promote part of it into the domain root. {}",
                    resolved.domain,
                    resolved.level,
                    setting_help(
                        &format!("To allow this depth (the current value is {max_depth})"),
                        "rules.thresholds",
                        "max_subdomain_depth",
                        &resolved.owners.len().to_string(),
                    )
                ),
            ));
        }
        let domain_parts: Vec<String> = resolved.domain.split('/').map(str::to_owned).collect();
        domain_tree = domain_tree.with_path(&domain_parts, &model.relative_path);
        let key = (resolved.domain, resolved.level);
        let root = groups.remove(&key).unwrap_or_default();
        groups.insert(key, root.with_path(&resolved.owners, &model.relative_path));
    }

    for ((domain, level), root) in &groups {
        faults.extend(inspect_owner(
            &OwnerInspection {
                evaluation,
                domain,
                level,
                min_prefixes,
            },
            root,
            &[],
        ));
    }
    if min_prefixes > 0
        && let Some(rule) = evaluation.selected.get("SQBRPROJECT204")
    {
        faults.extend(inspect_prefix_owners(
            rule,
            &domain_tree,
            min_prefixes,
            "domain directories",
        ));
    }
    faults
}

fn resolve_model_path(
    model: &Model,
    levels: &[Vec<&str>],
    domain_roots: &[Vec<&str>],
) -> ModelPathResolution {
    let Some(below_sql) = model
        .relative_path
        .strip_prefix(sqlbuild_core::constants::SQL_DIRECTORY)
        .and_then(|rest| rest.strip_prefix('/'))
    else {
        return ModelPathResolution::Missing;
    };
    let parts: Vec<&str> = below_sql.split('/').collect();
    if parts.len() < MIN_MODEL_PATH_COMPONENTS || parts.first().copied() != Some("models") {
        return ModelPathResolution::Missing;
    }
    let parent = &parts[..parts.len() - 1];
    let mut candidates: BTreeMap<(String, String, Vec<String>), ResolvedModelPath> =
        BTreeMap::new();
    if domain_roots.is_empty() {
        for start in 2..parent.len() {
            for level in levels {
                if parent[start..].starts_with(level) {
                    candidates = add_model_path_candidate(candidates, parent, start, level);
                }
            }
        }
    } else {
        for root in domain_roots {
            if parent[1..].starts_with(root) {
                let start = 1 + root.len();
                for level in levels {
                    if parent[start..].starts_with(level) {
                        candidates = add_model_path_candidate(candidates, parent, start, level);
                    }
                }
            }
        }
    }
    if candidates.is_empty() {
        return ModelPathResolution::Missing;
    }
    if candidates.len() == 1 {
        match candidates.pop_first() {
            Some((_key, candidate)) => return ModelPathResolution::Resolved(candidate),
            None => return ModelPathResolution::Missing,
        }
    }
    ModelPathResolution::Ambiguous(
        candidates
            .keys()
            .map(|(domain, level, _)| format!("{domain} at {level}"))
            .collect(),
    )
}

fn add_model_path_candidate(
    mut candidates: BTreeMap<(String, String, Vec<String>), ResolvedModelPath>,
    parent: &[&str],
    start: usize,
    level: &[&str],
) -> BTreeMap<(String, String, Vec<String>), ResolvedModelPath> {
    let domain = parent[1..start].join("/");
    if domain.is_empty() {
        return candidates;
    }
    let level_name = level.join("/");
    let owners: Vec<String> = parent[start + level.len()..]
        .iter()
        .map(|part| (*part).to_owned())
        .collect();
    candidates.insert(
        (domain.clone(), level_name.clone(), owners.clone()),
        ResolvedModelPath {
            domain,
            level: level_name,
            owners,
        },
    );
    candidates
}

fn inspect_owner(
    inspection: &OwnerInspection<'_>,
    node: &OwnerNode,
    owners: &[String],
) -> Vec<Fault> {
    let mut faults: Vec<Fault> = Vec::new();
    if let Some(rule) = inspection.evaluation.selected.get("SQBRPROJECT202")
        && !node.direct_paths.is_empty()
        && !node.children.is_empty()
    {
        let direct = &node.direct_paths[0];
        let child = node
            .children
            .values()
            .find_map(representative_path)
            .unwrap_or("unknown child model");
        let owner = if owners.is_empty() {
            format!("{}/{}", inspection.domain, inspection.level)
        } else {
            format!(
                "{}/{}/{}",
                inspection.domain,
                inspection.level,
                owners.join("/")
            )
        };
        faults.push(path_fault(
            rule,
            direct,
            format!(
                "owner {owner:?} mixes direct model {direct:?} with child-owned model {child:?}"
            ),
            "Keep this owner as a leaf and flatten its child models, or move every direct model into a meaningfully named child owner.".into(),
        ));
    }
    if inspection.min_prefixes > 0
        && let Some(rule) = inspection.evaluation.selected.get("SQBRPROJECT204")
    {
        let mut siblings: Vec<(String, String)> = Vec::new();
        for (name, child) in &node.children {
            if let Some(path) = representative_path(child) {
                siblings.push((name.clone(), path.to_owned()));
            }
        }
        faults.extend(prefix_faults(
            rule,
            &siblings,
            (inspection.min_prefixes, "owner directories"),
            shared_prefix_remedy,
        ));
    }
    for (name, child) in &node.children {
        let mut child_owners = owners.to_vec();
        child_owners.push(name.clone());
        faults.extend(inspect_owner(inspection, child, &child_owners));
    }
    faults
}

fn representative_path(node: &OwnerNode) -> Option<&str> {
    if let Some(path) = node.direct_paths.first() {
        return Some(path);
    }
    node.children.values().find_map(representative_path)
}

fn inspect_prefix_owners(
    rule: &RuleMetadata,
    node: &OwnerNode,
    minimum: u32,
    label: &str,
) -> Vec<Fault> {
    let mut siblings: Vec<(String, String)> = Vec::new();
    let mut faults: Vec<Fault> = Vec::new();
    for (name, child) in &node.children {
        if let Some(path) = representative_path(child) {
            siblings.push((name.clone(), path.to_owned()));
        }
        faults.extend(inspect_prefix_owners(rule, child, minimum, label));
    }
    faults.extend(prefix_faults(
        rule,
        &siblings,
        (minimum, label),
        shared_prefix_remedy,
    ));
    faults
}

fn evaluate_containers(evaluation: &ProjectEvaluationRequest<'_>) -> Vec<Fault> {
    let mut faults: Vec<Fault> = Vec::new();
    let max_depth = threshold(
        evaluation,
        "max_role_container_depth",
        DEFAULT_MAX_ROLE_CONTAINER_DEPTH,
    );
    let min_prefixes = threshold(
        evaluation,
        "min_shared_container_prefix_files",
        DEFAULT_MIN_SHARED_PREFIXES,
    );
    let mut containers: BTreeMap<String, Vec<ContainerEntry>> = BTreeMap::new();
    for declaration in &evaluation.request.scope_index.declarations {
        if matches!(declaration.scope, ScopeKind::Private) {
            continue;
        }
        if let Some((root, entry)) = container_entry(declaration) {
            containers.entry(root).or_default().push(entry);
        }
    }
    for (root, entries) in containers {
        faults.extend(inspect_container(
            &ContainerInspection {
                evaluation,
                root: &root,
                max_depth,
                min_prefixes,
            },
            &entries,
        ));
    }
    faults
}

fn container_entry(declaration: &ScopeDeclaration) -> Option<(String, ContainerEntry)> {
    let root = declaration.role_root.clone()?;
    let role = declaration.role.as_deref()?;
    if !RESERVED_BUCKET_NAMES.contains(&role) {
        return None;
    }
    let buckets: Vec<String> = declaration
        .bucket_path
        .as_deref()
        .map(|path| path.split('/').map(str::to_owned).collect())
        .unwrap_or_default();
    let file = declaration.path.rsplit('/').next()?;
    let stem = file
        .rsplit_once('.')
        .map_or_else(|| file.to_owned(), |(value, _)| value.into());
    let kind_folder: Option<&str> = matches!(declaration.kind, DeclarationKind::Macro)
        .then_some(())
        .and(declaration.path.strip_prefix(&format!("{root}/")))
        .and_then(|rest| rest.split_once('/'))
        .map(|(folder, _)| folder)
        .filter(|folder| MACRO_KIND_FOLDERS.contains(folder));
    let root: String = match kind_folder {
        Some(folder) => format!("{root}/{folder}"),
        None => root,
    };
    Some((
        root,
        ContainerEntry {
            path: declaration.path.clone(),
            buckets,
            stem,
            kind: declaration.kind.clone(),
        },
    ))
}

fn inspect_container(
    inspection: &ContainerInspection<'_>,
    entries: &[ContainerEntry],
) -> Vec<Fault> {
    let mut faults: Vec<Fault> = Vec::new();
    let mut seen_paths: BTreeSet<&str> = BTreeSet::new();
    let files: Vec<&ContainerEntry> = entries
        .iter()
        .filter(|entry| seen_paths.insert(entry.path.as_str()))
        .collect();
    let direct: Vec<&ContainerEntry> = files
        .iter()
        .copied()
        .filter(|entry| entry.buckets.is_empty())
        .collect();
    let grouped: Vec<&ContainerEntry> = files
        .iter()
        .copied()
        .filter(|entry| !entry.buckets.is_empty())
        .collect();
    if let Some(rule) = inspection.evaluation.selected.get("SQBRDECLARATION302")
        && !direct.is_empty()
        && !grouped.is_empty()
    {
        faults.push(path_fault(
            rule,
            &direct[0].path,
            format!(
                "declaration role {:?} mixes direct file {:?} with grouped file {:?}",
                inspection.root,
                direct[0].path, grouped[0].path
            ),
            "Keep every declaration file directly in this role, or move every file into one level of meaningful concern buckets.".into(),
        ));
    }
    if let Some(rule) = inspection.evaluation.selected.get("SQBRDECLARATION303") {
        for entry in files
            .iter()
            .copied()
            .filter(|entry| entry.buckets.len() as u32 > inspection.max_depth)
        {
            faults.push(path_fault(
                rule,
                &entry.path,
                format!(
                    "declaration role {:?} has bucket depth {}; configured maximum is {}",
                    inspection.root,
                    entry.buckets.len(),
                    inspection.max_depth
                ),
                format!(
                    "Flatten this bucket path. {}",
                    setting_help(
                        &format!(
                            "To allow this depth (the current value is {})",
                            inspection.max_depth
                        ),
                        "rules.thresholds",
                        "max_role_container_depth",
                        &entry.buckets.len().to_string(),
                    )
                ),
            ));
        }
    }
    if let Some(rule) = inspection.evaluation.selected.get("SQBRDECLARATION305") {
        let mut reported: BTreeSet<(String, String)> = BTreeSet::new();
        for entry in files.iter().copied() {
            for bucket in &entry.buckets {
                if (GENERIC_BUCKET_NAMES.contains(&bucket.as_str())
                    || RESERVED_BUCKET_NAMES.contains(&bucket.as_str()))
                    && reported.insert((entry.path.clone(), bucket.clone()))
                {
                    faults.push(path_fault(
                        rule,
                        &entry.path,
                        format!("declaration bucket {bucket:?} does not name a specific concern"),
                        "Rename this bucket after the concrete concern it contains.".into(),
                    ));
                }
            }
        }
    }
    if let Some(rule) = inspection.evaluation.selected.get("SQBRDECLARATION304") {
        let mut buckets: BTreeMap<String, Vec<&ContainerEntry>> = BTreeMap::new();
        for entry in files.iter().copied() {
            let key = entry.buckets.first().cloned().unwrap_or_default();
            buckets.entry(key).or_default().push(entry);
        }
        for (bucket, files) in buckets {
            let limit = container_limit(inspection.evaluation, &files[0].kind);
            if files.len() as u32 > limit {
                let label = if bucket.is_empty() {
                    inspection.root.to_owned()
                } else {
                    format!("{}/{bucket}", inspection.root)
                };
                faults.push(path_fault(
                    rule,
                    &files[0].path,
                    format!(
                        "declaration container {label:?} has {} files; configured maximum is {limit}",
                        files.len()
                    ),
                    "Group these files by a meaningful concern or increase the declaration-kind container threshold explicitly.".into(),
                ));
            }
        }
    }
    if inspection.min_prefixes > 0
        && let Some(rule) = inspection.evaluation.selected.get("SQBRDECLARATION306")
    {
        let mut locations: BTreeMap<String, Vec<(String, String)>> = BTreeMap::new();
        for entry in files.iter().copied() {
            let location = entry.buckets.join("/");
            locations
                .entry(location)
                .or_default()
                .push((entry.stem.clone(), entry.path.clone()));
        }
        let sql_macros: bool = inspection
            .root
            .rsplit('/')
            .next()
            .is_some_and(|folder| folder == SQL_MACRO_KIND_FOLDER)
            && files
                .iter()
                .all(|entry| matches!(entry.kind, DeclarationKind::Macro));
        for (location, siblings) in &locations {
            let folder: String = if location.is_empty() {
                inspection.root.to_owned()
            } else {
                format!("{}/{location}", inspection.root)
            };
            let label: &str = if sql_macros {
                "SQL macro files"
            } else {
                "declaration files"
            };
            let remedy = |prefix: &str| {
                if sql_macros {
                    format!(
                        "Move these files into {folder}/{prefix}/; a SQL macro is named by its file, so each macro keeps its name and its callers stay unchanged."
                    )
                } else {
                    shared_prefix_remedy(prefix)
                }
            };
            faults.extend(prefix_faults(
                rule,
                siblings,
                (inspection.min_prefixes, label),
                remedy,
            ));
        }
    }
    faults
}

fn container_limit(evaluation: &ProjectEvaluationRequest<'_>, kind: &DeclarationKind) -> u32 {
    let name = match kind {
        DeclarationKind::Macro => "max_macro_container_files",
        DeclarationKind::Constant => "max_constant_container_files",
        DeclarationKind::Enum => "max_enum_container_files",
        DeclarationKind::Audit
        | DeclarationKind::SingularAudit
        | DeclarationKind::Schema
        | DeclarationKind::SqlHook
        | DeclarationKind::PythonHook => return DEFAULT_MAX_CONTAINER_FILES,
    };
    threshold(evaluation, name, DEFAULT_MAX_CONTAINER_FILES)
}

/// The remedy for siblings that share a token prefix.
fn shared_prefix_remedy(prefix: &str) -> String {
    format!(
        "Consolidate these siblings beneath {prefix}/, or rename them when the shared token prefix is not a real concern."
    )
}

fn prefix_faults(
    rule: &RuleMetadata,
    siblings: &[(String, String)],
    (minimum, label): (u32, &str),
    remedy: impl Fn(&str) -> String,
) -> Vec<Fault> {
    let mut root = TokenNode::default();
    for (name, path) in siblings {
        let mut node = &mut root;
        for token in name.split('_') {
            node = node.children.entry(token.to_owned()).or_default();
        }
        node.terminals.push((name.clone(), path.clone()));
    }
    let (_, candidates) = collect_prefix_candidates(&root, &[], minimum);
    candidates
        .into_iter()
        .map(|(prefix, path, count)| {
            path_fault(
                rule,
                &path,
                format!("{count} sibling {label} encode the implicit owner or bucket {prefix:?}"),
                remedy(&prefix),
            )
        })
        .collect()
}

fn collect_prefix_candidates(
    node: &TokenNode,
    prefix: &[String],
    minimum: u32,
) -> (Vec<TokenTerminal>, Vec<PrefixCandidate>) {
    let mut descendants = node.terminals.clone();
    let mut candidates: Vec<PrefixCandidate> = Vec::new();
    for (token, child) in &node.children {
        let mut child_prefix = prefix.to_vec();
        child_prefix.push(token.clone());
        let (child_descendants, child_candidates) =
            collect_prefix_candidates(child, &child_prefix, minimum);
        descendants.extend(child_descendants);
        candidates.extend(child_candidates);
    }
    let outcomes = usize::from(!node.terminals.is_empty()) + node.children.len();
    if !prefix.is_empty() && outcomes >= MIN_BRANCH_OUTCOMES && descendants.len() as u32 >= minimum
    {
        candidates.push((
            prefix.join("_"),
            descendants[0].1.clone(),
            descendants.len(),
        ));
    }
    (descendants, candidates)
}

fn threshold(evaluation: &ProjectEvaluationRequest<'_>, name: &str, default: u32) -> u32 {
    evaluation
        .request
        .config
        .thresholds
        .get(name)
        .copied()
        .unwrap_or(default)
}
