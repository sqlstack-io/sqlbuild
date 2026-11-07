//! Reads cases as JSON lines on stdin and writes one comparison per case as a JSON line.
//!
//! Each case is a compiled BigQuery model with SQLBuild's Polyglot results. The model's SQL is
//! parsed with SQLQuack's BigQuery parser and analysed with SQLQuack's lineage and binding findings
//! under the BigQuery profile; the two results are compared for lineage, binding findings and
//! output types and nullability.

mod cases;
mod compare;
mod profile;

use std::io::{BufRead, Write};

use crate::cases::Case;
use crate::compare::compare_case;

fn main() -> Result<(), String> {
    std::thread::Builder::new()
        .stack_size(512 << 20)
        .spawn(run)
        .map_err(|error| error.to_string())?
        .join()
        .map_err(|_| "harness thread panicked".to_owned())?
}

fn run() -> Result<(), String> {
    std::panic::set_hook(Box::new(|_| {}));
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
