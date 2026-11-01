//! Read the location interpreter's installed dependencies, never its credentials or paths.

use std::path::Path;

use pyo3::prelude::{Bound, PyAny, PyAnyMethods, PyResult, Python};
use pyo3::types::{PyDict, PyDictMethods, PyTuple};
use sha2::{Digest, Sha256};
use sqlbuild_discovery::constants::{ORCHESTRATION_DIRECTORY, PYTHON_FILE_SUFFIX};
use sqlbuild_discovery::tree::main::rglob::rglob;
use sqlbuild_discovery::tree::models::ProjectTree;

/// A lockfile binds declared dependencies; without one, bind installed distribution versions.
pub(crate) fn environment_hash(py: Python<'_>, project_dir: &Path) -> PyResult<String> {
    let mut hash = Sha256::new();
    let version = py.version_info();
    hash.update(format!(
        "sqlbuild:{}\npython:{}.{}.{}{}\n",
        env!("CARGO_PKG_VERSION"),
        version.major,
        version.minor,
        version.patch,
        version.suffix.unwrap_or_default(),
    ));
    match py.detach(|| std::fs::read(project_dir.join("uv.lock"))) {
        Ok(lockfile) => {
            hash.update(b"uv.lock\n");
            hash.update(lockfile);
        }
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            let mut packages: Vec<(String, String)> = Vec::new();
            for distribution in py
                .import("importlib.metadata")?
                .call_method0("distributions")?
                .try_iter()?
            {
                let distribution = distribution?;
                let name: String = distribution
                    .getattr("metadata")?
                    .get_item("Name")?
                    .extract()?;
                let version: String = distribution.getattr("version")?.extract()?;
                packages.push((name.replace(['_', '.'], "-").to_ascii_lowercase(), version));
            }
            packages.sort();
            hash.update(b"installed-packages\n");
            hash.update(
                serde_json::to_vec(&packages)
                    .map_err(|error| pyo3::exceptions::PyValueError::new_err(error.to_string()))?,
            );
        }
        Err(error) => return Err(error.into()),
    }
    Ok(format!("{:x}", hash.finalize()))
}

/// Hash declaration/helper sources once per compilation, never once per sensor.
pub(crate) fn orchestration_code_hash(project_dir: &Path) -> PyResult<String> {
    let tree = ProjectTree::new(project_dir);
    let files = rglob(&tree, ORCHESTRATION_DIRECTORY, |entry| {
        !entry.is_dir && entry.name.ends_with(PYTHON_FILE_SUFFIX)
    })
    .map_err(|error| {
        pyo3::exceptions::PyValueError::new_err(format!(
            "sensor source discovery failed: {error:?}"
        ))
    })?;
    let mut hash = Sha256::new();
    for file in files {
        let bytes = std::fs::read(tree.absolute(&file))?;
        hash.update(file.len().to_le_bytes());
        hash.update(file.as_bytes());
        hash.update(bytes.len().to_le_bytes());
        hash.update(bytes);
    }
    Ok(format!("{:x}", hash.finalize()))
}

/// Include the callable's code even for public in-memory declaration compilation.
pub(crate) fn callback_code_hash(function: &Bound<'_, PyAny>, sources: &str) -> PyResult<String> {
    let bytes: Vec<u8> = function
        .py()
        .import("marshal")?
        .call_method1("dumps", (portable_code(function.getattr("__code__")?)?,))?
        .extract()?;
    let mut hash = Sha256::new();
    hash.update(sources.as_bytes());
    hash.update(bytes);
    Ok(format!("{:x}", hash.finalize()))
}

/// Code objects retain absolute import paths, including inside nested functions.
fn portable_code(code: Bound<'_, PyAny>) -> PyResult<Bound<'_, PyAny>> {
    let py = code.py();
    let code_type = py.import("types")?.getattr("CodeType")?;
    let mut constants: Vec<Bound<'_, PyAny>> = Vec::new();
    for value in code.getattr("co_consts")?.try_iter()? {
        let value = value?;
        constants.push(if value.is_instance(&code_type)? {
            portable_code(value)?
        } else {
            value
        });
    }
    let changes = PyDict::new(py);
    changes.set_item("co_filename", "<sensor>")?;
    changes.set_item("co_consts", PyTuple::new(py, constants)?)?;
    code.call_method("replace", (), Some(&changes))
}
