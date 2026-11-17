//! Register the exported native constants and the cursor grain order on `sqlbuild._native`.

use pyo3::prelude::{Bound, PyAnyMethods, PyModule, PyModuleMethods, PyResult};
use pyo3::types::{PyDict, PyDictMethods, PyFrozenSet, PyTuple};

use crate::bindings::_helpers::exported_constants::constants::EXPORTED_INTEGERS;
use crate::bindings::_helpers::exported_constants::constants::EXPORTED_TEXT_SETS;
use crate::bindings::_helpers::exported_constants::constants::EXPORTED_TEXT_TUPLES;
use crate::bindings::_helpers::exported_constants::constants::EXPORTED_TEXTS;
use sqlbuild_python_base::records::main::fields::field::field as native_field;
use sqlbuild_python_front::cursor::constants::GRAIN_NAMES;

/// Registers after the native enums, since `GRAIN_ORDER` is keyed by `CursorGrain` members.
pub(crate) fn register(module: &Bound<'_, PyModule>) -> PyResult<()> {
    let py = module.py();
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
