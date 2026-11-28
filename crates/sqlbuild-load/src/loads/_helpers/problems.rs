//! SQLLoad's messages about a load, with the setting they name as a path in its `load:` mapping.

use crate::loads::constants::{LOAD_KEY, PATH_MARKER, SOURCES, UNKNOWN_KEY};
use crate::loads::models::LoadProblem;

/// `message`, about the source SQLLoad knows `load` as, renamed to the `load:` mapping and its path read off.
pub(crate) fn problem(load: &str, message: &str) -> LoadProblem {
    let message: String = message.trim_end().to_owned();
    let Some((text, named)) = message.rsplit_once(PATH_MARKER) else {
        return in_table(load, message);
    };
    let segments: Vec<String> = segments(named.trim_end_matches('`'));
    let prefix: [&str; 2] = [SOURCES, load];
    if segments.len() < prefix.len() || segments[..prefix.len()] != prefix {
        return LoadProblem {
            message,
            path: Vec::new(),
        };
    }
    let path: Vec<String> = segments[prefix.len()..].to_vec();
    let shown: String = std::iter::once(LOAD_KEY.to_owned())
        .chain(path.iter().map(|segment| match segment.contains('.') {
            true => format!("\"{segment}\""),
            false => segment.clone(),
        }))
        .collect::<Vec<_>>()
        .join(".");
    LoadProblem {
        message: format!("{text}{PATH_MARKER}{shown}`"),
        path,
    }
}

/// A message naming its setting as `unknown key "<key>" in [sources.<load>...]`, the table renamed `[load...]`.
fn in_table(load: &str, message: String) -> LoadProblem {
    let opening: String = format!("[{SOURCES}.{load}");
    let Some(start) = message.find(&opening) else {
        return LoadProblem {
            message,
            path: Vec::new(),
        };
    };
    let rest: &str = &message[start + opening.len()..];
    let table: &str = rest.split(']').next().unwrap_or_default();
    let mut path: Vec<String> = segments(table.trim_start_matches('.'))
        .into_iter()
        .filter(|segment| !segment.is_empty())
        .collect();
    if let Some(key) = message
        .strip_prefix(UNKNOWN_KEY)
        .and_then(|rest| rest.split('"').next())
    {
        path.push(key.to_owned());
    }
    LoadProblem {
        message: message.replace(&opening, &format!("[{LOAD_KEY}")),
        path,
    }
}

/// A dotted TOML key path's segments, quoted segments kept whole.
fn segments(path: &str) -> Vec<String> {
    let mut found: Vec<String> = Vec::new();
    let mut current = String::new();
    let mut quoted = false;
    for character in path.chars() {
        match character {
            '"' => quoted = !quoted,
            '.' if !quoted => found.push(std::mem::take(&mut current)),
            other => current.push(other),
        }
    }
    found.push(current);
    found
}
