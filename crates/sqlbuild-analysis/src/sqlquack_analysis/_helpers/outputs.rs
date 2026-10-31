//! What SQLQuack's lineage of an output becomes in SQLBuild's lineage terms.

use sqlquack_semantics::lineage::models::TransformKind;
use sqlquack_syntax::{ComplexityGuardOptions, ParseOptions};

use crate::lineage::constants::MAX_FUNCTION_CALL_DEPTH;
use crate::lineage::models::{LineageConfidence, LineageTransformKind};

/// The parse options lineage reads models with: the call depth SQLBuild's analysis allows.
pub(crate) fn lineage_parse_options() -> ParseOptions {
    ParseOptions {
        complexity_guard: Some(ComplexityGuardOptions {
            max_function_call_depth: Some(MAX_FUNCTION_CALL_DEPTH),
            ..ComplexityGuardOptions::default()
        }),
    }
}

/// SQLBuild's transform kind for SQLQuack's.
pub(crate) fn transform_kind(kind: TransformKind) -> LineageTransformKind {
    match kind {
        TransformKind::Constant => LineageTransformKind::Constant,
        TransformKind::Direct => LineageTransformKind::Direct,
        TransformKind::Cast => LineageTransformKind::Cast,
        TransformKind::Expression => LineageTransformKind::Expression,
        TransformKind::Aggregation => LineageTransformKind::Aggregation,
    }
}

/// High when every name resolved, medium when some did not but sources were found, else unknown.
pub(crate) fn confidence(resolved: bool, has_sources: bool) -> LineageConfidence {
    match (resolved, has_sources) {
        (true, _) => LineageConfidence::High,
        (false, true) => LineageConfidence::Medium,
        (false, false) => LineageConfidence::Unknown,
    }
}
