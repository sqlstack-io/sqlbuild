//! A Python-authored family contract crosses into native ownership once.

use pyo3::prelude::{Bound, PyAny, PyAnyMethods, PyResult};
use sqlbuild_model_config::header_metadata::models::DynamicFamily;
use sqlbuild_python_base::records::main::fields::field::field;

/// A Python-authored family contract crosses into native ownership once.
pub fn family_specs(families: &Bound<'_, PyAny>) -> PyResult<Vec<DynamicFamily>> {
    families
        .try_iter()?
        .map(|family| {
            let family = family?;
            Ok(DynamicFamily {
                name: field(&family, "name")?.extract()?,
                pivot_column: field(&family, "pivot_column")?.extract()?,
                value_column: field(&family, "value_column")?.extract()?,
                aggregate: field(&family, "aggregate")?.extract()?,
                data_type: field(&family, "type")?.extract()?,
                name_pattern: field(&family, "name_pattern")?.extract()?,
            })
        })
        .collect()
}
