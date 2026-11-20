//! Dynamic models the plan keeps whose upstream it replaces, so they recompute in full.

use std::collections::BTreeSet;

use crate::dynamic_refresh::models::DynamicRead;

/// Each kept dynamic reader by name, with the first replaced upstream it reads by name.
pub fn reinitialised_readers(reads: &[DynamicRead]) -> Vec<(String, String)> {
    let replaced: BTreeSet<&str> = reads
        .iter()
        .filter(|read| read.replaced)
        .map(|read| read.name.as_str())
        .collect();
    let mut readers: Vec<(String, String)> = Vec::new();
    for read in reads.iter().filter(|read| read.dynamic && !read.replaced) {
        let mut upstreams: Vec<&str> = read
            .reads
            .iter()
            .map(String::as_str)
            .filter(|upstream| replaced.contains(upstream))
            .collect();
        upstreams.sort_unstable();
        if let Some(upstream) = upstreams.first() {
            readers.push((read.name.clone(), (*upstream).to_owned()));
        }
    }
    readers.sort();
    readers
}
