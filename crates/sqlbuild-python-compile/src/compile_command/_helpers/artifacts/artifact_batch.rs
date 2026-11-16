//! Compiled artifacts queued for one native write, keeping unchanged files when asked.

pub(crate) const PHYSICAL_WRITE_PHASE: &str = "physical_write_ms";

/// `contents` with platform line endings.
pub(crate) fn platform_lines(contents: Vec<u8>) -> Vec<u8> {
    if cfg!(windows) {
        contents
            .iter()
            .flat_map(|byte| {
                if *byte == b'\n' {
                    vec![b'\r', b'\n']
                } else {
                    vec![*byte]
                }
            })
            .collect()
    } else {
        contents
    }
}

/// `sql` with trailing whitespace replaced by one newline.
pub(crate) fn sql_artifact(sql: &str) -> Vec<u8> {
    let trimmed: &str =
        sql.trim_end_matches(sqlbuild_core::text::main::python_isspace::python_isspace);
    format!("{trimmed}\n").into_bytes()
}
