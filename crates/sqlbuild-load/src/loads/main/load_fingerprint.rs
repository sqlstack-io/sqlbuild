//! What a load's definition is, as `--changed` compares it: its settings and its sources' column types.

use crate::loads::models::LoadDeclaration;

/// A hash of `load`'s settings and the column types of the sources it fills, in a stable order.
pub fn load_fingerprint(load: &LoadDeclaration) -> String {
    let mut hasher = blake3::Hasher::new();
    hasher.update(load.name.as_bytes());
    hasher.update(b"\0");
    hasher.update(
        toml::Value::Table(load.settings.clone())
            .to_string()
            .as_bytes(),
    );
    let mut sources: Vec<_> = load.sources.iter().collect();
    sources.sort_by(|left, right| left.name.cmp(&right.name));
    for source in sources {
        hasher.update(b"\0");
        hasher.update(source.name.as_bytes());
        for (column, kind) in &source.columns {
            hasher.update(b"\0");
            hasher.update(column.as_bytes());
            hasher.update(b"=");
            hasher.update(kind.as_bytes());
        }
    }
    hasher.finalize().to_hex().to_string()
}
