//! Planner scope assembly using native graph and selector operations.

use crate::compiled_views::main::compiled_models::name_object::name_object;
use crate::compiled_views::main::project::compiled_models::compiled_models;
use crate::compiled_views::main::project::sources::sources;
use crate::graph::models::NativeProjectGraph;
use crate::native_host::models::ModelHandle;
use crate::planner_identity::main::execution_deps_with::execution_deps_with;
use crate::planner_identity::main::execution_order::execution_order;
use crate::planner_identity::main::execution_origins_with::execution_origins_with;
use crate::planner_scope::main::keys::keys;
use crate::planner_scope::main::project_graph::project_graph;
use crate::planner_scope::models::ScopeData;
use crate::planner_selection::main::py_strings::py_strings;
use crate::planner_selection::main::resolve_planner_keys::resolve_planner_keys;
use crate::planner_selection::models::Carrier;
use crate::references::main::native_execution_gate_edges::native_execution_gate_edges;
use crate::references::types::ExecutionGates;
use crate::resource_views::main::source_reads::source_entry::source_entry;
use pyo3::prelude::{Bound, PyAny, PyAnyMethods, PyResult, Python};
use pyo3::types::{PyDict, PyFrozenSet, PyString, PyStringMethods, PyTuple};
use sqlbuild_analysis::assembly::project::types::ObjectKey;
use sqlbuild_analysis::graph::constants::{AUDIT_KIND, RUN_SELECTOR_KINDS};
use sqlbuild_analysis::graph::main::downstream_edges::downstream_edges;
use sqlbuild_analysis::planner_scope::constants::SOURCE;
use sqlbuild_analysis::planner_scope::main::{
    expand_loaders::expand_loaders, managed_sources::managed_sources,
    source_selectors::source_selectors,
};
use sqlbuild_python_base::boundary::models::KeywordArguments;
use sqlbuild_python_base::records::main::construct::new_record::new_record;
use sqlbuild_python_base::records::main::fields::field::field as native_field;
use sqlbuild_python_base::records::types::RecordId;
use std::collections::{BTreeSet, HashSet};

pub(crate) fn build_planner_scope<'py>(
    py: Python<'py>,
    options: Option<Bound<'py, PyDict>>,
) -> PyResult<Bound<'py, PyAny>> {
    let args = KeywordArguments::read(
        py,
        "build_planner_scope",
        options,
        (
            &["project"],
            &[
                "select",
                "exclude",
                "auto_load_sources",
                "selected_keys",
                "python_read_source_names",
                "sql_test_selection",
            ],
        ),
    )?;
    let project = args.value("project")?;
    let select = default_tuple(&args.value("select")?);
    let exclude = default_tuple(&args.value("exclude")?);
    let auto_load = args.value("auto_load_sources")?.is_truthy()?;
    let gates: ExecutionGates = native_execution_gate_edges(&project)?;
    let mut upstream = execution_deps_with(&project, &gates)?;
    let lineage: Bound<'py, NativeProjectGraph> =
        native_field(&project, "lineage_graph")?.cast_into()?;
    let lineage = lineage.borrow();
    let selected = args.value("selected_keys")?;
    let caller_resolved_loader_upstream: bool = !selected.is_none();
    let (mut selected, audits) = if selected.is_none() {
        resolve_planner_keys(&project, lineage.graph(), (&select, &exclude))?
    } else {
        let (select, exclude) = (audit_selectors(&select)?, audit_selectors(&exclude)?);
        let audits = if select.is_truthy()? {
            resolve_planner_keys(&project, lineage.graph(), (&select, &exclude))?.1
        } else {
            BTreeSet::new()
        };
        (keys(&selected)?, audits)
    };
    let names = &lineage.graph().names;
    let texts = py_strings(&select)?;
    let carrier = Carrier::new(py, &texts, || {
        Ok(names.iter().map(|(name, _)| name.clone()).collect())
    })?;
    let sent = carrier.send_all(&texts)?;
    let source_keys =
        source_selectors(&sent, names, &selected, true).map_err(|error| carrier.error(error))?;
    let named_sources =
        source_selectors(&sent, names, &selected, false).map_err(|error| carrier.error(error))?;
    let mut executable = if auto_load && !caller_resolved_loader_upstream {
        source_keys
    } else {
        HashSet::new()
    };
    if auto_load {
        let mut managed: HashSet<String> = HashSet::new();
        for source in sources(&project)?.try_iter()? {
            let source = source?;
            let entry: Bound<'py, PyAny> = source_entry(&source)?;
            if !native_field(&entry, "loader")?.is_none()
                || !native_field(&entry, "load")?.is_none()
            {
                managed.insert(
                    crate::resource_views::main::source_reads::name::name(&source)?.extract()?,
                );
            }
        }
        let loaded = managed_sources(&selected, &upstream, &managed);
        let mut seen: HashSet<ObjectKey> = selected.iter().cloned().collect();
        selected.extend(
            loaded
                .iter()
                .filter(|key| seen.insert((*key).clone()))
                .cloned(),
        );
        executable.extend(loaded);
    }
    let selected_set: HashSet<_> = selected.iter().collect();
    executable.retain(|key| selected_set.contains(key));
    if auto_load || selected.iter().any(|key| key.0 == SOURCE) {
        (selected, upstream) =
            expand_loaders(&project_graph(&project)?, selected, upstream, &executable);
    }
    let end_audit_sources: HashSet<ObjectKey> = selected
        .iter()
        .filter(|key| key.0 == SOURCE && (!auto_load || named_sources.contains(key)))
        .cloned()
        .collect();
    let order = execution_order(py, &upstream, &execution_origins_with(&project, &gates)?)?;
    let selected: HashSet<_> = selected.into_iter().collect();
    let mut models: std::collections::HashMap<String, ModelHandle> =
        std::collections::HashMap::new();
    let mut model_order: Vec<String> = Vec::new();
    for model in compiled_models(&project)?.try_iter()? {
        let model = model?;
        let name: String = name_object(&model)?.extract()?;
        if !models.contains_key(&name) {
            model_order.push(name.clone());
        }
        models.insert(name, ModelHandle::read(&model));
    }
    let mut reads = args.value("python_read_source_names")?;
    if reads.is_none() {
        reads = PyFrozenSet::empty(py)?.into_any();
    }
    let mut tests = args.value("sql_test_selection")?;
    if !tests.is_truthy()? {
        tests = new_record(py, RecordId::SqlTestSelection, (), None)?;
    }
    ScopeData {
        downstream: downstream_edges(&upstream),
        upstream,
        names: names.clone(),
        models,
        model_order,
        user_selected: selected.clone(),
        selected,
        order,
        python_reads: reads.extract()?,
        tests: tests.extract()?,
        load_sources: auto_load,
        end_audit_sources,
        audits: audits.into_iter().collect(),
    }
    .object(py)
}

/// The selectors in `texts` that can select an audit: `audit:` and the past-run selectors; the rest select only nodes.
fn audit_selectors<'py>(texts: &Bound<'py, PyAny>) -> PyResult<Bound<'py, PyAny>> {
    let picks = |part: &str| {
        let part = part.trim_start_matches('+');
        part.split_once(':')
            .is_some_and(|(kind, _)| kind == AUDIT_KIND || RUN_SELECTOR_KINDS.contains(&kind))
    };
    let mut kept: Vec<Bound<'py, PyString>> = Vec::new();
    for text in py_strings(texts)? {
        if text.to_str()?.split(',').any(picks) {
            kept.push(text);
        }
    }
    Ok(PyTuple::new(texts.py(), kept)?.into_any())
}

fn default_tuple<'py>(value: &Bound<'py, PyAny>) -> Bound<'py, PyAny> {
    if value.is_none() {
        PyTuple::empty(value.py()).into_any()
    } else {
        value.clone()
    }
}
