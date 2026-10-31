//! Read the location interpreter's installed dependencies, never its credentials or paths.

use std::path::Path;

use pyo3::prelude::{PyAnyMethods, PyResult, Python};
use sha2::{Digest, Sha256};

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
