//! Native planner scope ownership, with projection only at public record boundaries.

use crate::planner_scope::main::edges::edges;
use crate::planner_scope::main::keys::keys;
use crate::planner_scope::models::RetainedScope;
use crate::planner_scope::models::ScopeData;
use std::collections::{HashMap, HashSet};
use std::sync::Arc;

use pyo3::prelude::{Bound, PyAny, PyAnyMethods, PyResult};

use crate::native_host::models::ModelHandle;
use sqlbuild_python_base::records::main::construct::new_record_with::new_record_with;
use sqlbuild_python_base::records::main::fields::field::field;
use sqlbuild_python_base::records::main::fields::mapping_items::mapping_items;
use sqlbuild_python_base::records::models::NativeRecord;
use sqlbuild_python_base::records::types::RecordId;

/// Read native ownership directly, or import a caller-authored public scope at the API edge.
pub(crate) fn scope_data(value: &Bound<'_, PyAny>) -> PyResult<Arc<ScopeData>> {
    if let Ok(scope) = value.cast::<RetainedScope>() {
        return Ok(scope.get().data.clone());
    }
    let mut models: HashMap<String, ModelHandle> = HashMap::new();
    let mut model_order: Vec<String> = Vec::new();
    for item in mapping_items(&field(value, "models_by_name")?)?.try_iter()? {
        let item = item?;
        let name: String = item.get_item(0)?.extract()?;
        model_order.push(name.clone());
        models.insert(name, ModelHandle::read(&item.get_item(1)?));
    }
    Ok(Arc::new(ScopeData {
        upstream: edges(&field(value, "upstream_deps")?)?,
        downstream: edges(&field(value, "downstream_deps")?)?,
        names: mapping_items(&field(value, "all_keys")?)?
            .try_iter()?
            .map(|item| {
                let item = item?;
                Ok((
                    item.get_item(0)?.extract()?,
                    crate::graph::main::nodes::native_key::native_key(&item.get_item(1)?)?,
                ))
            })
            .collect::<PyResult<_>>()?,
        models,
        model_order,
        selected: keys(&field(value, "selected_keys")?)?.into_iter().collect(),
        order: keys(&field(value, "execution_order")?)?,
        user_selected: keys(&field(value, "user_selected_keys")?)?
            .into_iter()
            .collect(),
        python_reads: field(value, "python_read_source_names")?.extract()?,
        tests: field(value, "sql_test_selection")?.extract()?,
        load_sources: field(value, "load_sources")?.is_truthy()?,
        end_audit_sources: keys(&field(value, "end_audit_source_keys")?)?
            .into_iter()
            .collect(),
        audits: match value.getattr_opt("selected_audit_names")? {
            Some(names) if !names.is_none() => names.extract()?,
            _ => HashSet::new(),
        },
    }))
}

/// Preserve public record identity and dataclass behavior on explicit projection.
pub(crate) fn project_scope<'py>(value: &Bound<'py, PyAny>) -> PyResult<Bound<'py, PyAny>> {
    let Ok(scope) = value.cast::<RetainedScope>() else {
        return Ok(value.clone());
    };
    scope
        .get()
        .projection
        .get_or_try_init(value.py(), || {
            let names = [
                "upstream_deps",
                "downstream_deps",
                "all_keys",
                "models_by_name",
                "selected_keys",
                "execution_order",
                "user_selected_keys",
                "python_read_source_names",
                "sql_test_selection",
                "load_sources",
                "end_audit_source_keys",
            ];
            let fields = names
                .iter()
                .map(|name| Ok((*name, scope.get().data.field(value.py(), name)?)))
                .collect::<PyResult<Vec<_>>>()?;
            Ok::<_, pyo3::PyErr>(
                new_record_with(value.py(), RecordId::PlannerScope, &fields)?
                    .cast_into::<NativeRecord>()
                    .map(Bound::unbind)?,
            )
        })
        .map(|value| value.bind(scope.py()).clone().into_any())
}
