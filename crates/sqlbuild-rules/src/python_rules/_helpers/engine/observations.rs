//! Answers to the project questions Fensu's rules ask about the files around the one they check.

use std::collections::HashMap;
use std::path::Path;

use fensu_facts::snapshot::models::{
    RepositoryObservationAnswer, RepositoryObservationIndex, RepositoryObservationQuery,
};
use fensu_native::NativeProjectQuery;

use crate::python_rules::_helpers::engine::placement::package_anchor;
use crate::python_rules::_helpers::engine::programs::Programs;
use crate::python_rules::constants::PROJECT_ROOT_QUERY_PATH;

const TRUE_TEXT: &str = "true";
const FALSE_TEXT: &str = "false";
const MEANINGFUL_RESULT: &str = "meaningful";
const EMPTY_RESULT: &str = "empty";
/// Query kinds answered from one walk of the project tree.
const FILESYSTEM_KINDS: [&str; 6] = [
    "exists",
    "is_file",
    "is_dir",
    "directory_entries",
    "python_anchor",
    "glob",
];

/// One walk of the project tree covering every filesystem query of the run.
pub(crate) fn repository_index(
    repo_root: &Path,
    queries: &[&NativeProjectQuery],
) -> Result<RepositoryObservationIndex, String> {
    let walked: Vec<RepositoryObservationQuery> = queries
        .iter()
        .filter(|query| FILESYSTEM_KINDS.contains(&query.kind.as_str()))
        .map(|query| filesystem_query(query))
        .collect();
    RepositoryObservationIndex::build(repo_root, &walked)
        .ok_or_else(|| format!("{}: cannot read the project folder", repo_root.display()))
}

/// The answers to `queries`, keyed as Fensu's rules look them up.
pub(crate) fn observations(
    queries: &[NativeProjectQuery],
    programs: &Programs<'_>,
    index: &RepositoryObservationIndex,
) -> Result<HashMap<String, Vec<String>>, String> {
    queries
        .iter()
        .map(|query| Ok((query.key(), answer(query, programs, index)?)))
        .collect()
}

fn answer(
    query: &NativeProjectQuery,
    programs: &Programs<'_>,
    index: &RepositoryObservationIndex,
) -> Result<Vec<String>, String> {
    let path: &str = project_path(&query.path);
    Ok(match query.kind.as_str() {
        kind if FILESYSTEM_KINDS.contains(&kind) => {
            match filesystem_query(query)
                .observe(index)
                .map(|state| state.answer)
            {
                Some(RepositoryObservationAnswer::Bool(value)) => vec![bool_text(value)],
                Some(RepositoryObservationAnswer::Paths(paths)) => paths,
                Some(RepositoryObservationAnswer::String(text)) => vec![text],
                Some(RepositoryObservationAnswer::None) | None => {
                    if matches!(kind, "exists" | "is_file" | "is_dir") {
                        vec![bool_text(false)]
                    } else {
                        Vec::new()
                    }
                }
            }
        }
        "module_function" => match programs.module_index(&query.path) {
            Some(module) => programs
                .program(module)?
                .project_rows()
                .0
                .iter()
                .find(|function| function.name == query.argument)
                .map(|function| {
                    if function.meaningful_result {
                        MEANINGFUL_RESULT.to_owned()
                    } else {
                        EMPTY_RESULT.to_owned()
                    }
                })
                .into_iter()
                .collect(),
            None => Vec::new(),
        },
        "public_facade" => vec![bool_text(match programs.index_of(path) {
            Some(module) => programs.program(module)?.is_public_facade(&query.argument),
            None => false,
        })],
        "package_anchor" => vec![bool_text(
            package_anchor(programs.sources, path).is_some_and(|anchor| anchor == query.argument),
        )],
        kind => {
            return Err(format!(
                "Python rules asked an unsupported project question: {kind}"
            ));
        }
    })
}

fn filesystem_query(query: &NativeProjectQuery) -> RepositoryObservationQuery {
    let (pattern, recursive) = match query.argument.split_once('\0') {
        Some((pattern, recursive)) => (Some(pattern.to_owned()), recursive == TRUE_TEXT),
        None => (None, false),
    };
    RepositoryObservationQuery {
        relative_path: project_path(&query.path).to_owned(),
        kind: query.kind.clone(),
        pattern,
        recursive,
    }
}

/// Each of `queries` with its path relative to `project_dir`, so its answer can be read again from the project.
pub(crate) fn relative_queries(
    queries: Vec<NativeProjectQuery>,
    project_dir: &str,
) -> Vec<NativeProjectQuery> {
    queries
        .into_iter()
        .map(|query| relative_query(query, project_dir))
        .collect()
}

/// `query` relative to `project_dir`, both spelled with `/` as Fensu spells query paths, even on Windows.
pub(crate) fn relative_query(query: NativeProjectQuery, project_dir: &str) -> NativeProjectQuery {
    let project_dir: String = project_dir.replace('\\', "/");
    let path: String = match query
        .path
        .replace('\\', "/")
        .strip_prefix(project_dir.as_str())
    {
        Some("") => PROJECT_ROOT_QUERY_PATH.to_owned(),
        Some(rest) if rest.starts_with('/') => rest.trim_start_matches('/').to_owned(),
        _ => query.path,
    };
    NativeProjectQuery { path, ..query }
}

/// A project-relative path; Fensu spells paths below a top-level root with a leading `/`.
fn project_path(path: &str) -> &str {
    path.strip_prefix('/').unwrap_or(path)
}

fn bool_text(value: bool) -> String {
    if value { TRUE_TEXT } else { FALSE_TEXT }.to_owned()
}
