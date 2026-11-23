//! The project's gitignore rules: its own `.gitignore`, those above it up to the repository root,
//! and the repository's `info/exclude`.

use std::path::{Path, PathBuf};

use ignore::Match;
use ignore::gitignore::{Gitignore, GitignoreBuilder};

use crate::project_layout::constants::{GIT_DIRECTORY, GIT_EXCLUDE_FILE, GITIGNORE_FILE};

/// Gitignore matchers, deepest folder first.
pub(crate) struct IgnoreRules {
    matchers: Vec<Gitignore>,
}

impl IgnoreRules {
    /// The rules that apply inside `project_dir`.
    pub(crate) fn read(project_dir: &Path) -> Self {
        let mut matchers: Vec<Gitignore> = Vec::new();
        let mut folder: Option<PathBuf> = Some(project_dir.to_path_buf());
        while let Some(current) = folder {
            let mut builder = GitignoreBuilder::new(&current);
            let file: PathBuf = current.join(GITIGNORE_FILE);
            if file.is_file() {
                let _ = builder.add(&file);
            }
            let git: PathBuf = current.join(GIT_DIRECTORY);
            let repository_root: bool = git.exists();
            if git.is_dir() {
                let exclude: PathBuf = GIT_EXCLUDE_FILE
                    .iter()
                    .fold(git.clone(), |path, part| path.join(part));
                if exclude.is_file() {
                    let _ = builder.add(&exclude);
                }
            }
            if let Ok(matcher) = builder.build()
                && !matcher.is_empty()
            {
                matchers.push(matcher);
            }
            folder = if repository_root {
                None
            } else {
                current.parent().map(Path::to_path_buf)
            };
        }
        Self { matchers }
    }

    /// Whether `path`, or a folder above it, is ignored; a deeper rule file decides first.
    pub(crate) fn is_ignored(&self, path: &Path, is_dir: bool) -> bool {
        for matcher in &self.matchers {
            match matcher.matched_path_or_any_parents(path, is_dir) {
                Match::Ignore(_) => return true,
                Match::Whitelist(_) => return false,
                Match::None => {}
            }
        }
        false
    }
}
