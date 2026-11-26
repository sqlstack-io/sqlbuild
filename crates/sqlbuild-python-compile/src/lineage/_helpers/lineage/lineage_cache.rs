//! The disposable SQLite cache of relation lineage, read and written through Python's `sqlite3`.

use sqlbuild_python_base::positional_record;
use std::collections::{HashMap, HashSet};
use std::hash::Hash;
use std::path::{Path, PathBuf};

use crate::lineage::_helpers::graph::relation_fingerprint::relation_lineage_fingerprint;
use pyo3::prelude::{Bound, PyAny, PyAnyMethods, PyErr, PyModule, PyResult, Python};
use pyo3::types::{PyBytes, PyBytesMethods, PyDict, PyDictMethods, PyList, PyTuple};
use sqlbuild_analysis::lineage_command::models::{LineageIndex, LineageKey, LineageNode};
use sqlbuild_python_base::records::main::fields::field::field as native_field;

pub(crate) const CACHE_SCHEMA_VERSION: &str = "1";
pub(crate) const CACHE_ALGORITHM_VERSION: &str = "relation-lineage-graph-v4";
const CACHE_RELATIVE_PATH: [&str; 5] = [
    "target",
    "cache",
    "lineage-native-v1",
    "v1",
    "structural-graph.sqlite3",
];
pub(crate) const CACHE_MAX_BYTES: u64 = 50_000_000;
const CACHE_MAX_ROWS: usize = 1_000_000;
const SQLITE_TIMEOUT_SECONDS: f64 = 5.0;
/// The first Python whose `rglob` skips a directory listing that fails mid-way.
const FIRST_PYTHON_SKIPPING_INTERRUPTED_LISTINGS: (u8, u8) = (3, 13);
const UNKNOWN_VERSION: &str = "0+unknown";
/// Exceptions that make the cache a miss, or skip publishing it.
const CACHE_FAULTS: [(&str, &str); 4] = [
    ("builtins", "KeyError"),
    ("builtins", "OSError"),
    ("builtins", "TypeError"),
    ("builtins", "ValueError"),
];
const FINGERPRINT_FAULTS: [&str; 3] = ["TypeError", "UnicodeError", "ValueError"];
const SCHEMA_SQL: &str = "
        CREATE TABLE metadata (key TEXT PRIMARY KEY, value TEXT NOT NULL);
        CREATE TABLE resources (
            resource_type TEXT NOT NULL,
            resource_name TEXT NOT NULL,
            relative_path TEXT,
            qualified_name TEXT,
            PRIMARY KEY (resource_type, resource_name)
        );
        CREATE TABLE lookups (
            lookup_name TEXT PRIMARY KEY,
            resource_type TEXT NOT NULL,
            resource_name TEXT NOT NULL
        );
        CREATE TABLE edges (
            upstream_type TEXT NOT NULL,
            upstream_name TEXT NOT NULL,
            downstream_type TEXT NOT NULL,
            downstream_name TEXT NOT NULL,
            PRIMARY KEY (upstream_type, upstream_name, downstream_type, downstream_name)
        );
        CREATE INDEX edges_downstream ON edges (downstream_type, downstream_name);
        CREATE INDEX edges_upstream ON edges (upstream_type, upstream_name);
        CREATE TABLE tags (
            tag TEXT NOT NULL,
            resource_type TEXT NOT NULL,
            resource_name TEXT NOT NULL,
            PRIMARY KEY (tag, resource_type, resource_name)
        );
        CREATE TABLE paths (
            resource_type TEXT NOT NULL,
            resource_name TEXT NOT NULL,
            folder TEXT NOT NULL,
            PRIMARY KEY (resource_type, resource_name)
        );
        ";

positional_record! {
    extract;
    /// One cached resource: its type, name, relative path and qualified name.
    struct ResourceRow {
        resource_type: String,
        name: String,
        relative_path: Option<String>,
        qualified_name: Option<String>,
    }
}

positional_record! {
    extract;
    /// One cached lineage edge: its upstream and downstream types and names.
    struct EdgeRow {
        upstream_type: String,
        upstream_name: String,
        downstream_type: String,
        downstream_name: String,
    }
}
type TripleRow = (String, String, String);

pub(crate) fn cache_path(project_dir: &Path) -> PathBuf {
    CACHE_RELATIVE_PATH
        .iter()
        .fold(project_dir.to_path_buf(), |path, part| path.join(part))
}

pub(crate) fn skips_cache(py: Python<'_>, error: &PyErr) -> PyResult<bool> {
    let database_error: Bound<'_, PyAny> = py.import("sqlite3")?.getattr("DatabaseError")?;
    if error.is_instance(py, &database_error) {
        return Ok(true);
    }
    for (module, name) in CACHE_FAULTS {
        if error.is_instance(py, &py.import(module)?.getattr(name)?) {
            return Ok(true);
        }
    }
    Ok(false)
}

/// Python's `sqlbuild` distribution version, as the cache key records it.
fn sqlbuild_version(py: Python<'_>) -> PyResult<String> {
    let metadata: Bound<'_, PyModule> = py.import("importlib.metadata")?;
    match native_field(&metadata, "version")?.call1(("sqlbuild",)) {
        Ok(version) => version.extract(),
        Err(error) if error.is_instance(py, &native_field(&metadata, "PackageNotFoundError")?) => {
            Ok(UNKNOWN_VERSION.to_owned())
        }
        Err(error) => Err(error),
    }
}

/// The cache key over authored inputs and `cli_vars`, or `None` when they cannot be hashed.
pub(crate) fn lineage_fingerprint(
    project_dir: &Bound<'_, PyAny>,
    cli_vars: &Bound<'_, PyAny>,
) -> PyResult<Option<String>> {
    let py: Python<'_> = project_dir.py();
    let options: Bound<'_, PyDict> = PyDict::new(py);
    options.set_item("ensure_ascii", true)?;
    options.set_item("separators", (",", ":"))?;
    options.set_item("sort_keys", true)?;
    let values: Bound<'_, PyAny> = if cli_vars.is_none() {
        PyDict::new(py).into_any()
    } else {
        cli_vars.clone()
    };
    let encoded: Bound<'_, PyAny> = match py
        .import("json")?
        .getattr("dumps")?
        .call((values,), Some(&options))
        .and_then(|text| text.call_method1("encode", ("ascii",)))
    {
        Ok(encoded) => encoded,
        Err(error) => {
            for name in FINGERPRINT_FAULTS {
                if error.is_instance(py, &py.import("builtins")?.getattr(name)?) {
                    return Ok(None);
                }
            }
            return Err(error);
        }
    };
    let mut prefix: Vec<u8> = CACHE_ALGORITHM_VERSION.as_bytes().to_vec();
    prefix.extend(sqlbuild_version(py)?.as_bytes());
    prefix.extend(encoded.cast::<PyBytes>()?.as_bytes());
    let uncacheable: bool = py.version_info() < FIRST_PYTHON_SKIPPING_INTERRUPTED_LISTINGS;
    relation_lineage_fingerprint(py, project_dir.extract()?, prefix, uncacheable)
}

pub(crate) fn rows<'py, T: pyo3::prelude::FromPyObjectOwned<'py>>(
    connection: &Bound<'py, PyAny>,
    sql: &str,
) -> PyResult<Vec<T>> {
    connection
        .call_method1("execute", (sql,))?
        .call_method0("fetchall")?
        .extract()
}

pub(crate) fn read_index(connection: &Bound<'_, PyAny>) -> PyResult<Option<LineageIndex>> {
    let resources: Vec<ResourceRow> = rows(
        connection,
        "SELECT resource_type, resource_name, relative_path, qualified_name FROM resources",
    )?;
    let edges: Vec<EdgeRow> = rows(
        connection,
        "SELECT upstream_type, upstream_name, downstream_type, downstream_name FROM edges",
    )?;
    let lookups: Vec<TripleRow> = rows(
        connection,
        "SELECT lookup_name, resource_type, resource_name FROM lookups",
    )?;
    let tags: Vec<TripleRow> = rows(
        connection,
        "SELECT tag, resource_type, resource_name FROM tags",
    )?;
    let paths: Vec<TripleRow> = rows(
        connection,
        "SELECT resource_type, resource_name, folder FROM paths",
    )?;
    if resources.len() + edges.len() + lookups.len() + tags.len() + paths.len() > CACHE_MAX_ROWS {
        return Ok(None);
    }
    let mut nodes: HashMap<LineageKey, LineageNode> = HashMap::new();
    for ResourceRow {
        resource_type,
        name,
        relative_path,
        qualified_name,
    } in resources
    {
        let key: LineageKey = LineageKey {
            resource_type,
            name,
        };
        nodes.insert(
            key.clone(),
            LineageNode {
                key,
                relative_path,
                qualified_name,
            },
        );
    }
    let known = |resource_type: String, name: String| -> Option<LineageKey> {
        let key: LineageKey = LineageKey {
            resource_type,
            name,
        };
        nodes.contains_key(&key).then_some(key)
    };
    let mut upstream: Groups<LineageKey, LineageKey> = Groups::new();
    let mut downstream: Groups<LineageKey, LineageKey> = Groups::new();
    for EdgeRow {
        upstream_type,
        upstream_name,
        downstream_type,
        downstream_name,
    } in edges
    {
        let (Some(parent), Some(child)) = (
            known(upstream_type, upstream_name),
            known(downstream_type, downstream_name),
        ) else {
            return Ok(None);
        };
        upstream.push(child.clone(), parent.clone());
        downstream.push(parent, child);
    }
    let mut tag_index: Groups<String, LineageKey> = Groups::new();
    for (tag, resource_type, name) in tags {
        let Some(key) = known(resource_type, name) else {
            return Ok(None);
        };
        tag_index.push(tag, key);
    }
    let mut path_index: Vec<(LineageKey, String)> = Vec::new();
    for (resource_type, name, folder) in paths {
        let Some(key) = known(resource_type, name) else {
            return Ok(None);
        };
        path_index.push((key, folder));
    }
    let mut all_keys: Vec<(String, LineageKey)> = Vec::new();
    for (lookup_name, resource_type, name) in lookups {
        let Some(key) = known(resource_type, name) else {
            return Ok(None);
        };
        all_keys.push((lookup_name, key));
    }
    Ok(Some(LineageIndex {
        nodes,
        upstream: upstream.groups,
        downstream: downstream.groups,
        tags: tag_index.groups,
        paths: path_index,
        all_keys,
    }))
}

/// Values grouped by key, keys in first-seen order.
struct Groups<K, V> {
    groups: Vec<(K, Vec<V>)>,
    positions: HashMap<K, usize>,
}

impl<K: Eq + Hash + Clone, V> Groups<K, V> {
    fn new() -> Self {
        Self {
            groups: Vec::new(),
            positions: HashMap::new(),
        }
    }

    fn push(&mut self, key: K, value: V) {
        if let Some(&position) = self.positions.get(&key) {
            self.groups[position].1.push(value);
            return;
        }
        self.positions.insert(key.clone(), self.groups.len());
        self.groups.push((key, vec![value]));
    }
}

pub(crate) fn connect<'py>(
    py: Python<'py>,
    target: &str,
    uri: bool,
) -> PyResult<Bound<'py, PyAny>> {
    let options: Bound<'py, PyDict> = PyDict::new(py);
    options.set_item("timeout", SQLITE_TIMEOUT_SECONDS)?;
    if uri {
        options.set_item("uri", true)?;
    }
    py.import("sqlite3")?
        .getattr("connect")?
        .call((target,), Some(&options))
}

fn insert(connection: &Bound<'_, PyAny>, sql: &str, rows: Bound<'_, PyList>) -> PyResult<()> {
    connection.call_method1("executemany", (sql, rows))?;
    Ok(())
}

fn key_parts(key: &LineageKey) -> (&str, &str) {
    (key.resource_type.as_str(), key.name.as_str())
}

pub(crate) fn write_index(
    connection: &Bound<'_, PyAny>,
    fingerprint: &str,
    index: &LineageIndex,
) -> PyResult<()> {
    let py: Python<'_> = connection.py();
    connection.call_method1("executescript", (SCHEMA_SQL,))?;
    insert(
        connection,
        "INSERT INTO metadata (key, value) VALUES (?, ?)",
        PyList::new(
            py,
            [
                ("algorithm_version", CACHE_ALGORITHM_VERSION),
                ("input_fingerprint", fingerprint),
                ("schema_version", CACHE_SCHEMA_VERSION),
            ],
        )?,
    )?;
    let mut resources: Vec<Bound<'_, PyTuple>> = Vec::new();
    let mut seen: HashSet<&LineageKey> = HashSet::new();
    for (_, key) in &index.all_keys {
        if !seen.insert(key) {
            continue;
        }
        let node: Option<&LineageNode> = index.nodes.get(key);
        resources.push(PyTuple::new(
            py,
            [
                Some(key.resource_type.as_str()),
                Some(key.name.as_str()),
                node.and_then(|node| node.relative_path.as_deref()),
                node.and_then(|node| node.qualified_name.as_deref()),
            ],
        )?);
    }
    insert(
        connection,
        "INSERT INTO resources VALUES (?, ?, ?, ?)",
        PyList::new(py, resources)?,
    )?;
    let lookups: Vec<(&str, &str, &str)> = index
        .all_keys
        .iter()
        .map(|(name, key)| (name.as_str(), key.resource_type.as_str(), key.name.as_str()))
        .collect();
    insert(
        connection,
        "INSERT INTO lookups VALUES (?, ?, ?)",
        PyList::new(py, lookups)?,
    )?;
    let mut edges: Vec<Bound<'_, PyTuple>> = Vec::new();
    for (child, parents) in &index.upstream {
        for parent in parents {
            let ((parent_type, parent_name), (child_type, child_name)) =
                (key_parts(parent), key_parts(child));
            edges.push(PyTuple::new(
                py,
                [parent_type, parent_name, child_type, child_name],
            )?);
        }
    }
    insert(
        connection,
        "INSERT INTO edges VALUES (?, ?, ?, ?)",
        PyList::new(py, edges)?,
    )?;
    let mut tags: Vec<(&str, &str, &str)> = Vec::new();
    for (tag, keys) in &index.tags {
        for key in keys {
            tags.push((tag.as_str(), key.resource_type.as_str(), key.name.as_str()));
        }
    }
    insert(
        connection,
        "INSERT INTO tags VALUES (?, ?, ?)",
        PyList::new(py, tags)?,
    )?;
    let paths: Vec<(&str, &str, &str)> = index
        .paths
        .iter()
        .map(|(key, folder)| {
            (
                key.resource_type.as_str(),
                key.name.as_str(),
                folder.as_str(),
            )
        })
        .collect();
    insert(
        connection,
        "INSERT INTO paths VALUES (?, ?, ?)",
        PyList::new(py, paths)?,
    )?;
    connection.call_method0("commit")?;
    Ok(())
}
