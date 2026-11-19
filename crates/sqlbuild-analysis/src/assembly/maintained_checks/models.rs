//! What the project-wide maintained model checks read.

/// One maintained model: whether it sets its own refresh, and the models it reads.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MaintainedModel {
    pub name: String,
    pub has_refresh: bool,
    pub reads: Vec<String>,
}
