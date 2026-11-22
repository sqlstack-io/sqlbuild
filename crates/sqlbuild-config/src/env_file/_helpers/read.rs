//! The variables an env file sets, read with dotenv semantics.

use std::path::Path;

use crate::env_file::errors::EnvFileError;
use crate::env_file::models::EnvFileChoice;

/// The physical line number where the logical line `text` starts, `1` when it cannot be found.
fn line_number(path: &Path, text: &str) -> usize {
    let Ok(contents) = std::fs::read_to_string(path) else {
        return 1;
    };
    let first: &str = text.lines().next().unwrap_or_default().trim_end();
    contents
        .lines()
        .position(|line| line.trim_end() == first)
        .map_or(1, |index| index + 1)
}

/// Each `KEY=value` in file order, a later duplicate replacing an earlier one; `None` when there is no file.
pub(crate) fn read_pairs(
    path: &Path,
    choice: &EnvFileChoice,
) -> Result<Option<Vec<(String, String)>>, EnvFileError> {
    let iter = match dotenvy::from_path_iter(path) {
        Ok(iter) => iter,
        Err(error) if error.not_found() => return Ok(None),
        Err(error) => {
            return Err(EnvFileError::Unreadable {
                choice: choice.clone(),
                reason: error.to_string(),
            });
        }
    };
    let mut pairs: Vec<(String, String)> = Vec::new();
    for item in iter {
        match item {
            Ok((name, value)) => {
                pairs.retain(|(kept, _)| *kept != name);
                pairs.push((name, value));
            }
            Err(dotenvy::Error::LineParse(text, _)) => {
                return Err(EnvFileError::Invalid {
                    choice: choice.clone(),
                    line: line_number(path, &text),
                });
            }
            Err(dotenvy::Error::Io(error)) => {
                return Err(EnvFileError::Unreadable {
                    choice: choice.clone(),
                    reason: error.kind().to_string(),
                });
            }
            Err(dotenvy::Error::EnvVar(_)) => {
                return Err(EnvFileError::Unreadable {
                    choice: choice.clone(),
                    reason: "a value is not valid Unicode".to_owned(),
                });
            }
            Err(_) => {
                return Err(EnvFileError::Unreadable {
                    choice: choice.clone(),
                    reason: "unexpected parse failure".to_owned(),
                });
            }
        }
    }
    Ok(Some(pairs))
}

/// The warning when the group or others may read the file.
#[cfg(unix)]
pub(crate) fn shared_read_warning(path: &Path, choice: &EnvFileChoice) -> Option<String> {
    use std::os::unix::fs::PermissionsExt;

    use crate::env_file::constants::SHARED_READ_BITS;

    let mode: u32 = match std::fs::metadata(path) {
        Ok(metadata) => metadata.permissions().mode(),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return None,
        Err(error) => {
            return Some(format!(
                "cannot check who may read env file {}: {error}",
                choice.display
            ));
        }
    };
    (mode & SHARED_READ_BITS != 0).then(|| {
        format!(
            "env file {} is readable by group or others (mode {:o}); restrict it with chmod 600 {}",
            choice.display,
            mode & 0o777,
            choice.display
        )
    })
}

/// Other platforms have no group or other read bits.
#[cfg(not(unix))]
pub(crate) fn shared_read_warning(_path: &Path, _choice: &EnvFileChoice) -> Option<String> {
    None
}
