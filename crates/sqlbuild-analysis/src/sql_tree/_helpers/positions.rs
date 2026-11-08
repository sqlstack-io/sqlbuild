//! Character offsets as 1-based lines and columns.

/// The character offset at which each line of `sql` starts.
pub(crate) fn line_starts(sql: &str) -> Vec<usize> {
    let mut starts: Vec<usize> = vec![0];
    for (offset, character) in sql.chars().enumerate() {
        if character == '\n' {
            starts.push(offset + 1);
        }
    }
    starts
}

/// The 1-based line and column of the character at `offset`.
pub(crate) fn line_column(starts: &[usize], offset: usize) -> (usize, usize) {
    let line: usize = starts.partition_point(|start| *start <= offset);
    let start: usize = starts.get(line.saturating_sub(1)).copied().unwrap_or(0);
    (line.max(1), offset - start + 1)
}
