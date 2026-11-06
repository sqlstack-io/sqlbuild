//! Reads cases as JSON lines on stdin and writes one comparison per case as a JSON line.
//!
//! Each case is a compiled Postgres model with SQLBuild's Polyglot results. The model's SQL is
//! parsed with SQLQuack's Postgres parser and analysed with SQLQuack's lineage and binding findings
//! under the Postgres profile; the two results are compared for lineage (B), binding findings (C)
//! and output types and nullability (D).

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
