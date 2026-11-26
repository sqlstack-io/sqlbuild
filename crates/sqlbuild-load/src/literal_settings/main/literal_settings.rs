//! SQLBuild owns templating, so SQLLoad must read every `${` in a setting as literal text.

/// `value` with every `${` written `$${`, SQLLoad's literal form, in each string it holds.
pub fn literal_settings(value: toml::Value) -> toml::Value {
    match value {
        toml::Value::String(text) => toml::Value::String(text.replace("${", "$${")),
        toml::Value::Array(items) => {
            toml::Value::Array(items.into_iter().map(literal_settings).collect())
        }
        toml::Value::Table(table) => toml::Value::Table(
            table
                .into_iter()
                .map(|(key, item)| (key, literal_settings(item)))
                .collect(),
        ),
        other => other,
    }
}
