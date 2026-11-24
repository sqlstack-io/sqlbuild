//! The long literal a finding points at, and the hint naming every file that repeats it.

use std::collections::BTreeSet;

use crate::rule_results::_helpers::paths::{common_folder, owner, posix};
use crate::rule_results::constants::{
    INHERITED_CONSTANTS, LISTED_PATHS, LOCAL_CONSTANTS, MIN_SCOPED_FOLDER_DEPTH, PROJECT_CONSTANTS,
    QUOTE,
};
use sqlbuild_core::constants::SQL_DIRECTORY;

/// The single-quoted literal starting at a one-based position, quotes included.
pub(crate) fn literal_at(lines: &[String], line: i64, column: i64) -> Option<String> {
    let (Ok(line), Ok(column)) = (usize::try_from(line), usize::try_from(column)) else {
        return None;
    };
    let text: Vec<char> = lines.get(line.checked_sub(1)?)?.chars().collect();
    let start: usize = column.checked_sub(1)?;
    if text.get(start) != Some(&QUOTE) {
        return None;
    }
    let mut position: usize = start + 1;
    while position < text.len() {
        if text[position] == QUOTE {
            if text.get(position + 1) == Some(&QUOTE) {
                position += 2;
                continue;
            }
            return Some(text[start..=position].iter().collect());
        }
        position += 1;
    }
    None
}

/// The hint listing the other files and the folder to declare the shared constant in.
pub(crate) fn hint(others: &[&Vec<String>], every: &BTreeSet<Vec<String>>) -> String {
    let listed: String = others
        .iter()
        .take(LISTED_PATHS)
        .map(|path| posix(path))
        .collect::<Vec<_>>()
        .join(", ");
    let more: String = if others.len() > LISTED_PATHS {
        format!(" and {} more", others.len() - LISTED_PATHS)
    } else {
        String::new()
    };
    let owners: BTreeSet<Vec<String>> = every.iter().map(|path| owner(path)).collect();
    let common: Vec<String> = common_folder(&owners.iter().cloned().collect::<Vec<_>>());
    let roots: usize = usize::from(common.first().is_some_and(|part| part == SQL_DIRECTORY));
    let placement: String = if common.len() < MIN_SCOPED_FOLDER_DEPTH + roots {
        format!(
            "no folder below the project roots holds every use, so declare it once in \
             top-level `{PROJECT_CONSTANTS}`"
        )
    } else {
        let directory: &str = if owners.len() == 1 && owners.contains(&common) {
            LOCAL_CONSTANTS
        } else {
            INHERITED_CONSTANTS
        };
        let folder: String = posix(&common);
        format!(
            "their nearest common folder is `{folder}`, so declare it once in `{folder}/{directory}`"
        )
    };
    format!(
        "The identical literal also appears in {listed}{more}; {placement} \
         and reference it with `@const` in each file."
    )
}
