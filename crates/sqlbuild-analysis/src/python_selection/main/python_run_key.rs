use crate::python_selection::constants::LOADER_KIND;

/// The key a selected Python node's run reports under; a loader reports as the source it writes.
pub fn python_run_key(sources: &[(String, Option<String>)], (kind, name): (&str, &str)) -> String {
    if kind != LOADER_KIND {
        return format!("{kind}:{name}");
    }
    let source = sources
        .iter()
        .find(|(_, loader)| loader.as_deref() == Some(name))
        .map_or(name, |(source, _)| source.as_str());
    format!("source:{source}")
}
