//! Executed-statement effects on the executor's cached relation metadata.

use pyo3::prelude::{Bound, Py, PyAny, PyModule, PyModuleMethods, PyResult, Python};
use pyo3::sync::PyOnceLock;
use pyo3::types::{PyBool, PyFrozenSet};
use pyo3::{pyfunction, wrap_pyfunction};
use sqlbuild_host::entries::models::AdapterHost;
use sqlbuild_sqltext::ddl_effects::main::qualified_relation_name_key::qualified_relation_name_key as native_qualified_relation_name_key;
use sqlbuild_sqltext::ddl_effects::main::relation_name_key::relation_name_key as native_relation_name_key;
use sqlbuild_sqltext::ddl_effects::main::statement_metadata_effect::statement_metadata_effect as native_statement_metadata_effect;
use sqlbuild_sqltext::ddl_effects::models::StatementMetadataEffect;

use sqlbuild_python_base::boundary::main::call_with::call_with;
use sqlbuild_python_base::native_host::main::host_entry::host_entry;

const EFFECT_MODEL: AdapterHost = AdapterHost::StatementMetadataEffect;

static EFFECT_CLASS: PyOnceLock<Py<PyAny>> = PyOnceLock::new();

/// Which cached relation names `sql` may have changed, as a `StatementMetadataEffect`.
#[pyfunction]
#[pyo3(signature = (sql, /))]
fn statement_metadata_effect<'py>(py: Python<'py>, sql: &str) -> PyResult<Bound<'py, PyAny>> {
    let effect: StatementMetadataEffect = py.detach(|| native_statement_metadata_effect(sql));
    let class: &Bound<'py, PyAny> = EFFECT_CLASS
        .get_or_try_init(py, || host_entry(py, EFFECT_MODEL).map(Bound::unbind))?
        .bind(py);
    call_with(
        class,
        &[
            (
                "relation_names",
                PyFrozenSet::new(py, &effect.relation_names)?.into_any(),
            ),
            (
                "invalidates_all",
                PyBool::new(py, effect.invalidates_all)
                    .to_owned()
                    .into_any(),
            ),
            (
                "ends_transaction",
                PyBool::new(py, effect.ends_transaction)
                    .to_owned()
                    .into_any(),
            ),
        ],
    )
}

/// The invalidation key for one unquoted or quoted unqualified relation name.
#[pyfunction]
#[pyo3(signature = (name, /))]
fn relation_name_key(name: &str) -> String {
    native_relation_name_key(name)
}

/// The key for the last part of a qualified name, or `None` when it cannot be parsed.
#[pyfunction]
#[pyo3(signature = (qualified, /))]
fn qualified_relation_name_key(qualified: &str) -> Option<String> {
    native_qualified_relation_name_key(qualified)
}

pub(crate) fn register(module: &Bound<'_, PyModule>) -> PyResult<()> {
    module.add_function(wrap_pyfunction!(statement_metadata_effect, module)?)?;
    module.add_function(wrap_pyfunction!(relation_name_key, module)?)?;
    module.add_function(wrap_pyfunction!(qualified_relation_name_key, module)?)?;
    Ok(())
}
