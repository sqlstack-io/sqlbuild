//! Planner scope assembly using native graph and selector operations.

use crate::bindings::_helpers::boundary::python_calls::KeywordArguments;
use crate::bindings::_helpers::compiled_views::model_reads;
use crate::bindings::_helpers::compiled_views::project_reads;
use crate::bindings::_helpers::compiled_views::source_reads;
use crate::bindings::_helpers::graph::project_graph::NativeProjectGraph;
use crate::bindings::_helpers::native_host::model_lookup::ModelHandle;
use crate::bindings::_helpers::planner_identity::graph::{
    execution_deps, execution_order, execution_origins,
};
use crate::bindings::_helpers::planner_scope::retained::ScopeData;
use crate::bindings::_helpers::planner_scope::values::{keys, project_graph};
use crate::bindings::_helpers::planner_selection::carrier::{Carrier, py_strings};
use crate::bindings::_helpers::planner_selection::selectors::resolve_graph_keys;
use crate::bindings::_helpers::records::construct::new_record;
use crate::bindings::_helpers::records::fields::field as native_field;
use crate::bindings::_helpers::records::ids::RecordId;
use pyo3::prelude::{Bound, PyAny, PyAnyMethods, PyResult, Python};
use pyo3::types::{PyDict, PyFrozenSet, PyTuple};
use sqlbuild_analysis::assembly::project::types::ObjectKey;
use sqlbuild_analysis::graph::main::downstream_edges::downstream_edges;
use sqlbuild_analysis::planner_scope::constants::SOURCE;
use sqlbuild_analysis::planner_scope::main::{
    expand_loaders::expand_loaders, managed_sources::managed_sources,
    source_selectors::source_selectors,
};
use std::collections::HashSet;

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
    let mut upstream = execution_deps(&project)?;
    let lineage: Bound<'py, NativeProjectGraph> =
        native_field(&project, "lineage_graph")?.cast_into()?;
    let lineage = lineage.borrow();
    let selected = args.value("selected_keys")?;
    let mut selected = if selected.is_none() {
        resolve_graph_keys(lineage.graph(), (&select, &exclude))?
    } else {
        keys(&selected)?
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
    let mut executable = if auto_load {
        source_keys
    } else {
        HashSet::new()
    };
    if auto_load {
        let mut managed: HashSet<String> = HashSet::new();
        for source in project_reads::sources(&project)?.try_iter()? {
            let source = source?;
            if !native_field(&source_reads::source_entry(&source)?, "loader")?.is_none() {
                managed.insert(source_reads::name(&source)?.extract()?);
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
    let order = execution_order(py, &upstream, &execution_origins(&project)?)?;
    let selected: HashSet<_> = selected.into_iter().collect();
    let mut models: std::collections::HashMap<String, ModelHandle> =
        std::collections::HashMap::new();
    let mut model_order: Vec<String> = Vec::new();
    for model in project_reads::models(&project)?.try_iter()? {
        let model = model?;
        let name: String = model_reads::name_object(&model)?.extract()?;
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
    }
    .object(py)
}

fn default_tuple<'py>(value: &Bound<'py, PyAny>) -> Bound<'py, PyAny> {
    if value.is_none() {
        PyTuple::empty(value.py()).into_any()
    } else {
        value.clone()
    }
}
