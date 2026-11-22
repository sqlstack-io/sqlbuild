//! The values the environment sets for the providers' secret settings, which a sensor's output must not show.

use pyo3::prelude::{Bound, PyAny, PyAnyMethods, PyResult};

use crate::boundary::main::imported::imported;
use crate::orchestration_reads::_helpers::provider_settings;
use crate::records::main::fields::field::field;

/// Each nonempty value of a secret setting of `providers`, read from the environment variable pydantic-settings reads it from, longest first so a value inside another is masked with it.
pub fn provider_secret_values(providers: &Bound<'_, PyAny>) -> PyResult<Vec<String>> {
    let environ = imported(providers.py(), "os", "environ")?;
    let mut values: Vec<String> = Vec::new();
    for provider in providers.try_iter()? {
        let provider_class = field(&provider?, "provider_class")?;
        for variable in provider_settings::secret_variables(&provider_class)? {
            for item in environ.call_method0("items")?.try_iter()? {
                let (name, value): (String, String) = item?.extract()?;
                if name.eq_ignore_ascii_case(&variable) && !value.is_empty() {
                    values.push(value);
                }
            }
        }
    }
    values.sort_by(|left, right| right.len().cmp(&left.len()).then_with(|| left.cmp(right)));
    values.dedup();
    Ok(values)
}
