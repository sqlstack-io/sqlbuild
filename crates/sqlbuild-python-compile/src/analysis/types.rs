//! Public traits and type aliases of analysis.

use sqlbuild_analysis::semantic_validation::models::Expansion;
use sqlbuild_python_project::analysis::models::Positional;

/// The expanded SQL and the expansion passes Python sends for a located model.
pub type ExpansionRow = Option<(String, Vec<Vec<Positional<Expansion>>>)>;
