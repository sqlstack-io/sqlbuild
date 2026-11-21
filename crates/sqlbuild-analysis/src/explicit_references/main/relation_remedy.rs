//! How Python code should read a project resource instead of naming its relation in SQL.

use crate::explicit_references::constants::{HOOK_OWNER, LOADER_OWNER, SOURCE_TYPE};
use crate::explicit_references::types::ResourceKey;

/// `hard_coded_relation_remedy`; `upstream_loaders` maps sources to the loaders writing them.
#[must_use]
pub fn relation_remedy(
    owner_kind: &str,
    (kind, name): &ResourceKey,
    upstream_loaders: &[(String, String)],
) -> String {
    let typed: String = format!("{kind}(\"{name}\")");
    if owner_kind == LOADER_OWNER {
        if kind != SOURCE_TYPE {
            return format!(
                "loaders run before models and seeds and cannot read them; read \
                 {kind}:{name} from an asset instead"
            );
        }
        return match upstream_loaders.iter().find(|(source, _)| source == name) {
            Some((_, loader)) => format!("use ctx.loader({loader}) instead of the relation name"),
            None => format!("use ctx.source(\"{name}\") instead of the relation name"),
        };
    }
    let declaration: String = if owner_kind == HOOK_OWNER {
        format!("@hook(reads={typed})")
    } else {
        format!("depends_on={typed}")
    };
    format!(
        "declare it with {declaration} and use ctx.relation({typed}) instead of the relation name"
    )
}
