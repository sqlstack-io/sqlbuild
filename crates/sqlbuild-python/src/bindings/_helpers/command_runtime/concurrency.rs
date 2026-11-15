//! The worker limit of one command: `--concurrency`, the target, then `[settings]`.

use pyo3::prelude::{Bound, PyAny, PyAnyMethods, PyResult};
use sqlbuild_config::effective::main::effective_concurrency::effective_concurrency;
use std::path::Path;

use pyo3::IntoPyObject;
use pyo3::exceptions::PyValueError;
use sqlbuild_config::effective::models::{
    ConcurrencyLayers, ConcurrencySource as Layer, EffectiveConcurrency,
};
use sqlbuild_config::project::main::load_project_setting_keys::load_project_setting_keys;
use sqlbuild_config::project::main::project_config_paths::project_config_paths;
use sqlbuild_orchestration::declarations::models::{ConcurrencySource, ResolvedConcurrency};

use crate::bindings::_helpers::compile_attachment::effective_config::build_effective_settings;
use crate::bindings::_helpers::contracts::spec_resolution::{
    resolve_target_config, resolve_target_name,
};
use sqlbuild_python_base::records::main::fields::field::field as native_field;

const CONCURRENCY: &str = "concurrency";

/// The effective concurrency of a command on `selected_target`, and the layer that set it.
pub(crate) fn resolve_concurrency(
    requested: &Bound<'_, PyAny>,
    discovered: &Bound<'_, PyAny>,
    selected_target: &Bound<'_, PyAny>,
) -> PyResult<EffectiveConcurrency> {
    resolve_configured_concurrency(
        requested,
        (
            &native_field(discovered, "project_config")?,
            &native_field(discovered, "local_config")?,
        ),
        selected_target,
    )
}

/// `resolve_concurrency` from the project and local configs themselves.
pub(crate) fn resolve_configured_concurrency(
    requested: &Bound<'_, PyAny>,
    (project_config, local_config): (&Bound<'_, PyAny>, &Bound<'_, PyAny>),
    selected_target: &Bound<'_, PyAny>,
) -> PyResult<EffectiveConcurrency> {
    let py = project_config.py();
    let target: Option<(String, i64)> =
        match resolve_target_name(py, project_config, local_config, selected_target)? {
            Some(name) => {
                let config: Bound<'_, PyAny> =
                    resolve_target_config(py, project_config, local_config, name.as_any())?;
                let limit: Option<i64> = native_field(&config, CONCURRENCY)?.extract()?;
                limit.map(|limit| Ok::<_, pyo3::PyErr>((name.extract::<String>()?, limit)))
            }
            None => None,
        }
        .transpose()?;
    let settings: i64 = native_field(
        &build_effective_settings(project_config, local_config)?,
        CONCURRENCY,
    )?
    .extract()?;
    let local_override: bool =
        native_field(local_config, "setting_overrides")?.contains(CONCURRENCY)?;
    Ok(effective_concurrency(&ConcurrencyLayers {
        requested: requested.extract()?,
        target: target.as_ref().map(|(name, limit)| (name.as_str(), *limit)),
        settings,
        local_override,
    }))
}

/// The concurrency a published job without its own setting runs with on the compiled `target`, and its layer.
pub(crate) fn published_concurrency(
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
