//! The placeholders a command selected to run while it cannot discover them.

use std::sync::PoisonError;

use pyo3::prelude::{Bound, PyAny, PyAnyMethods, PyResult};

use crate::discovered_sets::_helpers::state::SOURCE;
use crate::discovered_sets::models::DiscoverySource;
use sqlbuild_python_base::records::main::fields::field::field as native_field;

/// The `for_each` assets among `selected` names that compiled as placeholders outside a location.
pub fn publish_only_placeholders(
    inputs: &Bound<'_, PyAny>,
    selected: &Bound<'_, PyAny>,
) -> PyResult<Vec<String>> {
    let outside_location: bool = matches!(
        *SOURCE.lock().unwrap_or_else(PoisonError::into_inner),
        None | Some(DiscoverySource::Placeholders)
    );
    let mut assets: Vec<String> = Vec::new();
    if !outside_location {
        return Ok(assets);
    }
    for asset in native_field(inputs, "asset_functions")?.try_iter()? {
        let asset: Bound<'_, PyAny> = asset?;
        let discovered: Bound<'_, PyAny> = native_field(&asset, "discovered")?;
        if discovered.is_none()
            || !discovered.get_item("placeholder")?.is_truthy()?
            || !selected.contains(native_field(&asset, "name")?)?
        {
            continue;
        }
        assets.push(discovered.get_item("asset")?.extract()?);
    }
    Ok(assets)
}
