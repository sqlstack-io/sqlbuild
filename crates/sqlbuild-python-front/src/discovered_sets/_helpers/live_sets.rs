//! Each discover function's set for one compile: placeholders, saved sets, or a live call.

use chrono::Utc;
use pyo3::prelude::{Bound, PyAny, PyAnyMethods, PyResult, Python};
use pyo3::types::PyTuple;
use sqlbuild_orchestration::discovered_sets::main::never_discovered::never_discovered;
use sqlbuild_orchestration::discovered_sets::main::record_attempt::record_attempt;
use sqlbuild_orchestration::discovered_sets::main::remote_discovered_sets::remote_discovered_sets;
use sqlbuild_orchestration::discovered_sets::models::{
    DiscoveredResult, DiscoveredSet, DiscoveredSetStatus,
};
use sqlbuild_orchestration::discovered_sets::types::DiscoveredSets;
use sqlbuild_python_base::records::main::construct::new_plain_error::new_plain_error;
use sqlbuild_python_base::records::main::fields::field::field as native_field;
use sqlbuild_python_base::records::types::ErrorId;

use crate::discovered_sets::_helpers::project_nodes::{missing_node, usable_set};
use crate::discovered_sets::_helpers::state::RUNNER;
use crate::discovered_sets::models::{DiscoverySource, LiveDiscovery, Rediscover, SeedSource};

const ERROR: ErrorId = ErrorId::PythonNodeDiscovery;
/// The location credential a location reads its current snapshot with.
const LOCATION_TOKEN_ENV: &str = "SQLBUILD_LOCATION_TOKEN";

/// Every discover function's set under `source`; a repeated name, or a failure outside a location, raises.
pub(crate) fn resolved_sets(
    inputs: &Bound<'_, PyAny>,
    source: &DiscoverySource,
) -> PyResult<DiscoveredSets> {
    let py: Python<'_> = inputs.py();
    let mut sets: DiscoveredSets = DiscoveredSets::new();
    for record in native_field(inputs, "discover_functions")?.try_iter()? {
        let record: Bound<'_, PyAny> = record?;
        let name: String = native_field(&record, "name")?.extract()?;
        let check_every: Option<u64> = native_field(&record, "check_every_seconds")?.extract()?;
        let set: DiscoveredSet = match source {
            DiscoverySource::Placeholders => never_discovered(check_every),
            DiscoverySource::Saved(saved) => saved.get(&name).cloned().map_or_else(
                || never_discovered(check_every),
                |set| checked(set, check_every),
            ),
            DiscoverySource::Live(live) => {
                let requested: bool = match &live.rediscover {
                    Rediscover::All => true,
                    Rediscover::Only(names) => names.contains(&name),
                };
                live_set(inputs, (&record, &name, check_every), (live, requested))?
            }
        };
        if sets.insert(name.clone(), set).is_some() {
            return Err(new_plain_error(
                py,
                ERROR,
                &format!("Two @discover functions are named '{name}'; give one another name"),
            ));
        }
    }
    match source {
        DiscoverySource::Live(LiveDiscovery {
            seed: Some(seed), ..
        }) => seeded(py, sets, seed)
            .into_iter()
            .map(|(name, set)| Ok((name, usable_set(inputs, set)?)))
            .collect(),
        _ => Ok(sets),
    }
}

/// `sets` with each failed set that has no result given the server's last good one, if any.
fn seeded(py: Python<'_>, mut sets: DiscoveredSets, source: &SeedSource) -> DiscoveredSets {
    let unseeded: Vec<String> = sets
        .iter()
        .filter(|(_, set)| set.status == DiscoveredSetStatus::Failed && set.result.is_none())
        .map(|(name, _)| name.clone())
        .collect();
    if unseeded.is_empty() {
        return sets;
    }
    let server: Result<DiscoveredSets, String> = server_sets(py, source);
    for name in unseeded {
        let Some(set) = sets.get_mut(&name) else {
            continue;
        };
        match &server {
            Ok(server) => {
                if let Some(earlier) = server.get(&name) {
                    set.result.clone_from(&earlier.result);
                    set.digest.clone_from(&earlier.digest);
                    set.last_good_at = earlier.last_good_at;
                }
            }
            Err(reason) => {
                set.error = Some(format!(
                    "{}; its last good items could not be read: {reason}",
                    set.error.as_deref().unwrap_or("failed")
                ));
            }
        }
    }
    sets
}

/// The sets of the server's current snapshot, read with the location credential.
fn server_sets(py: Python<'_>, source: &SeedSource) -> Result<DiscoveredSets, String> {
    let bearer: String = std::env::var(LOCATION_TOKEN_ENV)
        .map_err(|error| format!("{LOCATION_TOKEN_ENV}: {error}"))?;
    py.detach(|| {
        tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .map_err(|error| error.to_string())?
            .block_on(remote_discovered_sets(
                (&source.server_url, &bearer),
                (source.location_id, &source.target),
            ))
            .map_err(|error| error.to_string())
    })
}

fn checked(mut set: DiscoveredSet, check_every_seconds: Option<u64>) -> DiscoveredSet {
    set.check_every_seconds = check_every_seconds;
    set
}

/// The set after calling its function if `requested` or never tried; otherwise the previous set.
pub(crate) fn live_set(
    inputs: &Bound<'_, PyAny>,
    (record, name, check_every): (&Bound<'_, PyAny>, &str, Option<u64>),
    (live, requested): (&LiveDiscovery, bool),
) -> PyResult<DiscoveredSet> {
    let py: Python<'_> = inputs.py();
    let previous: Option<&DiscoveredSet> = live.previous.get(name);
    let untried: bool = previous.is_none_or(|set| set.status == DiscoveredSetStatus::Never);
    if !requested && !untried {
        return usable_set(
            inputs,
            previous.cloned().map_or_else(
                || never_discovered(check_every),
                |set| checked(set, check_every),
            ),
        );
    }
    let runner = RUNNER.get().ok_or_else(|| {
        new_plain_error(
            py,
            ERROR,
            "this build of SQLBuild cannot run @discover functions",
        )
    })?;
    let assets = PyTuple::new(py, for_each_functions(inputs, name)?)?;
    let project_dir: Bound<'_, PyAny> = native_field(inputs, "project_dir")?;
    let attempt: Result<DiscoveredResult, String> = runner(
        py,
        (&project_dir, live.target.as_deref()),
        (record, assets.as_any()),
    )
    .and_then(|json| {
        serde_json::from_str(&json)
            .map_err(|error| format!("discover function returned an unreadable result: {error}"))
    });
    let attempt: Result<DiscoveredResult, String> = match attempt {
        Ok(result) => match missing_node(inputs, &result)? {
            Some(problem) => Err(problem),
            None => Ok(result),
        },
        Err(error) => Err(error),
    };
    let set: DiscoveredSet = usable_set(
        inputs,
        record_attempt(previous, attempt, (Utc::now(), check_every)),
    )?;
    if !live.keep_last_good && set.status == DiscoveredSetStatus::Failed {
        return Err(new_plain_error(
            py,
            ERROR,
            &format!(
                "Discover function '{name}' failed: {}",
                set.error.as_deref().unwrap_or("no reason given")
            ),
        ));
    }
    Ok(set)
}

/// The functions of the assets that run once per item of set `name`.
fn for_each_functions<'py>(
    inputs: &Bound<'py, PyAny>,
    name: &str,
) -> PyResult<Vec<Bound<'py, PyAny>>> {
    let mut functions: Vec<Bound<'py, PyAny>> = Vec::new();
    for template in native_field(inputs, "for_each_assets")?.try_iter()? {
        let template: Bound<'py, PyAny> = template?;
        if native_field(&template, "for_each")?.extract::<String>()? == name {
            functions.push(native_field(&template, "function")?);
        }
    }
    Ok(functions)
}
