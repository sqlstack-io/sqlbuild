//! Native dynamic-column contracts with a shared, lazy public projection.

use pyo3::prelude::{Bound, PyAny, PyAnyMethods, PyResult, Python};
use pyo3::types::PyTuple;
use sqlbuild_model_config::header_metadata::models::DynamicFamily;

use crate::bindings::_helpers::native_host::header_projection::FamilyProjection;
use crate::bindings::_helpers::records::construct::record_class;
use crate::bindings::_helpers::records::fields::field;
use crate::bindings::_helpers::records::ids::RecordId;

/// Parsed family specs stay native until a public schema attribute needs records.
pub(crate) struct HeaderFamilies {
    pub(crate) specs: Vec<DynamicFamily>,
    public: FamilyProjection,
}

impl HeaderFamilies {
    pub(crate) fn new(class: &Bound<'_, PyAny>, specs: Vec<DynamicFamily>) -> PyResult<Self> {
        let result = Self {
            specs,
            public: FamilyProjection::new(
                class,
                !class.is(&record_class(
                    class.py(),
                    RecordId::SchemaDynamicColumnFamily,
                )?),
            ),
        };
        if result.public.custom {
            let _ = result.project(class.py())?;
        }
        Ok(result)
    }

    pub(crate) fn read(&self, py: Python<'_>) -> PyResult<Vec<DynamicFamily>> {
        if self.public.custom {
            family_specs(self.project(py)?.as_any())
        } else {
            Ok(self.specs.clone())
        }
    }

    pub(crate) fn project<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyTuple>> {
        self.public.project(py, &self.specs)
    }
}

/// A Python-authored family contract crosses into native ownership once.
pub(crate) fn family_specs(families: &Bound<'_, PyAny>) -> PyResult<Vec<DynamicFamily>> {
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
