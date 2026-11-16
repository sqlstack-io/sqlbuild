//! Public records of project assembly.

use pyo3::prelude::Bound;
use pyo3::prelude::Py;
use pyo3::prelude::PyAny;
use pyo3::pyclass;
use pyo3::types::PyDict;
use sqlbuild_analysis::assembly::analysis_session::types::Pairs;
use sqlbuild_analysis::assembly::analysis_session::types::Shapes;
use std::collections::HashMap;

/// The three per-relation declaration maps the analysis starts from, and the declared shapes.
#[derive(Debug)]
pub struct DeclaredRelations<'py> {
    pub column_types: Bound<'py, PyDict>,
    pub nullability: Bound<'py, PyDict>,
    pub complete_schemas: Bound<'py, PyDict>,
    pub declared: DeclaredShapes,
}

/// Relation shapes as rows in a dict's insertion order, each value as `str(value)`.
#[derive(Debug, Clone, Default)]
pub struct ShapeRows {
    pub(crate) rows: Shapes,
    pub(crate) positions: HashMap<String, usize>,
}

impl ShapeRows {
    /// `shapes[name] = pairs`, keeping an existing name's position as a dict does.
    pub fn set(&mut self, name: String, pairs: Pairs) {
        match self.positions.get(&name) {
            Some(&position) => self.rows[position].1 = pairs,
            None => {
                let _ = self.positions.insert(name.clone(), self.rows.len());
                self.rows.push((name, pairs));
            }
        }
    }

    /// The rows in insertion order.
    pub fn rows(&self) -> &Shapes {
        &self.rows
    }
}

/// The native rows of the declared types, nullability and authoritative schemas.
#[derive(Debug, Clone, Default)]
pub struct DeclaredShapes {
    pub types: ShapeRows,
    pub nullability: ShapeRows,
    pub complete: ShapeRows,
}

/// Per-resource facts in input order, and whether each model's SQL passed syntax validation.
#[pyclass(module = "sqlbuild._native", frozen, get_all)]
#[derive(Debug)]
pub struct NativeProjectResources {
    pub model_deps: Py<PyAny>,
    pub model_syntax_valid: Py<PyAny>,
    pub source_namespaces: Py<PyAny>,
    pub seed_namespaces: Py<PyAny>,
    pub function_deps: Py<PyAny>,
    pub audit_scope_deps: Py<PyAny>,
    /// The `NativeResourceFailure` Python's assembly raises at, or `None`.
    pub failure: Py<PyAny>,
}

/// The resource Python's assembly raises at, and the exception it raises there.
#[pyclass(module = "sqlbuild._native", frozen, get_all)]
#[derive(Debug)]
pub struct NativeResourceFailure {
    pub slot: Py<PyAny>,
    pub index: Py<PyAny>,
    pub error: Py<PyAny>,
}

/// One model's native facts; its dependencies as `(resource type, name)` rows.
#[pyclass(module = "sqlbuild._native", frozen, get_all)]
#[derive(Debug)]
pub struct NativeModelFacts {
    pub deps: Py<PyAny>,
    pub dynamic_contract: Py<PyAny>,
    pub syntax_valid: Py<PyAny>,
}

/// Every model's native facts and the resource facts.
#[pyclass(module = "sqlbuild._native", frozen, get_all)]
#[derive(Debug)]
pub struct NativeProjectFacts {
    pub models: Py<PyAny>,
    pub resources: Py<PyAny>,
}
