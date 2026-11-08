//! Planner strategy errors.

use crate::planner_strategy::constants::{
    LINEAGE_INPUT_ERROR_CODE, MISSING_STRATEGY_ERROR_CODE, UNKNOWN_STRATEGY_ERROR_CODE,
};

/// An incremental model whose strategy is missing or unknown.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum StrategyError {
    Missing { model: String },
    Unknown { model: String, strategy: String },
}

impl StrategyError {
    /// The planner input error message.
    pub fn message(&self) -> String {
        match self {
            Self::Missing { model } => {
                format!("incremental model '{model}' is missing required incremental_strategy")
            }
            Self::Unknown { model, strategy } => {
                format!("incremental model '{model}' has unknown strategy '{strategy}'")
            }
        }
    }

    /// The planner input error code.
    pub fn code(&self) -> &'static str {
        match self {
            Self::Missing { .. } => MISSING_STRATEGY_ERROR_CODE,
            Self::Unknown { .. } => UNKNOWN_STRATEGY_ERROR_CODE,
        }
    }
}

/// A lineage walk that failed in the caller or did not find a watermark input.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum LineageFailure<E> {
    Graph(E),
    NotInLineage { model: String, input: String },
}

impl<E> LineageFailure<E> {
    /// The planner input error message of a missing input.
    pub fn message(model: &str, input: &str) -> String {
        format!(
            "model '{model}': cursor_inputs watermark references '{input}', but it is not in the \
             model's upstream lineage"
        )
    }

    /// The planner input error code of a missing input.
    pub fn code() -> &'static str {
        LINEAGE_INPUT_ERROR_CODE
    }
}

/// A watermark input served by a capped microbatch producer.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CappedWatermarkInput {
    pub model: String,
    pub input: String,
    pub producer: String,
}

impl CappedWatermarkInput {
    /// The compile input error message.
    pub fn message(&self) -> String {
        let Self {
            model,
            input,
            producer,
        } = self;
        if producer != input {
            return format!(
                "model '{model}' uses watermark input '{input}' derived from capped producer \
                 '{producer}'; capped producers cannot serve as watermark inputs"
            );
        }
        format!(
            "model '{model}' uses capped producer '{producer}' as a watermark input; capped \
             producers cannot serve as watermark inputs"
        )
    }
}

/// A warehouse type the parser could not read, which classifies as no cursor type.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TypeParseFailure {
    pub normalized_type: String,
    pub message: String,
}
