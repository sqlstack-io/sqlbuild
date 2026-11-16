//! The text report's model lines, captured or, in a delta, patched for the rerun models.

use sqlbuild_core::cli_text::models::ModelLine;
use sqlbuild_python_base::compile_context::main::delta::capture::capture;
use sqlbuild_python_base::compile_context::main::delta::injection::injection;
use std::collections::HashMap;

/// The text report's model lines, captured or, in a delta, patched for the rerun models.
pub fn text_model_lines(models: Vec<ModelLine>) -> Vec<ModelLine> {
    let models: Vec<ModelLine> = match injection() {
        None => models,
        Some(injection) => {
            let mut fresh: HashMap<String, ModelLine> = models
                .into_iter()
                .map(|line| (line.name.clone(), line))
                .collect();
            injection
                .text_models
                .iter()
                .map(|(name, failed, column_count)| {
                    fresh.remove(name).unwrap_or_else(|| ModelLine {
                        name: name.clone(),
                        failed: *failed,
                        column_count: *column_count,
                    })
                })
                .collect()
        }
    };
    capture(|capture| {
        capture.text_models = Some(
            models
                .iter()
                .map(|line| (line.name.clone(), line.failed, line.column_count))
                .collect(),
        );
    });
    models
}
