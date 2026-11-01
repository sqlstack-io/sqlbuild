//! `duplicate_literal_hints`: SQLCLEAN044 help naming the other files that repeat a literal.

use std::collections::{BTreeMap, BTreeSet, HashMap};

use crate::rule_results::_helpers::literals::{hint, literal_at};
use crate::rule_results::_helpers::paths::parts;
use crate::rule_results::constants::{LONG_LITERAL_CODE, MIN_SHARED_FILES};
use crate::rule_results::models::RuleFinding;

/// Each finding's remediation prefixed with its shared-literal hint, when other files repeat it.
pub fn duplicate_literal_hints<E>(
    findings: &[RuleFinding],
    mut lines: impl FnMut(&str) -> Result<Vec<String>, E>,
) -> Result<Vec<Option<String>>, E> {
    let mut texts: HashMap<String, Vec<String>> = HashMap::new();
    let mut literals: BTreeMap<usize, String> = BTreeMap::new();
    for (index, finding) in findings.iter().enumerate() {
        if finding.code != LONG_LITERAL_CODE || finding.unevaluated {
            continue;
        }
        if !texts.contains_key(&finding.path) {
            texts.insert(finding.path.clone(), lines(&finding.path)?);
        }
        let file_lines: &[String] = texts.get(&finding.path).map_or(&[], Vec::as_slice);
        if let Some(literal) = literal_at(file_lines, finding.line, finding.column) {
            literals.insert(index, literal);
        }
    }
    let mut paths_by_literal: HashMap<&str, BTreeSet<Vec<String>>> = HashMap::new();
    for (index, literal) in &literals {
        paths_by_literal
            .entry(literal)
            .or_default()
            .insert(parts(&findings[*index].path));
    }
    let mut hinted: Vec<Option<String>> = vec![None; findings.len()];
    for (index, literal) in &literals {
        let every: &BTreeSet<Vec<String>> = &paths_by_literal[literal.as_str()];
        if every.len() < MIN_SHARED_FILES {
            continue;
        }
        let own: Vec<String> = parts(&findings[*index].path);
        let others: Vec<&Vec<String>> = every.iter().filter(|path| **path != own).collect();
        hinted[*index] = Some(format!(
            "{} {}",
            hint(&others, every),
            findings[*index].remediation
        ));
    }
    Ok(hinted)
}
