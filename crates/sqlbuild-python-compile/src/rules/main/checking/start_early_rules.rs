//! Start early built-in rules over `project` when armed; only non-`Exception` errors propagate.

use crate::rules::_helpers::checking::early_builtin_rules::BUILD_PHASE;
use crate::rules::_helpers::checking::early_builtin_rules::EarlyBuiltinRules;
use crate::rules::_helpers::checking::early_builtin_rules::SLOT;
use crate::rules::_helpers::checking::early_builtin_rules::Slot;
use crate::rules::_helpers::checking::early_builtin_rules::early_request;
use crate::rules::models::EarlyRulesSettings;
use pyo3::exceptions::PyException;
use pyo3::prelude::{Bound, PyAny, PyResult, Python};
use sqlbuild_host::compile_context::main::record_compile_timing::record_compile_timing;

/// Start early built-in rules over `project` when armed; only non-`Exception` errors propagate.
pub fn start_early_rules(project: &Bound<'_, PyAny>) -> PyResult<()> {
    let settings: Option<EarlyRulesSettings> = SLOT.with(|slot| {
        let mut slot = slot.borrow_mut();
        match slot.as_mut() {
            Some(current @ Slot::Armed(_)) => match std::mem::replace(current, Slot::Spent) {
                Slot::Armed(settings) => Some(settings),
                _ => None,
            },
            _ => None,
        }
    });
    let Some(settings) = settings else {
        return Ok(());
    };
    let py: Python<'_> = project.py();
    let started: Option<EarlyBuiltinRules> =
        match record_compile_timing(BUILD_PHASE, || early_request(project, &settings)) {
            Ok(Some(rows)) => EarlyBuiltinRules::spawn(rows),
            Ok(None) => None,
            Err(error) if error.is_instance_of::<PyException>(py) => None,
            Err(error) => return Err(error),
        };
    if let Some(started) = started {
        SLOT.with(|slot| {
            if let Some(current) = slot.borrow_mut().as_mut() {
                *current = Slot::Started(started);
            }
        });
    }
    Ok(())
}
