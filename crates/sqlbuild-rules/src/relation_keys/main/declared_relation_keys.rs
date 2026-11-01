use std::collections::BTreeMap;

use crate::relation_keys::_helpers::declared_keys::{non_null_unique_columns, trusted_unique_key};
use crate::relation_keys::constants::{RELATION_KIND_REF, RELATION_KIND_SEED};
use crate::relation_keys::models::DeclaredRelation;

/// Keys proving one row per key by `kind:name`; seeds are also refs, one-row relations key `[]`.
pub fn declared_relation_keys(
    relations: &[DeclaredRelation],
) -> BTreeMap<String, Vec<Vec<String>>> {
    let mut keys: BTreeMap<String, Vec<Vec<String>>> = BTreeMap::new();
    for relation in relations {
        let mut found: Vec<Vec<String>> = trusted_unique_key(relation);
        for column in non_null_unique_columns(relation) {
            found.push(vec![column]);
        }
        if relation.at_most_one_row {
            found.push(Vec::new());
        }
        if found.is_empty() {
            continue;
        }
        let mut kinds: Vec<&str> = vec![relation.kind.as_str()];
        if relation.kind == RELATION_KIND_SEED {
            kinds.push(RELATION_KIND_REF);
        }
        for kind in kinds {
            let entry: &mut Vec<Vec<String>> =
                keys.entry(format!("{kind}:{}", relation.name)).or_default();
            for key in &found {
                if !entry.contains(key) {
                    entry.push(key.clone());
                }
            }
        }
    }
    keys
}
