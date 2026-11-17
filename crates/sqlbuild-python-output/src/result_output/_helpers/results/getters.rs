//! Direct `#[getter]` reads of this module's native pyclasses, installed into `records::getters`.

use pyo3::IntoPyObjectExt;
use pyo3::prelude::{Bound, PyAny, PyResult, Python};

use crate::result_output::_helpers::events::integration_envelope::IntegrationAssetResult;
use crate::result_output::_helpers::events::integration_envelope::IntegrationCheckResult;
use crate::result_output::_helpers::events::integration_envelope::IntegrationResultEnvelope;
use crate::result_output::_helpers::events::terminal_index::TerminalEventClaim;

pub(crate) fn read_integration_asset_result<'py>(
    value: &Bound<'py, PyAny>,
    name: &str,
) -> Option<PyResult<Bound<'py, PyAny>>> {
    let py: Python<'py> = value.py();
    let Ok(cell) = value.cast::<IntegrationAssetResult>() else {
        return None;
    };
    let Ok(this) = cell.try_borrow() else {
        return None;
    };
    Some(match name {
        "as_dict" => return None,
        _ => this.__getattr__(py, name).map(|value| value.into_bound(py)),
    })
}

pub(crate) fn read_integration_check_result<'py>(
    value: &Bound<'py, PyAny>,
    name: &str,
) -> Option<PyResult<Bound<'py, PyAny>>> {
    let py: Python<'py> = value.py();
    let Ok(cell) = value.cast::<IntegrationCheckResult>() else {
        return None;
    };
    let Ok(this) = cell.try_borrow() else {
        return None;
    };
    Some(match name {
        "as_dict" | "decoded" | "parsed" => return None,
        _ => this.__getattr__(py, name).map(|value| value.into_bound(py)),
    })
}

pub(crate) fn read_integration_result_envelope<'py>(
    value: &Bound<'py, PyAny>,
    name: &str,
) -> Option<PyResult<Bound<'py, PyAny>>> {
    let py: Python<'py> = value.py();
    let Ok(cell) = value.cast::<IntegrationResultEnvelope>() else {
        return None;
    };
    let Ok(this) = cell.try_borrow() else {
        return None;
    };
    Some(match name {
        "asset" => this.asset(py).into_bound_py_any(py),
        "checks" => this.checks(py).into_bound_py_any(py),
        "from_json" | "from_json_without_optional_enrichment" | "register" | "to_json" => {
            return None;
        }
        _ => this.__getattr__(py, name).map(|value| value.into_bound(py)),
    })
}

pub(crate) fn read_terminal_event_claim<'py>(
    value: &Bound<'py, PyAny>,
    name: &str,
) -> Option<PyResult<Bound<'py, PyAny>>> {
    let py: Python<'py> = value.py();
    let Ok(cell) = value.cast::<TerminalEventClaim>() else {
        return None;
    };
    let Ok(this) = cell.try_borrow() else {
        return None;
    };
    Some(match name {
        "terminal" => (&this.terminal).into_bound_py_any(py),
        "event_sequence" => (&this.event_sequence).into_bound_py_any(py),
        _ => return None,
    })
}
