//! The `for_each` assets that read each discovered set, for the snapshot.

use std::collections::BTreeMap;

use pyo3::prelude::{Bound, PyAny, PyAnyMethods, PyResult};
use sqlbuild_python_base::records::main::fields::field::field as native_field;

/// Each set's `for_each` assets by set name, in declaration order; empty without any.
pub fn for_each_assets(inputs: &Bound<'_, PyAny>) -> PyResult<BTreeMap<String, Vec<String>>> {
    let mut assets: BTreeMap<String, Vec<String>> = BTreeMap::new();
    for template in native_field(inputs, "for_each_assets")?.try_iter()? {
        let template: Bound<'_, PyAny> = template?;
        assets
            .entry(native_field(&template, "for_each")?.extract()?)
            .or_default()
            .push(native_field(&template, "name")?.extract()?);
    }
    Ok(assets)
}
