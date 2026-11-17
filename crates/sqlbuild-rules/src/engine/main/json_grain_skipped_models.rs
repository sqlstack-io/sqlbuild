//! The analysed models SQBRPERFORMANCE102 skipped for missing facts.

use crate::models::Model;

/// `(path, reason)` of each analysed model SQBRPERFORMANCE102 skipped for missing facts.
pub fn json_grain_skipped_models(models: &[Model]) -> Vec<(String, String)> {
    let mut skipped: Vec<(String, String)> = Vec::new();
    for model in models.iter().filter(|model| !model.sql_analysis_disabled) {
        if let Some(reason) = model
            .json_grain
            .as_ref()
            .and_then(|grain| grain.skipped.as_ref())
        {
            skipped.push((model.relative_path.clone(), reason.clone()));
        }
    }
    skipped
}
