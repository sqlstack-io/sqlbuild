//! Resolve function sql.

use crate::planner_resolution::models::Resolution;
use pyo3::prelude::{Bound, PyAny, PyAnyMethods, PyResult, Python};
use pyo3::pyfunction;
use pyo3::types::{PyDict, PyString};
use sqlbuild_python_base::boundary::models::KeywordArguments;
use sqlbuild_python_project::resource_views::main::function_reads::body_sql::body_sql;

#[pyfunction]
#[pyo3(signature = (**kwargs))]
pub fn resolve_function_sql<'py>(
    py: Python<'py>,
    kwargs: Option<Bound<'py, PyDict>>,
) -> PyResult<Bound<'py, PyAny>> {
    let args = KeywordArguments::read(
        py,
        "resolve_function_sql",
        kwargs,
        (
            &[
                "adapter",
                "function",
                "model_locations",
                "seed_locations",
                "function_locations",
                "source_map",
                "source_warehouse_columns",
                "star_exclude_keyword",
            ],
            &[],
        ),
    )?;
    let resolution = Resolution {
        adapter: args.value("adapter")?,
        external: py.None().into_bound(py),
        source_map: args.value("source_map")?,
        source_warehouse_columns: args.value("source_warehouse_columns")?,
        model_locations: args.value("model_locations")?,
        seed_locations: args.value("seed_locations")?,
        function_locations: args.value("function_locations")?,
    };
    let function = args.value("function")?;
    let name: String =
        sqlbuild_python_project::resource_views::main::function_reads::name::name(&function)?
            .extract()?;
    let sql: String = resolution.unbounded(body_sql(&function)?, &format!("Function '{name}'"))?;
    Ok(PyString::new(py, &sql).into_any())
}
