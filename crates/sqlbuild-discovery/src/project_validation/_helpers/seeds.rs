//! Python's `_validate_declared_seed_files`: every declared seed has one CSV whose header matches.

use std::collections::{HashMap, HashSet};

use sqlbuild_core::text::main::python_str_repr::python_str_repr;

use crate::_helpers::reading::read_failure;
use crate::models::{DiscoveryFailure, FailureKind};
use crate::project_validation::_helpers::csv_header::csv_header;
use crate::project_validation::errors::HeaderError;
use crate::project_validation::models::{
    ProjectFacts, SeedEntryFact, SeedFileFact, ValidationFailure,
};

/// `csv.field_size_limit()` before any project changes it.
const DEFAULT_FIELD_LIMIT: usize = 131_072;

fn seed_error(message: String) -> ValidationFailure {
    DiscoveryFailure::new(FailureKind::Seed, message).into()
}

/// Reject undeclared, duplicate or mismatched seed CSVs in Python's order.
pub(crate) fn validate_declared_seed_files(facts: &ProjectFacts) -> Result<(), ValidationFailure> {
    let field_limit: usize = facts.csv_field_limit.unwrap_or(DEFAULT_FIELD_LIMIT);
    unique_seed_csv_names(&facts.seeds)?;
    let mut by_name: HashMap<&str, Vec<&SeedFileFact>> = HashMap::new();
    for seed in &facts.seeds {
        by_name.entry(&seed.file.name).or_default().push(seed);
    }
    let mut declared: HashSet<&str> = HashSet::new();
    for schema in &facts.schemas {
        for entry in &schema.seeds {
            declared.insert(&entry.name);
            let matching: &[&SeedFileFact] =
                by_name.get(entry.name.as_str()).map_or(&[], Vec::as_slice);
            match matching {
                [] => {
                    return Err(seed_error(format!(
                        "Seed declaration '{}' in {} has no matching CSV file under seeds/",
                        entry.name, schema.path
                    )));
                }
                [seed] => seed_csv_header(entry, seed, field_limit)?,
                _ => {
                    let paths: Vec<&str> = matching
                        .iter()
                        .map(|seed| seed.file.path.as_str())
                        .collect();
                    return Err(seed_error(format!(
                        "Seed declaration '{}' matches multiple CSV files: {}",
                        entry.name,
                        paths.join(", ")
                    )));
                }
            }
        }
    }
    for seed in &facts.seeds {
        if !declared.contains(seed.file.name.as_str()) {
            return Err(seed_error(format!(
                "Seed CSV {} has no matching declaration for seed '{}' under sql/seeds/**/*.yml",
                seed.file.path, seed.file.name
            )));
        }
    }
    Ok(())
}

fn unique_seed_csv_names(seeds: &[SeedFileFact]) -> Result<(), ValidationFailure> {
    let mut seen: HashMap<&str, &str> = HashMap::new();
    for seed in seeds {
        if let Some(existing) = seen.get(seed.file.name.as_str()) {
            return Err(seed_error(format!(
                "Duplicate seed CSV name '{}' found: {existing}, {}. Seed CSV filenames must be \
                 unique under seeds/.",
                seed.file.name, seed.file.path
            )));
        }
        seen.insert(&seed.file.name, &seed.file.path);
    }
    Ok(())
}

fn seed_csv_header(
    entry: &SeedEntryFact,
    seed: &SeedFileFact,
    field_limit: usize,
) -> Result<(), ValidationFailure> {
    let unreadable = |failure| ValidationFailure::Unreadable {
        absolute_path: seed.absolute_path.clone(),
        failure,
    };
    let bytes: Vec<u8> =
        std::fs::read(&seed.absolute_path).map_err(|error| unreadable(read_failure(&error)))?;
    if let Some(error) = &entry.csv.dialect_error {
        return Err(ValidationFailure::CsvDialect(error.clone()));
    }
    let header: Vec<String> = match csv_header(&bytes, &entry.csv, field_limit) {
        Ok(header) => header,
        Err(HeaderError::Read(failure)) => return Err(unreadable(failure)),
        Err(HeaderError::Csv(message)) => return Err(ValidationFailure::Csv(message)),
    };
    let path: &str = &seed.file.path;
    if header.is_empty() {
        return Err(seed_error(format!("{path} must contain a CSV header row")));
    }
    let mut seen: HashSet<&str> = HashSet::new();
    for column in &header {
        if !seen.insert(column) {
            return Err(seed_error(format!(
                "{path} contains duplicate CSV header column '{column}'"
            )));
        }
    }
    if header != entry.columns {
        return Err(seed_error(format!(
            "{path} header {} does not match declared seed columns {} for '{}'",
            python_tuple_repr(&header),
            python_tuple_repr(&entry.columns),
            entry.name
        )));
    }
    Ok(())
}

/// Python's `repr` of a tuple of strings.
fn python_tuple_repr(items: &[String]) -> String {
    let parts: Vec<String> = items.iter().map(|item| python_str_repr(item)).collect();
    match parts.as_slice() {
        [single] => format!("({single},)"),
        _ => format!("({})", parts.join(", ")),
    }
}
