//! Native migration discovery over existing inspection and fingerprint cache boundaries.

use crate::planner_migrations::_helpers::discovery::discovery_events::renames;
use crate::planner_migrations::_helpers::discovery::discovery_models::ordered;
use crate::planner_migrations::_helpers::discovery::discovery_requests::automatic;
use crate::planner_migrations::_helpers::planning::matching::{
    match_migration_fingerprints, migration_destination_fingerprints,
};
use crate::planner_migrations::models::{
    DiscoveryRequest, FingerprintInputs, ModelMigrationDiscovery,
};
use pyo3::prelude::{PyAnyMethods, PyResult};
use pyo3::types::{PyDict, PyDictMethods, PyListMethods, PyTuple, PyTupleMethods};
use sqlbuild_python_base::boundary::main::call_with::call_with;
use sqlbuild_python_base::records::main::fields::field::field as native_field;
use sqlbuild_python_project::compiled_views::main::compiled_models::name_object::name_object;
use std::collections::BTreeSet;

/// Manual and automatically discovered migration requests with their destination fingerprints.
pub(crate) fn discover_model_migrations<'py>(
    request: DiscoveryRequest<'py>,
) -> PyResult<ModelMigrationDiscovery<'py>> {
    let DiscoveryRequest {
        runtime,
        scope,
        snapshot,
        manual_requests: manual,
        declarations,
        state,
        project_schemas,
        fingerprints: cache,
    } = request;
    let py = runtime.py();
    let models = ordered(&runtime)?;
    let declared_names = PyDict::new(py);
    for declaration in &declarations {
        declared_names.set_item(&declaration.model_name, py.None())?;
    }
    let triggered = crate::planner_migrations::_helpers::discovery::discovery_models::triggered(
        &models,
        &scope,
        &snapshot,
        &declared_names,
    )?;
    if !triggered && manual.is_empty() {
        return Ok(ModelMigrationDiscovery {
            requests: Vec::new(),
            warnings: Vec::new(),
            destination_fingerprints: PyDict::new(py),
        });
    }
    let metadata = crate::planner_migrations::_helpers::discovery::discovery_models::metadata(
        &runtime, &models,
    )?;
    let inputs = FingerprintInputs {
        metadata_jsons: metadata.into_any(),
        dialect: native_field(&runtime, "adapter")?.call_method0("sql_analysis_dialect")?,
        fingerprints: cache,
    };
    call_with(
        &native_field(&state, "inspect_schemas")?,
        &[("schemas", project_schemas)],
    )?;
    let declared = crate::planner_migrations::_helpers::discovery::discovery_events::declared(
        &declarations,
        &state,
    )?;
    let names = PyDict::new(py);
    for item in &models {
        names.set_item(name_object(item)?, item)?;
    }
    let event_renames = renames(&runtime, &names, &state)?;
    for (name, origin) in declared.iter() {
        event_renames.set_item(name, origin)?;
    }
    let seeds = PyTuple::new(py, [event_renames, declared])?.into_any();
    if !triggered {
        let destination_fingerprints =
            migration_destination_fingerprints(&manual, &seeds, &inputs)?;
        return Ok(ModelMigrationDiscovery {
            requests: manual,
            warnings: Vec::new(),
            destination_fingerprints,
        });
    }
    let unbuilt = PyDict::new(py);
    for item in &models {
        let name = name_object(item)?;
        if !declared_names.contains(&name)?
            && call_with(
                &native_field(&state, "named_fingerprint")?,
                &[("model_name", name.clone())],
            )?
            .is_none()
        {
            unbuilt.set_item(name, item)?;
        }
    }
    let resumed = crate::planner_migrations::_helpers::discovery::discovery_events::resumed(
        &runtime, &models, &unbuilt, &state,
    )?;
    let candidates =
        crate::planner_migrations::_helpers::discovery::discovery_requests::candidates(
            &runtime,
            &state,
            &names,
            (&declarations, &resumed),
        )?;
    for (name, _) in resumed.iter() {
        unbuilt.del_item(name)?;
    }
    let new_names: BTreeSet<String> = unbuilt
        .keys()
        .iter()
        .map(|name| name.extract())
        .collect::<PyResult<_>>()?;
    let (variant_maps, matches, ambiguous) =
        match_migration_fingerprints((&models, new_names), &seeds, candidates.as_any(), &inputs)?;
    let mut requests = manual;
    requests.extend(automatic(
        &runtime,
        &scope,
        &models,
        (matches.as_any(), &resumed),
    )?);
    let warnings = crate::planner_migrations::_helpers::discovery::discovery_requests::warnings(
        &models, &scope, &ambiguous,
    )?;
    let destination_fingerprints =
        migration_destination_fingerprints(&requests, &variant_maps, &inputs)?;
    Ok(ModelMigrationDiscovery {
        requests,
        warnings: warnings.iter().collect(),
        destination_fingerprints,
    })
}
