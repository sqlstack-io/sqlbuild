//! Custom-rule import closures, implementation fingerprints and statically found test cases.

use sqlbuild_python_base::positional_record;
use sqlbuild_python_base::records::main::construct::new_plain_error::new_plain_error;
use std::path::{Path, PathBuf};

use pyo3::prelude::{
    Bound, PyAny, PyAnyMethods, PyErr, PyModule, PyModuleMethods, PyResult, Python,
};
use pyo3::types::{PyDict, PyDictMethods, PySet, PySetMethods, PyString, PyTuple, PyTupleMethods};
use pyo3::{pyfunction, wrap_pyfunction};
use sqlbuild_pysource::rule_evidence::main::allowed_custom_rule_import::allowed_custom_rule_import;
use sqlbuild_pysource::rule_evidence::main::disallowed_import::disallowed_import;
use sqlbuild_pysource::rule_evidence::main::implementation_fingerprint::implementation_fingerprint;
use sqlbuild_pysource::rule_evidence::main::import_closure::import_closure as import_closure_of;
use sqlbuild_pysource::rule_evidence::main::test_evidence::test_evidence;
use sqlbuild_pysource::rule_evidence::models::{
    DisallowedImport, RuleTestEvidence, UnparsedSource,
};

use sqlbuild_python_base::boundary::main::imported::imported;
use sqlbuild_python_base::records::main::fields::field::field as native_field;
use sqlbuild_python_base::records::types::ErrorId;

const PATHLIB_MODULE: &str = "pathlib";
const INSPECT_MODULE: &str = "inspect";
const HASHLIB_MODULE: &str = "hashlib";
const AST_MODULE: &str = "ast";
const RULES_ERROR: ErrorId = ErrorId::Rules;
const NON_HERMETIC_RULE_ERROR: ErrorId = ErrorId::NonHermeticRule;

fn python_version(py: Python<'_>) -> (u8, u8) {
    let version = py.version_info();
    (version.major, version.minor)
}

/// `Path(rule.source).resolve()` as a Python path, or `None` for a rule without a source file.
fn resolved_source<'py>(rule: &Bound<'py, PyAny>) -> PyResult<Option<Bound<'py, PyAny>>> {
    let source: Bound<'py, PyAny> = native_field(rule, "source")?;
    if source.is_none() {
        return Ok(None);
    }
    let path: Bound<'py, PyAny> = imported(rule.py(), PATHLIB_MODULE, "Path")?.call1((source,))?;
    path.call_method0("resolve").map(Some)
}

fn native_path(path: &Bound<'_, PyAny>) -> PyResult<PathBuf> {
    path.extract()
}

fn closure_paths<'py>(py: Python<'py>, paths: Vec<PathBuf>) -> PyResult<Bound<'py, PyTuple>> {
    let class: Bound<'py, PyAny> = imported(py, PATHLIB_MODULE, "Path")?;
    let paths: Vec<Bound<'py, PyAny>> = paths
        .into_iter()
        .map(|path| class.call1((path,)))
        .collect::<PyResult<_>>()?;
    PyTuple::new(py, paths)
}

/// Repository-owned helper files imported by one rule module.
#[pyfunction]
#[pyo3(signature = (*, rule, project_dir))]
pub(crate) fn custom_rule_import_closure<'py>(
    rule: &Bound<'py, PyAny>,
    project_dir: &Bound<'py, PyAny>,
) -> PyResult<Bound<'py, PyTuple>> {
    let py: Python<'py> = rule.py();
    let Some(source) = resolved_source(rule)? else {
        return Ok(PyTuple::empty(py));
    };
    let root: PathBuf = native_path(&project_dir.call_method0("resolve")?)?;
    let closure: Vec<PathBuf> =
        import_closure_of(&native_path(&source)?, &root, python_version(py));
    closure_paths(py, closure)
}

/// One rule's source file and helper closure fingerprint, or its check's source digest.
#[pyfunction]
#[pyo3(signature = (*, rule, project_dir, import_closure = None))]
pub(crate) fn custom_rule_implementation_fingerprint(
    rule: &Bound<'_, PyAny>,
    project_dir: &Bound<'_, PyAny>,
    import_closure: Option<&Bound<'_, PyAny>>,
) -> PyResult<String> {
    let py: Python<'_> = rule.py();
    let Some(source) = resolved_source(rule)? else {
        let text: Bound<'_, PyAny> =
            imported(py, INSPECT_MODULE, "getsource")?.call1((native_field(rule, "check")?,))?;
        return imported(py, HASHLIB_MODULE, "sha256")?
            .call1((text.call_method0("encode")?,))?
            .call_method0("hexdigest")?
            .extract();
    };
    let root: PathBuf = native_path(&project_dir.call_method0("resolve")?)?;
    let source: PathBuf = native_path(&source)?;
    let closure: Vec<PathBuf> = match import_closure {
        Some(closure) if !closure.is_none() => closure
            .try_iter()?
            .map(|path| native_path(&path?))
            .collect::<PyResult<_>>()?,
        _ => import_closure_of(&source, &root, python_version(py)),
    };
    Ok(implementation_fingerprint(&source, &closure, &root)?)
}

positional_record! {
    into;
    /// `(path, line, column, owner)` of one public-harness case.
    struct EvidenceRow {
        path: String,
        line: usize,
        column: usize,
        owner: String,
    }
}

/// `(path, line, column, owner)` of each public-harness case that statically evaluates the rule.
#[pyfunction]
#[pyo3(signature = (*, rule, project_dir))]
fn custom_rule_test_evidence<'py>(
    rule: &Bound<'py, PyAny>,
    project_dir: &Bound<'py, PyAny>,
) -> PyResult<Vec<EvidenceRow>> {
    let py: Python<'py> = rule.py();
    let Some(source) = resolved_source(rule)? else {
        return Ok(Vec::new());
    };
    let root_path: Bound<'py, PyAny> = project_dir.call_method0("resolve")?;
    let check_name: String = match native_field(rule, "check")?.getattr("__name__") {
        Ok(name) => name.extract()?,
        Err(_) => String::new(),
    };
    let Some(evidence) = test_evidence(
        (&native_path(&source)?, &check_name),
        (&native_path(project_dir)?, &native_path(&root_path)?),
        python_version(py),
    ) else {
        source.call_method1("relative_to", (root_path,))?;
        return Ok(Vec::new());
    };
    Ok(evidence
        .into_iter()
        .map(|item: RuleTestEvidence| EvidenceRow {
            path: item.path,
            line: item.line,
            column: item.column,
            owner: item.owner,
        })
        .collect())
}

/// The native `error_id` exception raised with `message`.
fn rules_error(py: Python<'_>, error_id: ErrorId, message: String) -> PyErr {
    new_plain_error(py, error_id, &message)
}

/// Whether custom-rule code may import one absolute module name.
#[pyfunction]
#[pyo3(name = "allowed_custom_rule_import")]
fn allowed_custom_rule_import_binding(module: &str) -> bool {
    allowed_custom_rule_import(module)
}

/// Reject a repository-owned rule file that imports outside the allowlist.
fn verify_source(path: &Bound<'_, PyAny>, root: &Path) -> PyResult<()> {
    let py: Python<'_> = path.py();
    if !native_path(path)?.starts_with(root) {
        return Err(rules_error(
            py,
            RULES_ERROR,
            format!(
                "custom rule source must be repository-owned: {}",
                path.str()?
            ),
        ));
    }
    let source: Bound<'_, PyAny> = path.call_method("read_text", (), Some(&utf8_encoding(py)?))?;
    let text: String = source.extract()?;
    match disallowed_import(&text, python_version(py)) {
        Ok(None) => Ok(()),
        Ok(Some(DisallowedImport { line, module })) => Err(rules_error(
            py,
            NON_HERMETIC_RULE_ERROR,
            format!(
                "non-hermetic custom rule at {}:{line}: import {} is not allowed",
                path.str()?,
                PyString::new(py, &module).repr()?
            ),
        )),
        Err(UnparsedSource) => {
            let keywords: Bound<'_, PyDict> = PyDict::new(py);
            keywords.set_item("filename", path.str()?)?;
            imported(py, AST_MODULE, "parse")?.call((source,), Some(&keywords))?;
            Err(rules_error(
                py,
                RULES_ERROR,
                format!(
                    "custom rule source could not be checked for imports: {}",
                    path.str()?
                ),
            ))
        }
    }
}

fn utf8_encoding(py: Python<'_>) -> PyResult<Bound<'_, PyDict>> {
    let keywords: Bound<'_, PyDict> = PyDict::new(py);
    keywords.set_item("encoding", "utf-8")?;
    Ok(keywords)
}

/// Reject imports outside the deterministic allowlist in every fingerprinted rule file.
#[pyfunction]
#[pyo3(signature = (*, rules, project_dir))]
pub(crate) fn verify_custom_rules(
    rules: &Bound<'_, PyAny>,
    project_dir: &Bound<'_, PyAny>,
) -> PyResult<()> {
    let py: Python<'_> = rules.py();
    let root_path: Bound<'_, PyAny> = project_dir.call_method0("resolve")?;
    let root: PathBuf = native_path(&root_path)?;
    let checked: Bound<'_, PySet> = PySet::empty(py)?;
    for rule in rules.try_iter()? {
        let rule: Bound<'_, PyAny> = rule?;
        if !native_field(&rule, "custom")?.is_truthy()? {
            continue;
        }
        let Some(source) = resolved_source(&rule)? else {
            continue;
        };
        let closure: Vec<PathBuf> =
            import_closure_of(&native_path(&source)?, &root, python_version(py));
        let paths: Bound<'_, PyTuple> = closure_paths(py, closure)?;
        for path in std::iter::once(source).chain(paths.iter()) {
            if checked.contains(&path)? {
                continue;
            }
            checked.add(&path)?;
            verify_source(&path, &root)?;
        }
    }
    Ok(())
}

pub(crate) fn register(module: &Bound<'_, PyModule>) -> PyResult<()> {
    module.add_function(wrap_pyfunction!(
        allowed_custom_rule_import_binding,
        module
    )?)?;
    module.add_function(wrap_pyfunction!(verify_custom_rules, module)?)?;
    module.add_function(wrap_pyfunction!(custom_rule_import_closure, module)?)?;
    module.add_function(wrap_pyfunction!(
        custom_rule_implementation_fingerprint,
        module
    )?)?;
    module.add_function(wrap_pyfunction!(custom_rule_test_evidence, module)?)?;
    Ok(())
}
