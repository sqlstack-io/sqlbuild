//! The line of a sources file a load's setting is written on.

use sqlbuild_config::errors::ConfigError;
use sqlbuild_config::models::{ComposedYaml, ComposedYamlContent};
use sqlbuild_config::yaml::main::compose_marks::compose_marks;

use crate::loads::constants::LOAD_KEY;

/// The one-based line of the deepest key of `path` under `load:` that `contents` writes; `None` when it writes none of them.
pub fn yaml_line(contents: &str, path: &[String]) -> Result<Option<usize>, ConfigError> {
    let composed: ComposedYaml = compose_marks(contents)?;
    let Some(mut node) = composed.root else {
        return Ok(None);
    };
    let mut found: Option<usize> = None;
    for key in std::iter::once(LOAD_KEY).chain(path.iter().map(String::as_str)) {
        let Some(ComposedYamlContent::Mapping(entries)) =
            composed.nodes.get(node).map(|node| &node.content)
        else {
            break;
        };
        let Some((key_node, value)) = entries
            .iter()
            .rev()
            .find(|(name, _)| composed.nodes[*name].content.as_scalar() == Some(key))
        else {
            break;
        };
        found = Some(composed.nodes[*key_node].start);
        node = *value;
    }
    Ok(found.map(|start| line_of(contents, start)))
}

/// The one-based line holding the char at `start`.
fn line_of(contents: &str, start: usize) -> usize {
    let mut line: usize = 1;
    for character in contents.chars().take(start) {
        if character == '\n' {
            line += 1;
        }
    }
    line
}
