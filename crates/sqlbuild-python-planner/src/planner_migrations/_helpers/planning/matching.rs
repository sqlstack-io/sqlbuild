//! Fingerprint cache callbacks around native one-to-one migration matching.

use crate::planner_migrations::models::{FingerprintInputs, ModelMigrationRequest};
use pyo3::IntoPyObject;
use pyo3::prelude::{Bound, PyAny, PyAnyMethods, PyResult};
use pyo3::types::{PyDict, PyDictMethods, PyTuple};
use sqlbuild_analysis::planner_migrations::main::match_migrations::match_migrations;
use sqlbuild_analysis::planner_migrations::models::MigrationCandidate;
use sqlbuild_python_base::boundary::main::call_with::call_with;
use sqlbuild_python_base::records::main::fields::field::field as native_field;
use sqlbuild_python_base::records::main::fields::mapping_get::mapping_get;
use sqlbuild_python_project::compiled_views::main::compiled_models::model_name::model_name;
use sqlbuild_python_project::compiled_views::main::compiled_models::name_object::name_object;
use sqlbuild_python_project::compiled_views::main::compiled_models::query_sql::query_sql;
use std::collections::{BTreeMap, BTreeSet};

fn variants<'py>(
    inputs: &FingerprintInputs<'py>,
    model: &Bound<'py, PyAny>,
    maps: Bound<'py, PyAny>,
) -> PyResult<Vec<String>> {
    let name = name_object(model)?;
    let metadata = inputs.metadata_jsons.get_item(name)?;
    let cache = native_field(&inputs.fingerprints, "fingerprint")?;
    let mut hashes: Vec<String> = Vec::new();
    for map in maps.try_iter()? {
        let hash: Option<String> = call_with(
            &cache,
            &[
                ("query_sql", query_sql(model)?),
                ("metadata_json", metadata.clone()),
                ("ref_identities", map?),
                ("dialect", inputs.dialect.clone()),
            ],
        )?
        .extract()?;
        if let Some(hash) = hash
            && !hashes.contains(&hash)
        {
            hashes.push(hash);
        }
    }
    Ok(hashes)
}

/// The fingerprint variants tried, the matched candidate per new model, and ambiguous names.
pub(crate) fn match_migration_fingerprints<'py>(
    (models, new_names): (&[Bound<'py, PyAny>], BTreeSet<String>),
    seed_variants: &Bound<'py, PyAny>,
    candidates: &Bound<'py, PyAny>,
    inputs: &FingerprintInputs<'py>,
) -> PyResult<(Bound<'py, PyAny>, Bound<'py, PyDict>, Bound<'py, PyAny>)> {
    let py = seed_variants.py();
    let mut by_name: BTreeMap<String, Bound<'py, PyAny>> = BTreeMap::new();
    let mut names: Vec<String> = Vec::new();
    for model in models {
        let name: String = model_name(model)?;
        if new_names.contains(&name) {
            names.push(name.clone());
        }
        by_name.insert(name, model.clone());
    }
    let original: Vec<Bound<'py, PyAny>> = candidates.try_iter()?.collect::<PyResult<_>>()?;
    let mut fingerprints: Vec<MigrationCandidate> = Vec::new();
    for candidate in &original {
        fingerprints.push(MigrationCandidate {
            name: native_field(candidate, "node_name")?.extract()?,
            fingerprint: stored_migration_fingerprint(candidate)?.unwrap_or_default(),
        });
    }
    let seeds: Vec<BTreeMap<String, String>> = seed_variants.extract()?;
    let matched = match_migrations(&names, &seeds, &fingerprints, |name, maps| {
        variants(inputs, &by_name[name], PyTuple::new(py, maps)?.into_any())
    })?;
    let matches = PyDict::new(py);
    for (name, index) in matched.matches {
        matches.set_item(name, &original[index])?;
    }
    Ok((
        PyTuple::new(py, matched.variants)?.into_any(),
        matches,
        matched.ambiguous.into_pyobject(py)?.into_any(),
    ))
}

/// The fingerprint variants of each requested destination model with metadata.
pub(crate) fn migration_destination_fingerprints<'py>(
    requests: &[ModelMigrationRequest<'py>],
    variant_maps: &Bound<'py, PyAny>,
    inputs: &FingerprintInputs<'py>,
) -> PyResult<Bound<'py, PyDict>> {
    let py = variant_maps.py();
    let result = PyDict::new(py);
    for request in requests {
        let model = &request.model;
        let name = name_object(model)?;
        if mapping_get(&inputs.metadata_jsons, &name)?.is_none() {
            continue;
        }
        result.set_item(
            name,
            PyTuple::new(py, variants(inputs, model, variant_maps.clone())?)?,
        )?;
    }
    Ok(result)
}

/// The migration fingerprint stored in a model fingerprint's metadata.
pub(crate) fn stored_migration_fingerprint(
    fingerprint: &Bound<'_, PyAny>,
) -> PyResult<Option<String>> {
    let metadata: String = native_field(fingerprint, "metadata_json")?.extract()?;
    Ok(sqlbuild_analysis::planner_migrations::main::stored_migration_fingerprint::stored_migration_fingerprint(&metadata))
}
