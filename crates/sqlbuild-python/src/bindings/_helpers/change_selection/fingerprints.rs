//! Stable digests of compiled records, authored files, project variables and Python modules.

use std::cell::RefCell;
use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};

use pyo3::prelude::{Bound, PyAny, PyAnyMethods, PyResult, Python};
use pyo3::types::{
    PyBool, PyDict, PyDictMethods, PyFloat, PyFrozenSet, PyInt, PyList, PySet, PyString, PyTuple,
    PyTypeMethods,
};
use serde_json::{Map, Value};
use sqlbuild_analysis::definition_changes::main::definition_digest::definition_digest;
use sqlbuild_pysource::rule_evidence::main::project_import_closure::project_import_closure;

use sqlbuild_python_base::records::models::NativeRecord;

/// Fields that document a definition without changing what it does.
const DOCUMENTATION_FIELDS: [&str; 2] = ["description", "location"];
/// Python source that mentions this can read project variables through `ctx.vars`.
const VARS_ATTRIBUTE: &str = "vars";
const VARIABLE_SIGIL: &str = "@@";
const ENVIRONMENT_VARIABLE: &str = "ENV";
/// How SQL and configuration read an environment variable.
const ENVIRONMENT_MARKERS: [&str; 2] = ["@@ENV:", "${ENV:"];

/// What fingerprints of one project revision need: where it lives and its variables.
pub(crate) struct Fingerprinting {
    pub(crate) project_dir: PathBuf,
    /// Spellings of the project directory, stripped from values so both revisions compare.
    roots: Vec<String>,
    /// Effective project variables as canonical JSON, by name.
    vars: BTreeMap<String, String>,
    python_version: (u8, u8),
    /// Python module digests already computed, by module path; many macros share one file.
    module_digests: RefCell<BTreeMap<PathBuf, String>>,
}

impl Fingerprinting {
    pub(crate) fn new(
        project: &Bound<'_, PyAny>,
        project_dir: &Path,
        effective_vars: &Bound<'_, PyAny>,
    ) -> PyResult<Self> {
        let py: Python<'_> = project.py();
        let mut roots: Vec<String> = Vec::new();
        let canonical: Option<PathBuf> = match std::fs::canonicalize(project_dir) {
            Ok(resolved) => Some(resolved),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => None,
            Err(error) => return Err(error.into()),
        };
        for root in canonical
            .into_iter()
            .chain(std::iter::once(project_dir.to_path_buf()))
        {
            let text: String = format!("{}/", root.to_string_lossy().replace('\\', "/"));
            if !roots.contains(&text) {
                roots.push(text);
            }
        }
        roots.sort_by_key(|root| std::cmp::Reverse(root.len()));
        let mut fingerprinting: Self = Self {
            project_dir: project_dir.to_path_buf(),
            roots,
            vars: BTreeMap::new(),
            python_version: (py.version_info().major, py.version_info().minor),
            module_digests: RefCell::new(BTreeMap::new()),
        };
        if let Ok(values) = effective_vars.cast::<PyDict>() {
            for (name, value) in values.iter() {
                let encoded: String = fingerprinting.canonical(&value, &[])?.to_string();
                fingerprinting.vars.insert(name.str()?.to_string(), encoded);
            }
        }
        Ok(fingerprinting)
    }

    fn relative(&self, text: &str) -> String {
        let mut text: String = text.replace('\\', "/");
        for root in &self.roots {
            text = text.replace(root.as_str(), "");
        }
        text
    }

    /// A stable JSON form of a Python value, without documentation fields or `skipped` fields.
    pub(crate) fn canonical(&self, value: &Bound<'_, PyAny>, skipped: &[&str]) -> PyResult<Value> {
        if value.is_none() {
            return Ok(Value::Null);
        }
        if let Ok(cell) = value.cast::<NativeRecord>() {
            let record: &NativeRecord = cell.get();
            let mut fields: Map<String, Value> = Map::new();
            fields.insert("record".to_owned(), Value::from(record.spec.name));
            for (spec, item) in record.spec.fields.iter().zip(&record.values) {
                if DOCUMENTATION_FIELDS.contains(&spec.name) || skipped.contains(&spec.name) {
                    continue;
                }
                fields.insert(
                    spec.name.to_owned(),
                    self.canonical(item.bind(value.py()), skipped)?,
                );
            }
            return Ok(Value::Object(fields));
        }
        if value.is_instance_of::<PyBool>() {
            return Ok(Value::from(value.extract::<bool>()?));
        }
        if value.is_instance_of::<PyString>() {
            return Ok(Value::from(self.relative(&value.extract::<String>()?)));
        }
        if value.is_instance_of::<PyInt>() || value.is_instance_of::<PyFloat>() {
            return Ok(Value::from(value.repr()?.to_string()));
        }
        if value.is_instance_of::<PyTuple>() || value.is_instance_of::<PyList>() {
            let items: Vec<Value> = value
                .try_iter()?
                .map(|item| self.canonical(&item?, skipped))
                .collect::<PyResult<_>>()?;
            return Ok(Value::Array(items));
        }
        if value.is_instance_of::<PySet>() || value.is_instance_of::<PyFrozenSet>() {
            let mut items: Vec<String> = value
                .try_iter()?
                .map(|item| Ok(self.canonical(&item?, skipped)?.to_string()))
                .collect::<PyResult<_>>()?;
            items.sort();
            return Ok(Value::from(items));
        }
        if let Ok(mapping) = value.cast::<PyDict>() {
            let mut entries: BTreeMap<String, Value> = BTreeMap::new();
            for (key, item) in mapping.iter() {
                entries.insert(key.str()?.to_string(), self.canonical(&item, skipped)?);
            }
            return Ok(Value::Object(entries.into_iter().collect()));
        }
        let text: String = value.repr()?.to_string();
        if text.contains(" at 0x") {
            return Ok(Value::from(value.get_type().name()?.to_string()));
        }
        Ok(Value::from(self.relative(&text)))
    }

    /// The digest of `value`'s canonical form, without the top-level record fields `top_skipped`.
    pub(crate) fn canonical_digest(
        &self,
        value: &Bound<'_, PyAny>,
        (top_skipped, skipped): (&[&str], &[&str]),
    ) -> PyResult<String> {
        let mut encoded: Value = self.canonical(value, skipped)?;
        if let Value::Object(fields) = &mut encoded {
            for field in top_skipped {
                fields.remove(*field);
            }
        }
        Ok(definition_digest(&[&encoded.to_string()]))
    }

    /// The digest of the variables `names`, each by its effective value or its absence.
    fn vars_digest<'a>(&self, names: impl Iterator<Item = &'a String>) -> String {
        let mut parts: Vec<&str> = Vec::new();
        for name in names {
            parts.push(name);
            parts.push(self.vars.get(name).map_or("missing", String::as_str));
        }
        definition_digest(&parts)
    }

    /// The digest of the project and environment variables `text` reads, or `None` without any.
    pub(crate) fn referenced_vars_digest(&self, text: &str) -> Option<String> {
        let names: BTreeSet<String> = referenced_vars(text);
        let environment: BTreeSet<String> = referenced_environment(text);
        if names.is_empty() && environment.is_empty() {
            return None;
        }
        Some(definition_digest(&[
            &self.vars_digest(names.iter()),
            &environment_digest(&environment),
        ]))
    }

    /// A Python file and its imported project modules; code mentioning `vars` covers every variable.
    pub(crate) fn python_module_digest(&self, path: &Path) -> String {
        let source: PathBuf = if path.is_absolute() {
            path.to_path_buf()
        } else {
            self.project_dir.join(path)
        };
        if let Some(digest) = self.module_digests.borrow().get(&source) {
            return digest.clone();
        }
        let mut files: Vec<PathBuf> = vec![source.clone()];
        files.extend(project_import_closure(
            &source,
            &self.project_dir,
            self.python_version,
        ));
        let mut parts: Vec<String> = Vec::new();
        let mut reads_vars: bool = false;
        for file in files {
            let contents: String = std::fs::read(&file)
                .map(|bytes| String::from_utf8_lossy(&bytes).into_owned())
                .unwrap_or_else(|_| "missing".to_owned());
            reads_vars |= contents.contains(VARS_ATTRIBUTE);
            parts.push(self.relative(&file.to_string_lossy()));
            parts.push(definition_digest(&[&contents]));
        }
        if reads_vars {
            parts.push(self.vars_digest(self.vars.keys()));
        }
        let borrowed: Vec<&str> = parts.iter().map(String::as_str).collect();
        let digest: String = definition_digest(&borrowed);
        self.module_digests
            .borrow_mut()
            .insert(source, digest.clone());
        digest
    }
}

/// Names of the project variables `text` reads with `@@name`, without runtime `@@@` and `@@ENV:`.
fn referenced_vars(text: &str) -> BTreeSet<String> {
    let mut names: BTreeSet<String> = BTreeSet::new();
    let mut rest: &str = text;
    while let Some(index) = rest.find(VARIABLE_SIGIL) {
        let preceded: bool = rest[..index].ends_with('@');
        let after: &str = &rest[index + VARIABLE_SIGIL.len()..];
        rest = after.trim_start_matches('@');
        if preceded || after.starts_with('@') {
            continue;
        }
        let name: String = after
            .chars()
            .take_while(|character| character.is_ascii_alphanumeric() || *character == '_')
            .collect();
        if name.is_empty() || (name == ENVIRONMENT_VARIABLE && after[name.len()..].starts_with(':'))
        {
            continue;
        }
        names.insert(name);
    }
    names
}

/// Names of the environment variables `text` reads with `@@ENV:NAME` or `${ENV:NAME}`.
pub(crate) fn referenced_environment(text: &str) -> BTreeSet<String> {
    let mut names: BTreeSet<String> = BTreeSet::new();
    for marker in ENVIRONMENT_MARKERS {
        for (index, _) in text.match_indices(marker) {
            let name: String = text[index + marker.len()..]
                .chars()
                .take_while(|character| character.is_ascii_alphanumeric() || *character == '_')
                .collect();
            if !name.is_empty() {
                names.insert(name);
            }
        }
    }
    names
}

/// The value of environment variable `name` in this process.
pub(crate) fn process_environment(name: &str) -> Option<String> {
    std::env::var_os(name).map(|value| value.to_string_lossy().into_owned())
}

/// The digest of the environment variables `names` by their current values, which it never shows.
pub(crate) fn environment_digest(names: &BTreeSet<String>) -> String {
    environment_digest_with(names, &process_environment)
}

/// The digest of the environment variables `names` by the values `lookup` gives them.
pub(crate) fn environment_digest_with(
    names: &BTreeSet<String>,
    lookup: &dyn Fn(&str) -> Option<String>,
) -> String {
    let mut parts: Vec<String> = Vec::new();
    for name in names {
        parts.push(name.clone());
        parts.push(match lookup(name) {
            Some(value) => definition_digest(&[&value]),
            None => "unset".to_owned(),
        });
    }
    let borrowed: Vec<&str> = parts.iter().map(String::as_str).collect();
    definition_digest(&borrowed)
}

pub(crate) fn text_of(value: &Bound<'_, PyAny>) -> PyResult<String> {
    Ok(value.str()?.to_string().replace('\\', "/"))
}

/// The digest of a file's bytes, or of its absence.
pub(crate) fn file_digest(path: &Path) -> String {
    match std::fs::read(path) {
        Ok(bytes) => definition_digest(&[&String::from_utf8_lossy(&bytes)]),
        Err(_) => definition_digest(&["missing"]),
    }
}

/// The digest of every file below `directory`, by relative path, or `None` when it is absent.
pub(crate) fn directory_digest(directory: &Path) -> Option<String> {
    let mut files: Vec<(String, String)> = Vec::new();
    let mut pending: Vec<PathBuf> = vec![directory.to_path_buf()];
    while let Some(next) = pending.pop() {
        let entries: std::fs::ReadDir = match std::fs::read_dir(&next) {
            Ok(entries) => entries,
            Err(_) => return None,
        };
        for entry in entries.flatten() {
            let path: PathBuf = entry.path();
            if path.is_dir() {
                pending.push(path);
            } else {
                let relative: String = path
                    .strip_prefix(directory)
                    .unwrap_or(&path)
                    .to_string_lossy()
                    .replace('\\', "/");
                files.push((relative, file_digest(&path)));
            }
        }
    }
    files.sort();
    let parts: Vec<&str> = files
        .iter()
        .flat_map(|(path, digest)| [path.as_str(), digest.as_str()])
        .collect();
    Some(definition_digest(&parts))
}
