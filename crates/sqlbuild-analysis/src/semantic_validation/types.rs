use crate::semantic_validation::models::{Columns, ProjectCatalog};
use std::collections::HashMap;

pub type Relations = HashMap<String, Columns>;
pub type BindingRequest = (String, Vec<(String, bool)>, Relations);
/// One analysis SQL with its relation stubs and placeholder defaults.
pub type NormalizationRequest = (String, HashMap<String, String>, HashMap<String, String>);
/// Compact analysis whose catalog schemas are resolved; it runs with an options-only catalog.
pub(crate) type PreparedCompactAnalysis =
    Box<dyn FnOnce(&ProjectCatalog) -> Result<String, String> + Send>;
