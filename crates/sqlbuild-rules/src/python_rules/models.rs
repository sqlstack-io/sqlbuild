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

/// One project Python file placed the way Fensu places a file under its scan root.
#[derive(Clone, Debug)]
pub(crate) struct PythonTarget {
    /// The file's index in the request's Python sources.
    pub source: usize,
    pub repository_path: String,
    /// Whether the file belongs to the runtime package under `python/`.
    pub package: bool,
    /// The directory the file is placed under: `python` for package files, else its parent.
    pub root: String,
    /// Path parts below `root`.
    pub relative_parts: Vec<String>,
    pub role: Option<String>,
    pub is_main_module: bool,
    pub is_entry_module: bool,
}

/// One `evaluate_core_rules` call: a file, the Fensu codes it runs, and the context they read.
#[derive(Debug)]
pub(crate) struct RuleCall<'a> {
    pub target: &'a PythonTarget,
    pub codes: Vec<String>,
    pub context: fensu_native::NativeRuleContext,
    pub queries: Vec<fensu_native::NativeProjectQuery>,
}

/// What every call of one run reads.
#[derive(Debug)]
pub(crate) struct CallInputs<'a> {
    pub request: &'a crate::models::EvaluateRequest,
    pub settings: crate::python_rules::_helpers::contexts::ContextSettings,
    pub programs: &'a crate::python_rules::_helpers::programs::Programs<'a>,
}
