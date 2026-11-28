//! The destination names of a load's tables, where compile can know them: literal `include` entries named by SQLLoad's default template.

use sqlload::load_naming::main::loaded_name::loaded_name;
use sqlload::load_naming::models::Naming;

use crate::loads::constants::{
    GLOB_CHARACTERS, INCLUDE, LOAD_NAME_SEPARATOR, NAME, NAMING, TABLE_NAME, TABLES,
};

/// Each included `schema.table` and the destination it loads into, or `None` when a pattern or a name template leaves that to the plan.
pub(crate) fn literal_tables(load: &str, settings: &toml::Table) -> Option<Vec<(String, String)>> {
    if settings.contains_key(TABLE_NAME) {
        return None;
    }
    let naming: Naming = match settings.get(NAMING).and_then(toml::Value::as_str) {
        Some("source") => Naming::Source,
        Some("snake_case") | None => Naming::SnakeCase,
        Some(_) => return None,
    };
    let renamed = |key: &str| {
        settings
            .get(TABLES)
            .and_then(|tables| tables.get(key))
            .and_then(|table| table.get(NAME))
            .is_some()
    };
    let mut tables: Vec<(String, String)> = Vec::new();
    for entry in settings.get(INCLUDE)?.as_array()? {
        let key: &str = entry.as_str()?;
        if key.contains(GLOB_CHARACTERS) || renamed(key) {
            return None;
        }
        let table: &str = key.rsplit_once('.').map_or(key, |(_, table)| table);
        let filled: String = format!("{load}{LOAD_NAME_SEPARATOR}{table}");
        let filled: String = match naming {
            Naming::Source => filled.to_ascii_lowercase(),
            Naming::SnakeCase => filled,
        };
        tables.push((key.to_owned(), loaded_name(naming, &filled, None)));
    }
    Some(tables)
}
