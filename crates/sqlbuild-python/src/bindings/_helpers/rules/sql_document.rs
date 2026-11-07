//! Custom rules' SQL documents: the SQLQuack syntax tree of one SQL text and its common structures.

use pyo3::prelude::{Bound, Py, PyAny, PyErr, PyModule, PyModuleMethods, PyResult, Python};
use pyo3::sync::PyOnceLock;
use pyo3::types::PyTuple;
use pyo3::{IntoPyObject, pyclass, pymethods};
use sqlbuild_analysis::sql_tree::errors::SqlTreeError;
use sqlbuild_analysis::sql_tree::main::sql_tree_nodes::sql_tree_nodes;
use sqlbuild_analysis::sql_tree::models::SqlTreeNode as TreeNode;

use crate::bindings::_helpers::records::construct::{new_annotated_error, new_record_with};
use crate::bindings::_helpers::records::ids::{ErrorId, RecordId};

/// The kind of the root node that holds a document's statements.
const DOCUMENT_KIND: &str = "document";
/// The kind of an aggregate call, whose star argument, as in `COUNT(*)`, expands no columns.
const AGGREGATE_KIND: &str = "aggregate_function";

/// One node of a rule's SQL syntax tree.
#[pyclass(module = "sqlbuild._native", frozen)]
pub(crate) struct SqlTreeNode {
    #[pyo3(get)]
    kind: &'static str,
    #[pyo3(get)]
    line: usize,
    #[pyo3(get)]
    column: usize,
    children: Vec<Py<SqlTreeNode>>,
}

#[pymethods]
impl SqlTreeNode {
    /// The node's direct children, in SQLQuack's traversal order.
    #[getter]
    fn children<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyTuple>> {
        PyTuple::new(py, self.children.iter().map(|child| child.clone_ref(py)))
    }

    /// This node and every descendant, in pre-order.
    fn walk(slf: Bound<'_, Self>) -> PyResult<Vec<Py<SqlTreeNode>>> {
        let py: Python<'_> = slf.py();
        let mut walked: Vec<Py<SqlTreeNode>> = Vec::new();
        let mut stack: Vec<Py<SqlTreeNode>> = vec![slf.unbind()];
        while let Some(node) = stack.pop() {
            stack.extend(
                node.get()
                    .children
                    .iter()
                    .rev()
                    .map(|child| child.clone_ref(py)),
            );
            walked.push(node);
        }
        Ok(walked)
    }

    fn __repr__(&self) -> String {
        format!(
            "SqlTreeNode(kind={:?}, line={}, column={})",
            self.kind, self.line, self.column
        )
    }
}

/// One SQL text a custom rule reads, parsed on first use by its dialect's SQLQuack parser.
#[pyclass(module = "sqlbuild._native", frozen)]
pub(crate) struct SqlDocument {
    #[pyo3(get)]
    source: String,
    dialect: String,
    nodes: PyOnceLock<Vec<TreeNode>>,
    tree: PyOnceLock<Py<SqlTreeNode>>,
}

#[pymethods]
impl SqlDocument {
    #[new]
    #[pyo3(signature = (*, source, dialect))]
    fn new(source: String, dialect: String) -> Self {
        Self {
            source,
            dialect,
            nodes: PyOnceLock::new(),
            tree: PyOnceLock::new(),
        }
    }

    /// The syntax tree: a `document` node whose children are the statements.
    fn ast(&self, py: Python<'_>) -> PyResult<Py<SqlTreeNode>> {
        let tree = self.tree.get_or_try_init(py, || {
            let nodes: &[TreeNode] = self.parsed(py)?;
            python_tree(py, nodes)
        })?;
        Ok(tree.clone_ref(py))
    }

    /// Every node of `kind`, compared without case, as a stable source location.
    #[pyo3(signature = (*, kind))]
    fn nodes<'py>(&self, py: Python<'py>, kind: &str) -> PyResult<Bound<'py, PyTuple>> {
        self.source_nodes(py, kind, |_, _| true)
    }

    /// Every common table expression.
    fn ctes<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyTuple>> {
        self.nodes(py, "CTE")
    }

    /// Every join clause.
    fn joins<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyTuple>> {
        self.nodes(py, "Join")
    }

    /// Every parenthesized subquery.
    fn subqueries<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyTuple>> {
        self.nodes(py, "Subquery")
    }

    /// Every projected star, bare or qualified; a star a function call takes, such as `COUNT(*)`, is no projection.
    fn star_projections<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyTuple>> {
        self.source_nodes(py, "Star", |nodes, node| {
            node.parent
                .is_none_or(|parent| nodes[parent].kind != AGGREGATE_KIND)
        })
    }
}

impl SqlDocument {
    /// The nodes of `kind` that `keep` accepts, labelled `kind`, as `SqlNode` locations.
    fn source_nodes<'py>(
        &self,
        py: Python<'py>,
        kind: &str,
        keep: impl Fn(&[TreeNode], &TreeNode) -> bool,
    ) -> PyResult<Bound<'py, PyTuple>> {
        let wanted: String = kind.to_lowercase();
        let nodes: &[TreeNode] = self.parsed(py)?;
        let mut found: Vec<Bound<'py, PyAny>> = Vec::new();
        for node in nodes
            .iter()
            .filter(|node| node.kind == wanted && keep(nodes, node))
        {
            found.push(new_record_with(
                py,
                RecordId::SqlNode,
                &[
                    ("kind", kind.into_pyobject(py)?.into_any()),
                    ("line", node.line.into_pyobject(py)?.into_any()),
                    ("column", node.column.into_pyobject(py)?.into_any()),
                ],
            )?);
        }
        PyTuple::new(py, found)
    }

    /// The parsed nodes in pre-order; a failure raises `RuleUsageError` each time it is read.
    fn parsed(&self, py: Python<'_>) -> PyResult<&[TreeNode]> {
        self.nodes
            .get_or_try_init(py, || {
                py.detach(|| sql_tree_nodes(&self.source, &self.dialect))
                    .map_err(|error| usage_error(py, &error))
            })
            .map(Vec::as_slice)
    }
}

/// The Python tree of `nodes`, built from the last node back so children exist before parents.
fn python_tree(py: Python<'_>, nodes: &[TreeNode]) -> PyResult<Py<SqlTreeNode>> {
    let mut children: Vec<Vec<Py<SqlTreeNode>>> = (0..nodes.len()).map(|_| Vec::new()).collect();
    let mut statements: Vec<Py<SqlTreeNode>> = Vec::new();
    for (index, node) in nodes.iter().enumerate().rev() {
        let mut own: Vec<Py<SqlTreeNode>> = std::mem::take(&mut children[index]);
        own.reverse();
        let built: Py<SqlTreeNode> = Py::new(
            py,
            SqlTreeNode {
                kind: node.kind,
                line: node.line,
                column: node.column,
                children: own,
            },
        )?;
        match node.parent {
            Some(parent) => children[parent].push(built),
            None => statements.push(built),
        }
    }
    statements.reverse();
    Py::new(
        py,
        SqlTreeNode {
            kind: DOCUMENT_KIND,
            line: 1,
            column: 1,
            children: statements,
        },
    )
}

/// `RuleUsageError(message)` for a document without a tree.
fn usage_error(py: Python<'_>, error: &SqlTreeError) -> PyErr {
    new_annotated_error(py, ErrorId::RuleUsage, error.to_string(), &[])
}

pub(crate) fn register(module: &Bound<'_, PyModule>) -> PyResult<()> {
    module.add_class::<SqlDocument>()?;
    module.add_class::<SqlTreeNode>()?;
    Ok(())
}
