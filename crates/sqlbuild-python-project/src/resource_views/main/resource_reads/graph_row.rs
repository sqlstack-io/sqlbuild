//! A resource's graph facts without constructing its public keys or dependency tuple.

use crate::compiled_views::main::compiled_models::model_graph_row::model_graph_row;
use crate::compiled_views::models::NativeCompiledModel;
use crate::graph::main::nodes::native_key::native_key;
use crate::graph::models::ResourceRow;
use crate::resource_views::models::NativeCompiledFunction;
use crate::resource_views::models::NativeCompiledSeed;
use crate::resource_views::models::NativeCompiledSource;
use pyo3::prelude::{Bound, PyAny, PyAnyMethods, PyResult};
use sqlbuild_python_base::compile_driver::constants::COMPILED_VIEW_FALLBACKS;
use sqlbuild_python_base::compile_driver::main::count_stage::count_stage;
use sqlbuild_python_base::records::main::fields::field::field as native_field;

/// A resource's graph facts without constructing its public keys or dependency tuple.
pub fn graph_row(resource: &Bound<'_, PyAny>) -> PyResult<ResourceRow> {
    let py = resource.py();
    if let Ok(view) = resource.cast::<NativeCompiledModel>() {
        return model_graph_row(view.get(), py);
    }
    if let Ok(view) = resource.cast::<NativeCompiledFunction>() {
        return view.get().graph_row(py);
    }
    if let Ok(view) = resource.cast::<NativeCompiledSeed>() {
        return view.get().graph_row(py);
    }
    if let Ok(view) = resource.cast::<NativeCompiledSource>() {
        return view.get().graph_row(py);
    }
    count_stage(py, COMPILED_VIEW_FALLBACKS, 1)?;
    Ok(ResourceRow {
        key: native_key(&native_field(resource, "key")?)?,
        deps: native_field(resource, "deps")?
            .try_iter()?
            .map(|key| native_key(&key?))
            .collect::<PyResult<_>>()?,
        tags: Vec::new(),
        folder: None,
    })
}
