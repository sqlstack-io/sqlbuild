//! The orchestration rule codes and the option they read.

/// Limits how many nodes a job or sensor-condition selection names one by one.
pub(crate) const NAMED_SELECTOR_LIMIT_CODE: &str = "SQBRORCHESTRATION101";
/// The option holding that limit.
pub(crate) const MAX_NAMED_SELECTORS_OPTION: &str = "max_named_selectors";
/// The limit when the option is not set.
pub(crate) const DEFAULT_MAX_NAMED_SELECTORS: u32 = 3;
/// The owner label of a job selection.
pub(crate) const JOB_OWNER: &str = "job";
/// The owner label of a sensor-condition selection.
pub(crate) const SENSOR_OWNER: &str = "sensor";
/// The suffix of a Python module file.
pub(crate) const MODULE_SUFFIX: &str = ".py";
/// The file a Python package is imported from.
pub(crate) const INIT_MODULE: &str = "__init__.py";
