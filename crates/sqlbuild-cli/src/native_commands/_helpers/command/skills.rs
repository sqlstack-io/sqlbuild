//! `sqb skills` and the skill freshness check every observed command makes first.

use std::path::{Path, PathBuf};

use sqlbuild_config::workspace::main::install_skills::install_skills;
use sqlbuild_config::workspace::main::maintain_skills::maintain_skills;
use sqlbuild_config::workspace::models::SkillInstallRequest;

use crate::native_commands::constants::{HOME_VARIABLE, SKILL_FILES_HEADER};
use crate::native_commands::errors::PythonNeeded;
use crate::native_commands::models::{CommandOutput, InstalledPackage, SkillsOptions};

/// `Path.home()` from `HOME` (`USERPROFILE` on Windows), unless only Python could tell.
pub(crate) fn home_dir() -> Result<PathBuf, PythonNeeded> {
    std::env::var_os(HOME_VARIABLE)
        .filter(|home| !home.is_empty())
        .map(PathBuf::from)
        .ok_or(PythonNeeded::Installation)
}

/// The packaged skill files installed into `project_dir`; Python reports any failure.
pub(crate) fn run_skills(
    project_dir: &Path,
    package: &InstalledPackage,
    options: &SkillsOptions,
) -> Result<CommandOutput, PythonNeeded> {
    let written: Vec<PathBuf> = install_skills(&SkillInstallRequest {
        project_dir: project_dir.to_path_buf(),
        skills_root: package.skills_root.clone(),
        home_dir: home_dir()?,
        global_install: options.global_install,
        requested_targets: options.targets.clone(),
        force: options.force,
    })
    .map_err(|_| PythonNeeded::Failure)?;
    let mut stdout: String = SKILL_FILES_HEADER.to_owned();
    for path in written {
        stdout.push_str(&format!("  {}\n", path.display()));
    }
    Ok(CommandOutput {
        stdout,
        ..CommandOutput::default()
    })
}

/// The stale-skill notice, refreshing generated skills; failures stay silent, as in Python.
pub(crate) fn skill_freshness_notice(project_dir: &Path, package: &InstalledPackage) -> String {
    maintain_skills(project_dir, &package.skills_root).unwrap_or_default()
}
