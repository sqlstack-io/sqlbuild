//! The files of each collection in Python's discovery order, read and parsed in parallel.

use crate::_helpers::reading::{read_authored_text, undecodable_path_failure};
use crate::_helpers::scoped_paths::is_in_scoped_declaration_tree;
use crate::declaration_files::_helpers::checks::stops::ParseStop;
use crate::declaration_files::_helpers::listing::collection_reads::CollectionSource;
use crate::declaration_files::_helpers::listing::layouts::session_layout;
use crate::declaration_files::models::{CollectionFailure, FileScope, ScopedFile};
use crate::declarations::models::{
    DeclarationFileFact, DeclarationGroup, DeclarationKind, DeclarationLayout,
};
use crate::models::{DiscoveredFile, FileOutcome, ProjectRoot, StageFailure};
use crate::tree::main::path_order::compare_posix_text;
use crate::tree::main::rglob::rglob;
use crate::tree::models::{ProjectTree, TreeEntry};
use rayon::iter::{IntoParallelIterator, ParallelIterator};
use std::sync::Arc;

/// One file to read, with the scope facts of the role that holds it.
pub(crate) type ListedFile = (String, Option<FileScope>);

/// A named declaration role: `GLOBAL_NAMED_DECLARATION_ROOTS` and the grouped roles.
#[derive(Clone, Copy, PartialEq, Eq)]
pub(crate) enum NamedKind {
    Audit,
    SingularAudit,
    Schema,
    SqlHook,
    PythonHook,
}

impl NamedKind {
    fn as_str(self) -> &'static str {
        match self {
            Self::Audit => "audit",
            Self::SingularAudit => "singular_audit",
            Self::Schema => "schema",
            Self::SqlHook => "sql_hook",
            Self::PythonHook => "python_hook",
        }
    }

    /// Whether `name` matches the kind's file glob: `*.py` for Python hooks, `*.sql` otherwise.
    fn matches(self, name: &str) -> bool {
        match self {
            Self::PythonHook => matches_python_glob(name),
            _ => matches_sql_glob(name),
        }
    }
}

/// `GLOBAL_NAMED_DECLARATION_ROOTS`, in Python's dict order.
const GLOBAL_NAMED_ROOTS: [(&str, NamedKind); 5] = [
    ("sql/audits/generic", NamedKind::Audit),
    ("sql/audits/singular", NamedKind::SingularAudit),
    ("sql/schemas", NamedKind::Schema),
    ("sql/hooks/sql", NamedKind::SqlHook),
    ("sql/hooks/python", NamedKind::PythonHook),
];

/// `GROUPED_NAMED_DECLARATION_ROLES`, in Python's dict order.
const GROUPED_NAMED_ROLES: [(&str, NamedKind, &str); 9] = [
    ("audits/generic", NamedKind::Audit, "inherited"),
    ("_audits/generic", NamedKind::Audit, "local"),
    ("audits/singular", NamedKind::SingularAudit, "inherited"),
    ("schemas", NamedKind::Schema, "inherited"),
    ("_schemas", NamedKind::Schema, "local"),
    ("hooks/sql", NamedKind::SqlHook, "inherited"),
    ("_hooks/sql", NamedKind::SqlHook, "local"),
    ("hooks/python", NamedKind::PythonHook, "inherited"),
    ("_hooks/python", NamedKind::PythonHook, "local"),
];

const FUNCTIONS_ROOT: &str = "sql/functions/sql";
const SEEDS_ROOT: &str = sqlbuild_core::constants::SEEDS_PATH;
const SEED_FILE_SUFFIX: &str = ".csv";

/// The macro, enum or constant files of `kind`, from the whole layout or `kind`'s alone.
pub(crate) fn declaration_files(
    source: &CollectionSource<'_>,
    kind: DeclarationKind,
    isolate_kind: bool,
) -> Result<Vec<ListedFile>, CollectionFailure> {
    let layout: Arc<Result<DeclarationLayout, StageFailure>> =
        session_layout(source.session, source.tree, isolate_kind.then_some(kind));
    let layout: &DeclarationLayout = layout
        .as_ref()
        .as_ref()
        .map_err(|failure| CollectionFailure::Stage(failure.clone()))?;
    let facts: &[DeclarationFileFact] = layout
        .file_facts
        .as_deref()
        .map_err(|failure| CollectionFailure::Layout(failure.clone()))?;
    Ok(facts
        .iter()
        .filter(|fact| fact.kind == kind)
        .map(|fact| {
            let scope = FileScope {
                declaration_kind: fact.kind.as_str(),
                scope_kind: fact.scope_kind.as_str(),
                ownership_root: Some(fact.ownership_root.clone()),
                owning_path: fact.owning_path.clone(),
                declaration_root: Some(fact.declaration_root.clone()),
            };
            (fact.relative_path.clone(), Some(scope))
        })
        .collect())
}

/// Every file below the named roles of `kinds` matching its kind's glob, root by root as Python yields them.
pub(crate) fn named_declaration_files(
    source: &CollectionSource<'_>,
    kinds: &[NamedKind],
    skip_underscored: bool,
) -> Result<Vec<ListedFile>, CollectionFailure> {
    let tree: &ProjectTree = source.tree;
    let mut roots: Vec<(String, NamedKind, FileScope)> = Vec::new();
    for (role, kind) in GLOBAL_NAMED_ROOTS {
        if kinds.contains(&kind) && tree.is_dir(role) {
            let scope = named_scope(&NamedRole {
                kind,
                scope_kind: "global",
                ownership_root: None,
                owning_path: None,
                declaration_root: role,
            });
            roots.push((role.to_owned(), kind, scope));
        }
    }
    let layout: Arc<Result<DeclarationLayout, StageFailure>> =
        session_layout(source.session, tree, None);
    let layout: &DeclarationLayout = layout
        .as_ref()
        .as_ref()
        .map_err(|failure| CollectionFailure::Stage(failure.clone()))?;
    let groups: &[DeclarationGroup] = layout
        .named_groups
        .as_deref()
        .map_err(|failure| CollectionFailure::Layout(failure.clone()))?;
    for group in groups {
        let owning_path: &str = group
            .directory
            .rsplit_once('/')
            .map_or("", |(parent, _)| parent);
        for (role, kind, scope_kind) in GROUPED_NAMED_ROLES {
            let directory: String = format!("{}/{role}", group.directory);
            if kinds.contains(&kind) && tree.is_dir(&directory) {
                let scope = named_scope(&NamedRole {
                    kind,
                    scope_kind,
                    ownership_root: Some(&group.root),
                    owning_path: Some(owning_path),
                    declaration_root: &directory,
                });
                roots.push((directory, kind, scope));
            }
        }
    }
    roots.sort_by(|left, right| compare_posix_text(&left.0, &right.0));
    let mut files: Vec<ListedFile> = Vec::new();
    for (directory, kind, scope) in roots {
        for relative_path in rglob(tree, &directory, |entry| kind.matches(&entry.name))
            .map_err(CollectionFailure::Stage)?
        {
            if skip_underscored && file_name(&relative_path).starts_with('_') {
                continue;
            }
            files.push((relative_path, Some(scope.clone())));
        }
    }
    Ok(files)
}

/// One named declaration role directory and who owns it.
struct NamedRole<'a> {
    kind: NamedKind,
    scope_kind: &'static str,
    ownership_root: Option<&'a str>,
    owning_path: Option<&'a str>,
    declaration_root: &'a str,
}

fn named_scope(role: &NamedRole<'_>) -> FileScope {
    FileScope {
        declaration_kind: role.kind.as_str(),
        scope_kind: role.scope_kind,
        ownership_root: role.ownership_root.map(str::to_owned),
        owning_path: role.owning_path.map(str::to_owned),
        declaration_root: Some(role.declaration_root.to_owned()),
    }
}

/// The unscoped `*.sql` files below `functions/sql/`.
pub(crate) fn function_files(tree: &ProjectTree) -> Result<Vec<ListedFile>, CollectionFailure> {
    if !tree.is_dir(FUNCTIONS_ROOT) {
        return Ok(Vec::new());
    }
    Ok(
        rglob(tree, FUNCTIONS_ROOT, |entry| matches_sql_glob(&entry.name))
            .map_err(CollectionFailure::Stage)?
            .into_iter()
            .filter(|relative_path| !is_in_scoped_declaration_tree(relative_path))
            .map(|relative_path| (relative_path, None))
            .collect(),
    )
}

/// The regular `*.csv` files below `seeds/`, following links as `Path.is_file()` does.
pub(crate) fn seed_files(tree: &ProjectTree) -> Result<Vec<String>, CollectionFailure> {
    if !tree.is_dir(SEEDS_ROOT) {
        return Ok(Vec::new());
    }
    let paths: Vec<String> = rglob(tree, SEEDS_ROOT, |entry: &TreeEntry| {
        has_seed_suffix(&entry.name)
    })
    .map_err(CollectionFailure::Stage)?;
    Ok(paths
        .into_par_iter()
        .filter(|relative_path| {
            std::fs::metadata(tree.absolute(relative_path)).is_ok_and(|metadata| metadata.is_file())
        })
        .collect())
}

/// Python's `Path.suffix == ".csv"`.
fn has_seed_suffix(name: &str) -> bool {
    name.len() > SEED_FILE_SUFFIX.len() && name.ends_with(SEED_FILE_SUFFIX)
}

/// `fnmatch("*.sql")` with the platform's case rule: Windows ignores case as `re.IGNORECASE` does.
fn matches_sql_glob(name: &str) -> bool {
    if !cfg!(windows) {
        return name.ends_with(".sql");
    }
    let tail: Vec<char> = name.chars().rev().take(4).collect();
    matches!(
        tail.as_slice(),
        ['l' | 'L', 'q' | 'Q', 's' | 'S' | '\u{17f}', '.']
    )
}

/// `fnmatch("*.py")` with the platform's case rule.
pub(crate) fn matches_python_glob(name: &str) -> bool {
    if !cfg!(windows) {
        return name.ends_with(".py");
    }
    let tail: Vec<char> = name.chars().rev().take(3).collect();
    matches!(tail.as_slice(), ['y' | 'Y', 'p' | 'P', '.'])
}

/// The final component of a `/`-separated relative path.
pub(crate) fn file_name(relative_path: &str) -> &str {
    relative_path
        .rsplit_once('/')
        .map_or(relative_path, |(_, name)| name)
}

/// Read and parse each listed file in parallel; a name that is not UTF-8 defers to Python.
pub(crate) fn read_files<T: Send>(
    root: &ProjectRoot,
    tree: &ProjectTree,
    files: Vec<ListedFile>,
    parse: impl Fn(&str, &str, String) -> Result<T, ParseStop> + Sync,
) -> Vec<ScopedFile<T>> {
    files
        .into_par_iter()
        .map(|(relative_path, scope)| ScopedFile {
            file: DiscoveredFile {
                outcome: read_one(root, tree, &relative_path, &parse),
                relative_path,
            },
            scope,
        })
        .collect()
}

fn read_one<T>(
    root: &ProjectRoot,
    tree: &ProjectTree,
    relative_path: &str,
    parse: &impl Fn(&str, &str, String) -> Result<T, ParseStop>,
) -> FileOutcome<T> {
    if let Some(failure) = undecodable_path_failure(root, tree, relative_path) {
        return FileOutcome::Failed(failure);
    }
    match read_authored_text(&tree.absolute(relative_path)) {
        Ok(contents) => match parse(&root.display_path(relative_path), relative_path, contents) {
            Ok(parsed) => FileOutcome::Parsed(parsed),
            Err(ParseStop::Failed(failure)) => FileOutcome::Failed(failure),
        },
        Err(failure) => FileOutcome::Unreadable(failure),
    }
}
