//! Read the renames file `sqb migrate orchestration` writes.

use std::collections::BTreeMap;

use toml::Spanned;

use crate::orchestration_files::_helpers::reading::{check_keys, error_at, nonblank, toml_error};
use crate::orchestration_files::constants::RENAME_SECTIONS;
use crate::orchestration_files::models::{FileError, RenameMap, RenamePairs};

/// Each section's old and new names as written, or the first problem with the file.
pub fn read_renames_file(text: &str) -> Result<RenameMap, FileError> {
    check_keys(text, &RENAME_SECTIONS, "renames file")?;
    let sections: BTreeMap<String, BTreeMap<Spanned<String>, Spanned<String>>> =
        toml::from_str(text).map_err(|error| toml_error(text, &error))?;
    let mut map = RenameMap::default();
    for (section, pairs) in sections {
        let mut read: RenamePairs = Vec::new();
        let mut targets: BTreeMap<String, usize> = BTreeMap::new();
        let mut pairs: Vec<(Spanned<String>, Spanned<String>)> = pairs.into_iter().collect();
        pairs.sort_by_key(|(old, _)| old.span().start);
        for (old, new) in pairs {
            let old = nonblank(text, old, "old name")?;
            let new_start: usize = new.span().start;
            let new = nonblank(text, new, "new name")?;
            if old.value == new.value {
                return Err(error_at(
                    text,
                    new_start,
                    format!("'{}' is renamed to itself", old.value),
                ));
            }
            if targets.insert(new.value.clone(), read.len()).is_some() {
                return Err(error_at(
                    text,
                    new_start,
                    format!("two names are renamed to '{}'", new.value),
                ));
            }
            read.push((old, new));
        }
        for (_, new) in &read {
            if let Some((old, _)) = read.iter().find(|(old, _)| old.value == new.value) {
                return Err(FileError {
                    message: format!(
                        "'{}' is both a new name and an old name; renames cannot chain",
                        old.value
                    ),
                    position: old.position,
                    help: Some("map each old name straight to its final name".to_owned()),
                });
            }
        }
        match section.as_str() {
            "jobs" => map.jobs = read,
            "sensors" => map.sensors = read,
            _ => map.triggers = read,
        }
    }
    Ok(map)
}
