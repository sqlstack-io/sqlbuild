//! The adapter nullability rules as `(name, rule id)` rows, and the adapter's own rules.

use crate::analysis_session::_helpers::profile_inputs::AdapterRules;
use crate::analysis_session::_helpers::profile_inputs::KNOWN_RULES;
use crate::analysis_session::constants::ADAPTER_RULE_ID;
use pyo3::prelude::{Bound, PyAny, PyAnyMethods, PyResult, Python};
use pyo3::types::{PyDict, PyDictMethods};
use sqlbuild_analysis::assembly::analysis_session::types::Pairs;
use sqlbuild_python_base::native_host::main::host_entry::host_entry;
use sqlbuild_python_base::records::main::construct::record_class::record_class;
use sqlbuild_python_base::records::main::fields::field::field as native_field;
use sqlbuild_python_base::records::types::RecordId;

/// The adapter nullability rules as `(name, rule id)` rows, and the adapter's own rules.
pub fn nullability_rules(profile: &Bound<'_, PyAny>) -> PyResult<(Pairs, AdapterRules)> {
    let py: Python<'_> = profile.py();
    let known: Bound<'_, PyDict> = PyDict::new(py);
    for (rule, id) in KNOWN_RULES {
        known.set_item(host_entry(py, rule)?, id)?;
    }
    let adapter: Bound<'_, PyDict> = PyDict::new(py);
    let mut rows: Pairs = Vec::new();
    for item in native_field(profile, "function_nullability_rules")?
        .call_method0("items")?
        .try_iter()?
    {
        let (name, rule): (Bound<'_, PyAny>, Bound<'_, PyAny>) = item?.extract()?;
        let id: String = match known.get_item(&rule)? {
            Some(id) => id.extract()?,
            None => {
                adapter.set_item(&name, &rule)?;
                ADAPTER_RULE_ID.to_owned()
            }
        };
        rows.push((name.extract()?, id));
    }
    if adapter.is_empty() {
        return Ok((rows, None));
    }
    let nullability: Bound<'_, PyAny> = record_class(py, RecordId::InferredNullability)?;
    Ok((rows, Some((adapter.unbind(), nullability.unbind()))))
}
