//! Why a delta of `model_paths` would stop at custom rules, judged from their stored read sets.

use crate::rules::_helpers::custom::custom_evaluation::OUTSIDE_DELTA;
use crate::rules::_helpers::custom::custom_evaluation::PROJECT_SUBJECT_KEY;
use crate::rules::_helpers::custom::custom_evaluation::PROJECT_WIDE;
use crate::rules::_helpers::custom::custom_evaluation::READ_MORE;
use crate::rules::_helpers::custom::custom_rule_cache::stored_rules;
use crate::rules::_helpers::fact::fact_replay::delta_safe_fact;
use crate::rules::_helpers::fact::fact_replay::read_text_path;
use crate::rules::_helpers::rule::rule_run::rules_bulk_cache_path;
use crate::rules::constants::CUSTOM_RULES_CACHE_FILE;
use std::collections::BTreeSet;
use std::path::Path;

/// Why a delta of `model_paths` would stop at custom rules, judged from their stored read sets.
pub fn stored_custom_rule_refusal(
    project_dir: &Path,
    (model_paths, changed): (&BTreeSet<String>, &BTreeSet<String>),
    all_sources: bool,
) -> Option<&'static str> {
    let stored = stored_rules(&rules_bulk_cache_path(
        project_dir.to_path_buf(),
        CUSTOM_RULES_CACHE_FILE,
    ));
    for rule in stored.values() {
        for (subject, entry) in &rule.entries {
            let Some(readset) = rule.readsets.get(entry.readset) else {
                continue;
            };
            if model_paths.contains(subject) {
                if !readset
                    .iter()
                    .all(|key| delta_safe_fact(key, model_paths, all_sources))
                {
                    return Some(READ_MORE);
                }
            } else if readset
                .iter()
                .any(|key| read_text_path(key).is_some_and(|path| changed.contains(path)))
            {
                return Some(if subject == PROJECT_SUBJECT_KEY {
                    PROJECT_WIDE
                } else {
                    OUTSIDE_DELTA
                });
            }
        }
    }
    None
}
