//! How a plan writes SQL for its warehouse: the adapter's syntax, with no way to reach the warehouse.

use pyo3::exceptions::PyAttributeError;
use pyo3::prelude::{Bound, Py, PyAny, PyAnyMethods, PyResult, Python, pyclass, pymethods};

/// Adapter attributes that only write SQL or describe the dialect, by prefix.
const SYNTAX_PREFIXES: [&str; 3] = ["render_", "_render_", "supports_"];
/// Adapter attributes that only describe the dialect or its defaults; none takes a connection.
const SYNTAX_ATTRIBUTES: [&str; 33] = [
    "adapter_name",
    "allows_implicit_managed_write_schema",
    "build_cursor_filter",
    "collection_rendering",
    "default_database",
    "default_promotion_strategy",
    "default_schema",
    "default_table_promotion_mode",
    "expression_inference_profile",
    "format_row_diff_decimal_sql",
    "lists_relations_authoritatively",
    "max_identifier_length",
    "max_statement_size",
    "maximum_identifier_length",
    "normalize_row_diff_numeric_type",
    "persists_python_functions",
    "python_functions_inherit_default_namespace",
    "relation_grants_supported",
    "relation_names_match",
    "requires_derived_table_aliases",
    "retention_state_from_relation",
    "sql_analysis_dialect",
    "sql_analysis_dialect_name",
    "sql_lexical_syntax",
    "star_exclude_keyword",
    "state_timestamp_type",
    "table_fn",
    "table_type_entries",
    "table_type_downgrade",
    "untyped_column_type",
    "value_renderer",
    "views_bind_to_relation_identity",
    "views_read_with_reader_access",
];

/// The adapter's SQL syntax for a plan; anything that would reach the warehouse is refused.
#[pyclass(frozen, module = "sqlbuild._native", name = "Dialect")]
pub(crate) struct Dialect {
    adapter: Py<PyAny>,
}

/// Whether `name` only writes SQL or describes the dialect.
fn syntax(name: &str) -> bool {
    SYNTAX_PREFIXES
        .iter()
        .any(|prefix| name.starts_with(prefix))
        || SYNTAX_ATTRIBUTES.contains(&name)
}

impl Dialect {
    /// The dialect of `adapter`.
    pub(crate) fn of<'py>(adapter: &Bound<'py, PyAny>) -> PyResult<Bound<'py, Dialect>> {
        Bound::new(
            adapter.py(),
            Dialect {
                adapter: adapter.clone().unbind(),
            },
        )
    }

    /// A dialect passed as a keyword argument; an adapter or anything else is refused.
    pub(crate) fn passed(value: Bound<'_, PyAny>) -> PyResult<Bound<'_, Dialect>> {
        Ok(value.cast_into::<Dialect>()?)
    }

    /// One syntax attribute of the adapter, such as `render_qualified_name`.
    pub(crate) fn attribute<'py>(
        &self,
        py: Python<'py>,
        name: &str,
    ) -> PyResult<Bound<'py, PyAny>> {
        if !syntax(name) {
            return Err(PyAttributeError::new_err(format!(
                "a plan's SQL dialect writes SQL only; `{name}` would reach the warehouse"
            )));
        }
        self.adapter.bind(py).getattr(name)
    }
}

/// Reading one syntax attribute of a bound dialect.
pub(crate) trait Syntax<'py> {
    /// One syntax attribute, such as `render_qualified_name`; anything reaching the warehouse is refused.
    fn syntax(&self, name: &str) -> PyResult<Bound<'py, PyAny>>;
}

impl<'py> Syntax<'py> for Bound<'py, Dialect> {
    fn syntax(&self, name: &str) -> PyResult<Bound<'py, PyAny>> {
        self.get().attribute(self.py(), name)
    }
}

#[pymethods]
impl Dialect {
    #[new]
    #[pyo3(signature = (adapter))]
    fn new(adapter: Bound<'_, PyAny>) -> Self {
        Dialect {
            adapter: adapter.unbind(),
        }
    }

    /// Python code handed a dialect reads the same syntax attributes; anything else is refused.
    fn __getattr__<'py>(&self, py: Python<'py>, name: &str) -> PyResult<Bound<'py, PyAny>> {
        self.attribute(py, name)
    }
}
