//! Scenario artifact dataclasses and adapter identifier callbacks.

use crate::bindings::_helpers::planner_scenario::constants::{
    SCENARIO_PLAN_HASH_COLLISION, SCENARIO_PLAN_INVALID_HASH_PREFIX,
    SCENARIO_PLAN_RELATION_COLLISION,
};
use crate::bindings::_helpers::planner_scenario::values::{error, string};
use crate::bindings::_helpers::relation_naming::fitting::fitted_logical_name;
use pyo3::prelude::{Bound, PyAny, PyAnyMethods, PyResult, Python, pyfunction};
use pyo3::types::{PyDict, PyDictMethods, PyTuple};
use sqlbuild_analysis::planner_scenario::main::{hash_prefix, parse_artifact_name};
use sqlbuild_python_base::boundary::models::KeywordArguments;
use sqlbuild_python_base::records::main::construct::enum_from_value::enum_from_value;
use sqlbuild_python_base::records::main::construct::new_record_with::new_record_with;
use sqlbuild_python_base::records::main::fields::field::field as native_field;
use sqlbuild_python_base::records::types::RecordId;

const SCENARIO_ARTIFACT_LABEL: &str = "Scenario artifact";

#[pyfunction]
#[pyo3(signature = (**kwargs))]
pub(crate) fn compute_scenario_hash_prefix(
    py: Python<'_>,
    kwargs: Option<Bound<'_, PyDict>>,
) -> PyResult<String> {
    let args = KeywordArguments::read(
        py,
        "compute_scenario_hash_prefix",
        kwargs,
        (
            &["project_name", "scenario_name"],
            &["run_namespace", "prefix_length"],
        ),
    )?;
    let project: String = args.value("project_name")?.extract()?;
    let scenario: String = args.value("scenario_name")?.extract()?;
    let namespace: Option<String> = args.value("run_namespace")?.extract()?;
    let length: Option<i64> = args.value("prefix_length")?.extract()?;
    compute_prefix(
        py,
        (&project, &scenario, namespace.as_deref()),
        length.unwrap_or(12),
    )
}

fn compute_prefix(
    py: Python<'_>,
    identity: (&str, &str, Option<&str>),
    prefix_length: i64,
) -> PyResult<String> {
    if prefix_length < 1 {
        return Err(error(
            py,
            "Scenario hash prefix length must be at least 1",
            SCENARIO_PLAN_INVALID_HASH_PREFIX,
            None,
        )?);
    }
    Ok(hash_prefix::hash_prefix(
        identity.0,
        identity.1,
        identity.2,
        usize::try_from(prefix_length).unwrap_or(usize::MAX),
    ))
}

#[pyfunction]
#[pyo3(signature = (**kwargs))]
pub(crate) fn build_scenario_hash_index<'py>(
    py: Python<'py>,
    kwargs: Option<Bound<'py, PyDict>>,
) -> PyResult<Bound<'py, PyDict>> {
    let args = KeywordArguments::read(
        py,
        "build_scenario_hash_index",
        kwargs,
        (
            &["project_name", "scenarios"],
            &["run_namespace", "prefix_length"],
        ),
    )?;
    let project_name: String = args.value("project_name")?.extract()?;
    let scenarios = args.value("scenarios")?;
    let run_namespace: Option<String> = args.value("run_namespace")?.extract()?;
    let prefix_length: Option<i64> = args.value("prefix_length")?.extract()?;
    let result = PyDict::new(py);
    let mut seen: std::collections::HashMap<String, String> = std::collections::HashMap::new();
    for scenario in scenarios.try_iter()? {
        let name: String = native_field(&scenario?, "name")?.extract()?;
        let prefix = compute_prefix(
            py,
            (&project_name, &name, run_namespace.as_deref()),
            prefix_length.unwrap_or(12),
        )?;
        if let Some(existing) = seen.insert(prefix.clone(), name.clone()) {
            return Err(error(
                py,
                &format!(
                    "Scenario artifact hash collision: scenarios '{existing}' and '{name}' both map to hash prefix '{prefix}'. Rename one scenario file so SQLBuild can generate distinct warehouse artifact prefixes."
                ),
                SCENARIO_PLAN_HASH_COLLISION,
                Some(
                    "Rename one scenario file so SQLBuild can generate distinct artifact prefixes.",
                ),
            )?);
        }
        result.set_item(name, prefix)?;
    }
    Ok(result)
}

pub(crate) fn fit_scenario_artifact_logical_name(
    py: Python<'_>,
    logical_name: &str,
    fixed_prefix: &str,
    identifier_limit: i64,
) -> PyResult<String> {
    fitted_logical_name(
        py,
        (logical_name, fixed_prefix),
        identifier_limit,
        SCENARIO_ARTIFACT_LABEL,
    )
}

pub(crate) fn build_scenario_artifact_physical_name(
    hash_prefix: &str,
    kind: &str,
    logical_name: Bound<'_, PyAny>,
    identifier_limit: i64,
) -> PyResult<String> {
    let py = logical_name.py();
    let prefix = format!("__sqb_{hash_prefix}__{kind}__");
    let logical: String = fit_scenario_artifact_logical_name(
        py,
        &logical_name.extract::<String>()?,
        &prefix,
        identifier_limit,
    )?;
    Ok(format!("{prefix}{logical}"))
}

#[pyfunction]
#[pyo3(signature = (*, hash_prefix, kind, logical_name, identifier_limit=63))]
pub(crate) fn build_scenario_artifact_name(
    hash_prefix: &str,
    kind: Bound<'_, PyAny>,
    logical_name: Bound<'_, PyAny>,
    identifier_limit: i64,
) -> PyResult<String> {
    let py = kind.py();
    let kind: String = native_field(
        &enum_from_value(py, RecordId::ScenarioArtifactKind, kind)?,
        "value",
    )?
    .extract()?;
    build_scenario_artifact_physical_name(hash_prefix, &kind, logical_name, identifier_limit)
}

#[pyfunction]
pub(crate) fn parse_scenario_artifact_physical_name<'py>(
    py: Python<'py>,
    name: &str,
) -> PyResult<Bound<'py, PyAny>> {
    let Some((hash, kind, logical)) = parse_artifact_name::parse_artifact_name(name) else {
        return Ok(py.None().into_bound(py));
    };
    new_record_with(
        py,
        RecordId::ParsedScenarioArtifactName,
        &[
            ("hash_prefix", string(py, hash)),
            ("kind", string(py, kind)),
            ("logical_name", string(py, logical)),
        ],
    )
}

#[pyfunction]
pub(crate) fn is_scenario_artifact_physical_name(name: &str) -> bool {
    parse_artifact_name::parse_artifact_name(name).is_some()
}

#[pyfunction]
#[pyo3(signature = (**kwargs))]
pub(crate) fn build_scenario_relation_map<'py>(
    py: Python<'py>,
    kwargs: Option<Bound<'py, PyDict>>,
) -> PyResult<Bound<'py, PyAny>> {
    let args = KeywordArguments::read(
        py,
        "build_scenario_relation_map",
        kwargs,
        (
            &["scenario_name", "hash_prefix", "artifacts"],
            &["identifier_limit", "normalize_identifier"],
        ),
    )?;
    let scenario: String = args.value("scenario_name")?.extract()?;
    let hash: String = args.value("hash_prefix")?.extract()?;
    let limit = args.value("identifier_limit")?;
    let limit = if limit.is_none() {
        63
    } else {
        limit.extract()?
    };
    let normalize = args.value("normalize_identifier")?;
    let seen = PyDict::new(py);
    let mut artifacts: Vec<Bound<'py, PyAny>> = Vec::new();
    for artifact in args.value("artifacts")?.try_iter()? {
        let artifact = artifact?;
        let physical = build_scenario_artifact_name(
            &hash,
            native_field(&artifact, "kind")?,
            native_field(&artifact, "logical_name")?,
            limit,
        )?;
        let normalized = if normalize.is_none() {
            string(py, &physical)
        } else {
            normalize.call1((&physical,))?
        };
        if let Some(existing) = seen.get_item(&normalized)? {
            let old_kind: String = enum_from_value(
                py,
                RecordId::ScenarioArtifactKind,
                native_field(&existing, "kind")?,
            )?
            .getattr("value")?
            .extract()?;
            let new_kind: String = enum_from_value(
                py,
                RecordId::ScenarioArtifactKind,
                native_field(&artifact, "kind")?,
            )?
            .getattr("value")?
            .extract()?;
            let old_name: String = native_field(&existing, "logical_name")?.extract()?;
            let new_name: String = native_field(&artifact, "logical_name")?.extract()?;
            return Err(error(
                py,
                &format!(
                    "Scenario relation name collision in scenario '{scenario}'. Both scenario artifacts map to '{physical}': {old_kind} {old_name} and {new_kind} {new_name}. Rename one artifact or shorten one model/source name."
                ),
                SCENARIO_PLAN_RELATION_COLLISION,
                Some("Rename or shorten one of the colliding scenario artifacts."),
            )?);
        }
        seen.set_item(normalized, &artifact)?;
        artifacts.push(new_record_with(
            py,
            RecordId::ScenarioArtifactName,
            &[
                ("identity", artifact),
                ("physical_name", string(py, &physical)),
            ],
        )?);
    }
    new_record_with(
        py,
        RecordId::ScenarioRelationMap,
        &[
            ("scenario_name", args.value("scenario_name")?),
            ("hash_prefix", args.value("hash_prefix")?),
            ("artifacts", PyTuple::new(py, artifacts)?.into_any()),
        ],
    )
}
