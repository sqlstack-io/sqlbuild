//! Public traits and type aliases of analysis session.

use crate::analysis_session::_helpers::session::PivotModelRow;
use crate::analysis_session::models::FamilyRow;
use sqlbuild_analysis::assembly::analysis_session::types::Shapes;

/// `(succeeded, [(name, type, nullability)] or None, has star)`.
pub type ColumnsRow = (bool, Option<Vec<(String, Option<String>, String)>>, bool);

/// The tuple Python sends for a `PivotRequestRow`, in field order.
pub type PivotRequestTuple = (
    String,
    Shapes,
    Shapes,
    Shapes,
    Vec<(String, Vec<FamilyRow>)>,
    Vec<PivotModelRow>,
);
