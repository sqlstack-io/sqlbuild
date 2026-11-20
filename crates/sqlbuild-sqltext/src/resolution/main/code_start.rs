//! The next code after whitespace and SQL comments.

/// The first code byte after `start` past whitespace, `/* */` and `line_comments` comments.
#[must_use]
pub fn code_start(sql: &str, start: usize, line_comments: &[String]) -> Option<usize> {
    let mut position: usize = start;
    loop {
        let tail: &str = sql.get(position..)?;
        position += tail.len() - tail.trim_start().len();
        let rest: &str = &sql[position..];
        if line_comments
            .iter()
            .any(|prefix| rest.starts_with(prefix.as_str()))
        {
            position = rest
                .find('\n')
                .map_or(sql.len(), |newline| position + newline + 1);
        } else if let Some(body) = rest.strip_prefix("/*") {
            position += 2 + body.find("*/")? + 2;
        } else {
            return Some(position);
        }
    }
}
