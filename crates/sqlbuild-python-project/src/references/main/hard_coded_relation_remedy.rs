//! How an owner of `owner_kind` should read `target` instead of naming its relation in SQL.

use crate::references::_helpers::explicit_references;
use pyo3::prelude::Bound;
use pyo3::prelude::PyAny;
use pyo3::prelude::PyResult;

/// How an owner of `owner_kind` should read `target` instead of naming its relation in SQL.
#[inline]
pub fn hard_coded_relation_remedy(
    owner_kind: &str,
    r#ref: &Bound<'_, PyAny>,
    upstream_loader_by_source: Option<&Bound<'_, PyAny>>,
) -> PyResult<String> {
    explicit_references::hard_coded_relation_remedy(owner_kind, r#ref, upstream_loader_by_source)
}
