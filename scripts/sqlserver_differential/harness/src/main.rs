//! Reads SQL bodies as JSON lines on stdin and writes one comparison per body as a JSON line.
//!
//! Each body is SQL SQLBuild parses or emits for its SQL Server adapter. It is cleaned as SQLBuild's
//! syntax gate cleans it, parsed with Polyglot's SQL Server dialect as SQLBuild does, and parsed with
//! SQLQuack's SQL Server parser; the comparison records each parser's verdict per statement form.

mod cases;
mod compare;

use std::io::{BufRead, Write};

use crate::cases::Case;
use crate::compare::compare_case;

fn main() -> Result<(), String> {
    let stdin = std::io::stdin();
    let mut out = std::io::stdout().lock();
    for line in stdin.lock().lines() {
        let line: String = line.map_err(|error| error.to_string())?;
        if line.trim().is_empty() {
            continue;
        }
        let case: Case = serde_json::from_str(&line).map_err(|error| error.to_string())?;
        let result = std::panic::catch_unwind(|| compare_case(&case))
            .unwrap_or_else(|_| compare::panicked(&case));
        let text: String = serde_json::to_string(&result).map_err(|error| error.to_string())?;
        writeln!(out, "{text}").map_err(|error| error.to_string())?;
    }
    Ok(())
}
