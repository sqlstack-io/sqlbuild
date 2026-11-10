//! The compile phases and metrics `--json` reports, in presentation order.

/// Phases whose elapsed nanoseconds are reported as whole milliseconds.
pub const COMPILE_TIMING_PHASES: [&str; 15] = [
    "attachment_ms",
    "model_analysis_ms",
    "model_analysis_cpu_ms",
    "analysis_preparation_ms",
    "analysis_native_ms",
    "analysis_projection_ms",
    "binding_validation_ms",
    "test_input_compile_ms",
    "test_planning_ms",
    "comparison_render_ms",
    "cache_publication_ms",
    "physical_write_ms",
    "stale_traversal_ms",
    "early_rules_build_ms",
    "early_rules_wait_ms",
];

/// Counters reported as recorded.
pub const COMPILE_METRICS: [&str; 10] = [
    "analysis_batch_cache_hits",
    "analysis_entry_cache_hits",
    "analysis_cache_misses",
    "analysis_cache_bypasses",
    "sql_test_scan_cache_hits",
    "sql_test_scan_cache_misses",
    "early_rules_hits",
    "early_rules_misses",
    "native_analysis_shared",
    "native_analysis_unshared",
];
