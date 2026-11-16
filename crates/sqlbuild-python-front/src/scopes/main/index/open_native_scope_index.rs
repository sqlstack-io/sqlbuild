//! Python's `open_native_scope_index`: the natively built index of discovered inputs.

use crate::scopes::_helpers::index::scope_index::Walk;
use crate::scopes::_helpers::index::scope_index::materialise;
use crate::scopes::_helpers::index::scope_index::unindexable;
use crate::scopes::models::NativeScopeIndex;
use pyo3::prelude::{Bound, PyAny, PyResult, Python};
use pyo3::pyfunction;
use pyo3::types::PyTuple;
use sqlbuild_python_base::boundary::main::compiler_error::compiler_error;
use sqlbuild_python_base::types::CompilerDetach;
use sqlbuild_scopes::scope_index::main::build_scope_index::build_scope_index;
use sqlbuild_scopes::scope_index::models::{ScopeDeferral, ScopeIndex, ScopeInputs};

/// Python's `open_native_scope_index`: the natively built index of discovered inputs.
#[pyfunction]
#[pyo3(signature = (*, discovered_inputs, loaded_macros, static_macros = None))]
pub fn open_native_scope_index(
    py: Python<'_>,
    discovered_inputs: &Bound<'_, PyAny>,
    loaded_macros: &Bound<'_, PyAny>,
    static_macros: Option<Bound<'_, PyAny>>,
) -> PyResult<NativeScopeIndex> {
    let static_macros: Bound<'_, PyAny> = match static_macros {
        Some(static_macros) => static_macros,
        None => PyTuple::empty(py).into_any(),
    };
    let static_macros: &Bound<'_, PyAny> = &static_macros;
    let mut walk: Walk<'_> = Walk::new(py)?;
    walk.add_models(discovered_inputs)?;
    walk.add_resources(discovered_inputs)?;
    walk.add_declaration_files(discovered_inputs)?;
    walk.add_named_declarations(discovered_inputs)?;
    walk.add_macros(discovered_inputs, (loaded_macros, static_macros))?;
    if !walk.indexable {
        return Err(unindexable(py)?);
    }
    let inputs: ScopeInputs = std::mem::take(&mut walk.inputs);
    let built: Result<ScopeIndex, ScopeDeferral> = py
        .compiler_detach(|| Ok(build_scope_index(inputs)))
        .map_err(compiler_error)?;
    let Ok(native) = built else {
        return Err(unindexable(py)?);
    };
    materialise(walk, native)
}
