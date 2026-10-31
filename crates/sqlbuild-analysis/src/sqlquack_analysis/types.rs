//! Shapes SQLQuack analysis passes between its phases.

use sqlquack_semantics::lineage::models::SchemaColumn;

use crate::lineage::models::RichLineageOutcome;

/// A model's request index, its rich outcome and its outputs as later models read them.
pub(crate) type TracedModel = (usize, (RichLineageOutcome, Vec<SchemaColumn>));
