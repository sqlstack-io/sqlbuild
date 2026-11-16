//! The Python host's slots beside the compiled project's records.

use std::fmt::{Debug, Formatter, Result as FmtResult};

use crate::compiled_views::_helpers::audit_views::AuditProjections;
use crate::compiled_views::_helpers::compiled_model_analysis::AnalysisProjections;
use crate::compiled_views::_helpers::compiled_model_view::CompiledProjections;
use crate::compiled_views::_helpers::function_views::FunctionProjections;
use crate::compiled_views::_helpers::seed_views::SeedHandles;
use crate::compiled_views::_helpers::source_views::SourceProjections;
use crate::compiled_views::models::AuditHandles;
use crate::compiled_views::models::FunctionHandles;
use crate::compiled_views::models::ModelInputHandles;
use crate::compiled_views::models::SchemaHandles;
use crate::compiled_views::models::SourceHandles;

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
