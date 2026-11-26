//! SQLBuild owns templating, so SQLLoad must read every `${` in a setting as literal text.

/// `value` with every `${` written `$${`, SQLLoad's literal form, in each string it holds.
pub(crate) fn literal(value: toml::Value) -> toml::Value {
    match value {
        toml::Value::String(text) => toml::Value::String(text.replace("${", "$${")),
        toml::Value::Array(items) => toml::Value::Array(items.into_iter().map(literal).collect()),
        toml::Value::Table(table) => toml::Value::Table(
            table
                .into_iter()
                .map(|(key, item)| (key, literal(item)))
                .collect(),
        ),
        other => other,
    }
}
