//! Keep a recorded fingerprint while its recorded definition still fingerprints like the current one.

/// `recorded_hash` when the non-empty recorded definition fingerprints to `current` today, else `current`.
pub fn baseline_fingerprint(
    current: String,
    (recorded_definition, recorded_hash): (&str, &str),
    fingerprint: impl FnOnce(&str) -> Result<String, String>,
) -> String {
    if recorded_hash == current || recorded_hash.is_empty() || recorded_definition.is_empty() {
        return current;
    }
    match fingerprint(recorded_definition) {
        Ok(rehashed) if rehashed == current => recorded_hash.to_owned(),
        _ => current,
    }
}
