//! Read one project Python file as a rule source, digested so the memo sees every edit.

use std::path::Path;

use crate::models::PythonSource;

/// The project file at `relative_path`, with its digest.
pub(crate) fn read_source(
    project_dir: &Path,
    relative_path: String,
) -> Result<PythonSource, String> {
    let bytes: Vec<u8> = std::fs::read(project_dir.join(&relative_path))
        .map_err(|error| format!("{relative_path}: cannot read Python source: {error}"))?;
    let digest: String = blake3::hash(&bytes).to_hex().to_string();
    let text: String = String::from_utf8(bytes)
        .map_err(|_| format!("{relative_path}: Python source is not valid UTF-8"))?;
    Ok(PythonSource {
        relative_path,
        digest,
        text,
    })
}
