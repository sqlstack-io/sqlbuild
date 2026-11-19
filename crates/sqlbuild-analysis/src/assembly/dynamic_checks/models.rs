//! What the project-wide dynamic model checks read.

/// One dynamic model: whether it sets its own refresh, and the models it reads.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DynamicModel {
    pub name: String,
    pub has_refresh: bool,
    pub reads: Vec<String>,
}
