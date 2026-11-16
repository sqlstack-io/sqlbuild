use sqlbuild_config::models::ConfigValue;

/// A YAML string value.
pub(crate) fn text(value: &str) -> ConfigValue {
    ConfigValue::String(value.to_owned())
}
