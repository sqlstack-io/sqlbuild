//! The Python host's slots beside the compiled project's records.

use std::fmt::{Debug, Formatter, Result as FmtResult};

use sqlbuild_project::types::ProjectHost;

use crate::bindings::_helpers::compiled_views::audit_views::AuditHandles;
use crate::bindings::_helpers::compiled_views::audit_views::AuditProjections;
use crate::bindings::_helpers::compiled_views::compiled_model_analysis::AnalysisProjections;
use crate::bindings::_helpers::compiled_views::compiled_model_view::CompiledProjections;
use crate::bindings::_helpers::compiled_views::function_views::FunctionHandles;
use crate::bindings::_helpers::compiled_views::function_views::FunctionProjections;
use crate::bindings::_helpers::compiled_views::model_input_view::ModelInputHandles;
use crate::bindings::_helpers::compiled_views::model_input_view::SchemaHandles;
use crate::bindings::_helpers::compiled_views::seed_views::SeedHandles;
use crate::bindings::_helpers::compiled_views::source_views::SourceHandles;
use crate::bindings::_helpers::compiled_views::source_views::SourceProjections;

/// The extension's host: Python handles and the views built from them.
pub(crate) struct PyProjectHost;

impl ProjectHost for PyProjectHost {
    type ModelInput = ModelInputHandles;
    type Schema = SchemaHandles;
    type CompiledModel = CompiledProjections;
    type Analysis = AnalysisProjections;
    type AttachedSource = SourceHandles;
    type Source = SourceProjections;
    type AttachedFunction = FunctionHandles;
    type Function = FunctionProjections;
    type AttachedAudit = AuditHandles;
    type Audit = AuditProjections;
    type Seed = SeedHandles;
}

impl Debug for ModelInputHandles {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> FmtResult {
        formatter
            .debug_struct("ModelInputHandles")
            .finish_non_exhaustive()
    }
}

impl Debug for SchemaHandles {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> FmtResult {
        formatter
            .debug_struct("SchemaHandles")
            .finish_non_exhaustive()
    }
}

impl Debug for CompiledProjections {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> FmtResult {
        formatter
            .debug_struct("CompiledProjections")
            .finish_non_exhaustive()
    }
}

impl Debug for AnalysisProjections {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> FmtResult {
        formatter
            .debug_struct("AnalysisProjections")
            .finish_non_exhaustive()
    }
}

impl Debug for SourceHandles {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> FmtResult {
        formatter
            .debug_struct("SourceHandles")
            .finish_non_exhaustive()
    }
}

impl Debug for SourceProjections {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> FmtResult {
        formatter
            .debug_struct("SourceProjections")
            .finish_non_exhaustive()
    }
}

impl Debug for FunctionHandles {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> FmtResult {
        formatter
            .debug_struct("FunctionHandles")
            .finish_non_exhaustive()
    }
}

impl Debug for FunctionProjections {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> FmtResult {
        formatter
            .debug_struct("FunctionProjections")
            .finish_non_exhaustive()
    }
}

impl Debug for AuditHandles {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> FmtResult {
        formatter
            .debug_struct("AuditHandles")
            .finish_non_exhaustive()
    }
}

impl Debug for AuditProjections {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> FmtResult {
        formatter
            .debug_struct("AuditProjections")
            .finish_non_exhaustive()
    }
}

impl Debug for SeedHandles {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> FmtResult {
        formatter
            .debug_struct("SeedHandles")
            .finish_non_exhaustive()
    }
}
