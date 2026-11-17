//! The Python host's slots beside the compiled project's records.

use std::fmt::{Debug, Formatter, Result as FmtResult};

use crate::compiled_views::models::AnalysisProjections;
use crate::compiled_views::models::CompiledProjections;
use crate::compiled_views::models::{ModelInputHandles, SchemaHandles};
use crate::resource_views::models::AuditProjections;
use crate::resource_views::models::FunctionProjections;
use crate::resource_views::models::SeedHandles;
use crate::resource_views::models::SourceProjections;

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

impl Debug for SourceProjections {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> FmtResult {
        formatter
            .debug_struct("SourceProjections")
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
