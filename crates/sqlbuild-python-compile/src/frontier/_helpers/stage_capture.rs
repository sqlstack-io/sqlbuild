//! Stage captures: frontier objects written as canonical JSON so compiles compare stage by stage.

use crate::frontier::constants::CAPTURE_DIR_ENV_VAR;
use pyo3::prelude::{Bound, PyAny, PyAnyMethods, PyResult, Python};
use pyo3::pyfunction;

use sqlbuild_cache::stage_capture::main::mask_capture_noise::mask_capture_noise as masked;
use sqlbuild_cache::stage_capture::main::next_capture_number::next_capture_number;

use sqlbuild_python_base::boundary::main::call_with::call_with;
use sqlbuild_python_base::boundary::main::imported::imported;
use sqlbuild_python_base::boundary::main::keywords::keywords;
use sqlbuild_python_base::boundary::main::within::within;
use sqlbuild_python_base::frontier::main::emit_error::emit_error;
use sqlbuild_python_base::frontier::models::CaptureEncoder;
use sqlbuild_python_base::records::main::fields::field::field as native_field;

pub(crate) const CAPTURE_SUFFIX: &str = ".json";
pub(crate) const PLAN_CAPTURE_DIR_ENV_VAR: &str = "SQLBUILD_PLANNER_CAPTURE_DIR";
pub(crate) const PLAN_CAPTURE_NAME: &str = "plan_output";

/// The capture document of `value`, or the shared nodes written before encoding failed.
fn capture_text(py: Python<'_>, value: &Bound<'_, PyAny>) -> Result<String, (String, pyo3::PyErr)> {
    let mut encoder: CaptureEncoder<'_> =
        CaptureEncoder::new(py).map_err(|error| (String::new(), error))?;
    let root = match encoder.encode(value) {
        Ok(root) => root,
        Err(error) => return Err((encoder.written, error)),
    };
    match encoder.document.final_chunk(&root) {
        Ok(chunk) => Ok(encoder.written + &chunk),
        Err(error) => Err((encoder.written, emit_error(error))),
    }
}

/// The number of the next capture in `directory`: one more than the captures it already holds.
pub(crate) fn capture_number(directory: &Bound<'_, PyAny>) -> PyResult<u64> {
    let directory: String = directory.str()?.extract()?;
    let names: Vec<String> = match std::fs::read_dir(&directory) {
        Ok(entries) => entries
            .filter_map(Result::ok)
            .map(|entry| entry.file_name().to_string_lossy().into_owned())
            .collect(),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Vec::new(),
        Err(error) => return Err(pyo3::exceptions::PyOSError::new_err(error.to_string())),
    };
    Ok(next_capture_number(names.iter().map(String::as_str)))
}

/// Write one frontier object when SQLBUILD_COMPILER_STAGE_CAPTURE_DIR names a directory.
pub(crate) fn write_stage_capture(
    stage: &Bound<'_, PyAny>,
    value: &Bound<'_, PyAny>,
) -> PyResult<()> {
    let py: Python<'_> = stage.py();
    let directory: Bound<'_, PyAny> =
        imported(py, "os", "environ")?.call_method1("get", (CAPTURE_DIR_ENV_VAR,))?;
    if !directory.is_truthy()? {
        return Ok(());
    }
    let number: u64 = capture_number(&directory)?;
    let stage_name: String = native_field(stage, "value")?.str()?.extract()?;
    let path: Bound<'_, PyAny> = imported(py, "pathlib", "Path")?
        .call1((directory,))?
        .call_method1(
            "__truediv__",
            (format!("{number:03}-{stage_name}{CAPTURE_SUFFIX}"),),
        )?;
    write_capture_file(&path, value)
}

/// Write the capture document of `value` to `path`, creating its directory.
pub(crate) fn write_capture_file(
    path: &Bound<'_, PyAny>,
    value: &Bound<'_, PyAny>,
) -> PyResult<()> {
    let py: Python<'_> = path.py();
    call_with(
        &native_field(path, "parent")?.getattr("mkdir")?,
        &[
            (
                "parents",
                pyo3::types::PyBool::new(py, true).to_owned().into_any(),
            ),
            (
                "exist_ok",
                pyo3::types::PyBool::new(py, true).to_owned().into_any(),
            ),
        ],
    )?;
    let stream: Bound<'_, PyAny> = path.call_method(
        "open",
        ("w",),
        Some(&keywords(
            py,
            &[
                (
                    "encoding",
                    pyo3::types::PyString::new(py, "utf-8").into_any(),
                ),
                ("newline", pyo3::types::PyString::new(py, "\n").into_any()),
            ],
        )?),
    )?;
    within(&stream, |opened| match capture_text(py, value) {
        Ok(text) => opened.call_method1("write", (text,)).map(|_| ()),
        Err((written, error)) => {
            if !written.is_empty() {
                opened.call_method1("write", (written,))?;
            }
            Err(error)
        }
    })
}

/// Render a value as the JSON document a stage capture writes.
#[pyfunction]
pub(crate) fn render_stage_capture(py: Python<'_>, value: &Bound<'_, PyAny>) -> PyResult<String> {
    capture_text(py, value).map_err(|(_, error)| error)
}

/// Mask invocation ids, native store suffixes and refactor staging folders of a compile.
#[pyfunction]
pub(crate) fn mask_capture_noise(text: &str) -> String {
    masked(text)
}
