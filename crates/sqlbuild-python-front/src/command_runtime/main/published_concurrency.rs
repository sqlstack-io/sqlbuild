//! The concurrency a published job without its own setting runs with on the compiled `target`, and its layer.

use std::path::Path;

use pyo3::IntoPyObject;
use pyo3::exceptions::PyValueError;
use pyo3::prelude::{Bound, PyAny, PyResult};
use sqlbuild_config::effective::models::ConcurrencySource as Layer;
use sqlbuild_config::project::main::load_project_setting_keys::load_project_setting_keys;
use sqlbuild_config::project::main::project_config_paths::project_config_paths;
use sqlbuild_orchestration::declarations::models::{ConcurrencySource, ResolvedConcurrency};

use crate::command_runtime::_helpers::concurrency::CONCURRENCY;
use crate::command_runtime::main::resolve_concurrency::resolve_concurrency;

/// The concurrency a published job without its own setting runs with on the compiled `target`, and its layer.
pub fn published_concurrency(
    project_dir: &Path,
    discovered: &Bound<'_, PyAny>,
    target: Option<&str>,
) -> PyResult<ResolvedConcurrency> {
    let py = discovered.py();
    let effective = resolve_concurrency(
        &py.None().into_bound(py),
        discovered,
        target.into_pyobject(py)?.as_any(),
    )?;
    let project_sets = || -> PyResult<bool> {
        let keys = load_project_setting_keys(&project_config_paths(project_dir)).map_err(|_| {
            PyValueError::new_err("cannot read [settings] of sqlbuild_project.toml")
        })?;
        Ok(keys.iter().any(|key| key == CONCURRENCY))
    };
    let source = match effective.source {
        Layer::CommandLine => ConcurrencySource::Job,
        Layer::Target(_) => ConcurrencySource::Target,
        Layer::LocalSettings => ConcurrencySource::Local,
        Layer::ProjectSettings if project_sets()? => ConcurrencySource::Project,
        Layer::ProjectSettings => ConcurrencySource::Default,
    };
    let limit = u64::try_from(effective.limit)
        .map_err(|_| PyValueError::new_err("concurrency must be at least 1"))?;
    Ok(ResolvedConcurrency { limit, source })
}
