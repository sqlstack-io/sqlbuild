//! The run-on vocabulary and the text of its report sections.

pub(crate) const AUTO: &str = "auto";
pub(crate) const LOCAL: &str = "local";
pub(crate) const WAREHOUSE: &str = "warehouse";
pub(crate) const CANNOT_RUN_LOCALLY_TITLE: &str = "Can't run locally";
pub(crate) const ON_THE_WAREHOUSE_TITLE: &str = "On the warehouse";
pub(crate) const RUNS_ON_LABEL: &str = "runs on";
pub(crate) const NOTE_LABEL: &str = "= note:";
pub(crate) const EXPANDED_LABEL: &str = "= expanded from";
pub(crate) const USED_BY_LABEL: &str = "= used by";
/// The help of a cause that has none of its own under `--run-on local`: `run these <items> on the warehouse …`.
pub(crate) const RUN_ELSEWHERE_HELP_PREFIX: &str = "run these ";
pub(crate) const RUN_ELSEWHERE_HELP_SUFFIX: &str = " on the warehouse with --run-on auto";
pub(crate) const VERBOSE_HINT: &str = "  Every model a macro expands into: --verbose; every item each place blocks: --json (refusals[].tests).";
