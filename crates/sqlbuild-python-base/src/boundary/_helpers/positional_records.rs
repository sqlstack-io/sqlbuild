//! The tuple positions of records from other crates that Python exchanges as tuples.

use sqlbuild_analysis::binding_locations::models::AuthoredSpan;
use sqlbuild_analysis::semantic_validation::models::{DiagnosticRow, Expansion};
use sqlbuild_rules::lint_text::models::InterpolationSite;

positional_fields! {
    extract, into;
    InterpolationSite {
        sentinel: String,
        neutralized_start: usize,
        neutralized_end: usize,
        original_start: usize,
        original_end: usize,
        original_text: String,
    }
}

positional_fields! {
    extract, into;
    DiagnosticRow {
        code: String,
        message: String,
        line: Option<usize>,
        column: Option<usize>,
        start: Option<usize>,
        end: Option<usize>,
        severity: String,
    }
}

positional_fields! {
    extract;
    Expansion {
        source_start: usize,
        source_end: usize,
        output_start: usize,
        output_end: usize,
    }
}

positional_fields! {
    into;
    AuthoredSpan {
        line: usize,
        column: usize,
        end_line: Option<usize>,
        end_column: Option<usize>,
    }
}
