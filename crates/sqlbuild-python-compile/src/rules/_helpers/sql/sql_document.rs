//! Custom rules' SQL documents: the SQLQuack syntax tree of one SQL text and its common structures.

use std::sync::Arc;

use pyo3::prelude::{Bound, Py, PyAny, PyErr, PyResult, Python};
use pyo3::sync::PyOnceLock;
use pyo3::types::PyTuple;
use pyo3::{IntoPyObject, pyclass, pymethods};
use sqlbuild_analysis::sql_tree::errors::SqlTreeError;
use sqlbuild_analysis::sql_tree::main::sql_tree_nodes::sql_tree_nodes;
use sqlbuild_analysis::sql_tree::models::{
    NodeExtent, NodeLiteral, NodeName, SqlTreeNode as TreeNode,
};

use sqlbuild_python_base::records::main::construct::new_annotated_error::new_annotated_error;
use sqlbuild_python_base::records::main::construct::new_record_with::new_record_with;
use sqlbuild_python_base::records::types::ErrorId;
use sqlbuild_python_base::records::types::RecordId;

/// The kind of the root node that holds a document's statements.
const DOCUMENT_KIND: &str = "document";
/// The kind of an aggregate call, whose star argument, as in `COUNT(*)`, expands no columns.
const AGGREGATE_KIND: &str = "aggregate_function";

/// A parsed document's nodes and each node's children, shared by every node read from it.
struct Tree {
    nodes: Vec<TreeNode>,
    /// Child indexes by parent index; the last entry holds the statements, the document's children.
    children: Vec<Vec<usize>>,
}

/// One node of a rule's SQL syntax tree: an index into its shared tree, so freeing never recurses.
#[pyclass(module = "sqlbuild._native", frozen)]
pub(crate) struct SqlTreeNode {
    tree: Arc<Tree>,
    index: usize,
}

#[pymethods]
impl SqlTreeNode {
    #[getter]
    fn kind(&self) -> &'static str {
        self.tree
            .nodes
            .get(self.index)
            .map_or(DOCUMENT_KIND, |node| node.kind)
    }

    #[getter]
    fn line(&self) -> usize {
        self.tree.nodes.get(self.index).map_or(1, |node| node.line)
    }

    #[getter]
    fn column(&self) -> usize {
        self.tree
            .nodes
            .get(self.index)
            .map_or(1, |node| node.column)
    }

    /// The name an identifier, column, alias, table or CTE carries, unquoted; a relation marker's is its call text.
    #[getter]
    fn name(&self) -> Option<&str> {
        self.node_name().map(|name| name.name.as_str())
    }

    /// Whether the source quoted the name; `None` without one.
    #[getter]
    fn quoted(&self) -> Option<bool> {
        self.node_name().map(|name| name.quoted)
    }

    /// The names qualifying the name, outermost first, such as `("db", "sales")`; `None` without a name.
    #[getter]
    fn qualifier<'py>(&self, py: Python<'py>) -> PyResult<Option<Bound<'py, PyTuple>>> {
        self.node_name()
            .map(|name| PyTuple::new(py, &name.qualifier))
            .transpose()
    }

    /// A literal's value as written, without quotes.
    #[getter]
    fn value(&self) -> Option<&str> {
        self.node_literal().map(|literal| literal.value.as_str())
    }

    /// A literal's snake_case type, such as `string`, `number` or `date`.
    #[getter]
    fn literal_type(&self) -> Option<&'static str> {
        self.node_literal().map(|literal| literal.literal_type)
    }

    /// The 1-based line just after the node's last character; `None` when the parser recorded no span.
    #[getter]
    fn end_line(&self) -> Option<usize> {
        self.extent().map(|extent| extent.end_line)
    }

    /// The 1-based column just after the node's last character; `None` when the parser recorded no span.
    #[getter]
    fn end_column(&self) -> Option<usize> {
        self.extent().map(|extent| extent.end_column)
    }

    /// The node's `(start, end)` character offsets into the document's source, end exclusive.
    #[getter]
    fn span(&self) -> Option<(usize, usize)> {
        self.extent().map(|extent| extent.span)
    }

    /// The node's direct children, in SQLQuack's traversal order.
    #[getter]
    fn children<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyTuple>> {
        let children: PyResult<Vec<Py<SqlTreeNode>>> = self.tree.children[self.index]
            .iter()
            .map(|&index| tree_node(py, &self.tree, index))
            .collect();
        PyTuple::new(py, children?)
    }

    /// This node and every descendant, in pre-order.
    fn walk(&self, py: Python<'_>) -> PyResult<Vec<Py<SqlTreeNode>>> {
        let mut walked: Vec<Py<SqlTreeNode>> = Vec::new();
        let mut stack: Vec<usize> = vec![self.index];
        while let Some(index) = stack.pop() {
            stack.extend(self.tree.children[index].iter().rev());
            walked.push(tree_node(py, &self.tree, index)?);
        }
        Ok(walked)
    }

    fn __repr__(&self) -> String {
        format!(
            "SqlTreeNode(kind={:?}, line={}, column={})",
            self.kind(),
            self.line(),
            self.column()
        )
    }
}

impl SqlTreeNode {
    /// The node's tree record; `None` for the document root.
    fn record(&self) -> Option<&TreeNode> {
        self.tree.nodes.get(self.index)
    }

    fn node_name(&self) -> Option<&NodeName> {
        self.record().and_then(|node| node.name.as_ref())
    }

    fn node_literal(&self) -> Option<&NodeLiteral> {
        self.record().and_then(|node| node.literal.as_ref())
    }

    fn extent(&self) -> Option<&NodeExtent> {
        self.record().and_then(|node| node.extent.as_ref())
    }
}

/// One SQL text a custom rule reads, parsed on first use by its dialect's SQLQuack parser.
#[pyclass(module = "sqlbuild._native", frozen)]
pub(crate) struct SqlDocument {
    #[pyo3(get)]
    source: String,
    dialect: String,
    tree: PyOnceLock<Arc<Tree>>,
}

#[pymethods]
impl SqlDocument {
    #[new]
    #[pyo3(signature = (*, source, dialect))]
    fn new(source: String, dialect: String) -> Self {
        Self {
            source,
            dialect,
            tree: PyOnceLock::new(),
        }
    }

    /// The syntax tree: a `document` node whose children are the statements.
    fn ast(&self, py: Python<'_>) -> PyResult<Py<SqlTreeNode>> {
        let tree: &Arc<Tree> = self.parsed(py)?;
        tree_node(py, tree, tree.nodes.len())
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
        let nodes: &[TreeNode] = &self.parsed(py)?.nodes;
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

    /// The parsed tree; a failure raises `RuleUsageError` each time it is read.
    fn parsed(&self, py: Python<'_>) -> PyResult<&Arc<Tree>> {
        self.tree.get_or_try_init(py, || {
            py.detach(|| sql_tree_nodes(&self.source, &self.dialect).map(tree))
                .map_err(|error| usage_error(py, &error))
        })
    }
}

/// `nodes` with each node's children listed, in ascending index order, under its parent.
fn tree(nodes: Vec<TreeNode>) -> Arc<Tree> {
    let mut children: Vec<Vec<usize>> = vec![Vec::new(); nodes.len() + 1];
    for (index, node) in nodes.iter().enumerate() {
        children[node.parent.unwrap_or(nodes.len())].push(index);
    }
    Arc::new(Tree { nodes, children })
}

/// The Python node at `index` of `tree`; `tree.nodes.len()` is the document root.
fn tree_node(py: Python<'_>, tree: &Arc<Tree>, index: usize) -> PyResult<Py<SqlTreeNode>> {
    Py::new(
        py,
        SqlTreeNode {
            tree: Arc::clone(tree),
            index,
        },
    )
}

/// `RuleUsageError(message)` for a document without a tree.
fn usage_error(py: Python<'_>, error: &SqlTreeError) -> PyErr {
    new_annotated_error(py, ErrorId::RuleUsage, error.to_string(), &[])
}
