//! Map offsets in expanded SQL back onto the authored text that produced them.

use pyo3::prelude::{Bound, PyAny, PyAnyMethods, PyModule, PyModuleMethods, PyResult, Python};
use pyo3::types::PyDict;
use pyo3::{pyfunction, wrap_pyfunction};
use sqlbuild_analysis::compiled_project::main::map_expanded_offset::map_expanded_offset as mapped;
use sqlbuild_analysis::compiled_project::models::SplicedSpan;
use sqlbuild_python_base::records::main::construct::new_record::new_record;
use sqlbuild_python_base::records::main::fields::field::field as native_field;
use sqlbuild_python_base::records::types::RecordId;

/// `map_expanded_offset(*, offset, passes)`: a `MappedOffset` in the first pass's source.
#[pyfunction]
#[pyo3(signature = (*, offset, passes))]
fn map_expanded_offset<'py>(
    py: Python<'py>,
    offset: usize,
    passes: &Bound<'py, PyAny>,
) -> PyResult<Bound<'py, PyAny>> {
    let mut spans: Vec<Vec<SplicedSpan>> = Vec::new();
    for pass in passes.try_iter()? {
        let mut pass_spans: Vec<SplicedSpan> = Vec::new();
        for span in pass?.try_iter()? {
            let span: Bound<'py, PyAny> = span?;
            pass_spans.push(SplicedSpan {
                source_start: native_field(&span, "source_start")?.extract()?,
                source_end: native_field(&span, "source_end")?.extract()?,
                output_start: native_field(&span, "output_start")?.extract()?,
                output_end: native_field(&span, "output_end")?.extract()?,
            });
        }
        spans.push(pass_spans);
    }
    let (offset, generated) = mapped(offset, &spans);
    let arguments: Bound<'py, PyDict> = PyDict::new(py);
    arguments.set_item("offset", offset)?;
    arguments.set_item("generated", generated)?;
    new_record(py, RecordId::MappedOffset, (), Some(&arguments))
}

pub(crate) fn register(module: &Bound<'_, PyModule>) -> PyResult<()> {
    module.add_function(wrap_pyfunction!(map_expanded_offset, module)?)
}
