//! Recording and reading model facts by name, keeping production order.

use crate::compiled_project::models::{
    CompiledModelFacts, CompiledProjectFacts, ModelAnalysisFact,
};

impl CompiledProjectFacts {
    /// Keep `model`; a model recorded again under the same name replaces it in place.
    pub(crate) fn record_model(&mut self, model: CompiledModelFacts) {
        match self.model_index.get(&model.name) {
            Some(&position) => self.models[position] = model,
            None => {
                self.model_index
                    .insert(model.name.clone(), self.models.len());
                self.models.push(model);
            }
        }
    }

    /// Attach a model's typed analysis; `false` when no model of that name was recorded.
    pub(crate) fn record_analysis(&mut self, name: &str, analysis: ModelAnalysisFact) -> bool {
        match self.model_index.get(name) {
            Some(&position) => {
                self.models[position].analysis = Some(analysis);
                true
            }
            None => false,
        }
    }

    /// Mark a recorded model as skipped by its native parser; `false` when it was never recorded.
    pub(crate) fn record_skipped(&mut self, name: &str) -> bool {
        match self.model_index.get(name) {
            Some(&position) => {
                self.models[position].sql_analysis_skipped = true;
                true
            }
            None => false,
        }
    }

    pub(crate) fn model(&self, name: &str) -> Option<&CompiledModelFacts> {
        self.model_index
            .get(name)
            .map(|&position| &self.models[position])
    }
}
