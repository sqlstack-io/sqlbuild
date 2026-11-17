//! Custom rules' SQL documents: the SQLQuack syntax tree of one SQL text and its common structures.

use std::collections::HashMap;
use std::sync::Arc;

use pyo3::exceptions::{PyAttributeError, PyValueError};
use pyo3::prelude::{Bound, Py, PyAny, PyAnyMethods, PyErr, PyResult, Python};
use pyo3::sync::PyOnceLock;
use pyo3::types::PyTuple;
use pyo3::{IntoPyObject, pyclass, pymethods};
use sqlbuild_analysis::sql_tree::errors::SqlTreeError;
use sqlbuild_analysis::sql_tree::main::sql_tree_nodes::sql_tree_nodes;
use sqlbuild_analysis::sql_tree::models::{
    NodeExtent, NodeLiteral, NodeName, SqlTreeNode as TreeNode,
};
use sqlbuild_rules::engine::main::query_file_positions::query_file_positions;
use sqlbuild_rules::models::ModelQueryPlace;

use sqlbuild_python_base::records::main::construct::new_annotated_error::new_annotated_error;
use sqlbuild_python_base::records::main::construct::new_record_with::new_record_with;
use sqlbuild_python_base::records::main::fields::field::field as native_field;
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

/// A model query's character offsets and the file positions of its tree's node starts, built once.
struct FilePositions {
    /// The character each line starts at, then the source's length.
    line_starts: Vec<usize>,
    /// The file `(line, column)` of every tree node's first character, by that character.
    node_starts: HashMap<usize, (u64, u64)>,
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
    /// Where `source`, a model's expanded query, sits in the model file; `None` for other SQL.
    place: Option<ModelQueryPlace>,
    /// The model file `place` points into.
    file: Option<String>,
    tree: PyOnceLock<Arc<Tree>>,
    positions: PyOnceLock<FilePositions>,
}

#[pymethods]
impl SqlDocument {
    #[new]
    #[pyo3(signature = (*, source, dialect, place=None, file=None))]
    fn new(
        source: String,
        dialect: String,
        place: Option<&str>,
        file: Option<String>,
    ) -> PyResult<Self> {
        let place: Option<ModelQueryPlace> = place
            .map(serde_json::from_str)
            .transpose()
            .map_err(|error| PyValueError::new_err(format!("invalid query place: {error}")))?;
        Ok(Self {
            source,
            dialect,
            place,
            file,
            tree: PyOnceLock::new(),
            positions: PyOnceLock::new(),
        })
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

    /// Where `node`, read from this model query, is in the model file; `None` for other SQL.
    #[pyo3(signature = (*, node))]
    fn file_position(&self, node: &Bound<'_, PyAny>) -> PyResult<Option<(u64, u64)>> {
        let (Some(place), Some(file)) = (self.place.as_ref(), self.file.as_deref()) else {
            return Ok(None);
        };
        let (Some(line), Some(column)) = (node_index(node, "line")?, node_index(node, "column")?)
        else {
            return Ok(None);
        };
        let positions: &FilePositions = self
            .positions
            .get_or_init(node.py(), || self.file_positions(node.py(), (file, place)));
        let character: usize = positions.character(line, column);
        Ok(Some(
            positions
                .node_starts
                .get(&character)
                .copied()
                .unwrap_or_else(|| {
                    query_file_positions(&self.source, (file, place), &[character])[0]
                }),
        ))
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

    /// Line starts and the file positions of every tree node's start, mapped through `place` at once.
    fn file_positions(&self, py: Python<'_>, located: (&str, &ModelQueryPlace)) -> FilePositions {
        let mut line_starts: Vec<usize> = vec![0];
        let mut length: usize = 0;
        for character in self.source.chars() {
            length += 1;
            if character == '\n' {
                line_starts.push(length);
            }
        }
        line_starts.push(length);
        let mut positions: FilePositions = FilePositions {
            line_starts,
            node_starts: HashMap::new(),
        };
        let mut starts: Vec<usize> = self.parsed(py).map_or_else(
            |_| Vec::new(),
            |tree| {
                tree.nodes
                    .iter()
                    .map(|node| positions.character(node.line, node.column))
                    .collect()
            },
        );
        starts.sort_unstable();
        starts.dedup();
        let mapped: Vec<(u64, u64)> = query_file_positions(&self.source, located, &starts);
        positions.node_starts = starts.into_iter().zip(mapped).collect();
        positions
    }

    /// The parsed tree; a failure raises `RuleUsageError` each time it is read.
    fn parsed(&self, py: Python<'_>) -> PyResult<&Arc<Tree>> {
        self.tree.get_or_try_init(py, || {
            py.detach(|| sql_tree_nodes(&self.source, &self.dialect).map(tree))
                .map_err(|error| usage_error(py, &error))
        })
    }
}

impl FilePositions {
    /// The source character at 1-based `line` and `column`; a line past the end starts at the end.
    fn character(&self, line: usize, column: usize) -> usize {
        let last: usize = self.line_starts.len() - 1;
        self.line_starts[line.saturating_sub(1).min(last)] + column.saturating_sub(1)
    }
}

/// A node's 1-based `line` or `column` attribute; `None` when it has none or it is not a positive integer.
fn node_index(node: &Bound<'_, PyAny>, name: &str) -> PyResult<Option<usize>> {
    let value: i64 = match native_field(node, name) {
        Ok(value) => match value.extract() {
            Ok(value) => value,
            Err(_) => return Ok(None),
        },
        Err(error) if error.is_instance_of::<PyAttributeError>(node.py()) => return Ok(None),
        Err(error) => return Err(error),
    };
    Ok(match usize::try_from(value) {
        Ok(index) if index > 0 => Some(index),
        _ => None,
    })
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
