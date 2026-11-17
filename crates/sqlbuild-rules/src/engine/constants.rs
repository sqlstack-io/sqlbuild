//! SQBRPERFORMANCE102 grain constants.

pub(crate) const JSON_GRAIN_RULE_CODE: &str = "SQBRPERFORMANCE102";
pub(crate) const REFERENCE_KIND: &str = "ref";
pub(crate) const SEMI_STRUCTURED_TYPES: [&str; 3] = ["VARIANT", "OBJECT", "ARRAY"];
/// Lineage resource types and the relation-key kinds they are declared under.
pub(crate) const RESOURCE_KINDS: [(&str, &str); 3] =
    [("model", "ref"), ("source", "source"), ("seed", "seed")];
pub(crate) const NO_LINEAGE: &str = "no column lineage";
pub(crate) const NO_TYPES: &str = "no inferred column types";
