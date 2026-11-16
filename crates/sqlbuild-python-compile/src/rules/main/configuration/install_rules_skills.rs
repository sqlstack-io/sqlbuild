//! Install every owned rules skill target, or with `check` report whether all are fresh.

use crate::rules::_helpers::configuration::guidance::FILELOCK_MODULE;
use crate::rules::_helpers::configuration::guidance::FRESH_SKILL;
use crate::rules::_helpers::configuration::guidance::SKILL_PATHS;
use crate::rules::_helpers::configuration::guidance::current_content;
use crate::rules::_helpers::configuration::guidance::install_error;
use crate::rules::_helpers::configuration::guidance::lock_path;
use crate::rules::_helpers::configuration::guidance::render_skills;
use crate::rules::_helpers::configuration::skills::skill_state;
use pyo3::prelude::{Bound, PyAny, PyAnyMethods, PyResult, Python};
use pyo3::pyfunction;
use sqlbuild_python_base::boundary::main::compiler_guard::compiler_guard;
use sqlbuild_python_base::boundary::main::imported::imported;
use sqlbuild_rules::rule_catalogue::main::install_rendered_skills::install_rendered_skills;
use sqlbuild_rules::rule_catalogue::models::SkillTarget;
use std::path::PathBuf;

/// Install every owned rules skill target, or with `check` report whether all are fresh.
#[pyfunction]
#[pyo3(signature = (*, config, project_dir, check))]
pub fn install_rules_skills(
    config: &Bound<'_, PyAny>,
    project_dir: &Bound<'_, PyAny>,
    check: bool,
) -> PyResult<bool> {
    let py: Python<'_> = config.py();
    let (content, fingerprint): (String, String) = render_skills(config, project_dir)?;
    let root: PathBuf = project_dir.extract()?;
    let targets = || -> PyResult<Vec<SkillTarget>> {
        let mut targets: Vec<SkillTarget> = Vec::with_capacity(SKILL_PATHS.len());
        for relative in SKILL_PATHS {
            let path: PathBuf = root.join(relative);
            let state: String = skill_state(current_content(&path)?.as_deref(), &fingerprint);
            targets.push(SkillTarget { path, state });
        }
        Ok(targets)
    };
    if check {
        return Ok(targets()?.iter().all(|target| target.state == FRESH_SKILL));
    }
    let lock: Bound<'_, PyAny> =
        imported(py, FILELOCK_MODULE, "FileLock")?.call1((lock_path(&root)?,))?;
    let _ = lock.call_method0("acquire")?;
    let installed = (|| -> PyResult<()> {
        let targets: Vec<SkillTarget> = targets()?;
        compiler_guard(|| Ok(install_rendered_skills(&content, &targets)))?
            .map_err(|error| install_error(py, error))
    })();
    let released = lock.call_method0("release");
    installed?;
    let _ = released?;
    Ok(true)
}
