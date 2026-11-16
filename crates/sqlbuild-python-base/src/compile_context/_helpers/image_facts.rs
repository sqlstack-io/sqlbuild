//! Facts a full compile's stages leave for its project image as they run.

use pyo3::prelude::{Bound, PyAny, PyAnyMethods, PyResult};
use pyo3::types::PyStringMethods;
use sqlbuild_cache::digest::main::bytes_digest::bytes_digest;
use sqlbuild_cache::digest::main::hex_digest::hex_digest;

use crate::compile_context::_helpers::delta::capture;
use crate::compile_context::_helpers::delta::capture_unsupported;
use crate::records::main::fields::field::field as native_field;

/// Record the digest of one model's scope usages, which decide declaration placement.
pub(crate) fn record_model_usages(
    model: &Bound<'_, PyAny>,
    usages: (&[Bound<'_, PyAny>], &[Bound<'_, PyAny>]),
) -> PyResult<()> {
    if let Err(error) = model_usages(model, usages) {
        capture_unsupported(&format!(
            "a model's scope usages could not be recorded: {error}"
        ));
    }
    Ok(())
}

fn model_usages(
    model: &Bound<'_, PyAny>,
    (macro_usages, declaration_usages): (&[Bound<'_, PyAny>], &[Bound<'_, PyAny>]),
) -> PyResult<()> {
    let Ok(path) = native_field(model, "model_file")
        .and_then(|file| native_field(&file, "relative_path"))
        .and_then(|path| Ok(path.str()?.to_str()?.replace('\\', "/")))
    else {
        capture_unsupported("a model input has no file");
        return Ok(());
    };
    let mut text: String = String::new();
    for (group, usages) in [("macro", macro_usages), ("declaration", declaration_usages)] {
        text.push_str(group);
        text.push('\0');
        for usage in usages {
            text.push_str(usage.repr()?.to_str()?);
            text.push('\0');
        }
    }
    let digest: String = hex_digest(&bytes_digest(text.as_bytes()));
    capture(|capture| {
        capture.usages.insert(path, digest);
    });
    Ok(())
}
