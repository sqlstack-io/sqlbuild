//! Record semantic analysis cache outcomes for one invocation.

use crate::compile_driver::models::AnalysisCacheCounts;
use sqlbuild_host::compile_context::main::record_compile_metric::record_compile_metric;

/// Record semantic analysis cache outcomes for one invocation.
pub fn record_analysis_cache_metrics(
    AnalysisCacheCounts {
        batch_hits,
        entry_hits,
        misses,
        bypasses,
    }: AnalysisCacheCounts,
) {
    for (metric, value) in [
        ("analysis_batch_cache_hits", batch_hits),
        ("analysis_entry_cache_hits", entry_hits),
        ("analysis_cache_misses", misses),
        ("analysis_cache_bypasses", bypasses),
    ] {
        record_compile_metric(metric, i64::try_from(value).unwrap_or(i64::MAX));
    }
}
