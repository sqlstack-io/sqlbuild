//! How each Python rule maps to Fensu and which files it reads.

/// The Python files a rule applies to.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PythonFiles {
    /// Only the runtime package under `python/`, whose layout the rule checks.
    Package,
    /// Every supported project Python file, including hooks, UDFs and orchestration modules.
    All,
}

/// One `SQBRPYTHON` rule and the Fensu core rule it runs, if any.
#[derive(Clone, Copy, Debug)]
pub struct PythonRuleSpec {
    pub code: &'static str,
    /// The Fensu code it evaluates; `None` for SQLBuild's own layout rules.
    pub fensu_code: Option<&'static str>,
    pub slug: &'static str,
    /// Fensu's family, which with `owner` decides the file a project-wide finding is reported on.
    pub family: &'static str,
    pub owner: &'static str,
    pub applies: PythonFiles,
    pub enabled_by_default: bool,
    /// Whole-number options as name, default and description, named as Fensu names its thresholds.
    pub options: &'static [(&'static str, u32, &'static str)],
    pub message: &'static str,
    pub remediation: &'static str,
}
