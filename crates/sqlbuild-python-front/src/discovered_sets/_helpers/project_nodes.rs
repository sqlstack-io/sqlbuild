//! The models, sources, seeds and jobs a project declares, which a set's items may depend on.

use std::collections::BTreeSet;

use pyo3::prelude::{Bound, PyAny, PyAnyMethods, PyResult};
use sqlbuild_orchestration::discovered_sets::main::unknown_dependency::unknown_dependency;
use sqlbuild_orchestration::discovered_sets::models::{
    DiscoveredResult, DiscoveredSet, DiscoveredSetStatus, ItemDependency,
};
use sqlbuild_python_base::records::main::fields::field::field as native_field;
use sqlbuild_python_base::records::main::fields::path_stem::path_stem;

use crate::discovery::main::sources::orchestration_declarations::orchestration_declarations;
use crate::discovery::models::NativeModelFile;

/// The project's names of each kind an item names; a kind no item names is not read.
struct ProjectNodes {
    models: BTreeSet<String>,
    sources: BTreeSet<String>,
    seeds: BTreeSet<String>,
    jobs: BTreeSet<String>,
}

/// Why `result` can't be a set of the project `inputs` describe, if it can't.
pub(crate) fn missing_node(
    inputs: &Bound<'_, PyAny>,
    result: &DiscoveredResult,
) -> PyResult<Option<String>> {
    let named = |wanted: fn(&ItemDependency) -> bool| {
        result
            .items
            .iter()
            .any(|item| item.depends_on.iter().any(wanted))
    };
    let read = |wanted: fn(&ItemDependency) -> bool,
                names: fn(&Bound<'_, PyAny>) -> PyResult<BTreeSet<String>>| {
        if named(wanted) {
            names(inputs)
        } else {
            Ok(BTreeSet::new())
        }
    };
    let nodes: ProjectNodes = ProjectNodes {
        models: read(
            |dependency| matches!(dependency, ItemDependency::Model(_)),
            model_names,
        )?,
        sources: read(
            |dependency| matches!(dependency, ItemDependency::Source(_)),
            source_names,
        )?,
        seeds: read(
            |dependency| matches!(dependency, ItemDependency::Seed(_)),
            seed_names,
        )?,
        jobs: read(
            |dependency| matches!(dependency, ItemDependency::Job(_)),
            job_names,
        )?,
    };
    Ok(unknown_dependency(result, &|dependency| {
        nodes.declares(dependency)
    }))
}

/// `set` failed when its result names what the project lacks: its last good items are dropped.
pub(crate) fn usable_set(
    inputs: &Bound<'_, PyAny>,
    mut set: DiscoveredSet,
) -> PyResult<DiscoveredSet> {
    let Some(result) = &set.result else {
        return Ok(set);
    };
    let Some(problem) = missing_node(inputs, result)? else {
        return Ok(set);
    };
    set.error = Some(match (set.status, set.error.take()) {
        (DiscoveredSetStatus::Failed, Some(error)) => {
            format!("{error}; its last good items can't be kept: {problem}")
        }
        _ => problem,
    });
    set.status = DiscoveredSetStatus::Failed;
    set.result = None;
    set.digest = None;
    set.last_good_at = None;
    Ok(set)
}

impl ProjectNodes {
    fn declares(&self, dependency: &ItemDependency) -> bool {
        match dependency {
            ItemDependency::Model(name) => self.models.contains(name),
            ItemDependency::Source(name) => self.sources.contains(name),
            ItemDependency::Seed(name) => self.seeds.contains(name),
            ItemDependency::Job(set) => self.jobs.contains(&set.name),
            ItemDependency::Item(_) | ItemDependency::Tag(_) => true,
        }
    }
}

/// Each model file's name, its file's stem.
fn model_names(inputs: &Bound<'_, PyAny>) -> PyResult<BTreeSet<String>> {
    let mut names: BTreeSet<String> = BTreeSet::new();
    for file in native_field(inputs, "model_files")?.try_iter()? {
        let file: Bound<'_, PyAny> = file?;
        names.insert(match file.cast::<NativeModelFile>() {
            Ok(native) => native.get().path_texts(file.py())?.name,
            Err(_) => path_stem(&native_field(&file, "file_path")?)?,
        });
    }
    Ok(names)
}

/// Every declared source, and each loader, which a `+source` selection treats as one.
fn source_names(inputs: &Bound<'_, PyAny>) -> PyResult<BTreeSet<String>> {
    let mut names: BTreeSet<String> = BTreeSet::new();
    for file in native_field(inputs, "source_files")?.try_iter()? {
        for entry in native_field(&file?, "source_entries")?.try_iter()? {
            names.insert(native_field(&entry?, "name")?.extract()?);
        }
    }
    for loader in native_field(inputs, "loader_functions")?.try_iter()? {
        names.insert(native_field(&loader?, "name")?.extract()?);
    }
    Ok(names)
}

/// Each seed CSV's name; discovery checks that each has its declaration.
fn seed_names(inputs: &Bound<'_, PyAny>) -> PyResult<BTreeSet<String>> {
    let mut names: BTreeSet<String> = BTreeSet::new();
    for file in native_field(inputs, "seed_files")?.try_iter()? {
        names.insert(path_stem(&native_field(&file?, "file_path")?)?);
    }
    Ok(names)
}

/// Every job and job group under `orchestration/`, whatever targets they are declared for.
fn job_names(inputs: &Bound<'_, PyAny>) -> PyResult<BTreeSet<String>> {
    let inventory = orchestration_declarations(&native_field(inputs, "project_dir")?)?;
    Ok(inventory
        .jobs
        .iter()
        .map(|(_, job)| job.run.name.value.clone())
        .chain(
            inventory
                .groups
                .iter()
                .map(|(_, group)| group.run.name.value.clone()),
        )
        .collect())
}
