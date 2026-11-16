//! Register the exported native constants, the installed build identity and the cursor grain order on `sqlbuild._native`.

use pyo3::prelude::{Bound, PyAnyMethods, PyModule, PyModuleMethods, PyResult};
use pyo3::types::{PyDict, PyDictMethods, PyFrozenSet, PyTuple};

use crate::bindings::_helpers::cursor::conversion::GRAIN_NAMES;
use crate::bindings::_helpers::exported_constants::constants::{
    EXPORTED_INTEGERS, EXPORTED_TEXT_SETS, EXPORTED_TEXT_TUPLES, EXPORTED_TEXTS,
};
use sqlbuild_cli::launcher::main::install_native_build_identity::install_native_build_identity;
use sqlbuild_python_base::boundary::main::compiler_error::compiler_error;
use sqlbuild_python_base::records::main::fields::field::field as native_field;

/// Registers after the native enums, since `GRAIN_ORDER` is keyed by `CursorGrain` members.
pub(crate) fn register(module: &Bound<'_, PyModule>) -> PyResult<()> {
    let py = module.py();
    let identity: &str = install_native_build_identity().map_err(compiler_error)?;
    module.add("BUILD_IDENTITY", identity)?;
    for (name, value) in EXPORTED_TEXTS {
        module.add(*name, *value)?;
    }
    for (name, value) in EXPORTED_INTEGERS {
        module.add(*name, *value)?;
    }
    for (name, values) in EXPORTED_TEXT_TUPLES {
        module.add(*name, PyTuple::new(py, values.iter())?)?;
    }
    for (name, values) in EXPORTED_TEXT_SETS {
        module.add(*name, PyFrozenSet::new(py, values.iter())?)?;
    }
    let grain = native_field(module, "CursorGrain")?;
    let order = PyDict::new(py);
    for (name, rank) in GRAIN_NAMES {
        order.set_item(grain.call1((name,))?, rank as usize)?;
    }
    module.add("GRAIN_ORDER", order)
}
